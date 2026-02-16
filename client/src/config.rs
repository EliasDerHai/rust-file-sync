use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::endpoint::ServerEndpoint;
use shared::register::ClientConfigDto;
use std::fs;
use tracing::info;
use uuid::Uuid;

/// local config (config.yaml)
#[derive(Debug, Deserialize, Serialize)]
pub struct LocalConfig {
    pub client_id: Option<String>,
    pub server_url: String,
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

    Ok(config)
}

/// Fetch config from server, or register local config first if not found
pub async fn fetch_or_register_config(
    client: &Client,
    local_config: &LocalConfig,
) -> ClientConfigDto {
    let config_endpoint = ServerEndpoint::Config.to_uri(&local_config.server_url);

    match client.get(&config_endpoint).send().await {
        Ok(response) if response.status().is_success() => match response.json().await {
            Ok(config) => {
                info!("Fetched registered config from server");
                config
            }
            Err(e) => {
                panic!("Failed to parse server config after registration: {}", e);
            }
        },
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
