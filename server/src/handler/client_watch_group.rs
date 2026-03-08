use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use shared::dtos::{ClientWatchGroupCreateDto, ClientWatchGroupDto, ClientWatchGroupUpdateDto};
use tracing::{error, info};

/// GET /api/clients/{id}/watch-groups
pub async fn api_list_client_watch_groups(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Vec<ClientWatchGroupDto>>, (StatusCode, String)> {
    let assignments = state
        .db
        .client_watch_group()
        .list_for_client(&id)
        .await
        .map_err(|e| {
            error!("Failed to list watch groups for client {}: {}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    Ok(Json(assignments))
}

/// POST /api/clients/{id}/watch-groups
pub async fn api_create_client_watch_group(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(dto): Json<ClientWatchGroupCreateDto>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .client_watch_group()
        .create(
            &id,
            dto.server_watch_group_id,
            &dto.path_to_monitor,
            dto.exclude_dirs,
            dto.exclude_dot_dirs,
        )
        .await
        .map_err(|e| {
            error!(
                "Failed to create watch group assignment for client {}: {}",
                id, e
            );
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!(
        "Created watch group {} assignment for client {}",
        dto.server_watch_group_id, id
    );
    Ok(StatusCode::CREATED)
}

/// PUT /api/clients/{id}/watch-groups/{wg_id}
pub async fn api_update_client_watch_group(
    State(state): State<AppState>,
    axum::extract::Path((id, wg_id)): axum::extract::Path<(String, i64)>,
    Json(dto): Json<ClientWatchGroupUpdateDto>,
) -> Result<String, (StatusCode, String)> {
    let found = state
        .db
        .client_watch_group()
        .update(
            &id,
            wg_id,
            &dto.path_to_monitor,
            dto.exclude_dirs,
            dto.exclude_dot_dirs,
        )
        .await
        .map_err(|e| {
            error!(
                "Failed to update watch group {} for client {}: {}",
                wg_id, id, e
            );
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    if found {
        info!("Updated watch group {} for client {}", wg_id, id);
        Ok("Watch group assignment updated".to_string())
    } else {
        Err((StatusCode::NOT_FOUND, "Assignment not found".to_string()))
    }
}

/// DELETE /api/clients/{id}/watch-groups/{wg_id}
pub async fn api_delete_client_watch_group(
    State(state): State<AppState>,
    axum::extract::Path((id, wg_id)): axum::extract::Path<(String, i64)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let found = state
        .db
        .client_watch_group()
        .delete(&id, wg_id)
        .await
        .map_err(|e| {
            error!(
                "Failed to delete watch group {} for client {}: {}",
                wg_id, id, e
            );
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    if found {
        info!("Deleted watch group {} assignment for client {}", wg_id, id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "Assignment not found".to_string()))
    }
}
