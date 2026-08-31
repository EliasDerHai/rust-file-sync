use std::fs::{self, create_dir_all};
use std::io;
use std::path::Path;

/// directories to create (if not existent)
pub fn create_all_paths_if_not_exist(paths: Vec<&Path>) -> io::Result<()> {
    for path in paths.into_iter() {
        if !path.exists() {
            create_dir_all(path)?
        }
    }
    Ok::<(), io::Error>(())
}

pub fn create_file_if_not_exists(path: &Path) -> io::Result<()> {
    if !path.exists() {
        fs::write(path, "")?
    }

    Ok(())
}
