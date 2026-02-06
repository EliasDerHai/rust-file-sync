use serde::Serialize;
use shared::register::ClientConfigDto;
use sqlx::SqlitePool;

/// Full client info including ID and hostname (for admin UI)
#[derive(Debug, Clone, Serialize)]
pub struct ClientWithConfig {
    pub id: String,
    pub host_name: String,
    pub path_to_monitor: String,
    pub exclude_dirs: Vec<String>,
    pub exclude_dot_dirs: bool,
    pub min_poll_interval_in_ms: u16,
    pub server_watch_group_id: i64,
    pub server_watch_group_name: String,
}

pub struct ClientRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> ClientRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Register or update a client and its watch configuration
    pub async fn upsert_client_config(
        &self,
        client_id: &str,
        host_name: &str,
        request: ClientConfigDto,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let poll_interval = request.min_poll_interval_in_ms as i32;

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
            poll_interval
        )
        .execute(&mut *tx)
        .await?;

        // Delete existing watch groups for this client (cascade deletes excluded_dirs)
        sqlx::query!(
            "DELETE FROM client_watch_group WHERE client_id = ?",
            client_id
        )
        .execute(&mut *tx)
        .await?;

        // Insert new watch group
        let watch_group_id = sqlx::query_scalar!(
            r#"
            INSERT INTO client_watch_group (client_id, path_to_monitor, exclude_dot_dirs)
            VALUES (?, ?, ?)
            RETURNING id
            "#,
            client_id,
            request.path_to_monitor,
            request.exclude_dot_dirs
        )
        .fetch_one(&mut *tx)
        .await?;

        // Insert excluded dirs
        for exclude_dir in request.exclude_dirs {
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
        Ok(())
    }

    /// Get client config by client_id
    /// Returns None if client doesn't exist
    pub async fn get_client_config(&self, client_id: &str) -> Result<Option<ClientConfigDto>> {
        // Get client and watch group
        let watch_group = sqlx::query!(
            r#"
            SELECT
                c.min_poll_interval_in_ms,
                wg.id as watch_group_id,
                wg.path_to_monitor,
                wg.exclude_dot_dirs
            FROM client c
            JOIN client_watch_group wg ON wg.client_id = c.id
            WHERE c.id = ?
            "#,
            client_id
        )
        .fetch_optional(self.pool)
        .await?;

        let Some(wg) = watch_group else {
            return Ok(None);
        };

        // Get excluded dirs
        let exclude_dirs: Vec<String> = sqlx::query_scalar!(
            "SELECT exclude_dir FROM client_watch_group_excluded_dir WHERE client_watch_group = ?",
            wg.watch_group_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(Some(ClientConfigDto {
            path_to_monitor: wg.path_to_monitor,
            exclude_dirs,
            exclude_dot_dirs: wg.exclude_dot_dirs.unwrap_or(true),
            min_poll_interval_in_ms: wg.min_poll_interval_in_ms as u16,
        }))
    }

    /// Get all clients with their configs (for admin UI)
    pub async fn get_all_clients(&self) -> Result<Vec<ClientWithConfig>> {
        let clients = sqlx::query!(
            r#"
            SELECT
                c.id as "id!",
                c.host_name as "host_name!",
                c.min_poll_interval_in_ms as "min_poll_interval_in_ms!",
                wg.id as watch_group_id,
                wg.path_to_monitor as "path_to_monitor?",
                wg.exclude_dot_dirs as "exclude_dot_dirs?",
                wg.server_watch_group_id as "server_watch_group_id?",
                swg.name as "server_watch_group_name?"
            FROM client c
            LEFT JOIN client_watch_group wg ON wg.client_id = c.id
            LEFT JOIN server_watch_group swg ON swg.id = wg.server_watch_group_id
            ORDER BY c.host_name
            "#
        )
        .fetch_all(self.pool)
        .await?;

        let mut result = Vec::new();
        for client in clients {
            let exclude_dirs: Vec<String> = if let Some(wg_id) = client.watch_group_id {
                sqlx::query_scalar!(
                    "SELECT exclude_dir FROM client_watch_group_excluded_dir WHERE client_watch_group = ?",
                    wg_id
                )
                .fetch_all(self.pool)
                .await?
            } else {
                Vec::new()
            };

            result.push(ClientWithConfig {
                id: client.id,
                host_name: client.host_name,
                path_to_monitor: client.path_to_monitor.unwrap_or_default(),
                exclude_dirs,
                exclude_dot_dirs: client.exclude_dot_dirs.unwrap_or(true),
                min_poll_interval_in_ms: client.min_poll_interval_in_ms as u16,
                server_watch_group_id: client.server_watch_group_id.unwrap_or(1),
                server_watch_group_name: client.server_watch_group_name.unwrap_or_default(),
            });
        }

        Ok(result)
    }

    /// Get single client with config by ID (for admin UI edit page)
    pub async fn get_client_by_id(&self, client_id: &str) -> Result<Option<ClientWithConfig>> {
        let client = sqlx::query!(
            r#"
            SELECT
                c.id as "id!",
                c.host_name as "host_name!",
                c.min_poll_interval_in_ms as "min_poll_interval_in_ms!",
                wg.id as watch_group_id,
                wg.path_to_monitor as "path_to_monitor?",
                wg.exclude_dot_dirs as "exclude_dot_dirs?",
                wg.server_watch_group_id as "server_watch_group_id?",
                swg.name as "server_watch_group_name?"
            FROM client c
            LEFT JOIN client_watch_group wg ON wg.client_id = c.id
            LEFT JOIN server_watch_group swg ON swg.id = wg.server_watch_group_id
            WHERE c.id = ?
            "#,
            client_id
        )
        .fetch_optional(self.pool)
        .await?;

        let Some(client) = client else {
            return Ok(None);
        };

        let exclude_dirs: Vec<String> = if let Some(wg_id) = client.watch_group_id {
            sqlx::query_scalar!(
                "SELECT exclude_dir FROM client_watch_group_excluded_dir WHERE client_watch_group = ?",
                wg_id
            )
            .fetch_all(self.pool)
            .await?
        } else {
            Vec::new()
        };

        Ok(Some(ClientWithConfig {
            id: client.id,
            host_name: client.host_name,
            path_to_monitor: client.path_to_monitor.unwrap_or_default(),
            exclude_dirs,
            exclude_dot_dirs: client.exclude_dot_dirs.unwrap_or(true),
            min_poll_interval_in_ms: client.min_poll_interval_in_ms as u16,
            server_watch_group_id: client.server_watch_group_id.unwrap_or(1),
            server_watch_group_name: client.server_watch_group_name.unwrap_or_default(),
        }))
    }

    /// Update client config by ID (for admin UI)
    pub async fn update_client_config(
        &self,
        client_id: &str,
        config: ClientConfigDto,
        server_watch_group_id: i64,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let poll_interval = config.min_poll_interval_in_ms as i32;

        // Update client poll interval
        let result = sqlx::query!(
            "UPDATE client SET min_poll_interval_in_ms = ? WHERE id = ?",
            poll_interval,
            client_id
        )
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(false);
        }

        // Delete existing watch groups
        sqlx::query!(
            "DELETE FROM client_watch_group WHERE client_id = ?",
            client_id
        )
        .execute(&mut *tx)
        .await?;

        // Insert new watch group
        let watch_group_id = sqlx::query_scalar!(
            r#"
            INSERT INTO client_watch_group (client_id, path_to_monitor, exclude_dot_dirs, server_watch_group_id)
            VALUES (?, ?, ?, ?)
            RETURNING id
            "#,
            client_id,
            config.path_to_monitor,
            config.exclude_dot_dirs,
            server_watch_group_id
        )
        .fetch_one(&mut *tx)
        .await?;

        // Insert excluded dirs
        for exclude_dir in config.exclude_dirs {
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

    #[tokio::test]
    async fn test_register_client_insert() {
        let (pool, db) = setup_test_db().await;
        let repo = db.client();

        let request = ClientConfigDto {
            path_to_monitor: "/home/test/sync".to_string(),
            exclude_dirs: vec![".git".to_string(), "node_modules".to_string()],
            exclude_dot_dirs: true,
            min_poll_interval_in_ms: 5000,
        };

        repo.upsert_client_config("client-uuid-123", "test-host", request)
            .await
            .expect("Failed to register client");

        // Verify client was inserted
        let client = sqlx::query!("SELECT * FROM client WHERE id = ?", "client-uuid-123")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch client");

        assert_eq!(client.host_name, "test-host");
        assert_eq!(client.min_poll_interval_in_ms, 5000);

        // Verify watch group was inserted
        let watch_group = sqlx::query!(
            "SELECT * FROM client_watch_group WHERE client_id = ?",
            "client-uuid-123"
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to fetch watch group");

        assert_eq!(watch_group.path_to_monitor, "/home/test/sync");

        // Verify excluded dirs were inserted
        let excluded_dirs: Vec<String> = sqlx::query_scalar!(
            "SELECT exclude_dir FROM client_watch_group_excluded_dir WHERE client_watch_group = ?",
            watch_group.id
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch excluded dirs");

        assert_eq!(excluded_dirs, vec![".git", "node_modules"]);
    }

    #[tokio::test]
    async fn test_register_client_upsert_overwrites_existing() {
        let (pool, db) = setup_test_db().await;
        let repo = db.client();

        // First registration
        let request1 = ClientConfigDto {
            path_to_monitor: "/home/test/old-path".to_string(),
            exclude_dirs: vec![".git".to_string()],
            exclude_dot_dirs: true,
            min_poll_interval_in_ms: 3000,
        };

        repo.upsert_client_config("client-uuid-456", "old-hostname", request1)
            .await
            .expect("Failed to register client");

        // Second registration with same client_id but different data
        let request2 = ClientConfigDto {
            path_to_monitor: "/home/test/new-path".to_string(),
            exclude_dirs: vec!["target".to_string(), "dist".to_string()],
            exclude_dot_dirs: false,
            min_poll_interval_in_ms: 10000,
        };

        repo.upsert_client_config("client-uuid-456", "new-hostname", request2)
            .await
            .expect("Failed to upsert client");

        // Verify client was updated (not duplicated)
        let clients: Vec<_> = sqlx::query!("SELECT * FROM client WHERE id = ?", "client-uuid-456")
            .fetch_all(&pool)
            .await
            .expect("Failed to fetch clients");

        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].host_name, "new-hostname");
        assert_eq!(clients[0].min_poll_interval_in_ms, 10000);

        // Verify watch group was replaced
        let watch_groups: Vec<_> = sqlx::query!(
            "SELECT * FROM client_watch_group WHERE client_id = ?",
            "client-uuid-456"
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch watch groups");

        assert_eq!(watch_groups.len(), 1);
        assert_eq!(watch_groups[0].path_to_monitor, "/home/test/new-path");

        // Verify excluded dirs were replaced
        let excluded_dirs: Vec<String> = sqlx::query_scalar!(
            "SELECT exclude_dir FROM client_watch_group_excluded_dir WHERE client_watch_group = ?",
            watch_groups[0].id
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch excluded dirs");

        assert_eq!(excluded_dirs.len(), 2);
        assert!(excluded_dirs.contains(&"target".to_string()));
        assert!(excluded_dirs.contains(&"dist".to_string()));
        assert!(!excluded_dirs.contains(&".git".to_string())); // Old value should be gone
    }

    #[tokio::test]
    async fn test_get_client_config_returns_none_for_unknown_client() {
        let (_, db) = setup_test_db().await;
        let repo = db.client();

        let config = repo
            .get_client_config("nonexistent-client")
            .await
            .expect("Query should succeed");

        assert!(config.is_none());
    }

    #[tokio::test]
    async fn test_get_client_config_returns_registered_config() {
        let (_, db) = setup_test_db().await;
        let repo = db.client();

        // Register a client
        let request = ClientConfigDto {
            path_to_monitor: "/home/user/documents".to_string(),
            exclude_dirs: vec!["node_modules".to_string(), ".cache".to_string()],
            exclude_dot_dirs: false,
            min_poll_interval_in_ms: 7500,
        };

        repo.upsert_client_config("test-client-789", "my-laptop", request)
            .await
            .expect("Failed to register client");

        // Fetch the config
        let config = repo
            .get_client_config("test-client-789")
            .await
            .expect("Query should succeed")
            .expect("Config should exist");

        assert_eq!(config.path_to_monitor, "/home/user/documents");
        assert_eq!(config.exclude_dirs, vec!["node_modules", ".cache"]);
        assert!(!config.exclude_dot_dirs);
        assert_eq!(config.min_poll_interval_in_ms, 7500);
    }
}
