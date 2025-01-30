use std::{
    fs::{self, Metadata},
    io,
    path::Path,
    time::UNIX_EPOCH,
};

use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct FileDescription {
    name: String,
    size_in_kb: u32,
    file_type: String,
    last_updated_utc_millis: u64,
}

pub fn init_directories(upload_path: &Path, backup_path: &Path, csv_path: &Path) -> io::Result<()> {
    if !upload_path.exists() {
        fs::create_dir_all(upload_path)?;
    }
    if !backup_path.exists() {
        fs::create_dir_all(backup_path)?;
    }
    if !csv_path.is_file() {
        fs::write(csv_path, b"")?;
    }
    Ok(())
}

pub fn get_files_of_dir(path: &Path) -> Result<Vec<FileDescription>, String> {
    let mut descriptions = Vec::new();

    for entry_result in fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry_result.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            let file_size_in_bytes = metadata.len();
            let size_in_kb = file_size_in_bytes / 1024;
            let name = path
                .file_name()
                .and_then(|os| os.to_str())
                .unwrap_or("[invalid utf8]")
                .to_string();
            let file_type = path
                .extension()
                .and_then(|os| os.to_str())
                .unwrap_or("")
                .to_string();
            let last_updated_utc_millis =
                get_last_updated(metadata).ok_or("Could not determine last updated".to_string())?;
            descriptions.push(FileDescription {
                name,
                size_in_kb: size_in_kb as u32,
                file_type,
                last_updated_utc_millis,
            });
        }
    }

    Ok(descriptions)
}

fn get_last_updated(metadata: Metadata) -> Option<u64> {
    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
            return Some(duration.as_millis() as u64);
        }
    }
    None
}
