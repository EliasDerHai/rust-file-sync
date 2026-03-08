use shared::dtos::ServerWatchGroup;
use sqlx::SqlitePool;

pub struct ServerWatchGroupRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> ServerWatchGroupRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_all_watch_groups(&self) -> Result<Vec<ServerWatchGroup>> {
        sqlx::query_as!(
            ServerWatchGroup,
            "SELECT id, name FROM server_watch_group ORDER BY id"
        )
        .fetch_all(self.pool)
        .await
    }

    pub async fn insert_watch_group(&self, name: String) -> Result<()> {
        sqlx::query!("INSERT INTO server_watch_group (name) VALUES (?)", name)
            .execute(self.pool)
            .await?;

        Ok(())
    }

    pub async fn rename_watch_group(&self, id: i64, name: String) -> Result<()> {
        sqlx::query!(
            "UPDATE server_watch_group SET name = ? WHERE id = ?",
            name,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Delete a server watch group. Returns false if not found.
    pub async fn delete(&self, id: i64) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM server_watch_group WHERE id = ?", id)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
