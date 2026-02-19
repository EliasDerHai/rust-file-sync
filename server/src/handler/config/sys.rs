use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use shared::endpoint::{CLIENT_HOST_HEADER_KEY, CLIENT_ID_HEADER_KEY};
use shared::register::WatchConfigDto;
use tracing::{debug, error, info};

use super::super::header_value_as_string;

/// Get client config (or create)
pub async fn get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WatchConfigDto>, (StatusCode, String)> {
    let client_id = header_value_as_string(&headers, CLIENT_ID_HEADER_KEY)?;
    let host_name = header_value_as_string(&headers, CLIENT_HOST_HEADER_KEY)?;

    match state.db.client().get_client_config(client_id).await {
        Ok(Some(config)) => {
            debug!("Returning config for client {}", client_id);
            Ok(Json(config))
        }
        Ok(None) => {
            info!("No config found for client {} - adding one...", client_id);
            let watch_config = WatchConfigDto::default();
            state
                .db
                .client()
                .upsert_client_config(client_id, host_name, &watch_config)
                .await
                .map_err(|e| {
                    error!("Failed to register client: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                })?;
            info!("Registered client {} ({})", client_id, host_name);
            Ok(Json(watch_config))
        }
        Err(e) => {
            error!("Failed to get client config: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}
