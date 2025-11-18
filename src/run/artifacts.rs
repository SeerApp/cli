use anyhow::{Context, Result};
use solana_sdk::signature::read_keypair_file;
use solana_sdk::signer::Signer;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ProgramTarget {
    pub name: String,
    pub so_path: PathBuf,
    pub debug_path: PathBuf,
    pub pubkey: String,
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

    for (name, so_path) in so_files {
        let debug_path = match debug_files.get(&name) {
            Some(v) => v.clone(),
            None => continue, // skip incomplete target
        };

        let json_path = match json_files.get(&name) {
            Some(v) => v.clone(),
            None => continue,
        };

        let keypair = read_keypair_file(json_path);

        if keypair.is_err() {
            continue;
        }

        let pubkey = keypair.unwrap().pubkey().to_string();

        programs.push(ProgramTarget {
            name,
            so_path,
            debug_path,
            pubkey: pubkey,
        });
    }

    Ok(programs)
}
