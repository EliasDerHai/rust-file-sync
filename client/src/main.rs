use crate::config::read_config;
use crate::endpoints::ServerEndpoint;
use futures_util::future::join_all;
use reqwest::Client;
use shared::get_files_of_directory::{get_all_file_descriptions, FileDescription};
use shared::sync_instruction::SyncInstruction;
use std::path::PathBuf;
use task::spawn;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::task;

mod config;
mod execute;

pub mod endpoints {
    pub enum ServerEndpoint {
        Ping,
        Scan, // not needed
        Sync,
        Upload,
        Download,
        Delete,
    }

    impl ServerEndpoint {
        pub fn to_uri(&self, base: &str) -> String {
            match self {
                ServerEndpoint::Ping => format!("http://{}/ping", base),
                ServerEndpoint::Scan => format!("http://{}/scan", base),
                ServerEndpoint::Sync => format!("http://{}/sync", base),
                ServerEndpoint::Upload => format!("http://{}/upload", base),
                ServerEndpoint::Download => format!("http://{}/download", base),
                ServerEndpoint::Delete => format!("http://{}/delete", base),
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<Vec<FileDescription>>(100);
    let config = match read_config() {
        Err(error) => {
            panic!("Critical error: {:?}", error);
        }
        Ok(path) => path,
    };

    println!("Start monitoring changes in '{:?}'", config.path_to_monitor);

    let mut last_scan: Option<Vec<FileDescription>> = None; // maybe persist this ? needed if files are deleted between sessions
    let mut last_deleted_files: Vec<FileDescription> = Vec::new();
    let client = Client::new();
    let hello_endpoint = ServerEndpoint::Ping.to_uri(&config.server_url);
    println!("Testing server at '{}'", &hello_endpoint);
    match client.get(&hello_endpoint).send().await {
        Err(error) => panic!("{} not reachable - {}", &hello_endpoint, error),
        Ok(_) => println!("Server confirmed at {}!", &hello_endpoint),
    }

    spawn(watch_directory(config.path_to_monitor.clone(), tx));

    while let Some(scanned) = rx.recv().await {
        if let Some(last) = last_scan.clone() {
            last_deleted_files = determine_deleted_files(&last, &scanned);
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
                println!(
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
                    .for_each(|r| println!("Server replied with: {:?}", r));
            }
        }

        match send_to_server_and_receive_instructions(&client, &scanned, &config.server_url).await {
            Err(err) => println!("Error - failed to get instructions from server: {:?}", err),
            Ok(instructions) => {
                println!(
                    "{} Instructions received {:?}",
                    instructions.len(),
                    instructions
                );
                for instruction in instructions {
                    if let SyncInstruction::Download(ref path) = &instruction {
                        if last_deleted_files
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
                        Ok(msg) => println!("{msg}"),
                        // logging is fine - if something went wrong, we just try again at next poll cycle
                        Err(e) => eprintln!("{e}"),
                    }
                }

                // last_scan state should only be updated when everything runs through otherwise we
                // risk losing information (delete)
                last_scan = Some(scanned);
            }
        }
    }
}

async fn watch_directory(dir: PathBuf, tx: Sender<Vec<FileDescription>>) {
    loop {
        match get_all_file_descriptions(dir.as_path()) {
            Err(error) => eprintln!("Could not scan dir - {}", error),
            Ok(descriptions) => match tx.send(descriptions).await {
                Err(error) => eprintln!("Error while scanning {}", error),
                Ok(()) => println!("Scanned dir successfully"),
            },
        }

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
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
