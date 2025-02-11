use serde::Deserialize;
use serde_yaml;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server_url: String,
    pub path_to_monitor: PathBuf,
    pub min_poll_interval_in_ms: u16
}

pub fn read_config() -> Result<Config, String> {
    let content = fs::read_to_string("config.yaml")
        .or(fs::read_to_string("config.yml"))
        .map_err(|e| format!("Config file not found - {}", e))?;

    let config: Config =
        serde_yaml::from_str(&content).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    if !config.path_to_monitor.exists() {
        return Err(format!(
            "Configured vault_path ('{:?}') does not exist",
            config.path_to_monitor
        ));
    }
    if !config.path_to_monitor.is_dir() {
        return Err(format!(
            "Configured vault_path ('{:?}') is not a directory",
            config.path_to_monitor
        ));
    }
    Ok(config)
}
