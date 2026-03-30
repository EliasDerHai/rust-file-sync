use crate::AppState;
use crate::file_event::FileEvent;
use crate::file_history::FileHistory;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use shared::dtos::{FileDescription, ServerWatchGroup, WatchGroupNameDto};
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
