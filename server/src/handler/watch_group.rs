use crate::AppState;
use crate::db::ServerWatchGroup;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use tracing::{error, info};

#[derive(Deserialize)]
pub struct WatchGroupNameDto {
    pub name: String,
}

/// GET /api/watch-groups - JSON list of all watch groups
pub async fn api_list_watch_groups(
    State(state): State<AppState>,
) -> Result<Json<Vec<ServerWatchGroup>>, (StatusCode, String)> {
    let groups = state
        .db
        .server()
        .get_all_watch_groups()
        .await
        .map_err(|e| {
            error!("Failed to get watch groups: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    Ok(Json(groups))
}

/// POST /api/watch-groups - Create a new watch group
pub async fn api_create_watch_group(
    State(state): State<AppState>,
    Json(dto): Json<WatchGroupNameDto>,
) -> Result<(StatusCode, String), (StatusCode, String)> {
    state
        .db
        .server()
        .insert_watch_group(dto.name.clone())
        .await
        .map_err(|e| {
            error!("Failed to create watch group: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Created watch group '{}'", dto.name);
    Ok((StatusCode::CREATED, "Watch group created".to_string()))
}

/// PUT /api/watch-groups/{id} - Rename watch group
pub async fn api_update_watch_group(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(dto): Json<WatchGroupNameDto>,
) -> Result<String, (StatusCode, String)> {
    state
        .db
        .server()
        .rename_watch_group(id, dto.name)
        .await
        .map_err(|e| {
            error!("Failed to rename watch group: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Renamed watch group {}", id);
    Ok("Watch group updated successfully".to_string())
}
