use anyhow::{Result, Context};
use toml_edit::{Document, value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::temp_file::TempFile;

/// Path for seer.toml file (in same dir as Cargo.toml)
fn seer_toml_path(cargo_toml_path: &Path) -> PathBuf {
    let dir = cargo_toml_path.parent().unwrap_or_else(|| Path::new("."));
    dir.join("Seer.toml")
}

/// Creates seer.toml with [profile.release] debug = true
/// Always overwrites seer.toml
pub fn create_seer_toml(cleanup: bool, cargo_toml_path: &Path) -> Result<TempFile> {
    let seer_path = seer_toml_path(cargo_toml_path);
    let mut doc = Document::new();
    doc["profile"] = toml_edit::table();
    let profile_table = doc["profile"].as_table_mut().unwrap();
    profile_table["release"] = toml_edit::table();
    let release_table = profile_table["release"].as_table_mut().unwrap();
    release_table["debug"] = value(true);
    fs::write(&seer_path, doc.to_string())
        .with_context(|| format!("Failed to write {}", seer_path.display()))?;
    Ok(TempFile::new(cleanup, seer_path))
}
