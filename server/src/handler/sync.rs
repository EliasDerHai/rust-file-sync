use crate::client_file_event::{ClientFileEvent, ClientFileEventDto};
use crate::file_event::{FileEvent, FileEventType};
use crate::file_history::FileHistory;
use crate::{AppState, UPLOAD_PATH, UPLOAD_TMP_PATH, multipart};
use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use shared::content_hash::ContentHash;
use shared::dtos::FileDescription;
use shared::endpoint::{CLIENT_HOST_HEADER_KEY, CLIENT_ID_HEADER_KEY};
use shared::get_files_of_directory::get_all_file_descriptions;
use shared::matchable_path::MatchablePath;
use shared::sync_instruction::SyncInstruction;
use chrono::Local;
use shared::utc_millis::UtcMillis;
use std::ffi::OsStr;
use std::fs;
use std::fs::create_dir_all;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use super::{header_value_as_opt_string, header_value_as_string};

fn upload_path_for_wg(wg_id: i64) -> PathBuf {
    UPLOAD_PATH.join(wg_id.to_string())
}

/// returns list of file meta infos
pub async fn scan_disk(path: &Path) -> Result<Json<Vec<FileDescription>>, StatusCode> {
    match get_all_file_descriptions(path, &Vec::new(), true) {
        Ok(descriptions) => Ok(Json(descriptions)),
        Err(err) => {
            error!("IO Failure - {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn upload_handler(
    State(state): State<AppState>,
    axum::extract::Path(wg_id): axum::extract::Path<i64>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<String, (StatusCode, String)> {
    let upload_root_path = upload_path_for_wg(wg_id);
    let client_host = header_value_as_opt_string(&headers, CLIENT_HOST_HEADER_KEY);
    let client_id = header_value_as_string(&headers, CLIENT_ID_HEADER_KEY)
        .map(Uuid::from_str)?
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid X-Client-Id header — upload refused".to_string(),
            )
        })?;
    let dto = multipart::parse_multipart_request(
        &UPLOAD_TMP_PATH,
        &mut { multipart },
        client_id,
        client_host,
        wg_id,
    )
    .await?;

    process_upload(&upload_root_path, state, dto).await.map_err(
        |(tmp_file_path, status, error_msg)| {
            if let Some(tmp_file) = tmp_file_path
                && let Err(e) = fs::remove_file(tmp_file)
            {
                tracing::warn!("couldn't clean up tmp file - {e}");
            }
            (status, error_msg)
        },
    )
}

/// Resolves a conflict by renaming the existing server-side file before the incoming upload
/// overwrites it. The renamed file keeps the same directory and extension:
///
/// `notes.md` → `notes.conflict-2026-05-25_14-30-macbook_air.md`
async fn handle_conflict(latest: &FileEvent, upload_root_path: &Path, state: &AppState) {
    let existing_path = latest.relative_path.resolve(upload_root_path);

    if !existing_path.exists() {
        return;
    }

    let timestamp = UtcMillis::now();
    let date = Local::now().format("%Y-%m-%d_%H-%M");
    let host_tag = latest
        .client_host
        .as_deref()
        .unwrap_or("unknown")
        .replace([' ', '/'], "_");

    let original_name = existing_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let conflict_name = match original_name.rfind('.') {
        Some(pos) => format!(
            "{}.conflict-{}-{}{}",
            &original_name[..pos],
            date,
            host_tag,
            &original_name[pos..]
        ),
        None => format!(
            "{}.conflict-{}-{}",
            original_name,
            date,
            host_tag
        ),
    };

    let conflict_path = existing_path.with_file_name(&conflict_name);
    if let Err(e) = fs::rename(&existing_path, &conflict_path) {
        error!("Conflict rename failed for {:?}: {e}", existing_path);
        return;
    }
    info!(
        "Conflict detected — renamed {:?} → {:?}",
        existing_path, conflict_path
    );

    let mut conflict_parts = latest.relative_path.get().clone();
    if let Some(last) = conflict_parts.last_mut() {
        *last = conflict_name;
    }
    let conflict_relative_path = MatchablePath::from(conflict_parts);

    let conflict_event = FileEvent::new(
        Uuid::new_v4(),
        timestamp,
        conflict_relative_path,
        latest.size_in_bytes,
        latest.content_hash,
        FileEventType::ChangeEvent,
        latest.client_id,
        latest.client_host.clone(),
        latest.watch_group_id,
    );
    if let Err(e) = state.db.file_event().insert(&conflict_event).await {
        error!("Failed to persist conflict event to DB: {e}");
    }
    state.history.clone().add(conflict_event);
}

async fn process_upload(
    upload_root_path: &Path,
    state: AppState,
    dto: ClientFileEventDto,
) -> Result<String, (Option<PathBuf>, StatusCode, String)> {
    let tmp_file_path_cpy = dto.temp_file_path.clone();
    let wg_id = dto.watch_group_id;
    // map to domain object (FileEvent)
    match ClientFileEvent::try_from(dto) {
        Err(e) => Err((tmp_file_path_cpy, StatusCode::BAD_REQUEST, e)),
        Ok(event) => {
            let utc_millis_of_latest_history_event = state
                .history
                .get_latest_event(wg_id, &event.relative_path)
                .map(|e| e.utc_millis)
                .unwrap_or(UtcMillis::from(0));

            if event.utc_millis < utc_millis_of_latest_history_event {
                warn!(
                    "Skipping upload & event for {:?} - event ({:?}) older than latest history state event ({:?})",
                    &event.relative_path, utc_millis_of_latest_history_event, event.utc_millis
                );
                return Err((
                    event.temp_file_path,
                    StatusCode::BAD_REQUEST,
                    "not latest".to_string(),
                ));
            }

            let sub_path = event
                .relative_path
                .get()
                .iter()
                .map(|part| Component::Normal(part.as_ref()));
            let target_path: PathBuf = upload_root_path.components().chain(sub_path).collect();
            let temp_path: PathBuf = event.temp_file_path.clone().unwrap();
            let io_result = {
                create_dir_all(target_path.parent().unwrap_or(Path::new("./"))).map_err(|e| {
                    (
                        event.temp_file_path.clone(),
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Could not create dir - {}", e),
                    )
                })?;

                if let Some(latest) = state.history.get_latest_event(wg_id, &event.relative_path)
                    && latest.content_hash != event.base_hash
                {
                    handle_conflict(&latest, upload_root_path, &state).await;
                }

                let result = fs::rename(temp_path.as_path(), target_path.as_path());
                let was_success = result.is_ok();
                let temp_path = temp_path.as_path();
                let target_path = target_path.as_path();
                if !was_success {
                    let result_delete_temp = if fs::remove_file(temp_path).is_ok() {
                        "was successful"
                    } else {
                        "failed aswell"
                    };

                    warn!(
                        "moving failed - deleting temp file {} - {:?} -> {:?}",
                        result_delete_temp, temp_path, target_path,
                    );
                } else {
                    info!(
                        "moving was successful - {:?} -> {:?}",
                        temp_path, target_path
                    );
                }
                result
            };

            let path_str = target_path.to_string_lossy();
            match io_result {
                Ok(_) => {
                    let message = format!("Updated {} successfully", path_str);
                    let event = FileEvent::from(event);
                    // write to DB
                    if let Err(e) = state.db.file_event().insert(&event).await {
                        error!("Failed to persist file event to DB: {e}");
                    }
                    // add to in-mem state
                    state.history.clone().add(event);
                    info!("{message}");
                    Ok(message)
                }
                Err(e) => {
                    let message = format!("Updating {} failed - {}", path_str, e);
                    error!("{message}");
                    Err((None, StatusCode::INTERNAL_SERVER_ERROR, message))
                }
            }
        }
    }
}

pub async fn sync_handler(
    State(state): State<AppState>,
    axum::extract::Path(wg_id): axum::extract::Path<i64>,
    headers: HeaderMap,
    Json(client_sync_state): Json<Vec<FileDescription>>,
) -> Result<Json<Vec<SyncInstruction>>, (StatusCode, String)> {
    trace!("Client state received {:#?}", client_sync_state);
    let mut instructions = Vec::new();
    let target = state.history.clone().get_latest_events(wg_id);
    let client_id = header_value_as_string(&headers, CLIENT_ID_HEADER_KEY)
        .map(Uuid::from_str)?
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid X-Client-Id header — sync refused".to_string(),
            )
        })?;

    for event in &target {
        let event_path = event.relative_path.clone();
        match client_sync_state.iter().find(|client_file_description| {
            client_file_description.relative_path == event.relative_path
        }) {
            // client doesn't have the file at all
            None => {
                match event.event_type {
                    FileEventType::ChangeEvent => {
                        instructions.push(SyncInstruction::Download(event_path, event.content_hash))
                    }
                    FileEventType::DeleteEvent => (), // nothing to delete
                }
            }
            Some(client_equivalent) => {
                trace!(
                    "Server has {} - client has {}",
                    event.utc_millis, client_equivalent.last_updated_utc_millis
                );

                let client_behind = client_equivalent.last_updated_utc_millis < event.utc_millis;
                let client_was_last_editor = event.client_id == client_id;
                let content_unchanged = event.content_hash == client_equivalent.content_hash;

                match event.event_type {
                    FileEventType::ChangeEvent => {
                        if content_unchanged {
                            continue;
                        }

                        match client_behind {
                            true => {
                                if client_was_last_editor {
                                    // client was the event's sender - no need to act since client is always up-to-date with himself
                                    continue;
                                }
                                // client outdated needs to download new version
                                instructions
                                    .push(SyncInstruction::Download(event_path, event.content_hash))
                            }
                            false => {
                                // client ahead needs to upload new version
                                instructions.push(SyncInstruction::Upload(event_path))
                            }
                        }
                    }
                    FileEventType::DeleteEvent => {
                        match client_behind {
                            true => {
                                // client outdated needs to delete his version
                                instructions.push(SyncInstruction::Delete(event_path))
                            }
                            false => {
                                // server has a delete event but client has a new change - it's a new file and has to be uploaded!
                                instructions.push(SyncInstruction::Upload(event_path))
                            }
                        }
                    }
                }
            }
        }
    }

    for desc in client_sync_state {
        if !target.iter().any(|e| e.relative_path == desc.relative_path) {
            instructions.push(SyncInstruction::Upload(desc.relative_path));
        }
    }

    if !instructions.is_empty() {
        info!("Instructions {:#?}", instructions);
    }
    Ok(Json(instructions))
}

/// expects payload with plain string path (unix-delimiter) like:
/// `some/path/to/download/file.txt`
pub async fn download(
    axum::extract::Path(wg_id): axum::extract::Path<i64>,
    payload: String,
) -> impl IntoResponse {
    let upload_root_path = upload_path_for_wg(wg_id);
    let sub_path: PathBuf = MatchablePath::from(payload.split('/').collect::<Vec<&str>>())
        .get()
        .iter()
        .map(|part| Component::Normal(OsStr::new(part)))
        .collect();
    let p = upload_root_path.join(sub_path);
    let file_name = p.file_name().unwrap().to_string_lossy().to_string();
    let file = match tokio::fs::File::open(p).await {
        Ok(file) => file,
        Err(err) => return Err((StatusCode::NOT_FOUND, format!("File not found: {}", err))),
    };
    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    let headers = [(
        axum::http::header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", file_name),
    )];

    Ok((headers, body))
}

pub async fn delete(
    state: State<AppState>,
    axum::extract::Path(wg_id): axum::extract::Path<i64>,
    headers: HeaderMap,
    payload: String,
) -> Result<(), (StatusCode, String)> {
    let upload_path = upload_path_for_wg(wg_id);
    debug!("Received delete request for '{}'", payload);
    let matchable_path = MatchablePath::from(payload.as_str());
    let p = matchable_path.resolve(&upload_path);
    let millis = UtcMillis::now();
    let client_host = header_value_as_opt_string(&headers, CLIENT_HOST_HEADER_KEY);
    let client_id = header_value_as_string(&headers, CLIENT_ID_HEADER_KEY)
        .map(Uuid::from_str)?
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Missing X-Client-Id header — delete refused".to_string(),
            )
        })?;

    let event = FileEvent::new(
        Uuid::new_v4(),
        millis.clone(),
        matchable_path,
        0,
        ContentHash::unknown(),
        FileEventType::DeleteEvent,
        client_id,
        client_host,
        wg_id,
    );

    if !p.exists() {
        state.history.add(event);
        info!("Skip delete because file doesn't exist");
        return Err((
            StatusCode::OK,
            "Nothing to do, because file doesn't exist (could've been deleted by someone else)"
                .to_string(),
        ));
    }

    match tokio::fs::remove_file(&p).await {
        Ok(()) => {
            if let Err(e) = state.db.file_event().insert(&event).await {
                error!("Failed to persist delete event to DB: {e}");
            }
            state.history.add(event);
            info!("Deleted {} successfully", &p.to_string_lossy());
            info!("Added delete event with time {} to history", millis);
            Ok(())
        }
        Err(err) => {
            info!("Failed to delete file: {}", err);
            Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
        }
    }
}
