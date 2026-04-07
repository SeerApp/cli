use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;
use reqwest::Client;
use serde_json::Value;

const GITHUB_REPO: &str = "SeerApp/cli";
const CHECK_INTERVAL_SECS: u64 = 60 * 60 * 24;

pub struct UpdateInfo {
    pub latest_version: String,
}

pub async fn maybe_notify_update() -> anyhow::Result<()> {
    if env::var("SEER_NO_UPDATE_CHECK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Ok(());
    }

    if !should_check_now()? {
        return Ok(());
    }

    let current = env!("CARGO_PKG_VERSION");
    match check_for_update(current).await {
        Ok(Some(info)) => {
            println!(
                "A newer Seer CLI is available: {} (current {}). Run `seer update`.",
                info.latest_version, current
            );
        }
        Ok(None) => {}
        Err(_) => {
            // Update checks are best-effort and should never block command execution.
        }
    }

    write_last_check_now()?;
    Ok(())
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
        print!(
            "Update seer from {} to {}? [y/N]: ",
            current, target_tag
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let accepted = matches!(input.trim().to_lowercase().as_str(), "y" | "yes");
        if !accepted {
            println!("Update canceled.");
            return Ok(());
        }
    }

    let binary_name = if cfg!(target_os = "windows") {
        "seer.exe"
    } else {
        "seer"
    };

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
    let bytes = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    fs::write(&archive_path, &bytes)?;

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

    let target_path = install_target_path(binary_name)?;
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

pub async fn check_for_update(current_version: &str) -> anyhow::Result<Option<UpdateInfo>> {
    let latest_tag = resolve_target_tag(None).await?;
    let latest_clean = clean_version(&latest_tag);
    if compare_versions(&latest_clean, current_version) > 0 {
        Ok(Some(UpdateInfo {
            latest_version: latest_tag,
        }))
    } else {
        Ok(None)
    }
}

async fn resolve_target_tag(requested_version: Option<String>) -> anyhow::Result<String> {
    match requested_version {
        Some(v) => {
            let cleaned = clean_version(&v);
            Ok(format!("v{cleaned}"))
        }
        None => fetch_latest_tag().await,
    }
}

async fn fetch_latest_tag() -> anyhow::Result<String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let resp = github_client()?
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    let tag = body
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing tag_name in latest release response"))?;
    Ok(tag.to_string())
}

fn github_client() -> anyhow::Result<Client> {
    Ok(Client::builder()
        .user_agent(format!("seer-cli/{}", env!("CARGO_PKG_VERSION")))
        .build()?)
}

fn clean_version(v: &str) -> String {
    v.trim().trim_start_matches('v').to_string()
}

fn compare_versions(a: &str, b: &str) -> i32 {
    let pa = parse_version_parts(a);
    let pb = parse_version_parts(b);
    for i in 0..3 {
        if pa[i] > pb[i] {
            return 1;
        }
        if pa[i] < pb[i] {
            return -1;
        }
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
    let status = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(out_dir)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to execute tar: {e}"))?;
    if !status.success() {
        anyhow::bail!("tar failed to extract update archive");
    }
    Ok(())
}

fn install_target_path(binary_name: &str) -> anyhow::Result<PathBuf> {
    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            let candidate = dir.join(binary_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    if cfg!(target_os = "windows") {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        return Ok(home.join(".cargo").join("bin").join(binary_name));
    }

    if let Ok(home) = env::var("HOME") {
        let local_bin = PathBuf::from(home).join(".local").join("bin");
        return Ok(local_bin.join(binary_name));
    }

    Ok(PathBuf::from("/usr/local/bin").join(binary_name))
}

fn replace_binary_atomically(src: &Path, dest: &Path) -> anyhow::Result<()> {
    let staging = dest.with_extension("new");
    fs::copy(src, &staging)?;
    fs::rename(&staging, dest).or_else(|_| {
        if dest.exists() {
            fs::remove_file(dest)?;
        }
        fs::rename(&staging, dest)
    })?;
    Ok(())
}

fn should_check_now() -> anyhow::Result<bool> {
    let path = update_cache_path()?;
    if !path.exists() {
        return Ok(true);
    }
    let raw = fs::read_to_string(path)?;
    let json: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    let last_check = json
        .get("last_check_epoch_secs")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(now.saturating_sub(last_check) >= CHECK_INTERVAL_SECS)
}

fn write_last_check_now() -> anyhow::Result<()> {
    let path = update_cache_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let payload = serde_json::json!({
        "last_check_epoch_secs": now
    });
    fs::write(path, serde_json::to_vec(&payload)?)?;
    Ok(())
}

fn update_cache_path() -> anyhow::Result<PathBuf> {
    let proj = ProjectDirs::from("com", "seer", "seer")
        .ok_or_else(|| anyhow::anyhow!("Unable to determine config directory for Seer"))?;
    Ok(proj.config_dir().join("cli").join("update_check.json"))
}
