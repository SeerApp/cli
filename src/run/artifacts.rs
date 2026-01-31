use anyhow::{Context, Result};
use solana_sdk::signature::read_keypair_file;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;


#[derive(Debug, Clone)]
pub struct ProgramTarget {
    pub name: String,
    pub so_path: PathBuf,
    pub debug_path: PathBuf,
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
            name,
            so_path,
            debug_path,
        });
    }

    Ok(programs)
}

