use shared::matchable_path::MatchablePath;
use shared::utc_millis::UtcMillis;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::file_event::{FileEvent, FileEventType};

pub struct FileEventRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> FileEventRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        self.pool
    }

    pub async fn insert(&self, event: &FileEvent, client_id: &str) -> Result<()> {
        let id = event.id.to_string();
        let utc_millis = event.utc_millis.as_u64() as i64;
        let relative_path = event.relative_path.to_serialized_string();
        let size_in_bytes = event.size_in_bytes as i64;
        let event_type = event.event_type.serialize_to_string();
        let watch_group_id = event.watch_group_id;

        sqlx::query!(
            r#"
            INSERT INTO file_event (id, utc_millis, relative_path, size_in_bytes, event_type, client_id, watch_group_id)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            id,
            utc_millis,
            relative_path,
            size_in_bytes,
            event_type,
            client_id,
            watch_group_id,
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn bulk_insert(&self, events: Vec<(FileEvent, String)>) -> Result<u64> {
        let mut count = 0u64;
        for (event, client_id) in &events {
            let id = event.id.to_string();
            let utc_millis = event.utc_millis.as_u64() as i64;
            let relative_path = event.relative_path.to_serialized_string();
            let size_in_bytes = event.size_in_bytes as i64;
            let event_type = event.event_type.serialize_to_string();
            let watch_group_id = event.watch_group_id;

            sqlx::query!(
                r#"
                INSERT INTO file_event (id, utc_millis, relative_path, size_in_bytes, event_type, client_id, watch_group_id)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
                id,
                utc_millis,
                relative_path,
                size_in_bytes,
                event_type,
                client_id,
                watch_group_id,
            )
            .execute(self.pool)
            .await?;

            count += 1;
        }
        Ok(count)
    }

    pub async fn get_all_events(&self) -> Result<Vec<FileEvent>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                id as "id!",
                utc_millis,
                relative_path,
                size_in_bytes,
                event_type,
                client_id,
                watch_group_id
            FROM file_event
            ORDER BY utc_millis ASC
            "#
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                FileEvent::new(
                    Uuid::parse_str(&row.id).unwrap_or_else(|_| Uuid::new_v4()),
                    UtcMillis::from(row.utc_millis as u64),
                    MatchablePath::from(row.relative_path.as_str()),
                    row.size_in_bytes as u64,
                    FileEventType::try_from(row.event_type.as_str())
                        .unwrap_or(FileEventType::ChangeEvent),
                    Some(row.client_id),
                    row.watch_group_id,
                )
            })
            .collect())
    }
}
