use crate::db::ClientWithConfig;
use crate::AppState;
use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::Json;
use shared::register::ClientConfigDto;
use tracing::{error, info};

#[derive(Template)]
#[template(path = "configs.html")]
struct ConfigsTemplate {
    clients: Vec<ClientWithConfig>,
}

#[derive(Template)]
#[template(path = "config_edit.html")]
struct ConfigEditTemplate {
    client: ClientWithConfig,
}

/// GET /configs - List all client configs (admin UI)
pub async fn list_configs(
    State(state): State<AppState>,
) -> Result<Html<String>, (StatusCode, String)> {
    let clients = state
        .db
        .client_config()
        .get_all_clients()
        .await
        .map_err(|e| {
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

/// GET /config/{id} - Edit form for a single client (admin UI)
pub async fn get_config_edit(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Html<String>, (StatusCode, String)> {
    let client = state
        .db
        .client_config()
        .get_client_by_id(&id)
        .await
        .map_err(|e| {
            error!("Failed to get client: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or((StatusCode::NOT_FOUND, "Client not found".to_string()))?;

    let template = ConfigEditTemplate { client };
    let html = template.render().map_err(|e| {
        error!("Failed to render template: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Html(html))
}

/// PUT /config/:id - Update client config (admin UI)
pub async fn update_config(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(config): Json<ClientConfigDto>,
) -> Result<String, (StatusCode, String)> {
    let updated = state
        .db
        .client_config()
        .update_client_config(&id, config)
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
