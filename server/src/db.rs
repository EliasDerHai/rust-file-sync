use shared::register::RegisterClientRequest;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ServerDatabase {
    pool: SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl ServerDatabase {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Register or update a client and its watch configuration
    pub async fn register_client(
        &self,
        client_id: &str,
        host_name: &str,
        request: RegisterClientRequest,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::migrate::Migrator;
    use sqlx::sqlite::SqlitePoolOptions;

    static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

    async fn setup_test_db() -> ServerDatabase {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        MIGRATOR.run(&pool).await.expect("Failed to run migrations");

        ServerDatabase::new(pool)
    }

    #[tokio::test]
    async fn test_register_client_insert() {
        let db = setup_test_db().await;

        let request = RegisterClientRequest {
            path_to_monitor: "/home/test/sync".to_string(),
            exclude_dirs: vec![".git".to_string(), "node_modules".to_string()],
            exclude_dot_dirs: true,
            min_poll_interval_in_ms: 5000,
        };

        db.register_client("client-uuid-123", "test-host", request)
            .await
            .expect("Failed to register client");

        // Verify client was inserted
        let client = sqlx::query!("SELECT * FROM client WHERE id = ?", "client-uuid-123")
            .fetch_one(&db.pool)
            .await
            .expect("Failed to fetch client");

        assert_eq!(client.host_name, "test-host");
        assert_eq!(client.min_poll_interval_in_ms, 5000);

        // Verify watch group was inserted
        let watch_group = sqlx::query!(
            "SELECT * FROM client_watch_group WHERE client_id = ?",
            "client-uuid-123"
        )
        .fetch_one(&db.pool)
        .await
        .expect("Failed to fetch watch group");

        assert_eq!(watch_group.path_to_monitor, "/home/test/sync");

        // Verify excluded dirs were inserted
        let excluded_dirs: Vec<String> = sqlx::query_scalar!(
            "SELECT exclude_dir FROM client_watch_group_excluded_dir WHERE client_watch_group = ?",
            watch_group.id
        )
        .fetch_all(&db.pool)
        .await
        .expect("Failed to fetch excluded dirs");

        assert_eq!(excluded_dirs, vec![".git", "node_modules"]);
    }

    #[tokio::test]
    async fn test_register_client_upsert_overwrites_existing() {
        let db = setup_test_db().await;

        // First registration
        let request1 = RegisterClientRequest {
            path_to_monitor: "/home/test/old-path".to_string(),
            exclude_dirs: vec![".git".to_string()],
            exclude_dot_dirs: true,
            min_poll_interval_in_ms: 3000,
        };

        db.register_client("client-uuid-456", "old-hostname", request1)
            .await
            .expect("Failed to register client");

        // Second registration with same client_id but different data
        let request2 = RegisterClientRequest {
            path_to_monitor: "/home/test/new-path".to_string(),
            exclude_dirs: vec!["target".to_string(), "dist".to_string()],
            exclude_dot_dirs: false,
            min_poll_interval_in_ms: 10000,
        };

        db.register_client("client-uuid-456", "new-hostname", request2)
            .await
            .expect("Failed to upsert client");

        // Verify client was updated (not duplicated)
        let clients: Vec<_> = sqlx::query!("SELECT * FROM client WHERE id = ?", "client-uuid-456")
            .fetch_all(&db.pool)
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
        .fetch_all(&db.pool)
        .await
        .expect("Failed to fetch watch groups");

        assert_eq!(watch_groups.len(), 1);
        assert_eq!(watch_groups[0].path_to_monitor, "/home/test/new-path");

        // Verify excluded dirs were replaced
        let excluded_dirs: Vec<String> = sqlx::query_scalar!(
            "SELECT exclude_dir FROM client_watch_group_excluded_dir WHERE client_watch_group = ?",
            watch_groups[0].id
        )
        .fetch_all(&db.pool)
        .await
        .expect("Failed to fetch excluded dirs");

        assert_eq!(excluded_dirs.len(), 2);
        assert!(excluded_dirs.contains(&"target".to_string()));
        assert!(excluded_dirs.contains(&"dist".to_string()));
        assert!(!excluded_dirs.contains(&".git".to_string())); // Old value should be gone
    }
}
