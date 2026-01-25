use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub client_id: Option<String>,
    pub server_url: String,
    pub path_to_monitor: PathBuf,
    pub exclude_dirs: Vec<String>,
    pub min_poll_interval_in_ms: u16,
}

pub fn read_config() -> Result<Config, String> {
    let config_path = if fs::metadata("config.yaml").is_ok() {
        "config.yaml"
    } else {
        "config.yml"
    };

    let content =
        fs::read_to_string(config_path).map_err(|e| format!("Config file not found - {}", e))?;

    let mut config: Config =
        serde_yaml::from_str(&content).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    // Generate and persist client_id if missing
    if config.client_id.is_none() {
        let new_id = Uuid::new_v4().to_string();
        info!("Generated new client_id: {}", new_id);
        config.client_id = Some(new_id);

        let updated_content = serde_yaml::to_string(&config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(config_path, updated_content)
            .map_err(|e| format!("Failed to write config with new client_id: {}", e))?;

        info!("Persisted client_id to {}", config_path);
    }

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
    if !config.exclude_dirs.is_empty() {
        info!("Found exclude patterns in config {:?}", config.exclude_dirs);
    }
    Ok(config)
}
