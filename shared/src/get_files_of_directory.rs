use crate::matchable_path::MatchablePath;
use crate::utc_millis::UtcMillis;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::Metadata;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FileDescription {
    // eg. "test.txt"
    pub file_name: String,
    // contains file_name eg. "./dir/test.txt"
    pub relative_path: MatchablePath,
    pub size_in_bytes: u64,
    pub file_type: String,
    pub last_updated_utc_millis: UtcMillis,
}

pub fn get_file_description(
    target: &Path,
    reference_root: &Path,
) -> Result<FileDescription, String> {
    match fs::metadata(target) {
        Ok(m) => {
            if m.is_file() {
                let relative_path = target.strip_prefix(reference_root).unwrap();
                let name = relative_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                let file_type = name
                    .rfind('.')
                    .map(|p| name[p..].to_string())
                    .unwrap_or("".to_string());
                let last_updated_utc_millis =
                    get_last_updated(&m).ok_or("Could not determine last updated".to_string())?;
                let description = FileDescription {
                    file_name: name,
                    relative_path: MatchablePath::from(relative_path),
                    size_in_bytes: m.len(),
                    file_type,
                    last_updated_utc_millis,
                };
                Ok(description)
            } else {
                Err(format!("{:?} is not a file", target))
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn get_all_file_descriptions(
    path: &Path,
    exclude_dirs: &Vec<String>,
) -> Result<Vec<FileDescription>, String> {
    inner_get_files_of_dir_rec(path, path, Vec::new(), exclude_dirs)
}

fn inner_get_files_of_dir_rec(
    // the dir we're scanning in the scope of this recursive iteration
    current_path: &Path,
    // the reference to which we compare our path in order to determine the relative path ("./data")
    reference_root_path: &Path,
    // the already collected elements in prev. recursive iterations
    mut descriptions: Vec<FileDescription>,
    exclude_dirs: &Vec<String>,
) -> Result<Vec<FileDescription>, String> {
    for entry_result in fs::read_dir(current_path).map_err(|e| e.to_string())? {
        let entry = entry_result.map_err(|e| e.to_string())?;
        let entry_path = entry.path();

        for exclude_dir in exclude_dirs {
            if entry_path.to_string_lossy().contains(exclude_dir) {
                continue;
            }
        }

        if entry_path.is_file() {
            // mac os specific
            if let Some(s) = entry_path
                .file_name()
                .map(std::ffi::OsStr::to_string_lossy)
                .map(|s| s.to_lowercase())
                && &s == ".ds_store"
            {
                continue;
            }

            let relative_path = Path::new("./").join(
                entry_path
                    .strip_prefix(reference_root_path)
                    .map_err(|e| e.to_string())?,
            );
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            let name = entry_path.file_name().unwrap().to_os_string();
            let file_type = entry_path
                .extension()
                .and_then(|os| os.to_str())
                .unwrap_or("")
                .to_string();
            let last_updated_utc_millis = get_last_updated(&metadata)
                .ok_or("Could not determine last updated".to_string())?;
            let description = FileDescription {
                file_name: name.to_string_lossy().to_string(),
                relative_path: MatchablePath::from(relative_path),
                size_in_bytes: metadata.len(),
                file_type,
                last_updated_utc_millis,
            };
            descriptions.push(description);
        } else if entry_path.is_dir() {
            descriptions = inner_get_files_of_dir_rec(
                &entry_path,
                reference_root_path,
                descriptions,
                exclude_dirs,
            )?;
        }
    }

    Ok(descriptions)
}

fn get_last_updated(metadata: &Metadata) -> Option<UtcMillis> {
    if let Ok(modified) = metadata.modified() {
        return Some(UtcMillis::from(modified));
    }
    None
}
