mod client_repository;
mod link_repository;
mod server_repository;

pub use client_repository::{ClientRepository, ClientWithConfig};
pub use link_repository::SharedLinkRepository;
pub use server_repository::{ServerRepository, ServerWatchGroup};

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ServerDatabase {
    pool: SqlitePool,
}

impl ServerDatabase {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn server(&self) -> ServerRepository<'_> {
        ServerRepository::new(&self.pool)
    }

    pub fn client(&self) -> ClientRepository<'_> {
        ClientRepository::new(&self.pool)
    }

    pub fn link(&self) -> SharedLinkRepository<'_> {
        SharedLinkRepository::new(&self.pool)
    }
}
