use shared::dtos::ClientDto;
use sqlx::SqlitePool;

pub struct ClientRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> ClientRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Register or update a client
    pub async fn upsert_client(&self, client_id: &str, host_name: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let min_poll_interval_in_ms = 5000;

        // Upsert client
        sqlx::query!(
            r#"
            INSERT INTO client (id, host_name, min_poll_interval_in_ms)
            VALUES (?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                host_name = excluded.host_name,
                min_poll_interval_in_ms = excluded.min_poll_interval_in_ms
            "#,
            client_id,
            host_name,
            min_poll_interval_in_ms
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Get all clients with their configs
    pub async fn get_all_clients(&self) -> Result<Vec<ClientDto>> {
        let clients = sqlx::query!(
            r#"
            SELECT
                c.id,
                c.host_name,
                c.min_poll_interval_in_ms
            FROM client c
            ORDER BY c.host_name
            "#
        )
        .fetch_all(self.pool)
        .await?;

        Ok(clients
            .into_iter()
            .map(|r| ClientDto {
                id: r.id,
                host_name: r.host_name,
                min_poll_interval_in_ms: u16::try_from(r.min_poll_interval_in_ms)
                    .expect("should fit"),
            })
            .collect())
    }

    /// Get single client
    pub async fn get_client_by_id(&self, client_id: &str) -> Result<Option<ClientDto>> {
        let client = sqlx::query!(
            r#"
            SELECT
                c.id,
                c.host_name,
                c.min_poll_interval_in_ms
            FROM client c
            WHERE c.id = ?
            "#,
            client_id
        )
        .fetch_optional(self.pool)
        .await?;

        match client {
            Some(r) => Ok(Some(ClientDto {
                id: r.id,
                host_name: r.host_name,
                min_poll_interval_in_ms: u16::try_from(r.min_poll_interval_in_ms)
                    .expect("should fit"),
            })),
            None => Ok(None),
        }
    }

    /// Update a single watch group for a client
    pub async fn update_single_watch_group(
        &self,
        client_id: &str,
        server_watch_group_id: i64,
        path_to_monitor: &str,
        exclude_dirs: Vec<String>,
        exclude_dot_dirs: bool,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;

        let watch_group_id = sqlx::query_scalar!(
            r#"
            UPDATE client_watch_group SET path_to_monitor = ?, exclude_dot_dirs = ?
            WHERE server_watch_group_id = ? AND client_id = ?
            RETURNING id
            "#,
            path_to_monitor,
            exclude_dot_dirs,
            server_watch_group_id,
            client_id
        )
        .fetch_one(&mut *tx)
        .await?;

        for exclude_dir in exclude_dirs {
            sqlx::query!(
                r#"
                INSERT INTO client_watch_group_excluded_dir (client_watch_group, exclude_dir)
                VALUES (?, ?)
                "#,
                watch_group_id,
                exclude_dir
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ServerDatabase;
    use sqlx::migrate::Migrator;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Pool, Sqlite};

    static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

    async fn setup_test_db() -> (Pool<Sqlite>, ServerDatabase) {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        MIGRATOR.run(&pool).await.expect("Failed to run migrations");

        (pool.clone(), ServerDatabase::new(pool))
    }
}
