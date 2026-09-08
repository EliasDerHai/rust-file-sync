use shared::dtos::{LocationPointCreateDto, LocationPointDto};
use shared::utc_millis::UtcMillis;
use sqlx::SqlitePool;

pub struct LocationPointRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

/// Points with worse accuracy than this (meters) are flagged for review - typically
/// an indoor/urban-canyon fix rather than a real position.
const OUTLIER_ACCURACY_METERS: f32 = 100.0;
/// Points implying a speed above this (m/s, ~200 km/h) from the previous point are
/// flagged as likely GPS jumps. Both thresholds are read-time heuristics, not
/// persisted - retune here once real trip data is visible, no migration needed.
const OUTLIER_SPEED_METERS_PER_SECOND: f64 = 55.0;

/// Great-circle distance between two lat/lon points, in meters.
fn haversine_meters(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_METERS: f64 = 6_371_000.0;
    let (lat1r, lat2r) = (lat1.to_radians(), lat2.to_radians());
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2) + lat1r.cos() * lat2r.cos() * (dlon / 2.0).sin().powi(2);
    EARTH_RADIUS_METERS * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())
}

impl<'a> LocationPointRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Inserts a batch of points in one transaction - all-or-nothing, so a failure
    /// partway through a large batch never leaves a partial upload stored.
    pub async fn insert_batch(&self, points: &[LocationPointCreateDto]) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        let mut inserted = 0usize;

        for p in points {
            let timestamp_epoch_ms = p.timestamp_epoch_ms.as_u64() as i64;

            let result = sqlx::query!(
                r#"
                INSERT OR IGNORE INTO location_point
                    (timestamp_epoch_ms, latitude, longitude, altitude_meters, accuracy_meters, speed_meters_per_second)
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
                timestamp_epoch_ms,
                p.latitude,
                p.longitude,
                p.altitude_meters,
                p.accuracy_meters,
                p.speed_meters_per_second,
            )
            .execute(&mut *tx)
            .await?;

            inserted += result.rows_affected() as usize;
        }

        tx.commit().await?;

        Ok(inserted)
    }

    /// Active (non-excluded) points with `timestamp_epoch_ms` in `[since, until]`,
    /// ordered by time, with `is_flagged` computed fresh from the outlier
    /// heuristics above. Note: the speed check only sees predecessors within this
    /// range, so a range's first point is never speed-flagged - acceptable for v1.
    pub async fn get_range(&self, since_epoch_ms: i64, until_epoch_ms: i64) -> Result<Vec<LocationPointDto>> {
        let rows = sqlx::query!(
            r#"
            SELECT id as "id!: i64", timestamp_epoch_ms, latitude, longitude, altitude_meters,
                   accuracy_meters as "accuracy_meters: f32",
                   speed_meters_per_second as "speed_meters_per_second: f32"
            FROM location_point
            WHERE excluded_at IS NULL AND timestamp_epoch_ms BETWEEN ? AND ?
            ORDER BY timestamp_epoch_ms ASC
            "#,
            since_epoch_ms,
            until_epoch_ms,
        )
        .fetch_all(self.pool)
        .await?;

        let mut prev: Option<(i64, f64, f64)> = None;
        let points = rows
            .into_iter()
            .map(|row| {
                let flagged_accuracy = row
                    .accuracy_meters
                    .is_some_and(|a| a > OUTLIER_ACCURACY_METERS);
                let flagged_speed = prev.is_some_and(|(prev_ts, prev_lat, prev_lon)| {
                    let dt_s = (row.timestamp_epoch_ms - prev_ts) as f64 / 1000.0;
                    dt_s > 0.0
                        && haversine_meters(prev_lat, prev_lon, row.latitude, row.longitude) / dt_s
                            > OUTLIER_SPEED_METERS_PER_SECOND
                });
                prev = Some((row.timestamp_epoch_ms, row.latitude, row.longitude));

                LocationPointDto {
                    id: row.id,
                    timestamp_epoch_ms: UtcMillis::from(row.timestamp_epoch_ms as u64),
                    latitude: row.latitude,
                    longitude: row.longitude,
                    altitude_meters: row.altitude_meters,
                    accuracy_meters: row.accuracy_meters,
                    speed_meters_per_second: row.speed_meters_per_second,
                    is_flagged: flagged_accuracy || flagged_speed,
                }
            })
            .collect();

        Ok(points)
    }

    /// Soft-deletes the given point ids (sets `excluded_at`) - recoverable, since
    /// this is irreplaceable GPS history. Returns the number actually excluded.
    pub async fn soft_delete_batch(&self, ids: &[i64]) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        let mut excluded = 0usize;

        for id in ids {
            let result = sqlx::query!(
                "UPDATE location_point SET excluded_at = CURRENT_TIMESTAMP WHERE id = ? AND excluded_at IS NULL",
                id,
            )
            .execute(&mut *tx)
            .await?;

            excluded += result.rows_affected() as usize;
        }

        tx.commit().await?;

        Ok(excluded)
    }
}
