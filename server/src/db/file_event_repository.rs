use anyhow::anyhow;
use shared::content_hash::ContentHash;
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

    pub async fn insert(&self, event: &FileEvent) -> Result<()> {
        let id = event.id.to_string();
        let utc_millis = event.utc_millis.as_u64() as i64;
        let relative_path = event.relative_path.to_serialized_string();
        let size_in_bytes = event.size_in_bytes as i64;
        let content_hash = i64::from(event.content_hash);
        let event_type = event.event_type.serialize_to_string();
        let client_id = event.client_id.to_string();
        let watch_group_id = event.watch_group_id;

        sqlx::query!(
            r#"
            INSERT INTO file_event (id, utc_millis, relative_path, size_in_bytes, content_hash, event_type, client_id, watch_group_id)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            id,
            utc_millis,
            relative_path,
            size_in_bytes,
            content_hash,
            event_type,
            client_id,
            watch_group_id,
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_all_events(&self) -> Result<Vec<FileEvent>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                id,
                utc_millis,
                relative_path,
                size_in_bytes,
                content_hash,
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
            .flat_map(|row| -> anyhow::Result<FileEvent> {
                Ok(FileEvent::new(
                    Uuid::parse_str(&row.id)?,
                    UtcMillis::from(row.utc_millis as u64),
                    MatchablePath::from(row.relative_path.as_str()),
                    row.size_in_bytes as u64,
                    ContentHash::from(row.content_hash),
                    FileEventType::try_from(row.event_type.as_str()).map_err(|e| anyhow!("{e}"))?,
                    Uuid::parse_str(&row.client_id)?,
                    Some(row.client_id),
                    row.watch_group_id,
                ))
            })
            .collect())
    }
}
