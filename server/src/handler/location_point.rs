use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use shared::dtos::{LocationPointCreateDto, LocationPointUploadResultDto};
use tracing::{error, info};

use crate::AppState;

/// POST /api/locations - bulk-upload a batch of recorded GPS points.
pub async fn post_location_points(
    State(state): State<AppState>,
    Json(points): Json<Vec<LocationPointCreateDto>>,
) -> Result<Json<LocationPointUploadResultDto>, (StatusCode, String)> {
    let inserted = state
        .db
        .location_point()
        .insert_batch(&points)
        .await
        .map_err(|e| {
            error!("Failed to store location points: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Stored {} location points", inserted);

    Ok(Json(LocationPointUploadResultDto { inserted }))
}
