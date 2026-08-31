use crate::db::ServerDatabase;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::path::Path;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Spins up a fully-migrated, in-memory SQLite database for tests. Pinned to a
/// single pooled connection - SQLite's `:memory:` databases are otherwise
/// per-connection, so a pool handing out a second connection to a concurrent
/// query would see an empty, unmigrated database instead of the one just set up.
///
/// NOT suitable for anything exercising `ServerDatabase::vacuum_into`: SQLite's
/// `VACUUM INTO` reports success against an in-memory source database but writes
/// no file at all (verified empirically, not just a bind-parameter quirk - same
/// result with literal SQL). Use `setup_test_db_file` for that.
pub(crate) async fn setup_test_db() -> (Pool<Sqlite>, ServerDatabase) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    MIGRATOR.run(&pool).await.expect("Failed to run migrations");

    (pool.clone(), ServerDatabase::new(pool))
}

/// Same as `setup_test_db`, but backed by a real file at `path` rather than
/// `:memory:`. Required for anything exercising `ServerDatabase::vacuum_into`
/// (see note above) since that needs a real on-disk source database.
pub(crate) async fn setup_test_db_file(path: &Path) -> ServerDatabase {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("Failed to create file-backed database");

    MIGRATOR.run(&pool).await.expect("Failed to run migrations");

    ServerDatabase::new(pool)
}
