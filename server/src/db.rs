use sqlx::SqlitePool;

type Result<T> = sqlx::Result<T>;

#[derive(Clone)]
pub struct ServerDatabase {
    pool: SqlitePool,
}

impl ServerDatabase {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert_client(&self) -> Result<()> {
        Ok(())
    }
}
