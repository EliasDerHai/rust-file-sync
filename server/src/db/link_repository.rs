use sqlx::SqlitePool;

pub struct SharedLinkRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> SharedLinkRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Store a shared link from the PWA
    pub async fn store_shared_link(&self, url: &str, title: Option<&str>) -> Result<()> {
        sqlx::query!("INSERT INTO link (url, title) VALUES (?, ?)", url, title)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
