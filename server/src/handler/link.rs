use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use shared::dtos::{LinkDto, LinkTagPostDto};
use tracing::{error, info};

pub async fn post_link(
    State(state): State<AppState>,
    Json(request): Json<LinkDto>,
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
) -> Result<Json<Vec<LinkDto>>, (StatusCode, String)> {
    let links = state.db.link().get_links().await.map_err(|e| {
        error!("Failed to store shared link: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(links))
}

pub async fn post_link_tag(
    State(state): State<AppState>,
    Json(request): Json<LinkTagPostDto>,
) -> Result<String, (StatusCode, String)> {
    state
        .db
        .link_tag()
        .insert_link_tag(&request.tag, &request.url)
        .await
        .map_err(|e| {
            error!("Failed to store link tag: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Stored tag '{}' for link: {}", request.tag, request.url);
    Ok("ok".to_string())
}
