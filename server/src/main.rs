use crate::db::ServerDatabase;
use crate::sse::SseRegistry;
use crate::file_history::InMemoryFileHistory;
use crate::logs::LogBuffer;
use crate::write::{
    RotatingFileWriter, create_all_paths_if_not_exist, create_file_if_not_exists,
    schedule_data_backups,
};
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::routing::{post, put};

const UPLOAD_LIMIT_BYTES: usize = 500 * 1024 * 1024; // 500 MB
use axum::{Router, routing::get};
use shared::dtos::{LogLineDto, ServerEventDto};
use shared::endpoint::ServerEndpoint;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteConnectOptions;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod client_file_event;
mod db;
mod file_event;
mod file_history;
mod handler;
mod logs;
mod monitor;
mod multipart;
mod sse;
mod write;

/// base directory for files synced from clients (subdirs per watch group: upload/{wg_id}/)
pub(crate) static UPLOAD_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/upload"));
/// directory to hold zipped backup files
pub(crate) static BACKUP_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/backup"));
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
    version: &'static str,
    events: Arc<SseRegistry<ServerEventDto>>,
    log_buffer: Arc<LogBuffer>,
    log_events: Arc<SseRegistry<Vec<LogLineDto>>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log_level = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info"));
    let log_buffer = Arc::new(LogBuffer::new());

    tracing_subscriber::registry()
        .with(log_level)
        .with(tracing_subscriber::fmt::layer())
        .with(logs::LogCaptureLayer::new(log_buffer.clone()))
        .init();

    tokio::spawn(async {
        create_all_paths_if_not_exist(vec![
            UPLOAD_PATH.iter().as_path(),
            UPLOAD_TMP_PATH.iter().as_path(),
            BACKUP_PATH.iter().as_path(),
        ])?;
        create_file_if_not_exists(*DB_FILE_PATH)?;
        Ok::<(), std::io::Error>(())
    });

    let db = {
        let opts = SqliteConnectOptions::new()
            .filename(*DB_FILE_PATH)
            .create_if_missing(true)
            .pragma("foreign_keys", "ON");
        let pool = SqlitePool::connect_with(opts).await?;
        MIGRATOR.run(&pool).await?;
        ServerDatabase::new(pool)
    };

    tokio::spawn(schedule_data_backups(
        &UPLOAD_PATH,
        &BACKUP_PATH,
        db.clone(),
    ));

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
            "Timestamp;Total used mem in %;App used mem in %;Total used cpu in %;App used cpu in %;Disk used in %;Disk free in GiB"
                .to_string(),
        ),
    )
    .unwrap_or_else(|err| {
        panic!("Failed to create monitor writer: {}", err);
    });
    let monitor_writer = Arc::new(Mutex::new(monitor_writer));

    tokio::spawn(monitor::monitor_sys(monitor_writer.clone()));

    let log_events = Arc::new(SseRegistry::new());
    tokio::spawn(logs::flush_pending_periodically(
        log_buffer.clone(),
        log_events.clone(),
    ));

    let state = AppState {
        history: Arc::new(history),
        monitor_writer,
        db,
        version: env!("CARGO_PKG_VERSION"),
        events: Arc::new(SseRegistry::new()),
        log_buffer,
        log_events,
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
            get(|State(state): State<AppState>| async move { state.version }),
        )
        .route(ServerEndpoint::Config.to_str(), get(handler::get_config))
        // json api - for frontends
        .route(
            ServerEndpoint::ApiClients.to_str(),
            get(handler::api_list_clients),
        )
        .route(
            ServerEndpoint::ApiClient.to_str(),
            get(handler::api_get_client)
                .put(handler::api_update_client)
                .delete(handler::api_delete_client),
        )
        .route(
            ServerEndpoint::ApiClientWatchGroups.to_str(),
            get(handler::api_list_client_watch_groups).post(handler::api_create_client_watch_group),
        )
        .route(
            ServerEndpoint::ApiClientWatchGroup.to_str(),
            put(handler::api_update_client_watch_group)
                .delete(handler::api_delete_client_watch_group),
        )
        .route(
            ServerEndpoint::ApiWatchGroups.to_str(),
            get(handler::api_list_watch_groups).post(handler::api_create_watch_group),
        )
        .route(
            ServerEndpoint::ApiWatchGroup.to_str(),
            put(handler::api_update_watch_group).delete(handler::api_delete_watch_group),
        )
        .route(
            ServerEndpoint::ApiWatchGroupFiles.to_str(),
            get(handler::api_get_watch_group_files)
                .post(handler::api_upload_to_watch_group)
                .layer(DefaultBodyLimit::max(UPLOAD_LIMIT_BYTES)),
        )
        .route(
            ServerEndpoint::ApiWatchGroupFile.to_str(),
            get(handler::api_serve_watch_group_file).delete(handler::api_delete_watch_group_file),
        )
        .route(
            ServerEndpoint::ApiMonitor.to_str(),
            get(
                |state: State<AppState>, Query(q): Query<monitor::MonitorQuery>| {
                    let writer = state.monitor_writer.clone();
                    let points = q.points.unwrap_or(monitor::DEFAULT_MAX_POINTS);
                    monitor::api_get_monitoring(writer, points)
                },
            ),
        )
        .route(
            ServerEndpoint::ApiLinks.to_str(),
            get(handler::get_links)
                .post(handler::post_link)
                .delete(handler::delete_link),
        )
        .route(
            ServerEndpoint::ApiLinkTags.to_str(),
            post(handler::post_link_tag),
        )
        .route(
            ServerEndpoint::ApiBackups.to_str(),
            get(handler::list_backups),
        )
        .route(
            ServerEndpoint::ApiBackup.to_str(),
            get(handler::download_backup),
        )
        .route(
            ServerEndpoint::ApiEventsStream.to_str(),
            get(handler::api_events_stream),
        )
        .route(ServerEndpoint::ApiLogs.to_str(), get(handler::api_get_logs))
        .route(
            ServerEndpoint::ApiLogsStream.to_str(),
            get(handler::api_logs_stream),
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

    let port: u16 = match std::env::var("PORT") {
        Ok(port) => port.parse().expect("PORT is not a number"),
        Err(_) => 3000,
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Starting HTTP server on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
