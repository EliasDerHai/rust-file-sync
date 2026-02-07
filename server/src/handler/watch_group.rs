use crate::db::ServerWatchGroup;
use crate::AppState;
use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::Json;
use serde::Deserialize;
use tracing::{error, info};

#[derive(Template)]
#[template(path = "watch_groups.html")]
struct WatchGroupsTemplate {
    groups: Vec<ServerWatchGroup>,
}

#[derive(Deserialize)]
pub struct WatchGroupNameDto {
    pub name: String,
}

/// GET /api/watch-groups - JSON list of all watch groups
pub async fn api_list_watch_groups(
    State(state): State<AppState>,
) -> Result<Json<Vec<ServerWatchGroup>>, (StatusCode, String)> {
    let groups = state.db.server().get_all_watch_groups().await.map_err(|e| {
        error!("Failed to get watch groups: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(groups))
}

/// GET /admin/watch-groups - List all watch groups (admin UI)
pub async fn list_admin_watch_groups(
    State(state): State<AppState>,
) -> Result<Html<String>, (StatusCode, String)> {
    let groups = state.db.server().get_all_watch_groups().await.map_err(|e| {
        error!("Failed to get watch groups: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    let template = WatchGroupsTemplate { groups };
    let html = template.render().map_err(|e| {
        error!("Failed to render template: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Html(html))
}

/// POST /admin/watch-groups - Create a new watch group (admin UI)
pub async fn create_admin_watch_group(
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

/// PUT /admin/watch-group/{id} - Rename watch group (admin UI)
pub async fn update_admin_watch_group(
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
