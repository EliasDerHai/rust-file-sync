use crate::config::{RuntimeConfig, fetch_or_register_config, read_config};
use futures_util::future::join_all;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use shared::endpoint::{CLIENT_HOST_HEADER_KEY, CLIENT_ID_HEADER_KEY, ServerEndpoint};
use shared::get_files_of_directory::{FileDescription, get_all_file_descriptions};
use shared::sync_instruction::SyncInstruction;
use std::ops::Add;
use std::path::PathBuf;
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

    let (config, client) = load_runtime_config().await;
    let mut last_scan: Option<Vec<FileDescription>> = None;

    loop {
        let loop_start = Instant::now();

        match get_all_file_descriptions(config.path_to_monitor.as_path(), &config.exclude_dirs)
            .map_err(|e| format!("Could not scan directory - {}", e))
        {
            Err(error) => error!("Scanning directory failed - {}", error),
            Ok(descriptions) => {
                let mut deleted_files = Vec::new();
                if let Some(ref last) = last_scan {
                    deleted_files = send_potential_delete_events(
                        &config.server_url,
                        last,
                        &client,
                        &descriptions,
                    )
                    .await;
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
                            if let SyncInstruction::Download(path) = &instruction
                                && deleted_files
                                    .iter()
                                    .any(|deleted| deleted.relative_path == *path)
                            {
                                // no need to follow the download instruction,
                                // because we now that this file was just deleted (breaking the loop)
                                continue;
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

/// Load runtime config: read local config, connect to server, fetch/register config
async fn load_runtime_config() -> (RuntimeConfig, Client) {
    let local_config = match read_config() {
        Ok(config) => config,
        Err(error) => panic!(
            "Critical error - config could not be processed: {:?}",
            error
        ),
    };

    check_server_reachable(&local_config.server_url).await;

    let hostname = Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .ok();

    let client = build_http_client(&hostname, &local_config.client_id);

    let server_config = fetch_or_register_config(&client, &local_config).await;

    info!(
        "{}monitoring '{}'",
        hostname
            .as_ref()
            .map(|h| format!("{h} "))
            .unwrap_or_default(),
        server_config.path_to_monitor
    );
    info!(
        "Config: exclude_dirs={:?}, poll_interval={}ms",
        server_config.exclude_dirs, server_config.min_poll_interval_in_ms
    );

    (
        RuntimeConfig {
            server_url: local_config.server_url,
            path_to_monitor: PathBuf::from(server_config.path_to_monitor),
            exclude_dirs: server_config.exclude_dirs,
            min_poll_interval_in_ms: server_config.min_poll_interval_in_ms,
        },
        client,
    )
}

fn build_http_client(hostname: &Option<String>, client_id: &Option<String>) -> Client {
    let mut headers = HeaderMap::new();
    if let Some(h) = hostname {
        headers.insert(
            CLIENT_HOST_HEADER_KEY,
            HeaderValue::from_str(h).expect("Invalid hostname for header"),
        );
    }
    if let Some(id) = client_id {
        headers.insert(
            CLIENT_ID_HEADER_KEY,
            HeaderValue::from_str(id).expect("Invalid client_id for header"),
        );
    }
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

async fn send_potential_delete_events(
    server_url: &str,
    last_scan: &[FileDescription],
    client: &Client,
    descriptions: &[FileDescription],
) -> Vec<FileDescription> {
    let last_deleted_files = determine_deleted_files(last_scan, descriptions);
    let futures = last_deleted_files
        .iter()
        .map(|deleted| {
            client
                .post(ServerEndpoint::Delete.to_uri(server_url))
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
