use anyhow::{Result, Context};
use cargo_metadata::{MetadataCommand, Package};
use std::path::PathBuf;

/// Represents a Solana program package
#[derive(Debug)]
pub struct SolanaProgram {
    pub name: String,
    pub manifest_path: PathBuf,
}

/// Detects Solana native programs in the current project (single-package or workspace)
pub fn detect_solana_programs() -> Result<Vec<SolanaProgram>> {
    let metadata = MetadataCommand::new()
        .exec()
        .context("Failed to run cargo metadata")?;
    let mut programs = Vec::new();
    for package in metadata.packages {
        if is_program(&package) {
            programs.push(SolanaProgram {
                name: package.name.clone(),
                manifest_path: package.manifest_path.clone().into_std_path_buf(),
            });
        }
    }
    Ok(programs)
}

/// Heuristically checks if a package is a Solana native or Anchor program
fn is_program(package: &Package) -> bool {
    // Check for solana-program or anchor-lang dependency and crate-type = ["cdylib"]
    let has_solana_or_anchor_dep = package.dependencies.iter().any(|d| d.name == "solana-program" || d.name == "anchor-lang");
    let is_cdylib = package.targets.iter().any(|t| t.crate_types.contains(&"cdylib".to_string()));
    has_solana_or_anchor_dep && is_cdylib
}
