use sqlx::SqlitePool;

pub struct LinkTagRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> LinkTagRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Store a shared link from the PWA
    pub async fn insert_link_tag(&self, tag: &str, link_id: i32) -> Result<()> {
        if sqlx::query_scalar!(
            "SELECT count(1) FROM link_tag WHERE name = ? AND link_id = ?",
            tag,
            link_id
        )
        .fetch_one(self.pool)
        .await?
            > 0
        {
            return Ok(());
        };

        sqlx::query!(
            "INSERT INTO link_tag (name, link_id) VALUES (?, ?)",
            tag,
            link_id
        )
        .execute(self.pool)
        .await?;
        Ok(())
    }
}
