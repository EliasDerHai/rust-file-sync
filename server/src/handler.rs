use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::client_file_event::{ClientFileEvent, ClientFileEventDto};
use crate::file_event::{FileEvent, FileEventType};
use crate::file_history::FileHistory;
use crate::write::append_line;
use crate::{write, AppState};
use axum::body::Bytes;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, NaiveDateTime};
use shared::get_files_of_directory::{get_all_file_descriptions, FileDescription};
use shared::matchable_path::MatchablePath;
use shared::sync_instruction::SyncInstruction;
use tokio_util::io::ReaderStream;

/// expecting no payload
/// returning list of file meta infos
pub async fn scan_disk(path: &Path) -> Result<Json<Vec<FileDescription>>, StatusCode> {
    match get_all_file_descriptions(path) {
        Ok(descriptions) => Ok(Json(descriptions)),
        Err(err) => {
            eprintln!("IO Failure - {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// expecting payload like
/// {
///   utc_millis: 42,
///   relative_path: "./directory/file.txt",
///   event_type: "create",
///   file: @File
/// }
pub async fn upload_handler(
    upload_root_path: &Path,
    State(state): State<AppState>,
    mut multipart: Multipart,
    history_file_path: &Path,
) -> Result<String, (StatusCode, String)> {
    // parse incoming request
    let mut utc_millis: Option<u64> = None;
    let mut relative_path: Option<Vec<String>> = None;
    let mut file_event_type: Option<FileEventType> = None;
    let mut file_bytes: Option<Bytes> = None;

    while let Some(field) = multipart.next_field().await.unwrap() {
        match field.name() {
            None => eprintln!("No field name in upload handler!"),
            Some("utc_millis") => {
                utc_millis = field
                    .text()
                    .await
                    .map(|t| t.parse::<u64>().ok())
                    .ok()
                    .flatten();
                println!("UTC: {}ms", utc_millis.unwrap_or(0));
            }
            Some("relative_path") => {
                relative_path = field
                    .text()
                    .await
                    .map(|t| t.split("/").map(|str| str.to_string()).collect())
                    .ok();
            }
            Some("event_type") => {
                file_event_type = field
                    .text()
                    .await
                    .map(|t| FileEventType::try_from(t.as_str()).ok())
                    .ok()
                    .flatten()
            }
            Some("file") => file_bytes = field.bytes().await.ok(),
            Some(other) => eprintln!("Unknown field name '{other}' in upload handler"),
        }
    }

    // map to domain object (FileEvent)
    match ClientFileEvent::try_from(ClientFileEventDto {
        utc_millis,
        relative_path,
        file_event_type,
        file_bytes,
    }) {
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
        Ok(event) => {
            let utc_millis_of_latest_history_event = state
                .history
                .get_latest_event(&event.relative_path)
                .map(|e| e.utc_millis)
                .unwrap_or(0);

            if event.utc_millis < utc_millis_of_latest_history_event {
                eprintln!(
                    "Dropping event for {:?} - event ({:?}) older than latest history state event ({:?})",
                    &event.relative_path, utc_millis_of_latest_history_event, event.utc_millis
                );
                Err((StatusCode::BAD_REQUEST, "not latest".to_string()))
            } else {
                // actual disk io
                let sub_path = event
                    .relative_path
                    .get()
                    .iter()
                    .map(|part| Component::Normal(part.as_ref()));
                println!("@@@ sub-path {:?}", sub_path);
                let path = upload_root_path.components().chain(sub_path).collect();
                let io_result = match event.event_type {
                    FileEventType::CreateEvent | FileEventType::UpdateEvent => {
                        // safe to unwrap because we know it was set for Create & UpdateEvent
                        let bytes = event.file_bytes.as_ref().unwrap();
                        write::create_all_dir_and_write(&path, bytes)
                    }
                    FileEventType::DeleteEvent => fs::remove_file(&path),
                };

                let path_str = path.to_string_lossy();
                match io_result {
                    Ok(_) => {
                        let message = match event.event_type {
                            FileEventType::CreateEvent => {
                                format!("Created {} successfully", path_str)
                            }
                            FileEventType::UpdateEvent => {
                                format!("Replaced {} successfully", path_str)
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
                        println!("{message}");
                        Ok(message)
                    }
                    Err(e) => {
                        let message = match event.event_type {
                            FileEventType::CreateEvent => {
                                format!("Creating {} failed - {}", path_str, e)
                            }
                            FileEventType::UpdateEvent => {
                                format!("Replacing {} failed - {}", path_str, e)
                            }
                            FileEventType::DeleteEvent => {
                                format!("Deleting {} failed - {}", path_str, e)
                            }
                        };
                        eprintln!("{message}");
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
    println!("Client state received {:#?}", client_sync_state);
    let mut instructions = Vec::new();
    let target = state.history.clone().get_latest_non_deleted_events();

    for event in target.clone() {
        match client_sync_state.iter().find(|client_file_description| {
            client_file_description.relative_path == event.relative_path
        }) {
            // client doesn't have the file at all
            None => instructions.push(SyncInstruction::Download(event.relative_path)),
            Some(client_equivalent) => {
                println!(
                    "Server has {} ({}) - client has {} ({})",
                    get_utc_millis_as_date_string(event.utc_millis),
                    event.utc_millis,
                    get_utc_millis_as_date_string(client_equivalent.last_updated_utc_millis),
                    client_equivalent.last_updated_utc_millis
                );

                if client_equivalent.size_in_bytes == event.size_in_bytes {
                    // same size - just ignore even if timestamps differ (might have been write-operation without change)
                    continue;
                } else if client_equivalent.last_updated_utc_millis < event.utc_millis {
                    // differs in size and is outdated -> needs to be updated
                    instructions.push(SyncInstruction::Download(event.relative_path))
                } else {
                    instructions.push(SyncInstruction::Upload(event.relative_path))
                    // // differs in size but server's version is older - client must be ahead of server
                    // let string = format!("Client ahead of server: client's version of '{:?}' is newer than the respective server version", event.relative_path);
                    // eprintln!("{string}");
                    // return Err((StatusCode::BAD_REQUEST, string));
                }
            }
        }
    }

    for desc in client_sync_state {
        if !target.iter().any(|e| e.relative_path == desc.relative_path) {
            instructions.push(SyncInstruction::Upload(desc.relative_path));
        }
    }

    println!("Instructions {:#?}", instructions);
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
            "text; charset=utf-8".to_string(),
        ),
        (
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file_name),
        ),
    ];

    Ok((headers, body))
}
