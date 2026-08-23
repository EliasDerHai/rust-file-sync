use crate::handler::PWA_CLIENT_UUID;
use crate::handler::WEB_CLIENT_UUID;
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
    pub async fn upsert_client(
        &self,
        client_id: &str,
        host_name: &str,
        version: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let min_poll_interval_in_ms = 5000;

        // Upsert client
        sqlx::query!(
            r#"
            INSERT INTO client (id, host_name, min_poll_interval_in_ms, version)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                host_name = excluded.host_name,
                min_poll_interval_in_ms = excluded.min_poll_interval_in_ms,
                version = excluded.version
            "#,
            client_id,
            host_name,
            min_poll_interval_in_ms,
            version
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Update only the reported version for an existing client.
    pub async fn update_version(&self, client_id: &str, version: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE client SET version = ? WHERE id = ?",
            version,
            client_id
        )
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Get all clients with their configs
    pub async fn get_all_clients(&self, server_version: &str) -> Result<Vec<ClientDto>> {
        let clients = sqlx::query!(
            r#"
            SELECT
                c.id,
                c.host_name,
                c.min_poll_interval_in_ms,
                c.version
            FROM client c
            ORDER BY c.host_name
            "#
        )
        .fetch_all(self.pool)
        .await?;

        Ok(clients
            .into_iter()
            .map(|r| {
                let version = if r.version == WEB_CLIENT_UUID.to_string().as_str()
                    || r.version == PWA_CLIENT_UUID.to_string().as_str()
                {
                    server_version.to_string()
                } else {
                    r.version
                };

                ClientDto {
                    id: r.id,
                    host_name: r.host_name,
                    min_poll_interval_in_ms: u16::try_from(r.min_poll_interval_in_ms)
                        .expect("should fit"),
                    version,
                }
            })
            .collect())
    }

    /// Update client-level settings. Returns false if client not found.
    pub async fn update(&self, client_id: &str, min_poll_interval_in_ms: u16) -> Result<bool> {
        let poll_interval = min_poll_interval_in_ms as i64;
        let rows = sqlx::query!(
            "UPDATE client SET min_poll_interval_in_ms = ? WHERE id = ? RETURNING id",
            poll_interval,
            client_id
        )
        .fetch_optional(self.pool)
        .await?;
        Ok(rows.is_some())
    }

    /// Delete a client by id. Returns false if not found.
    pub async fn delete(&self, client_id: &str) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM client WHERE id = ?", client_id)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get single client
    pub async fn get_client_by_id(&self, client_id: &str) -> Result<Option<ClientDto>> {
        let client = sqlx::query!(
            r#"
            SELECT
                c.id,
                c.host_name,
                c.min_poll_interval_in_ms,
                c.version
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
                version: r.version,
            })),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
#[allow(unused_imports, dead_code)]
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
