use crate::db::{ClientWithConfig, ServerWatchGroup};
use crate::AppState;
use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::Json;
use serde::Deserialize;
use shared::register::ClientConfigDto;
use tracing::{error, info};

/// Admin-specific DTO for config updates (includes server_watch_group_id)
#[derive(Debug, Deserialize)]
pub struct AdminConfigUpdateDto {
    pub path_to_monitor: String,
    pub min_poll_interval_in_ms: u16,
    pub exclude_dirs: Vec<String>,
    pub exclude_dot_dirs: bool,
    pub server_watch_group_id: i64,
}

#[derive(Template)]
#[template(path = "configs.html")]
struct ConfigsTemplate {
    clients: Vec<ClientWithConfig>,
}

#[derive(Template)]
#[template(path = "config_edit.html")]
struct ConfigEditTemplate {
    client: ClientWithConfig,
    watch_groups: Vec<ServerWatchGroup>,
}

/// GET /api/configs - JSON list of all client configs
pub async fn api_list_configs(
    State(state): State<AppState>,
) -> Result<Json<Vec<ClientWithConfig>>, (StatusCode, String)> {
    let clients = state.db.client().get_all_clients().await.map_err(|e| {
        error!("Failed to get clients: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(clients))
}

/// GET /api/config/{id} - JSON single client config
pub async fn api_get_config(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<ClientWithConfig>, (StatusCode, String)> {
    let client = state
        .db
        .client()
        .get_client_by_id(&id)
        .await
        .map_err(|e| {
            error!("Failed to get client: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or((StatusCode::NOT_FOUND, "Client not found".to_string()))?;
    Ok(Json(client))
}

/// GET /configs - List all client configs (admin UI)
pub async fn list_admin_configs(
    State(state): State<AppState>,
) -> Result<Html<String>, (StatusCode, String)> {
    let clients = state.db.client().get_all_clients().await.map_err(|e| {
        error!("Failed to get clients: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    let template = ConfigsTemplate { clients };
    let html = template.render().map_err(|e| {
        error!("Failed to render template: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Html(html))
}

/// GET /config/{id} - Get client config (admin UI)
pub async fn get_admin_config(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Html<String>, (StatusCode, String)> {
    let client = state
        .db
        .client()
        .get_client_by_id(&id)
        .await
        .map_err(|e| {
            error!("Failed to get client: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or((StatusCode::NOT_FOUND, "Client not found".to_string()))?;

    let watch_groups = state
        .db
        .server()
        .get_all_watch_groups()
        .await
        .map_err(|e| {
            error!("Failed to get watch groups: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    let template = ConfigEditTemplate {
        client,
        watch_groups,
    };
    let html = template.render().map_err(|e| {
        error!("Failed to render template: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Html(html))
}

/// PUT /config/:id - Update client config (admin UI)
pub async fn update_admin_config(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(update): Json<AdminConfigUpdateDto>,
) -> Result<String, (StatusCode, String)> {
    let config = ClientConfigDto {
        path_to_monitor: update.path_to_monitor,
        min_poll_interval_in_ms: update.min_poll_interval_in_ms,
        exclude_dirs: update.exclude_dirs,
        exclude_dot_dirs: update.exclude_dot_dirs,
    };

    let updated = state
        .db
        .client()
        .update_client_config(&id, config, update.server_watch_group_id)
        .await
        .map_err(|e| {
            error!("Failed to update client config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    if updated {
        info!("Updated config for client {}", id);
        Ok("Config updated successfully".to_string())
    } else {
        Err((StatusCode::NOT_FOUND, "Client not found".to_string()))
    }
}
