use shared::dtos::LocationPointCreateDto;
use sqlx::SqlitePool;

pub struct LocationPointRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> LocationPointRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Inserts a batch of points in one transaction - all-or-nothing, so a failure
    /// partway through a large batch never leaves a partial upload stored.
    pub async fn insert_batch(&self, points: &[LocationPointCreateDto]) -> Result<usize> {
        let mut tx = self.pool.begin().await?;

        for p in points {
            let timestamp_epoch_ms = p.timestamp_epoch_ms.as_u64() as i64;

            sqlx::query!(
                r#"
                INSERT INTO location_point
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
        }

        tx.commit().await?;

        Ok(points.len())
    }
}
