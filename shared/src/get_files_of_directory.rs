use std::fs;
use std::fs::Metadata;
use std::path::Path;
use std::time::UNIX_EPOCH;
use serde::{Deserialize, Serialize};
use crate::matchable_path::MatchablePath;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FileDescription {
    // eg. "test.txt"
    pub file_name: String,
    // contains file_name eg. "./dir/test.txt"
    pub relative_path: MatchablePath,
    pub size_in_bytes: u64,
    pub file_type: String,
    pub last_updated_utc_millis: u64,
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
                relative_path: MatchablePath::from(relative_path),
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
