use anyhow::Result;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anchor_lang_idl_spec::Idl;
use codama_nodes::RootNode;

#[derive(Debug, Clone)]
pub enum IdlFormat {
    Anchor,
    Codama,
}

impl std::fmt::Display for IdlFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdlFormat::Anchor => write!(f, "Anchor"),
            IdlFormat::Codama => write!(f, "Codama"),
        }
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct IdlFile {
    pub path: PathBuf,
    pub program_name: String,
    pub format: IdlFormat,
}

/// Try to parse IDL content as Anchor or Codama format.
/// Returns the detected format or an error if neither matches.
pub fn validate_idl(content: &str) -> Result<IdlFormat> {
    if serde_json::from_str::<Idl>(content).is_ok() {
        return Ok(IdlFormat::Anchor);
    }

    if serde_json::from_str::<RootNode>(content).is_ok() {
        return Ok(IdlFormat::Codama);
    }

    anyhow::bail!("IDL file is neither a valid Anchor IDL nor a valid Codama IDL")
}

/// Discover IDL files by directly probing known directories for `{program_name}.json`.
fn discover_idl_files(cwd: &Path, program_names: &[String]) -> Vec<PathBuf> {
    let known_dirs = [cwd.join("target/idl"), cwd.join("target/types")];
    let mut found = Vec::new();

    for name in program_names {
        for dir in &known_dirs {
            let candidate = dir.join(format!("{}.json", name));
            if candidate.is_file() {
                found.push(candidate);
                break; // first match per program wins
            }
        }
    }

    found
}

/// Collect and validate IDL files.
/// If user provided `--idl-file` paths, only those are used (no auto-discovery).
/// Otherwise, auto-discover from known directories.
pub fn collect_idl_files(
    cwd: &Path,
    program_names: &[String],
    user_idl_paths: &[PathBuf],
) -> Result<Vec<IdlFile>> {
    let candidate_paths = if !user_idl_paths.is_empty() {
        user_idl_paths.to_vec()
    } else {
        discover_idl_files(cwd, program_names)
    };

    if candidate_paths.is_empty() {
        return Ok(Vec::new());
    }

    let mut valid_idls = Vec::new();
    let mut invalid_idls = Vec::new();

    for path in &candidate_paths {
        if !path.exists() {
            eprintln!(
                "[seer][warn] IDL file does not exist: {}",
                path.display()
            );
            invalid_idls.push(path.clone());
            continue;
        }

        // Derive program name from file stem and validate it matches a known program
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let program_name = match program_names.iter().find(|n| n.as_str() == stem) {
            Some(name) => name.clone(),
            None => {
                eprintln!(
                    "[seer][warn] IDL file '{}' does not match any program .so name. Expected one of: {:?}",
                    path.display(),
                    program_names
                );
                invalid_idls.push(path.clone());
                continue;
            }
        };

        // Read and validate format
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "[seer][warn] Could not read IDL file {}: {}",
                    path.display(),
                    e
                );
                invalid_idls.push(path.clone());
                continue;
            }
        };

        match validate_idl(&content) {
            Ok(format) => {
                valid_idls.push(IdlFile {
                    path: path.clone(),
                    program_name,
                    format,
                });
            }
            Err(e) => {
                eprintln!(
                    "  [FAIL] {}: {}",
                    path.display(),
                    e
                );
                invalid_idls.push(path.clone());
            }
        }
    }

    if !invalid_idls.is_empty() {
        println!(
            "\n{} IDL file(s) could not be validated.",
            invalid_idls.len()
        );

        if valid_idls.is_empty() {
            if !ask_continue_without_idl() {
                anyhow::bail!("Aborted by user due to invalid IDL files.");
            }
            return Ok(Vec::new());
        } else {
            println!(
                "{} valid IDL(s) will be included. Invalid ones will be skipped.",
                valid_idls.len()
            );
            if !ask_continue_with_partial_idl() {
                anyhow::bail!("Aborted by user due to invalid IDL files.");
            }
        }
    }

    Ok(valid_idls)
}

fn ask_continue_without_idl() -> bool {
    loop {
        print!("No valid IDL files found. Continue without IDL? (yes/no): ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        match input.trim().to_ascii_lowercase().as_str() {
            "yes" | "y" => return true,
            "no" | "n" => return false,
            _ => println!("Please enter 'yes' or 'no'."),
        }
    }
}

fn ask_continue_with_partial_idl() -> bool {
    loop {
        print!("Continue with valid IDLs only, skipping invalid ones? (yes/no): ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        match input.trim().to_ascii_lowercase().as_str() {
            "yes" | "y" => return true,
            "no" | "n" => return false,
            _ => println!("Please enter 'yes' or 'no'."),
        }
    }
}
