use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use shared::dtos::{
    LocationPointCreateDto, LocationPointDeleteDto, LocationPointDto,
    LocationPointUploadResultDto,
};
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

#[derive(serde::Deserialize)]
pub struct LocationRangeQuery {
    pub since_epoch_ms: Option<i64>,
    pub until_epoch_ms: Option<i64>,
}

/// GET /api/locations?since_epoch_ms=&until_epoch_ms= - points in the given range
/// (whole trip so far when both are omitted), excluding soft-deleted ones, with
/// `is_flagged` computed fresh for outlier review in the web admin's Locations tab.
pub async fn get_location_points(
    State(state): State<AppState>,
    Query(q): Query<LocationRangeQuery>,
) -> Result<Json<Vec<LocationPointDto>>, (StatusCode, String)> {
    let since = q.since_epoch_ms.unwrap_or(0);
    let until = q.until_epoch_ms.unwrap_or(i64::MAX);

    state
        .db
        .location_point()
        .get_range(since, until)
        .await
        .map(Json)
        .map_err(|e| {
            error!("Failed to load location points: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })
}

/// DELETE /api/locations - bulk soft-delete of user-confirmed outlier points.
pub async fn delete_location_points(
    State(state): State<AppState>,
    Json(body): Json<LocationPointDeleteDto>,
) -> Result<Json<usize>, (StatusCode, String)> {
    let excluded = state
        .db
        .location_point()
        .soft_delete_batch(&body.ids)
        .await
        .map_err(|e| {
            error!("Failed to exclude location points: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Excluded {} location points", excluded);

    Ok(Json(excluded))
}
