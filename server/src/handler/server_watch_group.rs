use crate::{AppState, UPLOAD_PATH};
use crate::file_event::FileEvent;
use crate::file_history::FileHistory;
use axum::Json;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use shared::dtos::{FileDescription, ServerWatchGroup, WatchGroupNameDto};
use std::collections::HashMap;
use std::path::{Component, PathBuf};
use tokio_util::io::ReaderStream;
use tracing::{error, info};

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
