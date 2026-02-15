use crate::db::ServerDatabase;
use crate::file_history::InMemoryFileHistory;
use crate::write::{
    RotatingFileWriter, create_all_paths_if_not_exist, create_csv_file_if_not_exists,
    create_file_if_not_exists, schedule_data_backups,
};
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::HeaderMap;
use axum::routing::{post, put};
use axum::{Router, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use shared::endpoint::ServerEndpoint;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteConnectOptions;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use std::{path::Path, sync::LazyLock};
use tracing::error;
use tracing_subscriber::EnvFilter;

mod client_file_event;
mod db;
mod file_history;
mod handler;
mod monitor;
mod multipart;
mod write;

/// base directory for files synced from clients
static UPLOAD_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/upload"));
/// directory to hold zipped backup files
static BACKUP_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/backup"));
/// file to persist the in-mem state ([`InMemoryFileHistory`])
static HISTORY_CSV_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/history.csv"));
static MONITORING_DIR: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/monitor"));
/// dir to which multipart-files can be saved to, before being moved to the actual 'mirrored path'
/// temporary and might be cleaned upon encountering errors or on scheduled intervals
static UPLOAD_TMP_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/upload_in_progress"));
/// sqlite file
static DB_FILE_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/sqlite.db"));
/// migrations
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
struct AppState {
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
        create_csv_file_if_not_exists(
            HISTORY_CSV_PATH.iter().as_path(),
            Some(vec![
                "id".to_string(),
                "utc_millis".to_string(),
                "relative_path".to_string(),
                "size_in_bytes".to_string(),
                "event_type".to_string(),
            ]),
        )?;
        Ok::<(), std::io::Error>(())
    });
    tokio::spawn(schedule_data_backups(&UPLOAD_PATH, &BACKUP_PATH));

    let db = {
        let opts = SqliteConnectOptions::new()
            .filename(*DB_FILE_PATH)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(opts).await?;
        MIGRATOR.run(&pool).await?;
        ServerDatabase::new(pool)
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

    let history =
        InMemoryFileHistory::try_from(HISTORY_CSV_PATH.iter().as_path()).unwrap_or_else(|err| {
            error!("Failed to load history: {}", err);
            InMemoryFileHistory::from(Vec::new())
        });

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
            post(
                |state: State<AppState>, headers: HeaderMap, multipart: Multipart| {
                    handler::upload_handler(
                        &UPLOAD_PATH,
                        &UPLOAD_TMP_PATH,
                        &HISTORY_CSV_PATH,
                        state,
                        multipart,
                        headers,
                    )
                },
            )
            .layer(DefaultBodyLimit::max(
                10 * 1024 * 1024 * 1024, /* 10gb */
            )),
        )
        .route(ServerEndpoint::Sync.to_str(), post(handler::sync_handler))
        .route(
            ServerEndpoint::Download.to_str(),
            get(|payload: String| handler::download(&UPLOAD_PATH, payload)),
        )
        .route(
            ServerEndpoint::Delete.to_str(),
            post(
                |state: State<AppState>, headers: HeaderMap, payload: String| {
                    handler::delete(&UPLOAD_PATH, &HISTORY_CSV_PATH, payload, state, headers)
                },
            ),
        )
        .route(
            ServerEndpoint::Version.to_str(),
            get(|| async { env!("CARGO_PKG_VERSION") }),
        )
        .route(
            ServerEndpoint::Config.to_str(),
            get(handler::get_config).post(handler::post_config),
        )
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
