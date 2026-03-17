use anyhow::{Result, Context};
use cargo_metadata::{MetadataCommand, Package};
use std::path::PathBuf;
use std::process::Command;

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

/// Runs `solana --version` and parses the major version number.
/// Expected output format: "solana-cli X.Y.Z ..."
pub fn get_solana_cli_major_version() -> Result<u64> {
    let output = Command::new("solana")
        .arg("--version")
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run 'solana --version': {}", e))?;

    if !output.status.success() {
        anyhow::bail!("'solana --version' exited with non-zero status");
    }

    let version_str = String::from_utf8_lossy(&output.stdout);

    let version_part = version_str
        .split_whitespace()
        .find(|s| s.chars().next().map_or(false, |c| c.is_ascii_digit()))
        .ok_or_else(|| anyhow::anyhow!("Could not find version number in: {}", version_str))?;

    let major: u64 = version_part
        .split('.')
        .next()
        .ok_or_else(|| anyhow::anyhow!("Could not parse major version from: {}", version_part))?
        .parse()
        .map_err(|_| anyhow::anyhow!("Major version is not a number in: {}", version_part))?;

    Ok(major)
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
