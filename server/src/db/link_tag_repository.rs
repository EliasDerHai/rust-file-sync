use sqlx::SqlitePool;

pub struct LinkTagRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> LinkTagRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert_link_tag(&self, tag: &str, link_url: &str) -> Result<()> {
        sqlx::query!(
            "INSERT INTO link_tag (name, link_url) VALUES (?, ?)",
            tag,
            link_url
        )
        .execute(self.pool)
        .await?;
        Ok(())
    }
}
