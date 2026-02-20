use crate::csv_migration::migrate_csv_history_to_db;
use crate::db::ServerDatabase;
use crate::file_history::InMemoryFileHistory;
use crate::write::{
    RotatingFileWriter, create_all_paths_if_not_exist, create_file_if_not_exists,
    schedule_data_backups,
};
use axum::extract::{DefaultBodyLimit, State};
use axum::routing::{post, put};
use axum::{Router, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use shared::endpoint::ServerEndpoint;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteConnectOptions;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod client_file_event;
mod csv_migration;
mod db;
mod file_event;
mod file_history;
mod handler;
mod monitor;
mod multipart;
mod write;

/// base directory for files synced from clients (subdirs per watch group: upload/{wg_id}/)
pub(crate) static UPLOAD_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/upload"));
/// directory to hold zipped backup files
static BACKUP_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/backup"));
/// path to legacy CSV history file (used only for one-time migration)
static HISTORY_CSV_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/history.csv"));
static MONITORING_DIR: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/monitor"));
/// dir to which multipart-files can be saved to, before being moved to the actual 'mirrored path'
/// temporary and might be cleaned upon encountering errors or on scheduled intervals
pub(crate) static UPLOAD_TMP_PATH: LazyLock<&Path> =
    LazyLock::new(|| Path::new("./data/upload_in_progress"));
/// sqlite file
static DB_FILE_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/sqlite.db"));
/// migrations
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub(crate) struct AppState {
    history: Arc<InMemoryFileHistory>,
    monitor_writer: Arc<Mutex<RotatingFileWriter>>,
    db: ServerDatabase,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log_level = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(log_level).init();

    tokio::spawn(async {
        create_all_paths_if_not_exist(vec![
            UPLOAD_PATH.iter().as_path(),
            UPLOAD_TMP_PATH.iter().as_path(),
            BACKUP_PATH.iter().as_path(),
        ])?;
        create_file_if_not_exists(*DB_FILE_PATH)?;
        Ok::<(), std::io::Error>(())
    });
    tokio::spawn(schedule_data_backups(&UPLOAD_PATH, &BACKUP_PATH));

    let db = {
        let opts = SqliteConnectOptions::new()
            .filename(*DB_FILE_PATH)
            .create_if_missing(true)
            .pragma("foreign_keys", "ON");
        let pool = SqlitePool::connect_with(opts).await?;
        MIGRATOR.run(&pool).await?;
        ServerDatabase::new(pool)
    };

    // Migrate CSV history to DB (one-time)
    migrate_csv_history_to_db(&db).await;

    // Load history from DB into in-memory store
    let history = match db.file_event().get_all_events().await {
        Ok(events) => {
            info!("Loaded {} file events from database", events.len());
            InMemoryFileHistory::from(events)
        }
        Err(err) => {
            error!("Failed to load file events from database: {}", err);
            InMemoryFileHistory::from(Vec::new())
        }
    };

    // Create rotating file writer for monitoring (4 files, 5MB each)
    let monitor_writer = RotatingFileWriter::new(
        MONITORING_DIR.to_path_buf(),
        "monitor".to_string(),
        5 * 1024 * 1024, // 5MB
        4,
        Some(
            "Timestamp;Total used mem in %;App used mem in %;Total used cpu in %;App used cpu in %"
                .to_string(),
        ),
    )
    .unwrap_or_else(|err| {
        panic!("Failed to create monitor writer: {}", err);
    });
    let monitor_writer = Arc::new(Mutex::new(monitor_writer));

    tokio::spawn(monitor::monitor_sys(monitor_writer.clone()));

    let state = AppState {
        history: Arc::new(history),
        monitor_writer,
        db,
    };

    let app = Router::new()
        .route(ServerEndpoint::Hello.to_str(), get(|| async { "hello" }))
        .route(ServerEndpoint::Ping.to_str(), get(|| async { "pong" }))
        .route(
            ServerEndpoint::Scan.to_str(),
            get(|| handler::scan_disk(&UPLOAD_PATH)),
        )
        .route(
            ServerEndpoint::Upload.to_str(),
            post(handler::upload_handler).layer(DefaultBodyLimit::max(
                10 * 1024 * 1024 * 1024, /* 10gb */
            )),
        )
        .route(ServerEndpoint::Sync.to_str(), post(handler::sync_handler))
        .route(ServerEndpoint::Download.to_str(), get(handler::download))
        .route(ServerEndpoint::Delete.to_str(), post(handler::delete))
        .route(
            ServerEndpoint::Version.to_str(),
            get(|| async { env!("CARGO_PKG_VERSION") }),
        )
        .route(ServerEndpoint::Config.to_str(), get(handler::get_config))
        // json api - for frontends
        .route(
            ServerEndpoint::ApiConfigs.to_str(),
            get(handler::api_list_configs),
        )
        .route(
            ServerEndpoint::ApiConfig.to_str(),
            get(handler::api_get_config).put(handler::api_update_config),
        )
        .route(
            ServerEndpoint::ApiWatchGroups.to_str(),
            get(handler::api_list_watch_groups).post(handler::api_create_watch_group),
        )
        .route(
            ServerEndpoint::ApiWatchGroup.to_str(),
            put(handler::api_update_watch_group),
        )
        .route(
            ServerEndpoint::ApiMonitor.to_str(),
            get(|state: State<AppState>| monitor::api_get_monitoring(state.monitor_writer.clone())),
        )
        .route(
            ServerEndpoint::ApiLinks.to_str(),
            get(handler::get_links).post(handler::post_link),
        )
        // apps
        .nest_service(
            ServerEndpoint::ServePWA.to_str(),
            get(handler::serve_embedded_pwa),
        )
        .nest_service(
            ServerEndpoint::App.to_str(),
            get(handler::serve_embedded_app),
        )
        // .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    match (env::var("TLS_CERT_PATH"), env::var("TLS_KEY_PATH")) {
        (Ok(cert_path), Ok(key_path)) => {
            tracing::info!("Starting HTTPS server on {addr}");
            let tls_config = RustlsConfig::from_pem_file(&cert_path, &key_path)
                .await
                .expect("Failed to load TLS certificate/key");
            axum_server::bind_rustls(addr, tls_config)
                .serve(app.into_make_service())
                .await
                .unwrap();
        }
        _ => {
            tracing::info!("Starting HTTP server on {addr} (no TLS_CERT_PATH/TLS_KEY_PATH)");
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        }
    }

    Ok(())
}
