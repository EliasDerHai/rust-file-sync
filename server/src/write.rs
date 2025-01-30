use std::fs;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

use axum::body::Bytes;

pub fn schedule_data_backups(data_path: &Path, backup_path: &Path) {
    todo!()
}

pub fn create_all_dir_and_write(path: &PathBuf, bytes: &Bytes) -> Result<(), std::io::Error> {
    create_dir_all(path.parent().unwrap_or(Path::new("./"))).and_then(|()| fs::write(&path, bytes))
}
