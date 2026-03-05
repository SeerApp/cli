use std::fs;
use std::path::{Component, PathBuf};

pub struct TempFile {
    cleanup: bool,
    path: PathBuf,
}

impl TempFile {
    pub fn new(cleanup: bool, path: PathBuf) -> Self {
        let path = path
            .components()
            .filter(|c| *c != Component::CurDir)
            .collect::<PathBuf>();

        Self { cleanup, path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = fs::remove_file(&self.path);
        }
    }
}

