use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use tracing::{error, info};

#[derive(Deserialize, serde::Serialize)]
pub struct ShareLinkRequest {
    pub url: String,
    pub title: Option<String>,
}

pub async fn post_link(
    State(state): State<AppState>,
    Json(request): Json<ShareLinkRequest>,
) -> Result<String, (StatusCode, String)> {
    state
        .db
        .link()
        .insert_link(&request.url, request.title.as_deref())
        .await
        .map_err(|e| {
            error!("Failed to store shared link: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Stored shared link: {}", request.url);
    Ok("ok".to_string())
}

pub async fn get_links(
    State(state): State<AppState>,
) -> Result<Json<Vec<ShareLinkRequest>>, (StatusCode, String)> {
    let links = state.db.link().get_links().await.map_err(|e| {
        error!("Failed to store shared link: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(links))
}
