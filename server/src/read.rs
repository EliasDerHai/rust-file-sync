use std::{
    fs::{self, Metadata},
    io,
    path::Path,
    time::UNIX_EPOCH,
};

use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct FileDescription {
    // eg. "test.txt"
    file_name: String,
    // contains file_name eg. "./dir/test.txt"
    relative_path: String,
    size_in_bytes: u64,
    file_type: String,
    last_updated_utc_millis: u64,
}

pub async fn init_directories(
    upload_path: &Path,
    backup_path: &Path,
    csv_path: &Path,
) -> io::Result<()> {
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

pub fn get_files_of_dir_rec(path: &Path) -> Result<Vec<FileDescription>, String> {
    inner_get_files_of_dir_rec(path, path, Vec::new())
}

fn inner_get_files_of_dir_rec(
    // the dir we're scanning in the scope of this recursive iteration
    current_path: &Path,
    // the reference to which we compare our path in order to determine the relative path ("./data")
    reference_root_path: &Path,
    // the already collected elements in prev. recursive iterations
    mut descriptions: Vec<FileDescription>,
) -> Result<Vec<FileDescription>, String> {
    for entry_result in fs::read_dir(current_path).map_err(|e| e.to_string())? {
        let entry = entry_result.map_err(|e| e.to_string())?;
        let entry_path = entry.path();

        if entry_path.is_file() {
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            let name = entry_path.file_name().unwrap().to_os_string();
            let relative_path = Path::new("./").join(
                entry_path
                    .strip_prefix(reference_root_path)
                    .map_err(|e| e.to_string())?,
            );
            let file_type = entry_path
                .extension()
                .and_then(|os| os.to_str())
                .unwrap_or("")
                .to_string();
            let last_updated_utc_millis = get_last_updated(&metadata)
                .ok_or("Could not determine last updated".to_string())?;
            descriptions.push(FileDescription {
                file_name: name.to_string_lossy().to_string(),
                relative_path: relative_path.to_string_lossy().to_string(),
                size_in_bytes: metadata.len(),
                file_type,
                last_updated_utc_millis,
            });
        } else if entry_path.is_dir() {
            descriptions =
                inner_get_files_of_dir_rec(&entry_path, reference_root_path, descriptions)?;
        }
    }

    Ok(descriptions)
}

fn get_last_updated(metadata: &Metadata) -> Option<u64> {
    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
            return Some(duration.as_millis() as u64);
        }
    }
    None
}
