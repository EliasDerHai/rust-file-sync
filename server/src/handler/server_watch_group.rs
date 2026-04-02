use crate::file_event::{FileEvent, FileEventType};
use crate::file_history::FileHistory;
use crate::write::write_all_chunks_of_field;
use crate::{AppState, UPLOAD_PATH, UPLOAD_TMP_PATH};

/// UUID of the sentinel 'pwa' client row — must match the migration.
const PWA_CLIENT_ID: &str = "f4a7b3c2-8d5e-4f6a-9b2c-1e3d5f7a9b0c";

use axum::Json;
use axum::body::Body;
use axum::extract::{Multipart, Query, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use shared::dtos::{FileDescription, ServerWatchGroup, WatchGroupNameDto};
use shared::matchable_path::MatchablePath;
use shared::utc_millis::UtcMillis;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use tokio_util::io::ReaderStream;
use tracing::{error, info, warn};
use uuid::Uuid;

/// GET /api/watch-groups
pub async fn api_list_watch_groups(
    State(state): State<AppState>,
) -> Result<Json<Vec<ServerWatchGroup>>, (StatusCode, String)> {
    let groups = state
        .db
        .server_watch_group()
        .get_all_watch_groups()
        .await
        .map_err(|e| {
            error!("Failed to get watch groups: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    Ok(Json(groups))
}

/// POST /api/watch-groups
pub async fn api_create_watch_group(
    State(state): State<AppState>,
    Json(dto): Json<WatchGroupNameDto>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .server_watch_group()
        .insert_watch_group(dto.name.clone())
        .await
        .map_err(|e| {
            error!("Failed to create watch group: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Created watch group '{}'", dto.name);
    Ok(StatusCode::CREATED)
}

/// PUT /api/watch-groups/{id}
pub async fn api_update_watch_group(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(dto): Json<WatchGroupNameDto>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .server_watch_group()
        .rename_watch_group(id, dto.name)
        .await
        .map_err(|e| {
            error!("Failed to rename watch group: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Renamed watch group {}", id);
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/watch-groups/{id}
pub async fn api_delete_watch_group(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let found = state
        .db
        .server_watch_group()
        .delete(id)
        .await
        .map_err(|e| {
            error!("Failed to delete watch group: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    if found {
        info!("Deleted watch group {}", id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "Watch group not found".to_string()))
    }
}

/// GET /api/watch-groups/{id}/file?path=dir/subdir/file.ext — inline file preview
pub async fn api_serve_watch_group_file(
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let path_str = match params.get("path") {
        Some(p) if !p.is_empty() => p.clone(),
        _ => return Err((StatusCode::BAD_REQUEST, "Missing path parameter".to_string())),
    };

    // Filter to Normal components only — strips "..", "/", "~" (traversal safety)
    let components: Vec<String> = std::path::Path::new(&path_str)
        .components()
        .filter_map(|c| match c {
            Component::Normal(os) => Some(os.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();

    if components.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Invalid path".to_string()));
    }

    let rel: PathBuf = components.iter().collect();
    let full_path = UPLOAD_PATH.join(id.to_string()).join(rel);

    let file = match tokio::fs::File::open(&full_path).await {
        Ok(f) => f,
        Err(e) => return Err((StatusCode::NOT_FOUND, format!("File not found: {}", e))),
    };

    let ext = full_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let content_type: &'static str = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" | "md" | "rs" | "toml" | "json" | "yaml" | "yml" | "sh" | "log" => {
            "text/plain; charset=utf-8"
        }
        _ => "application/octet-stream",
    };

    let body = Body::from_stream(ReaderStream::new(file));
    Ok(([(CONTENT_TYPE, content_type)], body))
}

pub async fn api_get_watch_group_files(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<Vec<FileDescription>>, (StatusCode, String)> {
    let events = state
        .history
        .get_latest_events(id)
        .into_iter()
        .filter(|e| e.event_type.is_change())
        .map(FileEvent::into)
        .collect();

    Ok(Json(events))
}

/// POST /api/watch-groups/{id}/files
pub async fn api_upload_to_watch_group(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    mut multipart: Multipart,
) -> Result<StatusCode, (StatusCode, String)> {
    let exists = state
        .db
        .server_watch_group()
        .exists(id)
        .await
        .map_err(|e| {
            error!("Failed to check watch group existence: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    if !exists {
        return Err((StatusCode::NOT_FOUND, format!("Watch group {id} not found")));
    }

    let (tmp_path, filename, size) = extract_file(&mut multipart).await?;

    let target_dir = UPLOAD_PATH.join(id.to_string());
    let target_path = target_dir.join(&filename);

    if let Err(e) = fs::create_dir_all(&target_dir) {
        let _ = fs::remove_file(&tmp_path);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create directory: {e}"),
        ));
    }

    if let Err(e) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to move file: {e}"),
        ));
    }

    let event = FileEvent::new(
        Uuid::new_v4(),
        UtcMillis::now(),
        MatchablePath::from(vec![filename.as_str()]),
        size as u64,
        FileEventType::ChangeEvent,
        Some("pwa".to_string()),
        id,
    );

    if let Err(e) = state.db.file_event().insert(&event, PWA_CLIENT_ID).await {
        error!("Failed to persist file event for PWA upload: {e}");
    }
    state.history.add(event);

    info!("PWA uploaded '{}' to watch group {id}", filename);
    Ok(StatusCode::CREATED)
}

async fn extract_file(
    multipart: &mut Multipart,
) -> Result<(PathBuf, String, usize), (StatusCode, String)> {
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        if field.name() != Some("file") {
            continue;
        }

        let raw_name = field.file_name().unwrap_or("upload").to_string();
        let filename = sanitize_filename(&raw_name)?;

        let tmp_path = UPLOAD_TMP_PATH.join(format!("{}_{}", Uuid::new_v4(), filename));
        let size = write_all_chunks_of_field(tmp_path.as_path(), field)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to write upload: {e}"),
                )
            })?;

        return Ok((tmp_path, filename, size));
    }

    Err((StatusCode::BAD_REQUEST, "No 'file' field in request".to_string()))
}

fn sanitize_filename(raw: &str) -> Result<String, (StatusCode, String)> {
    let name = Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .filter(|n| !n.is_empty() && n != "." && n != "..")
        .ok_or_else(|| {
            warn!("Rejected unsafe filename: {:?}", raw);
            (StatusCode::BAD_REQUEST, format!("Invalid filename: {raw}"))
        })?;
    Ok(name)
}
