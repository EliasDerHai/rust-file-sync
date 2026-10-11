use futures_util::future::join_all;
use reqwest::Client;
use reqwest::multipart::Form;
use shared::content_hash::ContentHash;
use shared::dtos::FileDescription;
use shared::dtos::SyncInstruction;
use shared::endpoint::ServerEndpoint;
use shared::get_files_of_dir::get_all_file_descriptions;
use shared::get_files_of_dir::get_file_description;
use shared::matchable_path::MatchablePath;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tokio::fs::{create_dir_all, remove_file};
use tracing::{error, info};

use crate::WatchGroup;

/// What `execute()` changed on the local filesystem, if anything - used to keep
/// `descriptions` (persisted as the next loop's `last_scan`) in sync with disk
/// reality at the END of the loop, not just the start.
enum DiskChange {
    NoChange,
    Upserted(FileDescription),
    Removed(MatchablePath),
}

pub async fn loop_scan(
    server_url: &str,
    wg_id: i64,
    watch_group: &WatchGroup,
    client: &Client,
    last_scan: Option<Vec<FileDescription>>,
    synced: &mut HashMap<MatchablePath, ContentHash>,
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
        Ok(mut descriptions) => {
            let mut deleted_files = Vec::new();
            if let Some(ref last) = last_scan {
                deleted_files =
                    send_potential_delete_events(server_url, wg_id, last, client, &descriptions)
                        .await;
                for deleted in &deleted_files {
                    synced.remove(&deleted.relative_path);
                }
            }

            match send_to_server_and_receive_instructions(client, &descriptions, server_url, wg_id)
                .await
            {
                Err(err) => error!("Error - failed to get instructions from server: {err}"),
                Ok(instructions) => {
                    if !instructions.is_empty() {
                        info!(
                            "{} Instructions received {:?}",
                            instructions.len(),
                            instructions
                        );
                    }
                    for instruction in instructions {
                        if let SyncInstruction::Download(path, _) = &instruction
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
                            synced,
                        )
                        .await
                        {
                            Ok((msg, disk_change)) => {
                                info!("{msg}");
                                apply_disk_change(&mut descriptions, disk_change);
                            }
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
) -> Result<Vec<SyncInstruction>, String> {
    let response = client
        .post(ServerEndpoint::Sync.to_uri_with_wg(base, wg_id))
        .json(scanned)
        .send()
        .await
        .map_err(|e| format!("request failed - {e}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("server responded {status} - {body}"));
    }

    response
        .json()
        .await
        .map_err(|e| format!("failed to decode instructions - {e}"))
}

/// Keeps `descriptions` (persisted as the next loop's `last_scan`) in sync with the
/// local filesystem mutation `execute()` just performed, so the scan reflects disk
/// state at the END of the loop rather than the start.
fn apply_disk_change(descriptions: &mut Vec<FileDescription>, change: DiskChange) {
    match change {
        DiskChange::NoChange => {}
        DiskChange::Upserted(description) => {
            match descriptions
                .iter_mut()
                .find(|d| d.relative_path == description.relative_path)
            {
                Some(existing) => *existing = description,
                None => descriptions.push(description),
            }
        }
        DiskChange::Removed(path) => descriptions.retain(|d| d.relative_path != path),
    }
}

/// on os level files are just there or not so we got to keep track of the last state
/// and diff it with the new one in order to determine file deletion and propagate the event accordingly
/// [`https://docs.rs/notify/latest/notify/index.html`] could also be an option for later, but
/// I prefer a more native approach for now
///
/// Note: `last` (the previously persisted scan) is kept in lockstep with every disk mutation
/// `execute()` performs during `loop_scan` (see `apply_disk_change`), so by the time this runs
/// again next cycle, a server-instructed delete from the previous cycle is already reflected in
/// `last` and will not be mistaken for a fresh local deletion here.
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
    synced: &mut HashMap<MatchablePath, ContentHash>,
) -> Result<(String, DiskChange), String> {
    match instruction {
        SyncInstruction::Upload(p) => {
            let file_path = p.resolve(root);
            let description = get_file_description(file_path.as_path(), root)?;
            let relative_path_to_send = description.relative_path.get().join("/");
            let base_hash = synced.get(&p).copied().unwrap_or(ContentHash::unknown());
            let form: Form = Form::new()
                .text(
                    "utc_millis",
                    serde_json::to_string(&description.last_updated_utc_millis).unwrap(),
                )
                .text("relative_path", relative_path_to_send)
                .text("base_hash", base_hash.as_u32().to_string())
                .file("file", file_path)
                .await
                .map_err(|e| e.to_string())?;

            let response = client
                .post(ServerEndpoint::Upload.to_uri_with_wg(base, wg_id))
                .multipart(form)
                .send()
                .await
                .map_err(|e| format!("Upload failed - {e}"))?
                .text()
                .await
                .map_err(|e| format!("BOM sniffing failed - {e}"))?;

            synced.insert(p, description.content_hash);
            Ok((
                format!("Upload successful - server replied with '{response}'"),
                DiskChange::NoChange,
            ))
        }

        SyncInstruction::Download(p, hash) => {
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

            fs::write(&file_path, &bytes)
                .await
                .map_err(|e| format!("Could not save downloaded file ({:?}): {}", &file_path, e))?;

            let description = get_file_description(file_path.as_path(), root)
                .map_err(|e| format!("Downloaded file but failed to describe it - {e}"))?;
            let file_name = file_path
                .file_name()
                .map(|osstr| osstr.to_string_lossy().to_string())
                .unwrap_or_else(|| "?".to_string());
            synced.insert(p, hash);
            Ok((
                format!("Downloaded {file_name} successfully"),
                DiskChange::Upserted(description),
            ))
        }

        SyncInstruction::Delete(p) => {
            let file_path = p.resolve(root);

            remove_file(&file_path)
                .await
                .map_err(|e| format!("Deleting file failed - {e}"))
                .map(|_| {
                    let file_name = file_path
                        .file_name()
                        .map(|osstr| osstr.to_string_lossy().to_string())
                        .unwrap_or("?".to_string());
                    synced.remove(&p);
                    (
                        format!("Deleted file '{file_name}'"),
                        DiskChange::Removed(p),
                    )
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::utc_millis::UtcMillis;

    fn description(path: &str) -> FileDescription {
        FileDescription {
            file_name: path.to_string(),
            relative_path: MatchablePath::new(vec![path.to_string()]),
            size_in_bytes: 0,
            file_type: "txt".to_string(),
            last_updated_utc_millis: UtcMillis::from(0),
            content_hash: ContentHash::unknown(),
        }
    }

    #[test]
    fn no_change_leaves_descriptions_untouched() {
        let mut descriptions = vec![description("a.txt")];
        apply_disk_change(&mut descriptions, DiskChange::NoChange);
        assert_eq!(descriptions, vec![description("a.txt")]);
    }

    #[test]
    fn upserted_replaces_existing_entry_for_same_path() {
        let mut descriptions = vec![description("a.txt"), description("b.txt")];
        let mut updated = description("a.txt");
        updated.size_in_bytes = 42;
        apply_disk_change(&mut descriptions, DiskChange::Upserted(updated.clone()));
        assert_eq!(descriptions, vec![updated, description("b.txt")]);
    }

    #[test]
    fn upserted_pushes_new_entry_when_path_is_unknown() {
        let mut descriptions = vec![description("a.txt")];
        let new = description("b.txt");
        apply_disk_change(&mut descriptions, DiskChange::Upserted(new.clone()));
        assert_eq!(descriptions, vec![description("a.txt"), new]);
    }

    #[test]
    fn removed_drops_only_the_matching_entry() {
        let mut descriptions = vec![description("a.txt"), description("b.txt")];
        apply_disk_change(
            &mut descriptions,
            DiskChange::Removed(MatchablePath::new(vec!["a.txt".to_string()])),
        );
        assert_eq!(descriptions, vec![description("b.txt")]);
    }
}
