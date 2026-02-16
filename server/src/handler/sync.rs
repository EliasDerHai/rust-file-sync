use crate::client_file_event::{ClientFileEvent, ClientFileEventDto};
use crate::file_history::FileHistory;
use crate::{AppState, UPLOAD_PATH, UPLOAD_TMP_PATH, multipart};
use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use shared::endpoint::{CLIENT_HOST_HEADER_KEY, CLIENT_ID_HEADER_KEY};
use shared::file_event::{FileEvent, FileEventType};
use shared::get_files_of_directory::{FileDescription, get_all_file_descriptions};
use shared::matchable_path::MatchablePath;
use shared::sync_instruction::SyncInstruction;
use shared::utc_millis::UtcMillis;
use std::ffi::OsStr;
use std::fs;
use std::fs::create_dir_all;
use std::path::{Component, Path, PathBuf};
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
    let dto =
        multipart::parse_multipart_request(&UPLOAD_TMP_PATH, &mut { multipart }, wg_id).await?;

    let client_host = header_value_as_opt_string(&headers, CLIENT_HOST_HEADER_KEY);
    let client_id = header_value_as_string(&headers, CLIENT_ID_HEADER_KEY)
        .map(|s| s.to_string())
        .ok();

    process_upload(&upload_root_path, state, dto, client_host, client_id)
        .await
        .map_err(|(tmp_file_path, status, error_msg)| {
            if let Some(tmp_file) = tmp_file_path
                && let Err(e) = fs::remove_file(tmp_file)
            {
                tracing::warn!("couldn't clean up tmp file - {e}");
            }
            (status, error_msg)
        })
}

async fn process_upload(
    upload_root_path: &Path,
    state: AppState,
    dto: ClientFileEventDto,
    client_host: Option<String>,
    client_id: Option<String>,
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
                    let mut fe = FileEvent::from(event);
                    fe.client_host = client_host;
                    // write to DB
                    if let Some(ref cid) = client_id {
                        if let Err(e) = state.db.file_event().insert(&fe, cid).await {
                            error!("Failed to persist file event to DB: {e}");
                        }
                    } else {
                        warn!("No client_id header — file event not persisted to DB");
                    }
                    // add to in-mem state
                    state.history.clone().add(fe);
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
    Json(client_sync_state): Json<Vec<FileDescription>>,
) -> Result<Json<Vec<SyncInstruction>>, (StatusCode, String)> {
    trace!("Client state received {:#?}", client_sync_state);
    let mut instructions = Vec::new();
    let target = state.history.clone().get_latest_events(wg_id);

    for event in target.clone() {
        match client_sync_state.iter().find(|client_file_description| {
            client_file_description.relative_path == event.relative_path
        }) {
            // client doesn't have the file at all
            None => {
                if event.event_type != FileEventType::DeleteEvent {
                    instructions.push(SyncInstruction::Download(event.relative_path))
                }
            }
            Some(client_equivalent) => {
                trace!(
                    "Server has {} - client has {}",
                    event.utc_millis, client_equivalent.last_updated_utc_millis
                );

                if client_equivalent.size_in_bytes == event.size_in_bytes
                    && event.event_type.is_change()
                {
                    // same size - just ignore even if timestamps differ (might have been write-operation without change)
                    continue;
                } else if client_equivalent.last_updated_utc_millis < event.utc_millis {
                    // differs in size and client is outdated
                    match event.event_type {
                        FileEventType::ChangeEvent => {
                            // client outdated needs to download new version
                            instructions.push(SyncInstruction::Download(event.relative_path))
                        }
                        FileEventType::DeleteEvent => {
                            // client outdated needs to delete his version
                            instructions.push(SyncInstruction::Delete(event.relative_path))
                        }
                    }
                } else {
                    instructions.push(SyncInstruction::Upload(event.relative_path))
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
        .map(|s| s.to_string())
        .ok();
    let event = FileEvent::new(
        Uuid::new_v4(),
        millis.clone(),
        matchable_path,
        0,
        FileEventType::DeleteEvent,
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
            // write to DB
            if let Some(ref cid) = client_id {
                if let Err(e) = state.db.file_event().insert(&event, cid).await {
                    error!("Failed to persist delete event to DB: {e}");
                }
            } else {
                warn!("No client_id header — delete event not persisted to DB");
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
