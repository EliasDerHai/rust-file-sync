use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use shared::dtos::{ClientDto, ClientUpdateDto};
use tracing::{error, info};

/// GET /api/clients
pub async fn api_list_clients(
    State(state): State<AppState>,
) -> Result<Json<Vec<ClientDto>>, (StatusCode, String)> {
    let clients = state.db.client().get_all_clients().await.map_err(|e| {
        error!("Failed to get clients: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(clients))
}

/// GET /api/clients/{id}
pub async fn api_get_client(
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

/// PUT /api/clients/{id}
pub async fn api_update_client(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(update): Json<ClientUpdateDto>,
) -> Result<String, (StatusCode, String)> {
    let found = state
        .db
        .client()
        .update(&id, update.min_poll_interval_in_ms)
        .await
        .map_err(|e| {
            error!("Failed to update client: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    if found {
        info!("Updated client {}", id);
        Ok("Client updated".to_string())
    } else {
        Err((StatusCode::NOT_FOUND, "Client not found".to_string()))
    }
}

/// DELETE /api/clients/{id}
pub async fn api_delete_client(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let found = state.db.client().delete(&id).await.map_err(|e| {
        error!("Failed to delete client: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    if found {
        info!("Deleted client {}", id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "Client not found".to_string()))
    }
}
