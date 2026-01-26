use crate::client_file_event::{ClientFileEvent, ClientFileEventDto};
use crate::file_history::FileHistory;
use crate::write::append_line;
use crate::{multipart, AppState};
use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use shared::endpoint::{CLIENT_HOST_HEADER_KEY, CLIENT_ID_HEADER_KEY};
use shared::file_event::{FileEvent, FileEventType};
use shared::get_files_of_directory::{get_all_file_descriptions, FileDescription};
use shared::matchable_path::MatchablePath;
use shared::register::ClientConfigDto;
use shared::sync_instruction::SyncInstruction;
use shared::utc_millis::UtcMillis;
use std::ffi::OsStr;
use std::fs;
use std::fs::create_dir_all;
use std::path::{Component, Path, PathBuf};
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// expecting no payload
/// returning list of file meta infos
pub async fn scan_disk(path: &Path) -> Result<Json<Vec<FileDescription>>, StatusCode> {
    match get_all_file_descriptions(path, &Vec::new()) {
        Ok(descriptions) => Ok(Json(descriptions)),
        Err(err) => {
            error!("IO Failure - {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// expecting payload like
/// {
///   utc_millis: 42,
///   relative_path: "./directory/file.txt",
///   file: @File
/// }
///
pub async fn upload_handler(
    upload_root_path: &Path,
    upload_root_tmp_path: &Path,
    history_file_path: &Path,
    State(state): State<AppState>,
    mut multipart: Multipart,
    headers: HeaderMap,
) -> Result<String, (StatusCode, String)> {
    // parse incoming request
    let dto = multipart::parse_multipart_request(upload_root_tmp_path, &mut multipart).await?;

    let client_host = header_value_as_opt_string(&headers, CLIENT_HOST_HEADER_KEY);

    process_upload(upload_root_path, history_file_path, state, dto, client_host).map_err(
        |(tmp_file_path, status, error_msg)| {
            if let Some(tmp_file) = tmp_file_path {
                if let Err(e) = fs::remove_file(tmp_file) {
                    tracing::warn!("couldn't clean up tmp file - {e}");
                }
            }
            (status, error_msg)
        },
    )
}

fn process_upload(
    upload_root_path: &Path,
    history_file_path: &Path,
    state: AppState,
    dto: ClientFileEventDto,
    client_host: Option<String>,
) -> Result<String, (Option<PathBuf>, StatusCode, String)> {
    let tmp_file_path_cpy = dto.temp_file_path.clone();
    // map to domain object (FileEvent)
    match ClientFileEvent::try_from(dto) {
        Err(e) => Err((tmp_file_path_cpy, StatusCode::BAD_REQUEST, e)),
        Ok(event) => {
            let utc_millis_of_latest_history_event = state
                .history
                .get_latest_event(&event.relative_path)
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
                    // create FileEvent with client_host
                    let mut fe = FileEvent::from(event);
                    fe.client_host = client_host;
                    // write to history.csv
                    append_line(history_file_path, &fe.serialize_to_csv_line());
                    // add to in-mem state
                    state.history.clone().add(fe);
                    // log & return
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

/// compares incoming payload with FileHistory to determine and return a list of instructions
/// in order for the client to achieve a synchronized state
///
/// expects payload like:
/// [{
///  "file_name": "history.csv",
///    "relative_path": "./history.csv",
///    "size_in_bytes": 103,
///    "file_type": ".csv",
///    "last_updated_utc_millis": 9585834893
///  }]
///
pub async fn sync_handler(
    State(state): State<AppState>,
    Json(client_sync_state): Json<Vec<FileDescription>>,
) -> Result<Json<Vec<SyncInstruction>>, (StatusCode, String)> {
    trace!("Client state received {:#?}", client_sync_state);
    let mut instructions = Vec::new();
    let target = state.history.clone().get_latest_events();

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
                    event.utc_millis,
                    client_equivalent.last_updated_utc_millis
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
pub async fn download(upload_root_path: &Path, payload: String) -> impl IntoResponse {
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

    // should we add content_type header? works fine without so I guess we're good
    let headers = [(
        axum::http::header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", file_name),
    )];

    Ok((headers, body))
}

pub async fn delete(
    upload_path: &Path,
    history_file_path: &Path,
    payload: String,
    state: State<AppState>,
    headers: HeaderMap,
) -> Result<(), (StatusCode, String)> {
    debug!("Received delete request for '{}'", payload);
    let matchable_path = MatchablePath::from(payload.as_str());
    let p = matchable_path.resolve(upload_path);
    let millis = UtcMillis::now(); // good enough, but could be specified by client and sent as part of request
    let client_host = header_value_as_opt_string(&headers, CLIENT_HOST_HEADER_KEY);
    let event = FileEvent::new(
        Uuid::new_v4(),
        millis.clone(),
        matchable_path,
        0,
        FileEventType::DeleteEvent,
        client_host,
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
            append_line(history_file_path, &event.clone().serialize_to_csv_line());
            state.history.add(event);
            info!("Deleted {} successfully", &p.to_string_lossy());
            info!("Added delete event with time {} to history/csv", millis);
            Ok(())
        }
        Err(err) => {
            info!("Failed to delete file: {}", err);
            Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
        }
    }
}

/// Temporary endpoint for migrating client configs to server DB
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClientConfigDto>,
) -> Result<String, (StatusCode, String)> {
    let client_id = header_value_as_string(&headers, CLIENT_ID_HEADER_KEY)?;
    let host_name = header_value_as_string(&headers, CLIENT_HOST_HEADER_KEY)?;

    state
        .db
        .register_client(client_id, host_name, request)
        .await
        .map_err(|e| {
            error!("Failed to register client: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Registered client {} ({})", client_id, host_name);
    Ok(format!("Client {} registered successfully", client_id))
}

/// Get client config by client_id header
pub async fn get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ClientConfigDto>, (StatusCode, String)> {
    let client_id = header_value_as_string(&headers, CLIENT_ID_HEADER_KEY)?;

    match state.db.get_client_config(client_id).await {
        Ok(Some(config)) => {
            debug!("Returning config for client {}", client_id);
            Ok(Json(config))
        }
        Ok(None) => {
            debug!("No config found for client {}", client_id);
            Err((StatusCode::NOT_FOUND, "Client not registered".to_string()))
        }
        Err(e) => {
            error!("Failed to get client config: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

fn header_value_as_opt_string(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
}

fn header_value_as_string<'header>(
    headers: &'header HeaderMap,
    key: &str,
) -> Result<&'header str, (StatusCode, String)> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, format!("Missing {key} header")))
}
