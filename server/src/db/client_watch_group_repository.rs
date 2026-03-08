use shared::dtos::{ClientWatchGroupDto, WatchGroupConfigDto};
use sqlx::SqlitePool;
use std::collections::HashMap;

pub struct ClientWatchGroupRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> ClientWatchGroupRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Get all watch groups for a client, keyed by server_watch_group_id.
    /// Returns an empty map if the client has no assigned watch groups.
    pub async fn get_for_client(
        &self,
        client_id: &str,
    ) -> Result<HashMap<i64, WatchGroupConfigDto>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                cwg.server_watch_group_id,
                cwg.path_to_monitor,
                cwg.exclude_dot_dirs as "exclude_dot_dirs: bool",
                swg.name,
                cwged.exclude_dir as "exclude_dir?"
            FROM client_watch_group cwg
            INNER JOIN server_watch_group swg ON swg.id = cwg.server_watch_group_id
            LEFT JOIN client_watch_group_excluded_dir cwged ON cwged.client_watch_group = cwg.id
            WHERE cwg.client_id = ?
            "#,
            client_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut map: HashMap<i64, WatchGroupConfigDto> = HashMap::new();
        for row in rows {
            let entry =
                map.entry(row.server_watch_group_id)
                    .or_insert_with(|| WatchGroupConfigDto {
                        path_to_monitor: row.path_to_monitor.clone(),
                        exclude_dirs: Vec::new(),
                        exclude_dot_dirs: row.exclude_dot_dirs,
                        name: row.name.clone(),
                    });
            if let Some(dir) = row.exclude_dir {
                entry.exclude_dirs.push(dir);
            }
        }

        Ok(map)
    }

    /// List all watch group assignments for a client (for the admin API).
    /// Returns a Vec ordered by server_watch_group_id.
    pub async fn list_for_client(&self, client_id: &str) -> Result<Vec<ClientWatchGroupDto>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                cwg.server_watch_group_id,
                cwg.path_to_monitor,
                cwg.exclude_dot_dirs as "exclude_dot_dirs: bool",
                swg.name,
                cwged.exclude_dir as "exclude_dir?"
            FROM client_watch_group cwg
            INNER JOIN server_watch_group swg ON swg.id = cwg.server_watch_group_id
            LEFT JOIN client_watch_group_excluded_dir cwged ON cwged.client_watch_group = cwg.id
            WHERE cwg.client_id = ?
            ORDER BY cwg.server_watch_group_id
            "#,
            client_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut map: std::collections::BTreeMap<i64, ClientWatchGroupDto> =
            std::collections::BTreeMap::new();
        for row in rows {
            let entry = map
                .entry(row.server_watch_group_id)
                .or_insert_with(|| ClientWatchGroupDto {
                    server_watch_group_id: row.server_watch_group_id,
                    server_watch_group_name: row.name.clone(),
                    path_to_monitor: row.path_to_monitor.clone(),
                    exclude_dirs: Vec::new(),
                    exclude_dot_dirs: row.exclude_dot_dirs,
                });
            if let Some(dir) = row.exclude_dir {
                entry.exclude_dirs.push(dir);
            }
        }

        Ok(map.into_values().collect())
    }

    /// Create a new client watch group assignment.
    pub async fn create(
        &self,
        client_id: &str,
        server_watch_group_id: i64,
        path_to_monitor: &str,
        exclude_dirs: Vec<String>,
        exclude_dot_dirs: bool,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let watch_group_id = sqlx::query_scalar!(
            r#"
            INSERT INTO client_watch_group (client_id, server_watch_group_id, path_to_monitor, exclude_dot_dirs)
            VALUES (?, ?, ?, ?)
            RETURNING id
            "#,
            client_id,
            server_watch_group_id,
            path_to_monitor,
            exclude_dot_dirs
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
        Ok(())
    }

    /// Delete a client watch group assignment. Returns false if not found.
    pub async fn delete(&self, client_id: &str, server_watch_group_id: i64) -> Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM client_watch_group WHERE client_id = ? AND server_watch_group_id = ?",
            client_id,
            server_watch_group_id
        )
        .execute(self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update a single watch group for a client.
    /// Returns true if the row was found and updated, false otherwise.
    pub async fn update(
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
        .fetch_optional(&mut *tx)
        .await?;

        let Some(watch_group_id) = watch_group_id else {
            return Ok(false);
        };

        sqlx::query!(
            "DELETE FROM client_watch_group_excluded_dir WHERE client_watch_group = ?",
            watch_group_id
        )
        .execute(&mut *tx)
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
