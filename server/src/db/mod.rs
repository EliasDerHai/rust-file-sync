mod config_repository;
mod link_repository;

pub use config_repository::{ClientConfigRepository, ClientWithConfig};
pub use link_repository::SharedLinkRepository;

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ServerDatabase {
    pool: SqlitePool,
}

impl ServerDatabase {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn client_config(&self) -> ClientConfigRepository<'_> {
        ClientConfigRepository::new(&self.pool)
    }

    pub fn shared_link(&self) -> SharedLinkRepository<'_> {
        SharedLinkRepository::new(&self.pool)
    }

    #[cfg(test)]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
