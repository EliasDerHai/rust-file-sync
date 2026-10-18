use shared::dtos::ClientDto;
use shared::endpoint::WEB_CLIENT_ID;
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
    pub async fn get_all_clients(&self) -> Result<Vec<ClientDto>> {
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
            .filter(|r| r.id != WEB_CLIENT_ID)
            .map(|r| ClientDto {
                id: r.id,
                host_name: r.host_name,
                min_poll_interval_in_ms: u16::try_from(r.min_poll_interval_in_ms)
                    .expect("should fit"),
                version: r.version,
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
    use crate::db::test_support::setup_test_db;

    #[tokio::test]
    async fn pwa_client_is_removed_and_web_client_kept() {
        let (pool, _db) = setup_test_db().await;

        let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM client")
            .fetch_all(&pool)
            .await
            .unwrap();

        assert!(
            !ids.iter()
                .any(|id| id == "f4a7b3c2-8d5e-4f6a-9b2c-1e3d5f7a9b0c")
        );
        assert!(ids.iter().any(|id| id == WEB_CLIENT_ID));
    }
}
