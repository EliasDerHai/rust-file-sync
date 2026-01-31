use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::endpoint::ServerEndpoint;
use shared::register::ClientConfigDto;
use std::fs;
use std::path::PathBuf;
use tracing::{error, info, warn};
use uuid::Uuid;

/// local config (config.yaml)
#[derive(Debug, Deserialize, Serialize)]
pub struct LocalConfig {
    pub client_id: Option<String>,
    pub server_url: String,
    /// gonna be removed after being migrated
    path_to_monitor: PathBuf,
    exclude_dirs: Vec<String>,
    min_poll_interval_in_ms: u16,
}

pub fn read_config() -> Result<LocalConfig, String> {
    let config_path = if fs::metadata("config.yaml").is_ok() {
        "config.yaml"
    } else {
        "config.yml"
    };

    let content =
        fs::read_to_string(config_path).map_err(|e| format!("Config file not found - {}", e))?;

    let mut config: LocalConfig =
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

/// Runtime configuration combining local config (server_url) with server-provided config
pub struct RuntimeConfig {
    pub server_url: String,
    pub path_to_monitor: PathBuf,
    pub exclude_dirs: Vec<String>,
    pub min_poll_interval_in_ms: u16,
}

/// Fetch config from server, or register local config first if not found
pub async fn fetch_or_register_config(
    client: &Client,
    local_config: &LocalConfig,
) -> ClientConfigDto {
    let config_endpoint = ServerEndpoint::Config.to_uri(&local_config.server_url);

    // Try to fetch config from server
    match client.get(&config_endpoint).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<ClientConfigDto>().await {
                Err(e) => error!("Failed to parse server config: {}", e),
                Ok(config) => {
                    info!("Fetched config from server");
                    return config;
                }
            }
        }
        Ok(response) if response.status() == reqwest::StatusCode::NOT_FOUND => {
            info!("No config found on server, registering local config...");
        }
        Ok(response) => {
            warn!(
                "Unexpected response from server: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }
        Err(e) => {
            error!("Failed to fetch config from server: {}", e);
        }
    }

    // Register local config
    let request = ClientConfigDto {
        path_to_monitor: local_config.path_to_monitor.to_string_lossy().to_string(),
        exclude_dirs: local_config.exclude_dirs.clone(),
        exclude_dot_dirs: true,
        min_poll_interval_in_ms: local_config.min_poll_interval_in_ms,
    };

    match client.post(&config_endpoint).json(&request).send().await {
        Ok(response) if response.status().is_success() => {
            info!("Registered local config with server");
        }
        Ok(response) => {
            panic!(
                "Failed to register config: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }
        Err(e) => {
            panic!("Failed to register config: {}", e);
        }
    }

    // Fetch the registered config
    match client.get(&config_endpoint).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<ClientConfigDto>().await {
                Ok(config) => {
                    info!("Fetched registered config from server");
                    config
                }
                Err(e) => {
                    panic!("Failed to parse server config after registration: {}", e);
                }
            }
        }
        Ok(response) => {
            panic!(
                "Failed to fetch config after registration: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }
        Err(e) => {
            panic!("Failed to fetch config after registration: {}", e);
        }
    }
}
