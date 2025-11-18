use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

use crate::run::utils::normalize_sha;

pub fn make_blob(source_file: &PathBuf) -> Result<String> {
    let bytes = fs::read(source_file)?;
    Ok(normalize_sha(&format!("{:x}", Sha256::digest(&bytes))))
}
