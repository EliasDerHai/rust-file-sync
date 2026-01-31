use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use tracing::{error, info};

#[derive(Deserialize)]
pub struct ShareLinkRequest {
    pub url: String,
    pub title: Option<String>,
}

pub async fn receive_shared_link(
    State(state): State<AppState>,
    Json(request): Json<ShareLinkRequest>,
) -> Result<String, (StatusCode, String)> {
    state
        .db
        .shared_link()
        .store_shared_link(&request.url, request.title.as_deref())
        .await
        .map_err(|e| {
            error!("Failed to store shared link: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Stored shared link: {}", request.url);
    Ok("ok".to_string())
}
