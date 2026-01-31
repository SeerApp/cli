use std::path::PathBuf;
use std::fs;

/// Attempts to detect the correct artifacts (deploy) directory for a Solana project.
/// Handles Anchor, solo, and workspace layouts.
/// Returns the PathBuf to the deploy/target directory, or an error if not found.
pub fn detect_artifacts_dir(project_root: &PathBuf) -> anyhow::Result<PathBuf> {
    fn rel<P: AsRef<std::path::Path>>(root: &PathBuf, path: P) -> PathBuf {
        path.as_ref().strip_prefix(root).unwrap_or(path.as_ref()).to_path_buf()
    }

    let anchor_toml = project_root.join("Anchor.toml");
    if anchor_toml.exists() {
        let anchor_deploy = project_root.join("target/deploy");
        if anchor_deploy.exists() {
            return Ok(rel(project_root, anchor_deploy));
        }
    }

    let cargo_toml = project_root.join("Cargo.toml");
    if cargo_toml.exists() {
        let solo_deploy = project_root.join("target/deploy");
        if solo_deploy.exists() {
            return Ok(rel(project_root, solo_deploy));
        }
        let solo_target = project_root.join("target");
        if solo_target.exists() {
            return Ok(rel(project_root, solo_target));
        }
    }

    let programs_dir = project_root.join("programs");  
    if programs_dir.is_dir() {  
        for entry in fs::read_dir(&programs_dir)? {  
            let path = entry?.path();  
            if !path.is_dir() {  
                continue;  
            }  
            
            let deploy = path.join("target/deploy");  
            if deploy.exists() {  
                return Ok(rel(project_root, deploy));  
            }  
            let target = path.join("target");
            if target.exists() {
                return Ok(rel(project_root, target));
            }
        }
    }

    for entry in fs::read_dir(project_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let deploy = path.join("target/deploy");
            if deploy.exists() {
                return Ok(rel(project_root, deploy));
            }
        }
    }

    anyhow::bail!("Could not detect artifacts (deploy) directory in {:?}", project_root)
}