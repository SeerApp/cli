use anyhow::Result;
use object::Object;
use std::fs;
use std::path::Path;

/// Convert failed debug checks to error messages for summary output
pub fn collect_failed_debug_infos(failed_debugs_raw: &[(String, std::path::PathBuf)]) -> Vec<super::build::FailedDebugInfo> {
    let mut failed_debugs = Vec::new();
    for (name, path) in failed_debugs_raw {
        let error_msg = if !path.exists() {
            format!(".debug file not found: {}", path.display())
        } else {
            // This is a fallback, ideally debug_check should return error string
            format!("debug check failed: {}", path.display())
        };
        failed_debugs.push(super::build::FailedDebugInfo { name: name.clone(), path: path.clone(), error: error_msg });
    }
    failed_debugs
}

pub fn check_debug_file(debug_path: &Path) -> Result<bool> {
    let data = fs::read(&debug_path)?;
    let obj = object::File::parse(&*data)?;

    if let Some(_) = obj.section_by_name(".debug_info") {
        return Ok(true);
    }

    Ok(false)
}

pub fn check_all_debug_files(
    programs: &[super::project::SolanaProgram],
    silent: bool,
) -> (Vec<std::path::PathBuf>, Vec<(String, std::path::PathBuf)>) {
    let mut valid_debug_files = Vec::new();
    let mut failed_programs = Vec::new();
    for prog in programs {
        let manifest_dir = prog.manifest_path.parent().unwrap();
        let mut workspace_root = manifest_dir;
        let mut found = false;
        for ancestor in manifest_dir.ancestors() {
            let deploy_dir = ancestor.join("target/deploy");
            if deploy_dir.exists() {
                workspace_root = ancestor;
                found = true;
                break;
            }
        }
        if !found {
            workspace_root = manifest_dir.parent().unwrap_or(manifest_dir);
        }
        // Rust replaces hyphens with underscores in output filenames
        let file_base = prog.name.replace('-', "_");
        let debug_path = workspace_root
            .join("target/deploy")
            .join(format!("{}.debug", file_base));
        if debug_path.exists() {
            match check_debug_file(&debug_path) {
                Ok(true) => {
                    valid_debug_files.push(debug_path.clone());
                    if !silent {
                        println!("[OK] {}", debug_path.display());
                    }
                }
                Ok(false) => {
                    failed_programs.push((prog.name.clone(), debug_path.clone()));
                    eprintln!("[FAIL] {}: insufficient DWARF info", debug_path.display());
                }
                Err(e) => {
                    failed_programs.push((prog.name.clone(), debug_path.clone()));
                    eprintln!("[ERROR] {}: {}", debug_path.display(), e);
                }
            }
        } else {
            failed_programs.push((prog.name.clone(), debug_path.clone()));
            eprintln!("[ERROR] .debug file not found: {}", debug_path.display());
        }
    }
    if !silent {
        println!("\nSummary of .debug files for Seer:");
        if !valid_debug_files.is_empty() {
            println!("Ready for Seer:");
            for path in &valid_debug_files {
                println!("- {}", path.display());
            }
        } else {
            println!("No valid .debug files found.");
        }
        if !failed_programs.is_empty() {
            println!("\nNot ready for Seer:");
            for (name, path) in &failed_programs {
                println!("- {}: {}", name, path.display());
            }
        }
    }
    (valid_debug_files, failed_programs)
}
