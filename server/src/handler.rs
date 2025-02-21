use crate::client_file_event::ClientFileEvent;
use crate::file_history::FileHistory;
use crate::write::append_line;
use crate::{multipart, AppState};
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::DateTime;
use shared::file_event::{FileEvent, FileEventType};
use shared::get_files_of_directory::{get_all_file_descriptions, FileDescription};
use shared::matchable_path::MatchablePath;
use shared::sync_instruction::SyncInstruction;
use std::ffi::OsStr;
use std::fs;
use std::fs::create_dir_all;
use std::path::{Component, Path, PathBuf};
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, trace};
use uuid::Uuid;

/// expecting no payload
/// returning list of file meta infos
pub async fn scan_disk(path: &Path) -> Result<Json<Vec<FileDescription>>, StatusCode> {
    let excluded_patterns = Vec::new();
    match get_all_file_descriptions(path, &excluded_patterns) {
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
///   event_type: "change",
///   file: @File
/// }
pub async fn upload_handler(
    upload_root_path: &Path,
    upload_root_tmp_path: &Path,
    history_file_path: &Path,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<String, (StatusCode, String)> {
    // parse incoming request
    let dto = multipart::parse_multipart_request(upload_root_tmp_path, &mut multipart).await?;

    // map to domain object (FileEvent)
    match ClientFileEvent::try_from(dto) {
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
        Ok(event) => {
            let utc_millis_of_latest_history_event = state
                .history
                .get_latest_event(&event.relative_path)
                .map(|e| e.utc_millis)
                .unwrap_or(0);

            if event.utc_millis < utc_millis_of_latest_history_event {
                error!(
                    "Dropping event for {:?} - event ({:?}) older than latest history state event ({:?})",
                    &event.relative_path, utc_millis_of_latest_history_event, event.utc_millis
                );
                Err((StatusCode::BAD_REQUEST, "not latest".to_string()))
            } else {
                let sub_path = event
                    .relative_path
                    .get()
                    .iter()
                    .map(|part| Component::Normal(part.as_ref()));
                let target_path: PathBuf = upload_root_path.components().chain(sub_path).collect();
                let io_result = match event.event_type {
                    FileEventType::ChangeEvent => {
                        create_dir_all(target_path.parent().unwrap_or(Path::new("./"))).map_err(
                            |e| {
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    format!("Cannot create dir - {}", e),
                                )
                            },
                        )?;
                        info!(
                            "moving {:?} -> {:?}",
                            event.temp_file_path.as_ref().unwrap(),
                            target_path
                        );
                        fs::rename(
                            event.temp_file_path.as_ref().unwrap().as_path(),
                            target_path.as_path(),
                        )
                    }
                    FileEventType::DeleteEvent => fs::remove_file(&target_path),
                };

                let path_str = target_path.to_string_lossy();
                match io_result {
                    Ok(_) => {
                        let message = match event.event_type {
                            FileEventType::ChangeEvent => {
                                format!("Updated {} successfully", path_str)
                            }
                            FileEventType::DeleteEvent => {
                                format!("Deleted {} successfully", path_str)
                            }
                        };
                        // write to history.csv
                        append_line(
                            history_file_path,
                            &FileEvent::from(event.clone()).serialize_to_csv_line(),
                        );
                        // add to in-mem state
                        state.history.clone().add(FileEvent::from(event));
                        // log & return
                        info!("{message}");
                        Ok(message)
                    }
                    Err(e) => {
                        let message = match event.event_type {
                            FileEventType::ChangeEvent => {
                                format!("Updating {} failed - {}", path_str, e)
                            }
                            FileEventType::DeleteEvent => {
                                format!("Deleting {} failed - {}", path_str, e)
                            }
                        };
                        error!("{message}");
                        Err((StatusCode::INTERNAL_SERVER_ERROR, message))
                    }
                }
            }
        }
    }
}

fn get_utc_millis_as_date_string(utc_millis: u64) -> String {
    DateTime::from_timestamp_millis(utc_millis as i64)
        .map(|t| t.format("%d.%m.%Y %H:%M:%S").to_string())
        .unwrap_or("invalid datetime".to_string())
}

/// compares incoming payload with FileHistory to determine and return a list of instructions
/// in order for the client to achieve a synchronized state
///
/// expects payload like:
/// [{
///     "file_name": "history.csv",
///		"relative_path": "./history.csv",
///		"size_in_bytes": 103,
///		"file_type": ".csv",
///		"last_updated_utc_millis": 9585834893
///	}]
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
            None => instructions.push(SyncInstruction::Download(event.relative_path)),
            Some(client_equivalent) => {
                trace!(
                    "Server has {} ({}) - client has {} ({})",
                    get_utc_millis_as_date_string(event.utc_millis),
                    event.utc_millis,
                    get_utc_millis_as_date_string(client_equivalent.last_updated_utc_millis),
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

    info!("Instructions {:#?}", instructions);
    Ok(Json(instructions))
}

/// expects payload with plain string path (unix-delimiter) like:
/// `some/path/to/download/file.txt`
pub async fn download(upload_root_path: &Path, payload: String) -> impl IntoResponse {
    let sub_path: PathBuf = MatchablePath::from(payload.split('/').collect::<Vec<&str>>())
        .get()
        .into_iter()
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

    let headers = [
        (
            axum::http::header::CONTENT_TYPE,
            "text; charset=utf-8".to_string(), // TODO
        ),
        (
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file_name),
        ),
    ];

    Ok((headers, body))
}

pub async fn delete(
    upload_path: &Path,
    payload: String,
    state: State<AppState>,
) -> Result<(), (StatusCode, String)> {
    debug!("Received delete request for '{}'", payload);
    let matchable_path = MatchablePath::from(payload.as_str());
    let p = matchable_path.resolve(upload_path);
    let millis = chrono::Utc::now().timestamp_millis() as u64;
    let event = FileEvent::new(
        Uuid::new_v4(),
        millis,
        matchable_path,
        0,
        FileEventType::DeleteEvent,
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
            info!("Deleted {} successfully", &p.to_string_lossy());
            state.history.add(event);
            info!(
                "Added delete event with time {}",
                get_utc_millis_as_date_string(millis)
            );
            Ok(())
        }
        Err(err) => {
            info!("Failed to delete file: {}", err);
            Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
        }
    }
}

pub async fn get_monitoring(monitoring_path: &Path) -> impl IntoResponse {
    let file = match tokio::fs::File::open(monitoring_path).await {
        Ok(file) => file,
        Err(err) => return Err((StatusCode::NOT_FOUND, format!("File not found: {}", err))),
    };
    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    let headers = [
        (
            axum::http::header::CONTENT_TYPE,
            "text/CSV; charset=utf-8".to_string(),
        ),
        (
            axum::http::header::CONTENT_DISPOSITION,
            "attachment; filename=\"monitoring.csv\"".to_string(),
        ),
    ];
    Ok((headers, body))
}
