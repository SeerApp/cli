use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};

const GITHUB_REPO: &str = "SeerApp/cli";
const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60 * 24);

/// Urgency level embedded in the GitHub release body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Minor,
    Major,
    Critical,
}

impl Severity {
    /// Parses `<!-- seer-severity: <level> -->` from a release body.
    fn from_release_body(body: &str) -> Self {
        for line in body.lines() {
            let l = line.trim();
            if !l.starts_with("<!--") || !l.contains("seer-severity:") {
                continue;
            }
            if l.contains("critical") { return Severity::Critical; }
            if l.contains("major")    { return Severity::Major; }
            return Severity::Minor;
        }
        Severity::Minor
    }

    fn as_str(self) -> &'static str {
        match self {
            Severity::Minor    => "minor",
            Severity::Major    => "major",
            Severity::Critical => "critical",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "critical" => Severity::Critical,
            "major"    => Severity::Major,
            _          => Severity::Minor,
        }
    }
}

pub struct UpdateCheckHandle {
    rx: Option<std::sync::mpsc::Receiver<(String, Severity)>>,
}

impl UpdateCheckHandle {
    /// Prints an update notification after the main command, if a newer version
    /// is available. Uses the background thread result when it finished in time,
    /// falls back to the stale-while-revalidate cached value otherwise.
    pub fn show_notice(self) {
        if no_update_check() {
            return;
        }
        let current = env!("CARGO_PKG_VERSION");
        let latest: Option<(String, Severity)> = self
            .rx
            .and_then(|rx| rx.try_recv().ok())
            .or_else(read_cached_latest);
        if let Some((ref version, severity)) = latest {
            if compare_versions(&clean_version(version), current) > 0 {

                let (colour, reset) = match severity {
                    Severity::Minor    => ("\x1b[32m", "\x1b[0m"), // green
                    Severity::Major    => ("\x1b[33m", "\x1b[0m"), // yellow
                    Severity::Critical => ("\x1b[31m", "\x1b[0m"), // red
                };
                let label = match severity {
                    Severity::Minor    => "minor",
                    Severity::Major    => "major",
                    Severity::Critical => "critical",
                };
                let suffix = if severity == Severity::Critical { " now" } else { "" };
                println!(
                    "\nNew {colour}{label}{reset} update available: {version} (current {current}). \
                     Run `seer update`{suffix}."
                );
            }
        }
    }
}

/// Spawns a background thread to refresh the update cache if stale, and
/// returns a handle. Call `handle.show_notice()` after the command finishes.
/// Never blocks. Safe to call unconditionally.
pub fn begin_update_check() -> UpdateCheckHandle {
    if no_update_check() {
        return UpdateCheckHandle { rx: None };
    }
    if !cache_is_stale() {
        return UpdateCheckHandle { rx: None };
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio rt for update check");
        if let Ok((tag, severity)) = rt.block_on(fetch_latest_release_info()) {
            let _ = write_cached_latest(&tag, severity);
            let _ = tx.send((tag, severity));
        }
        // On error: channel closes, show_notice() falls back to cached value.
    });
    UpdateCheckHandle { rx: Some(rx) }
}

/// Runs `f`, then prints an update notice if a newer version is available.
/// Background cache refresh runs concurrently with `f`.
pub fn with_update_check<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    let handle = begin_update_check();
    let result = f();
    handle.show_notice();
    result
}

pub async fn run_update_command(yes: bool, requested_version: Option<String>) -> anyhow::Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let target_tag = resolve_target_tag(requested_version).await?;
    let target_clean = clean_version(&target_tag);

    if compare_versions(&target_clean, current) <= 0 {
        println!("seer is already up to date (current {}).", current);
        return Ok(());
    }

    if !yes {
        print!("Update seer from {} to {}? [yes/no]: ", current, target_tag);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("Update canceled.");
            return Ok(());
        }
    }

    let binary_name = if cfg!(target_os = "windows") { "seer.exe" } else { "seer" };
    let (os, arch) = detect_platform()?;
    let file_name = format!("seer-{os}-{arch}.tar.gz");
    let url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        GITHUB_REPO, target_tag, file_name
    );
    let temp_dir = make_temp_dir()?;
    let archive_path = temp_dir.join(&file_name);
    let extracted_bin = temp_dir.join(binary_name);

    let client = github_client()?;

    // Fetch expected checksum from release body — best-effort, old releases may not have it.
    let expected_hash = fetch_checksum_from_release(&client, &target_tag, &file_name).await.ok();

    println!("Downloading {}...", file_name);
    stream_download(&client, &url, &archive_path).await?;

    if let Some(ref hash) = expected_hash {
        verify_sha256(&archive_path, hash)?;
        println!("Checksum verified.");
    }

    extract_tar_gz(&archive_path, &temp_dir)?;
    if !extracted_bin.exists() {
        anyhow::bail!("Downloaded archive did not contain {binary_name}");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&extracted_bin)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&extracted_bin, perms)?;
    }

    let target_path = install_target_path()?;
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    replace_binary_atomically(&extracted_bin, &target_path)?;
    let _ = fs::remove_dir_all(&temp_dir);

    println!(
        "Updated seer from {} to {} at {}",
        current,
        target_tag,
        target_path.display()
    );
    Ok(())
}

fn read_cached_latest() -> Option<(String, Severity)> {
    let content = fs::read_to_string(update_cache_path().ok()?).ok()?;
    let content = content.trim();
    if content.is_empty() { return None; }
    let mut parts = content.splitn(2, ':');
    let version  = parts.next()?.to_string();
    let severity = parts.next().map(Severity::from_str).unwrap_or(Severity::Minor);
    Some((version, severity))
}

fn write_cached_latest(version: &str, severity: Severity) -> anyhow::Result<()> {
    let path = update_cache_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, format!("{}:{}", version, severity.as_str()))?;
    Ok(())
}

fn cache_is_stale() -> bool {
    let Ok(path) = update_cache_path() else { return true };
    let Ok(meta) = fs::metadata(&path) else { return true };
    let Ok(modified) = meta.modified() else { return true };
    modified.elapsed().unwrap_or(Duration::MAX) >= CHECK_INTERVAL
}

fn update_cache_path() -> anyhow::Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
    Ok(base.join("seer").join("cli").join("update_check"))
}


async fn resolve_target_tag(requested_version: Option<String>) -> anyhow::Result<String> {
    match requested_version {
        Some(v) => {
            let tag = format!("v{}", clean_version(&v));
            validate_tag_exists(&tag).await?;
            Ok(tag)
        }
        None => fetch_latest_tag().await,
    }
}

async fn validate_tag_exists(tag: &str) -> anyhow::Result<()> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        GITHUB_REPO, tag
    );
    github_client()?
        .get(&url)
        .send()
        .await?
        .error_for_status()
        .map_err(|_| anyhow::anyhow!("Release tag '{tag}' not found on GitHub"))?;
    Ok(())
}

/// Fetches the latest release tag and its severity from the release body.
async fn fetch_latest_release_info() -> anyhow::Result<(String, Severity)> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let release: Value = github_client()?
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let tag = release
        .get("tag_name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Missing tag_name in latest release response"))?;
    let severity = release
        .get("body")
        .and_then(Value::as_str)
        .map(Severity::from_release_body)
        .unwrap_or(Severity::Minor);
    Ok((tag, severity))
}

/// Fetches only the tag name (used by resolve_target_tag and check_for_update).
async fn fetch_latest_tag() -> anyhow::Result<String> {
    Ok(fetch_latest_release_info().await?.0)
}

/// Fetches the SHA256 checksum for `file_name` from the hidden checksum comment in the release body.
async fn fetch_checksum_from_release(
    client: &Client,
    tag: &str,
    file_name: &str,
) -> anyhow::Result<String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        GITHUB_REPO, tag
    );
    let release: Value = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let body = release.get("body").and_then(Value::as_str).unwrap_or("");
    parse_checksum_from_body(body, file_name)
        .ok_or_else(|| anyhow::anyhow!("Checksum for {} not found in release body", file_name))
}

fn parse_checksum_from_body(body: &str, file_name: &str) -> Option<String> {
    let start = body.find("<!-- seer-checksums")?;
    let end = body[start..].find("-->")?;
    let section = &body[start + "<!-- seer-checksums".len()..start + end];
    for line in section.lines() {
        if let Some((name, hash)) = line.split_once('=') {
            if name.trim() == file_name {
                return Some(hash.trim().to_string());
            }
        }
    }
    None
}


async fn stream_download(client: &Client, url: &str, dest: &Path) -> anyhow::Result<()> {
    let resp = client.get(url).send().await?.error_for_status()?;
    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(dest).await?;
    while let Some(chunk) = stream.next().await {
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk?).await?;
    }
    Ok(())
}

fn github_client() -> anyhow::Result<Client> {
    Ok(Client::builder()
        .user_agent(format!("seer-cli/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(5))
        .build()?)
}

fn verify_sha256(path: &Path, expected_hex: &str) -> anyhow::Result<()> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected_hex {
        anyhow::bail!(
            "Checksum mismatch for {}:\n  expected: {}\n  got:      {}",
            path.display(),
            expected_hex,
            actual
        );
    }
    Ok(())
}

fn detect_platform() -> anyhow::Result<(&'static str, &'static str)> {
    let os = match env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        other => anyhow::bail!("Unsupported OS: {other}"),
    };
    let arch = match env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "arm" => "armv7",
        other => anyhow::bail!("Unsupported architecture: {other}"),
    };
    Ok((os, arch))
}

fn make_temp_dir() -> anyhow::Result<PathBuf> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let dir = env::temp_dir().join(format!("seer-update-{nonce}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn extract_tar_gz(archive: &Path, out_dir: &Path) -> anyhow::Result<()> {
    let file = fs::File::open(archive)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(gz);
    tar.unpack(out_dir)?;
    Ok(())
}

fn install_target_path() -> anyhow::Result<PathBuf> {
    Ok(std::env::current_exe()?.canonicalize()?)
}

fn replace_binary_atomically(src: &Path, dest: &Path) -> anyhow::Result<()> {
    // Stage next to dest to guarantee same filesystem for atomic rename.
    let staging = dest.with_extension("new");
    fs::copy(src, &staging)?;
    if let Err(e) = fs::rename(&staging, dest) {
        let _ = fs::remove_file(&staging);
        return Err(e.into());
    }
    Ok(())
}


fn clean_version(v: &str) -> String {
    v.trim().trim_start_matches('v').to_string()
}

fn compare_versions(a: &str, b: &str) -> i32 {
    let pa = parse_version_parts(a);
    let pb = parse_version_parts(b);
    for i in 0..3 {
        if pa[i] > pb[i] { return 1; }
        if pa[i] < pb[i] { return -1; }
    }
    0
}

fn parse_version_parts(v: &str) -> [u64; 3] {
    let mut out = [0_u64; 3];
    for (idx, part) in v.split('.').take(3).enumerate() {
        let numeric_prefix: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !numeric_prefix.is_empty() {
            out[idx] = numeric_prefix.parse::<u64>().unwrap_or(0);
        }
    }
    out
}

fn no_update_check() -> bool {
    env::var("SEER_NO_UPDATE_CHECK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
