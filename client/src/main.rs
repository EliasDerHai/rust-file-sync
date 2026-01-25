use crate::config::{read_config, Config};
use futures_util::future::join_all;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use shared::endpoint::{ServerEndpoint, CLIENT_HOST_HEADER_KEY, CLIENT_ID_HEADER_KEY};
use shared::get_files_of_directory::{get_all_file_descriptions, FileDescription};
use shared::register::RegisterClientRequest;
use shared::sync_instruction::SyncInstruction;
use std::ops::Add;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{error, info, trace, warn};
use tracing_subscriber::EnvFilter;

mod config;
mod execute;

#[tokio::main]
async fn main() {
    let log_level = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(log_level).init();

    let config = match read_config() {
        Err(error) => {
            panic!(
                "Critical error - config could not be processed: {:?}",
                error
            );
        }
        Ok(path) => path,
    };

    let mut last_scan: Option<Vec<FileDescription>> = None; // maybe persist this ? needed if files are deleted between sessions
    check_server_reachable(&config).await;

    let hostname = Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .ok();

    match hostname {
        Some(ref h) => info!(
            "{h} starts monitoring changes in '{:?}'",
            config.path_to_monitor
        ),
        None => info!("Start monitoring changes in '{:?}'", config.path_to_monitor),
    }

    let client = {
        let mut headers = HeaderMap::new();
        if let Some(ref h) = hostname {
            headers.insert(
                CLIENT_HOST_HEADER_KEY,
                HeaderValue::from_str(h).expect("Invalid hostname for header"),
            );
        }
        if let Some(ref id) = config.client_id {
            headers.insert(
                CLIENT_ID_HEADER_KEY,
                HeaderValue::from_str(id).expect("Invalid client_id for header"),
            );
        }
        Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build HTTP client")
    };

    // Register config with server (temporary migration)
    register_with_server(&client, &config).await;

    loop {
        let loop_start = Instant::now();

        let dir = config.path_to_monitor.clone();
        let excluded_paths: Vec<String> = Vec::new();
        match get_all_file_descriptions(dir.as_path(), &excluded_paths)
            .map_err(|e| format!("Could not scan directory - {}", e))
        {
            Err(error) => error!("Scanning directory failed - {}", error),
            Ok(descriptions) => {
                let mut deleted_files = Vec::new();
                if let Some(ref last) = last_scan {
                    deleted_files =
                        send_potential_delete_events(&config, last, &client, &descriptions).await;
                }

                match send_to_server_and_receive_instructions(
                    &client,
                    &descriptions,
                    &config.server_url,
                )
                .await
                {
                    Err(err) => error!("Error - failed to get instructions from server: {:?}", err),
                    Ok(instructions) => {
                        if !instructions.is_empty() {
                            info!(
                                "{} Instructions received {:?}",
                                instructions.len(),
                                instructions
                            );
                        }
                        for instruction in instructions {
                            if let SyncInstruction::Download(ref path) = &instruction {
                                if deleted_files
                                    .iter()
                                    .any(|deleted| deleted.relative_path == *path)
                                {
                                    // no need to follow the download instruction,
                                    // because we now that this file was just deleted (breaking the loop)
                                    continue;
                                }
                            }

                            match execute::execute(
                                &client,
                                instruction,
                                config.path_to_monitor.as_path(),
                                &config.server_url,
                            )
                            .await
                            {
                                Ok(msg) => info!("{msg}"),
                                // logging is fine if something went wrong, we just try again at next poll cycle
                                Err(e) => error!("{e}"),
                            }
                        }

                        // last_scan state should only be updated when everything runs through otherwise we
                        // risk losing information (delete)
                        last_scan = Some(descriptions);
                    }
                }
            }
        }

        trace!("Loop took {:?}", Instant::now().duration_since(loop_start));
        tokio::time::sleep_until(
            loop_start.add(Duration::from_millis(config.min_poll_interval_in_ms as u64)),
        )
        .await;
    }
}

async fn check_server_reachable(config: &Config) {
    let hello_endpoint = ServerEndpoint::Ping.to_uri(&config.server_url);
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

/// Temporary: Register client config with server for migration
async fn register_with_server(client: &Client, config: &Config) {
    let request = RegisterClientRequest {
        path_to_monitor: config.path_to_monitor.to_string_lossy().to_string(),
        exclude_dirs: config.exclude_dirs.clone(),
        min_poll_interval_in_ms: config.min_poll_interval_in_ms,
    };

    let endpoint = ServerEndpoint::Register.to_uri(&config.server_url);
    match client.post(&endpoint).json(&request).send().await {
        Ok(response) if response.status().is_success() => {
            info!("Registered with server successfully");
        }
        Ok(response) => {
            warn!(
                "Failed to register with server: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }
        Err(e) => {
            warn!("Failed to register with server: {}", e);
        }
    }
}

async fn send_potential_delete_events(
    config: &Config,
    last_scan: &[FileDescription],
    client: &Client,
    descriptions: &[FileDescription],
) -> Vec<FileDescription> {
    let last_deleted_files = determine_deleted_files(last_scan, descriptions);
    let futures = last_deleted_files
        .iter()
        .map(|deleted| {
            client
                .post(ServerEndpoint::Delete.to_uri(&config.server_url))
                .body(deleted.relative_path.to_serialized_string())
                .send()
        })
        .collect::<Vec<_>>();
    let results = join_all(futures).await;
    let c = last_deleted_files.len();
    if c > 0 {
        info!(
            "Sent {} delete events to server: [{}]",
            c,
            last_deleted_files
                .iter()
                .map(|desc| desc.file_name.clone())
                .collect::<Vec<String>>()
                .join(", ")
        );
        results
            .iter()
            .for_each(|r| info!("Server received delete-event and replied with: {:?}", r));
    }
    last_deleted_files
}

async fn send_to_server_and_receive_instructions(
    client: &Client,
    scanned: &Vec<FileDescription>,
    base: &str,
) -> Result<Vec<SyncInstruction>, reqwest::Error> {
    client
        .post(ServerEndpoint::Sync.to_uri(base))
        .json(scanned)
        .send()
        .await?
        .json()
        .await
}

/// on os level files are just there or not so we got to keep track of the last state
/// and diff it with the new one in order to determine file deletion and propagate the event accordingly
/// [`https://docs.rs/notify/latest/notify/index.html`] could also be an option for later, but
/// I prefer a more native approach for now
///
/// Note: Deletes will reverb one time as server instructed deletes are again sent to the server as delete events.
/// This is due to the stateless nature of the client.
/// Since it doesn't do harm, it doesn't make sense to introduce state in order to deal with this reverb.
fn determine_deleted_files(
    last: &[FileDescription],
    curr: &[FileDescription],
) -> Vec<FileDescription> {
    last.iter()
        .filter(|prev_description| {
            !curr.iter().any(|curr_description| {
                prev_description.relative_path == curr_description.relative_path
            })
        })
        .cloned()
        .collect()
}
