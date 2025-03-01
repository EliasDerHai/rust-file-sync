use crate::config::{read_config, Config};
use futures_util::future::join_all;
use reqwest::Client;
use shared::get_files_of_directory::{get_all_file_descriptions, FileDescription};
use shared::sync_instruction::SyncInstruction;
use std::ops::Add;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{error, info, trace};
use tracing_subscriber::EnvFilter;
use shared::endpoint::ServerEndpoint;

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
    let client = Client::new();

    check_server_reachable(&config, &client).await;

    info!("Start monitoring changes in '{:?}'", config.path_to_monitor);
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
                        if instructions.len() > 0 {
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

async fn check_server_reachable(config: &Config, client: &Client) {
    let hello_endpoint = ServerEndpoint::Ping.to_uri(&config.server_url);
    info!("Testing server at '{}'", &hello_endpoint);
    match client.get(&hello_endpoint).send().await {
        Err(error) => panic!("{} not reachable - {}", &hello_endpoint, error),
        Ok(_) => info!("Server confirmed at {}!", &hello_endpoint),
    }
}

async fn send_potential_delete_events(
    config: &Config,
    last_scan: &Vec<FileDescription>,
    client: &Client,
    descriptions: &Vec<FileDescription>,
) -> Vec<FileDescription> {
    let last_deleted_files = determine_deleted_files(last_scan, &descriptions);
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
        .json::<Vec<SyncInstruction>>()
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
    last: &Vec<FileDescription>,
    curr: &Vec<FileDescription>,
) -> Vec<FileDescription> {
    last.into_iter()
        .filter(|prev_description| {
            !curr.iter().any(|curr_description| {
                prev_description.relative_path == curr_description.relative_path
            })
        })
        .map(|d| d.clone())
        .collect()
}
