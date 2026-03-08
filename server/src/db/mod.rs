mod client_repository;
mod client_watch_group_repository;
mod file_event_repository;
mod link_repository;
mod server_watch_group_repository;

pub use client_repository::ClientRepository;
pub use client_watch_group_repository::ClientWatchGroupRepository;
pub use file_event_repository::FileEventRepository;
pub use link_repository::SharedLinkRepository;
pub use server_watch_group_repository::ServerWatchGroupRepository;

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ServerDatabase {
    pool: SqlitePool,
}

impl ServerDatabase {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn server_watch_group(&self) -> ServerWatchGroupRepository<'_> {
        ServerWatchGroupRepository::new(&self.pool)
    }

    pub fn client(&self) -> ClientRepository<'_> {
        ClientRepository::new(&self.pool)
    }

    pub fn client_watch_group(&self) -> ClientWatchGroupRepository<'_> {
        ClientWatchGroupRepository::new(&self.pool)
    }

    pub fn link(&self) -> SharedLinkRepository<'_> {
        SharedLinkRepository::new(&self.pool)
    }

    pub fn file_event(&self) -> FileEventRepository<'_> {
        FileEventRepository::new(&self.pool)
    }
}
