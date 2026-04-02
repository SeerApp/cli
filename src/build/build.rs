use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Stdio;
use std::process::Command;

use crate::temp_file::TempFile;

#[allow(dead_code)]
pub struct BuildResult {
    pub name: String,
    pub manifest_path: PathBuf,
    pub status: BuildStatus,
    pub error: Option<String>,
}

/// Represents a failed debug check with error message
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FailedDebugInfo {
    pub name: String,
    pub path: std::path::PathBuf,
    pub error: String,
}

pub enum BuildStatus {
    Success,
    Failed,
}

/// Build all programs and collect results
pub fn build_all_programs(programs: &[super::project::SolanaProgram], seer_toml_paths: &[(String, TempFile)]) -> Vec<BuildResult> {
    let mut build_results = Vec::new();
    for (i, prog) in programs.iter().enumerate() {
        println!("Building {}...", prog.name);
        let seer_toml_path = &seer_toml_paths[i].1;
        match build_program(&prog.manifest_path, seer_toml_path) {
            Ok(_) => {
                println!("Built {} successfully.", prog.name);
                if prog.is_anchor {
                    if let Some(dir) = prog.manifest_path.parent() {
                        build_anchor_idl_for_program(&prog.name, dir);
                    }
                }
                build_results.push(BuildResult {
                    name: prog.name.clone(),
                    manifest_path: prog.manifest_path.clone(),
                    status: BuildStatus::Success,
                    error: None,
                });
            }
            Err(e) => {
                eprintln!("Build failed for {}", prog.name);
                build_results.push(BuildResult {
                    name: prog.name.clone(),
                    manifest_path: prog.manifest_path.clone(),
                    status: BuildStatus::Failed,
                    error: Some(format!("{}", e)),
                });
            }
        }
    }
    build_results
}

pub fn build_program(manifest_path: &Path, seer_toml_path: &TempFile) -> Result<()> {
    let manifest_dir = manifest_path.parent().context("No parent directory for manifest")?;
    let mut child = Command::new("cargo-build-sbf")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--debug")
        .arg("--")
        .arg("--config")
        .arg(seer_toml_path.path())
        .current_dir(manifest_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to spawn cargo-build-sbf")?;
    let status = child.wait().context("Failed to wait for cargo-build-sbf")?;
    if !status.success() {
        // To get error details, rerun and capture output
        let output = Command::new("cargo-build-sbf")
            .arg("--manifest-path")
            .arg(manifest_path)
            .arg("--debug")
            .arg("--")
            .arg("--config")
            .arg(seer_toml_path.path())
            .current_dir(manifest_dir)
            .output()
            .context("Failed to rerun cargo-build-sbf for error output")?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Build failed for {}\nCargo error output:\n{}",
            manifest_path.display(),
            stderr
        );
    }
    Ok(())
}

/// Print build summary, considering both build and debug check results
pub fn print_build_summary(
    build_results: &[BuildResult],
    failed_debugs: &[FailedDebugInfo],
) {
    println!("\nBuild summary:");
    // Collect failed debug programs into a map for quick lookup
    let mut failed_debug_map: HashMap<&str, &FailedDebugInfo> = HashMap::new();
    for info in failed_debugs {
        failed_debug_map.insert(info.name.as_str(), info);
    }
    // Print succeeded builds first (only those that succeeded and passed debug check)
    for result in build_results {
        if let BuildStatus::Success = result.status {
            if !failed_debug_map.contains_key(result.name.as_str()) {
                println!("- {}: SUCCESS", result.name);
            }
        }
    }
    // Then print failed builds (either build failed or debug check failed)
    for result in build_results {
        let mut fail_reason = None;
        if let BuildStatus::Failed = result.status {
            fail_reason = result.error.clone();
        } else if let Some(info) = failed_debug_map.get(result.name.as_str()) {
            fail_reason = Some(info.error.clone());
        }
        if let Some(ref err) = fail_reason {
            println!("- {}: FAILED ({})", result.name, err);
        }
    }
    println!("seer build complete.");
}

/// Build all programs silently, printing only minimal info
pub fn build_all_programs_silent(programs: &[super::project::SolanaProgram], seer_toml_paths: &[(String, TempFile)]) -> Vec<BuildResult> {
    let mut build_results = Vec::new();
    for (i, prog) in programs.iter().enumerate() {
        println!("Building {}...", prog.name);
        let seer_toml_path = &seer_toml_paths[i].1;
        match build_program_silent(&prog.manifest_path, seer_toml_path) {
            Ok(_) => {
                println!("Built {} successfully.", prog.name);
                if prog.is_anchor {
                    if let Some(dir) = prog.manifest_path.parent() {
                        build_anchor_idl_for_program(&prog.name, dir);
                    }
                }
                build_results.push(BuildResult {
                    name: prog.name.clone(),
                    manifest_path: prog.manifest_path.clone(),
                    status: BuildStatus::Success,
                    error: None,
                });
            }
            Err(e) => {
                eprintln!("Build failed for {}", prog.name);
                build_results.push(BuildResult {
                    name: prog.name.clone(),
                    manifest_path: prog.manifest_path.clone(),
                    status: BuildStatus::Failed,
                    error: Some(format!("{}", e)),
                });
            }
        }
    }
    build_results
}

/// Run `anchor idl build` for a single program from its manifest directory.
fn build_anchor_idl_for_program(name: &str, manifest_dir: &Path) {
    println!("Building Anchor IDL for {}...", name);
    let output = Command::new("anchor")
        .arg("idl")
        .arg("build")
        .current_dir(manifest_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    match output {
        Ok(out) if out.status.success() => {
            println!("Anchor IDL built for {}.", name);
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("[seer][warn] Anchor IDL build failed for {}: {}", name, stderr.trim());
        }
        Err(e) => {
            eprintln!("[seer][warn] Could not run 'anchor idl build' for {}: {}", name, e);
        }
    }
}

/// Build a single program silently (no build output, just status)
pub fn build_program_silent(manifest_path: &Path, seer_toml_path: &TempFile) -> Result<()> {
    use std::process::Stdio;
    let manifest_dir = manifest_path.parent().context("No parent directory for manifest")?;
    let status = Command::new("cargo-build-sbf")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--debug")
        .arg("--")
        .arg("--config")
        .arg(seer_toml_path.path())
        .current_dir(manifest_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to run cargo-build-sbf")?;
    if !status.success() {
        // Rerun and capture output for error details
        let output = Command::new("cargo-build-sbf")
            .arg("--manifest-path")
            .arg(manifest_path)
            .arg("--debug")
            .arg("--")
            .arg("--config")
            .arg(seer_toml_path.path())
            .current_dir(manifest_dir)
            .output()
            .context("Failed to rerun cargo-build-sbf for error output")?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Build failed for {}\nCargo error output:\n{}",
            manifest_path.display(),
            stderr
        );
    }
    Ok(())
}
