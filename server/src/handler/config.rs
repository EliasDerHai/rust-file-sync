use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use shared::dtos::WatchConfigDto;
use shared::endpoint::{CLIENT_HOST_HEADER_KEY, CLIENT_ID_HEADER_KEY};
use tracing::{debug, error, info};

use super::header_value_as_string;

/// Get client config (or create)
pub async fn get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WatchConfigDto>, (StatusCode, String)> {
    let client_id = header_value_as_string(&headers, CLIENT_ID_HEADER_KEY)?;
    let host_name = header_value_as_string(&headers, CLIENT_HOST_HEADER_KEY)?;

    match state.db.client().get_client_by_id(client_id).await {
        Ok(Some(client)) => {
            debug!("Returning config for client {}", client_id);
            let watch_groups = state
                .db
                .client_watch_group()
                .get_for_client(client_id)
                .await
                .map_err(|e| {
                    error!("Failed to get watch groups for client: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                })?;
            Ok(Json(WatchConfigDto {
                min_poll_interval_in_ms: client.min_poll_interval_in_ms,
                watch_groups,
            }))
        }
        Ok(None) => {
            info!("No config found for client {} - adding one...", client_id);
            state
                .db
                .client()
                .upsert_client(client_id, host_name)
                .await
                .map_err(|e| {
                    error!("Failed to register client: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                })?;
            info!("Registered client {} ({})", client_id, host_name);
            Ok(Json(WatchConfigDto::default()))
        }
        Err(e) => {
            error!("Failed to get client config: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}
