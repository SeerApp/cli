use anyhow::{Result, Context};
use cargo_metadata::{MetadataCommand, Package};
use std::path::PathBuf;

/// Represents a Solana program package
#[derive(Debug)]
pub struct SolanaProgram {
    pub name: String,
    pub manifest_path: PathBuf,
    pub is_anchor: bool,
}

/// Detects Solana native programs in the current project (single-package or workspace).
/// Only returns programs whose manifest is under the current working directory,
/// so running from a subdirectory of a large workspace won't build everything.
pub fn detect_solana_programs() -> Result<Vec<SolanaProgram>> {
    let cwd = std::env::current_dir().context("Failed to get current working directory")?;
    let cwd = dunce::canonicalize(&cwd).unwrap_or(cwd);

    let metadata = MetadataCommand::new()
        .exec()
        .context("Failed to run cargo metadata")?;

    let mut programs = Vec::new();
    for package in metadata.packages {
        let manifest_std = package.manifest_path.clone().into_std_path_buf();
        let manifest_dir = manifest_std.parent().unwrap_or(&manifest_std);
        let manifest_dir = dunce::canonicalize(manifest_dir).unwrap_or(manifest_dir.to_path_buf());

        if !manifest_dir.starts_with(&cwd) {
            continue;
        }

        if is_program(&package) {
            programs.push(SolanaProgram {
                name: package.name.clone(),
                manifest_path: manifest_std,
                is_anchor: is_anchor_package(&package),
            });
        }
    }
    Ok(programs)
}

/// Checks if a package uses anchor-lang
fn is_anchor_package(package: &Package) -> bool {
    package.dependencies.iter().any(|d| d.name == "anchor-lang")
}

/// Heuristically checks if a package is a Solana native or Anchor program
fn is_program(package: &Package) -> bool {
    // Check for solana-program, anchor-lang, or pinocchio dependency and crate-type = ["cdylib"]
    let has_solana_dep = package.dependencies.iter().any(|d| {
        d.name == "solana-program" || d.name == "anchor-lang" || d.name == "pinocchio"
    });
    let is_cdylib = package.targets.iter().any(|t| t.crate_types.contains(&"cdylib".to_string()));
    has_solana_dep && is_cdylib
}
