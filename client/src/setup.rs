// SETUP -----------------------------------------------------------------------

use std::{path::PathBuf, process::Command, thread::sleep, time::Duration};

use crate::{
    ClientState, WatchGroup,
    config::{self, fetch_watch_config},
};
use reqwest::{
    Client,
    header::{HeaderMap, HeaderValue},
};
use shared::endpoint::{CLIENT_HOST_HEADER_KEY, CLIENT_ID_HEADER_KEY, ServerEndpoint};
use tracing::{info, warn};
use uuid::Uuid;

pub async fn setup() -> (ClientState, Client) {
    let config = match config::read_config() {
        Ok(config) => config,
        Err(error) => panic!("Config could not be processed: {:?}", error),
    };

    check_server_reachable(&config.server_url).await;

    let hostname = Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .ok();

    let client = build_http_client(&hostname, &config.client_id);

    let watch_config = fetch_watch_config(&client, &config).await;

    watch_config.watch_groups.values().for_each(|wg| {
        info!(
            "{}monitoring '{}'",
            hostname
                .as_ref()
                .map(|h| format!("{h} "))
                .unwrap_or_default(),
            wg.path_to_monitor
        )
    });

    info!("Poll_interval={}ms", watch_config.min_poll_interval_in_ms);

    (
        ClientState {
            server_url: config.server_url,
            min_poll_interval_in_ms: watch_config.min_poll_interval_in_ms,
            watch_groups: watch_config
                .watch_groups
                .into_iter()
                .map(|(key, value)| {
                    (
                        key,
                        WatchGroup {
                            name: value.name,
                            path_to_monitor: PathBuf::from(value.path_to_monitor),
                            exclude_dirs: value.exclude_dirs,
                            exclude_dot_dirs: value.exclude_dot_dirs,
                        },
                    )
                })
                .collect(),
        },
        client,
    )
}

fn build_http_client(hostname: &Option<String>, client_id: &Uuid) -> Client {
    let mut headers = HeaderMap::new();
    if let Some(h) = hostname {
        headers.insert(
            CLIENT_HOST_HEADER_KEY,
            HeaderValue::from_str(h).expect("Invalid hostname for header"),
        );
    }
    headers.insert(
        CLIENT_ID_HEADER_KEY,
        HeaderValue::from_str(&client_id.to_string()).expect("Invalid client_id for header"),
    );
    Client::builder()
        .default_headers(headers)
        .build()
        .expect("Failed to build HTTP client")
}

async fn check_server_reachable(server_url: &str) {
    let hello_endpoint = ServerEndpoint::Ping.to_uri(server_url);
    let client = Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    info!("Testing server at '{}'", &hello_endpoint);

    let mut confirmed_availablity = false;
    let mut attempts = 0;

    while !confirmed_availablity {
        match client.get(&hello_endpoint).send().await {
            Err(_) => {
                let time_out = Duration::from_secs(5 * attempts * attempts);
                warn!(
                    "{hello_endpoint} not reachable - attempt {attempts} - retrying in {}",
                    humantime::format_duration(time_out)
                );
                sleep(time_out);
                attempts += 1;
            }
            Ok(_) => {
                info!("Server confirmed at {}!", &hello_endpoint);
                confirmed_availablity = true;
            }
        }
    }
}
