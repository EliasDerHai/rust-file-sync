use futures_util::future::join_all;
use reqwest::Client;
use reqwest::multipart::Form;
use shared::endpoint::ServerEndpoint;
use shared::get_files_of_directory::get_file_description;
use shared::get_files_of_directory::{FileDescription, get_all_file_descriptions};
use shared::sync_instruction::SyncInstruction;
use std::path::Path;
use tokio::fs;
use tokio::fs::{create_dir_all, remove_file};
use tracing::{error, info};

use crate::WatchGroup;

pub async fn loop_scan(
    server_url: &str,
    wg_id: i64,
    watch_group: &WatchGroup,
    client: &Client,
    last_scan: Option<Vec<FileDescription>>,
) -> Vec<FileDescription> {
    match get_all_file_descriptions(
        watch_group.path_to_monitor.as_path(),
        &watch_group.exclude_dirs,
        watch_group.exclude_dot_dirs,
    )
    .map_err(|e| format!("Could not scan directory - {}", e))
    {
        Err(error) => {
            error!(
                "Scanning directory for {} failed - {}",
                watch_group.name, error
            );
            last_scan.unwrap_or_default()
        }
        Ok(descriptions) => {
            let mut deleted_files = Vec::new();
            if let Some(ref last) = last_scan {
                deleted_files =
                    send_potential_delete_events(server_url, wg_id, last, client, &descriptions)
                        .await;
            }

            match send_to_server_and_receive_instructions(client, &descriptions, server_url, wg_id)
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

                        match execute(
                            client,
                            instruction,
                            watch_group.path_to_monitor.as_path(),
                            server_url,
                            wg_id,
                        )
                        .await
                        {
                            Ok(msg) => info!("{msg}"),
                            // logging is fine if something went wrong, we just try again at next poll cycle
                            Err(e) => error!("{e}"),
                        }
                    }
                }
            }

            descriptions
        }
    }
}

async fn send_potential_delete_events(
    server_url: &str,
    wg_id: i64,
    last_scan: &[FileDescription],
    client: &Client,
    descriptions: &[FileDescription],
) -> Vec<FileDescription> {
    let last_deleted_files = determine_deleted_files(last_scan, descriptions);
    let futures = last_deleted_files
        .iter()
        .map(|deleted| {
            client
                .post(ServerEndpoint::Delete.to_uri_with_wg(server_url, wg_id))
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
    wg_id: i64,
) -> Result<Vec<SyncInstruction>, reqwest::Error> {
    client
        .post(ServerEndpoint::Sync.to_uri_with_wg(base, wg_id))
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

/// executes an instruction of the server (see [`SyncInstruction`])
async fn execute(
    client: &Client,
    instruction: SyncInstruction,
    root: &Path,
    base: &str,
    wg_id: i64,
) -> Result<String, String> {
    match instruction {
        SyncInstruction::Upload(p) => {
            let file_path = p.resolve(root);
            let description = get_file_description(file_path.as_path(), root)?;
            let relative_path_to_send = description.relative_path.get().join("/");
            let form: Form = Form::new()
                .text(
                    "utc_millis",
                    serde_json::to_string(&description.last_updated_utc_millis).unwrap(),
                )
                .text("relative_path", relative_path_to_send)
                .file("file", file_path)
                .await
                .map_err(|e| e.to_string())?;

            client
                .post(ServerEndpoint::Upload.to_uri_with_wg(base, wg_id))
                .multipart(form)
                .send()
                .await
                .map_err(|e| format!("Upload failed - {e}"))?
                .text()
                .await
                .map_err(|e| format!("BOM sniffing failed - {e}"))
                .map(|response| format!("Upload successful - server replied with '{response}'",))
        }

        SyncInstruction::Download(p) => {
            let file_path = p.resolve(root);

            let response = client
                .get(ServerEndpoint::Download.to_uri_with_wg(base, wg_id))
                .body(p.to_serialized_string())
                .send()
                .await
                .map_err(|e| format!("Download request failed - {e}",))?
                .error_for_status()
                .map_err(|e| {
                    format!("Download request failed - {} - {}", e.status().unwrap(), e)
                })?;

            let bytes = response
                .bytes()
                .await
                .map_err(|e| format!("Download failed - cannot read response body - {e}"))?;

            create_dir_all(file_path.parent().unwrap())
                .await
                .unwrap_or_else(|_| {
                    panic!(
                        "Should be able to create parent directory of file ({:?})",
                        &file_path
                    )
                });

            fs::write(&file_path, bytes)
                .await
                .map_err(|e| format!("Could not save downloaded file ({:?}): {}", &file_path, e))?;

            Ok(format!(
                "Downloaded {} successfully",
                file_path
                    .file_name()
                    .map(|osstr| osstr.to_string_lossy().to_string())
                    .unwrap_or_else(|| "?".to_string())
            ))
        }

        SyncInstruction::Delete(p) => {
            let file_path = p.resolve(root);

            remove_file(&file_path)
                .await
                .map_err(|e| format!("Deleting file failed - {e}"))
                .map(|_| {
                    format!(
                        "Deleted file '{}'",
                        &file_path
                            .file_name()
                            .map(|osstr| osstr.to_string_lossy().to_string())
                            .unwrap_or("?".to_string())
                    )
                })
        }
    }
}
