use std::fs;
use std::path::PathBuf;

pub fn read_config() -> Result<PathBuf, String> {
    let content = fs::read_to_string("config")
        .map_err(|e| format!("Config file not found - {}", e))?
        .trim()
        .to_string();
    let path_buf = PathBuf::from(content);

    if !path_buf.exists() {
        return Err(format!("Configured path ('{:?}') does not exist", path_buf));
    }
    if !path_buf.is_dir() {
        return Err(format!(
            "Configured path ('{:?}') does not point to directory",
            path_buf
        ));
    }

    Ok(path_buf)
}
