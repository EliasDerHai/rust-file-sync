use std::{fs, io, path::Path};

use serde::Serialize;


#[derive(Debug, Serialize, Clone)]
pub struct FileDescription {
    name: String,
    size_in_kb: u32,
    file_type: String,
    // created:
}

pub fn init_directory(path: &Path) -> io::Result<()> {
    if !path.exists() {
        fs::create_dir(path)
    } else {
        Ok(())
    }
}

pub fn get_files_of_dir(path: &Path) -> io::Result<Vec<FileDescription>> {
    let mut descriptions = Vec::new();

    for entry_result in fs::read_dir(path)? {
        let entry = entry_result?;
        let path = entry.path();

        if path.is_file() {
            let metadata = entry.metadata()?;
            let file_size_in_bytes = metadata.len();
            let size_in_kb =  file_size_in_bytes / 1024;
            let name = path.file_name()
                .and_then(|os| os.to_str())
                .unwrap_or("[invalid utf8]")
                .to_string();
            let file_type = path.extension()
                .and_then(|os| os.to_str())
                .unwrap_or("")
                .to_string();

            descriptions.push(FileDescription {
                name,
                size_in_kb: size_in_kb as u32,
                file_type,
            });
        }
    }

    Ok(descriptions)
}
