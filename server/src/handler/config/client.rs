use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use shared::endpoint::{CLIENT_HOST_HEADER_KEY, CLIENT_ID_HEADER_KEY};
use shared::register::ClientConfigDto;
use tracing::{debug, error, info};

use super::super::header_value_as_string;

/// Get client config by client_id header
pub async fn get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ClientConfigDto>, (StatusCode, String)> {
    let client_id = header_value_as_string(&headers, CLIENT_ID_HEADER_KEY)?;

    match state.db.client().get_client_config(client_id).await {
        Ok(Some(config)) => {
            debug!("Returning config for client {}", client_id);
            Ok(Json(config))
        }
        Ok(None) => {
            debug!("No config found for client {}", client_id);
            Err((StatusCode::NOT_FOUND, "Client not registered".to_string()))
        }
        Err(e) => {
            error!("Failed to get client config: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Register or upsert a client config
pub async fn post_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClientConfigDto>,
) -> Result<String, (StatusCode, String)> {
    let client_id = header_value_as_string(&headers, CLIENT_ID_HEADER_KEY)?;
    let host_name = header_value_as_string(&headers, CLIENT_HOST_HEADER_KEY)?;

    state
        .db
        .client()
        .upsert_client_config(client_id, host_name, request)
        .await
        .map_err(|e| {
            error!("Failed to register client: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Registered client {} ({})", client_id, host_name);
    Ok(format!("Client {} registered successfully", client_id))
}
