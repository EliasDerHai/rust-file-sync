use sqlx::SqlitePool;

use crate::handler::share_link::ShareLinkRequest;

pub struct SharedLinkRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> SharedLinkRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Store a shared link from the PWA
    pub async fn insert_link(&self, url: &str, title: Option<&str>) -> Result<()> {
        if sqlx::query_scalar!("SELECT count(1) FROM link WHERE url = ?", url)
            .fetch_one(self.pool)
            .await?
            > 0
        {
            return Ok(());
        };

        sqlx::query!("INSERT INTO link (url, title) VALUES (?, ?)", url, title)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_links(&self) -> Result<Vec<ShareLinkRequest>> {
        sqlx::query!("SELECT url, title FROM link")
            .fetch_all(self.pool)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| ShareLinkRequest {
                        url: row.url,
                        title: row.title,
                    })
                    .collect()
            })
    }
}
