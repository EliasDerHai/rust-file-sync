use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use shared::dtos::{LinkCreateDto, LinkDeleteDto, LinkDto, LinkTagCreateDto};
use tracing::{error, info};

pub async fn get_links(
    State(state): State<AppState>,
) -> Result<Json<Vec<LinkDto>>, (StatusCode, String)> {
    let links = state.db.link().get_links().await.map_err(|e| {
        error!("Failed to get link: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(links))
}

pub async fn post_link(
    State(state): State<AppState>,
    Json(request): Json<LinkCreateDto>,
) -> Result<String, (StatusCode, String)> {
    state
        .db
        .link()
        .insert_link(&request.url, request.title.as_deref())
        .await
        .map_err(|e| {
            error!("Failed to store link: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Stored link: {}", request.url);
    Ok("ok".to_string())
}

pub async fn delete_link(
    State(state): State<AppState>,
    Json(request): Json<LinkDeleteDto>,
) -> Result<String, (StatusCode, String)> {
    state
        .db
        .link()
        .delete_link(&request.url)
        .await
        .map_err(|e| {
            error!("Failed to delete link: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Deleted link: {}", request.url);
    Ok("ok".to_string())
}

// tags

pub async fn post_link_tag(
    State(state): State<AppState>,
    Json(request): Json<LinkTagCreateDto>,
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
