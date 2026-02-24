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
    exclude_dot_dirs: bool,
) -> Result<Vec<FileDescription>, String> {
    inner_get_files_of_dir_rec(path, path, Vec::new(), exclude_dirs, exclude_dot_dirs)
}

fn inner_get_files_of_dir_rec(
    // the dir we're scanning in the scope of this recursive iteration
    current_path: &Path,
    // the reference to which we compare our path in order to determine the relative path ("./data")
    reference_root_path: &Path,
    // the already collected elements in prev. recursive iterations
    mut descriptions: Vec<FileDescription>,
    exclude_dirs: &Vec<String>,
    exclude_dot_dirs: bool,
) -> Result<Vec<FileDescription>, String> {
    for entry_result in fs::read_dir(current_path).map_err(|e| e.to_string())? {
        let entry = entry_result.map_err(|e| e.to_string())?;
        let entry_path = entry.path();

        let entry_name = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if exclude_dot_dirs && entry_name.starts_with('.') {
            continue;
        }

        if exclude_dirs
            .iter()
            .any(|excl| entry_path.to_string_lossy().contains(excl.as_str()))
        {
            continue;
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
                exclude_dot_dirs,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Creates a file at `path`, creating parent directories as needed.
    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    fn names(descriptions: &[FileDescription]) -> Vec<String> {
        let mut names: Vec<String> = descriptions.iter().map(|d| d.file_name.clone()).collect();
        names.sort();
        names
    }

    #[test]
    fn excludes_dot_dirs_when_flag_is_true() {
        let root = std::env::temp_dir().join("rfs_test_dot_dirs_excluded");
        let _ = fs::remove_dir_all(&root);
        touch(&root.join("normal.txt"));
        touch(&root.join(".obsidian").join("workspace.json"));
        touch(&root.join(".git").join("config"));

        let result = get_all_file_descriptions(&root, &vec![], true).unwrap();

        assert_eq!(names(&result), vec!["normal.txt"]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn includes_dot_dirs_when_flag_is_false() {
        let root = std::env::temp_dir().join("rfs_test_dot_dirs_included");
        let _ = fs::remove_dir_all(&root);
        touch(&root.join("normal.txt"));
        touch(&root.join(".obsidian").join("workspace.json"));

        let result = get_all_file_descriptions(&root, &vec![], false).unwrap();

        assert_eq!(names(&result), vec!["normal.txt", "workspace.json"]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn excludes_named_dirs() {
        let root = std::env::temp_dir().join("rfs_test_named_dirs");
        let _ = fs::remove_dir_all(&root);
        touch(&root.join("keep.txt"));
        touch(&root.join("node_modules").join("lodash").join("index.js"));
        touch(&root.join("src").join("main.rs"));

        let result =
            get_all_file_descriptions(&root, &vec!["node_modules".to_string()], false).unwrap();

        assert_eq!(names(&result), vec!["keep.txt", "main.rs"]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn excludes_dot_dirs_even_when_exclude_dirs_is_empty() {
        // This was the primary bug: exclude_dot_dirs was nested inside the
        // `for exclude_dir in exclude_dirs` loop, so it never ran when exclude_dirs
        // was empty.
        let root = std::env::temp_dir().join("rfs_test_dot_dirs_empty_excl");
        let _ = fs::remove_dir_all(&root);
        touch(&root.join("readme.md"));
        touch(&root.join(".obsidian").join("workspace.json"));

        let result = get_all_file_descriptions(&root, &vec![], true).unwrap();

        assert_eq!(names(&result), vec!["readme.md"]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn dot_files_at_root_are_also_excluded_when_flag_is_true() {
        let root = std::env::temp_dir().join("rfs_test_dot_files_root");
        let _ = fs::remove_dir_all(&root);
        touch(&root.join("normal.txt"));
        touch(&root.join(".hidden_file"));

        let result = get_all_file_descriptions(&root, &vec![], true).unwrap();

        assert_eq!(names(&result), vec!["normal.txt"]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn combines_named_and_dot_exclusions() {
        let root = std::env::temp_dir().join("rfs_test_combined");
        let _ = fs::remove_dir_all(&root);
        touch(&root.join("keep.txt"));
        touch(&root.join(".obsidian").join("workspace.json"));
        touch(&root.join("node_modules").join("index.js"));
        touch(&root.join("src").join("lib.rs"));

        let result =
            get_all_file_descriptions(&root, &vec!["node_modules".to_string()], true).unwrap();

        assert_eq!(names(&result), vec!["keep.txt", "lib.rs"]);
        fs::remove_dir_all(&root).unwrap();
    }
}
