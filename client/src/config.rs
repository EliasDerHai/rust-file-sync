use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::endpoint::ServerEndpoint;
use shared::register::WatchConfigDto;
use std::{collections::VecDeque, env, fs, path::PathBuf};
use tracing::info;
use uuid::Uuid;

/// local config (config.yaml)
#[derive(Debug, Deserialize, Serialize)]
struct LocalConfig {
    client_id: Option<String>,
    server_url: String,
}

#[derive(Debug)]
pub struct Config {
    pub client_id: Uuid,
    pub server_url: String,
}

fn read_local_config() -> Option<(PathBuf, LocalConfig)> {
    let mut paths: VecDeque<PathBuf> = ["./config.yaml", "./config.yml"]
        .into_iter()
        .map(PathBuf::from)
        .collect();

    if let Some(arg1) = env::args().nth(1) {
        paths.push_front(PathBuf::from(arg1));
    }

    paths.into_iter().find_map(|path| {
        fs::read_to_string(&path)
            .map_err(|e| format!("Config read failed ({}): {}", path.display(), e))
            .and_then(|s| {
                serde_yaml::from_str::<LocalConfig>(&s)
                    .map_err(|e| format!("Config parse failed ({}): {}", path.display(), e))
            })
            .ok()
            .map(|config| (path, config))
    })
}

pub fn read_config() -> Result<Config, String> {
    match read_local_config() {
        None => Err("no config.yaml found".to_string()),
        Some((ref config_path, mut config)) => {
            Ok(match config.client_id {
                Some(client_id) => {
                    let server_url = config.server_url;
                    let client_id = Uuid::parse_str(&client_id)
                        .map_err(|e| format!("Could not parse uuid from yaml: {}", e))?;
                    Config {
                        client_id,
                        server_url,
                    }
                }
                // Generate and persist client_id if missing
                None => {
                    let new_id = Uuid::new_v4();
                    info!("Generated new client_id: {}", new_id);
                    config.client_id = Some(new_id.to_string());

                    let updated_content = serde_yaml::to_string(&config)
                        .map_err(|e| format!("Failed to serialize config: {}", e))?;
                    fs::write(config_path, updated_content)
                        .map_err(|e| format!("Failed to write config with new client_id: {}", e))?;

                    info!(
                        "Persisted client_id to {}",
                        config_path.to_str().unwrap_or("")
                    );
                    Config {
                        client_id: new_id,
                        server_url: config.server_url,
                    }
                }
            })
        }
    }
}

/// Fetch config from server
pub async fn fetch_watch_config(client: &Client, config: &Config) -> WatchConfigDto {
    let config_endpoint = ServerEndpoint::Config.to_uri(&config.server_url);

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
