use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use shared::dtos::{ClientDto, ClientWatchGroupUpdateDto};
use tracing::{error, info};

/// GET /api/configs - JSON list of all client configs
pub async fn api_list_configs(
    State(state): State<AppState>,
) -> Result<Json<Vec<ClientDto>>, (StatusCode, String)> {
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
) -> Result<Json<ClientDto>, (StatusCode, String)> {
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

/// PUT /api/config/{id} - Update client config
pub async fn api_update_config(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(update): Json<ClientWatchGroupUpdateDto>,
) -> Result<String, (StatusCode, String)> {
    let updated = state
        .db
        .client_watch_group()
        .update(
            &id,
            update.server_watch_group_id,
            &update.path_to_monitor,
            update.exclude_dirs,
            update.exclude_dot_dirs,
        )
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
