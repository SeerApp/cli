use crate::run::SessionArtifact;
use crate::temp_file::TempFile;
use anyhow::{Context, Result};
use solana_sdk::signature::read_keypair_file;
use solana_sdk::signer::Signer;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use crate::run::blobs::make_blob;


#[derive(Debug, Clone)]
pub struct ProgramTarget {
    pub so_path: PathBuf,
    pub debug_path: PathBuf,
    pub json_path: PathBuf,
}

pub fn get_targets(artifacts_dir: PathBuf) -> Result<Vec<ProgramTarget>> {
    if !artifacts_dir.exists() {
        anyhow::bail!("Artifacts folder does not exist: {:?}", artifacts_dir);
    }

    let mut so_files = HashMap::<String, PathBuf>::new();
    let mut debug_files = HashMap::<String, PathBuf>::new();
    let mut json_files = HashMap::<String, PathBuf>::new();

    for entry in fs::read_dir(&artifacts_dir)
        .with_context(|| format!("Failed to read artifacts dir: {:?}", artifacts_dir))?
    {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }

        match (
            path.extension().and_then(|e| e.to_str()),
            path.file_stem().and_then(|s| s.to_str()),
        ) {
            (Some("so"), Some(stem)) => {
                so_files.insert(stem.to_string(), path.clone());
            }
            (Some("debug"), Some(stem)) => {
                debug_files.insert(stem.to_string(), path.clone());
            }
            (Some("json"), Some(stem)) => {
                if let Some(name) = stem.strip_suffix("-keypair") {
                    json_files.insert(name.to_string(), path.clone());
                }
            }
            _ => {}
        }
    }

    let mut programs = Vec::<ProgramTarget>::new();

    let mut all_names = std::collections::HashSet::new();
    for name in so_files.keys() { all_names.insert(name.clone()); }
    for name in debug_files.keys() { all_names.insert(name.clone()); }
    for name in json_files.keys() { all_names.insert(name.clone()); }

    for name in all_names {
        let so_path = if let Some(v) = so_files.get(&name) {
            v.clone()
        } else {
            println!("[seer][warn] Skipping program '{}' due to missing {}.so file.", name, name);
            continue;
        };

        let debug_path = if let Some(v) = debug_files.get(&name) {
            v.clone()
        } else {
            println!("[seer][warn] Skipping program '{}' due to missing {}.debug file.", name, name);
            continue;
        };

        let json_path = if let Some(v) = json_files.get(&name) {
            v.clone()
        } else {
            println!("[seer][warn] Skipping program '{}' due to missing {}-keypair.json file.", name, name);
            continue;
        };

        let keypair = read_keypair_file(json_path.clone());
        if keypair.is_err() {
            println!("[seer][warn] Skipping program '{}' due to invalid keypair file.", name);
            continue;
        }

        programs.push(ProgramTarget {
            so_path,
            debug_path,
            json_path,
        });
    }

    Ok(programs)
}


/// Checks all target `.debug` files for proper DWARF info.
/// Returns `Ok(None)` if the user declines to proceed.
/// Returns `Ok(Some(valid_targets))` — with invalid targets excluded — if the user proceeds.
pub fn check_debug_artifacts(targets: &[ProgramTarget]) -> Result<Option<Vec<ProgramTarget>>> {
    let (valid, failed_names): (Vec<&ProgramTarget>, Vec<(String, &std::path::Path)>) = targets
        .iter()
        .fold((Vec::new(), Vec::new()), |(mut valid, mut failed), t| {
            let name = t
                .debug_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            match crate::build::debug_check::check_debug_file(&t.debug_path) {
                Ok(true) => valid.push(t),
                Ok(false) | Err(_) => failed.push((name, t.debug_path.as_path())),
            }
            (valid, failed)
        });

    if failed_names.is_empty() {
        return Ok(Some(targets.to_vec()));
    }

    let cwd = std::env::current_dir().ok();

    if failed_names.len() == 1 {
        eprintln!("\n[seer][warn] The following .debug file does not contain proper DWARF debug information:");
    } else {
        eprintln!("\n[seer][warn] The following .debug files do not contain proper DWARF debug information:");
    }
    for (name, path) in &failed_names {
        let display = cwd
            .as_deref()
            .and_then(|cwd| path.strip_prefix(cwd).ok())
            .unwrap_or(path);
        eprintln!("  - {} ({})", name, display.display());
    }
    eprintln!("\nThis typically happens when building with Solana CLI below v3.");
    eprintln!("These programs would not work correctly and would not be deployed by Seer.");

    if valid.is_empty() {
        eprintln!("No valid programs remaining — nothing to deploy.");
        return Ok(None);
    }

    loop {
        print!("\nDo you want to proceed with the remaining {} valid program(s)? (yes/no): ", valid.len());
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        match input.trim().to_ascii_lowercase().as_str() {
            "yes" | "y" => return Ok(Some(valid.into_iter().cloned().collect())),
            "no" | "n" => return Ok(None),
            _ => eprintln!("Please enter 'yes'/'y' or 'no'/'n'."),
        }
    }
}


pub fn process_artifact(
    path: &PathBuf,
    rel: &dyn Fn(&PathBuf) -> PathBuf,
    files_to_send: &mut Vec<String>,
    artifacts: &mut Vec<SessionArtifact>,
    file_map: &mut HashMap<String, (PathBuf, u64)>
) -> Result<()> {
    let hash = make_blob(path)?;
    let size = std::fs::metadata(path)?.len();
    let rel_path = rel(path);
    files_to_send.push(rel_path.to_string_lossy().to_string());
    artifacts.push(SessionArtifact {
        file_path: rel_path.to_string_lossy().to_string(),
        file_hash: hash.clone(),
        file_size: size,
    });
    file_map.insert(hash.clone(), (rel_path.clone(), size));
    Ok(())
}

/// Creates a temporary `-pubkey.json` file from a `-keypair.json` file.
/// The pubkey file contains the base58-encoded public key as a plain string.
/// Returns the path to the created pubkey file.
pub fn create_pubkey_file(cleanup: bool, keypair_path: &PathBuf) -> Result<TempFile> {
    let keypair = read_keypair_file(keypair_path)
        .map_err(|e| anyhow::anyhow!("Failed to read keypair file {:?}: {}", keypair_path, e))?;
    let pubkey_str = keypair.pubkey().to_string();

    let file_name = keypair_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid keypair filename"))?;

    let program_name = file_name
        .strip_suffix("-keypair")
        .ok_or_else(|| anyhow::anyhow!("Keypair file doesn't end with -keypair: {}", file_name))?;

    let pubkey_path = keypair_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Keypair path has no parent directory"))?
        .join(format!("{}-pubkey.json", program_name));

    fs::write(&pubkey_path, format!("\"{}\"", &pubkey_str))?;

    Ok(TempFile::new(cleanup, pubkey_path))
}

/// Reads the operator pubkey from `~/.config/solana/id.json`.
pub fn get_operator_pubkey() -> Result<String> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let id_path = home.join(".config").join("solana").join("id.json");

    if !id_path.exists() {
        anyhow::bail!(
            "Solana identity keypair not found at {}. Run 'solana-keygen new' to create one.",
            id_path.display()
        );
    }

    let keypair = read_keypair_file(&id_path)
        .map_err(|e| anyhow::anyhow!(
            "Failed to read Solana identity keypair at {}: {}",
            id_path.display(),
            e
        ))?;

    Ok(keypair.pubkey().to_string())
}
