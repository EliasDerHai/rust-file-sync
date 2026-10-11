use crate::db::ServerDatabase;
use crate::file_history::InMemoryFileHistory;
use crate::logs::LogBuffer;
use crate::monitor::MonitorDb;
use crate::sse::SseRegistry;
use crate::write::{
    create_all_paths_if_not_exist, create_file_if_not_exists, schedule_data_backups,
};
use axum::extract::{DefaultBodyLimit, State};
use axum::routing::{post, put};

const UPLOAD_LIMIT_BYTES: usize = 500 * 1024 * 1024; // 500 MB
use axum::{Router, routing::get};
use shared::dtos::{LogLineDto, MapConfigDto, ServerEventDto};
use shared::endpoint::ServerEndpoint;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteConnectOptions;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock;
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
mod thumbnail;
mod write;

/// base directory for files synced from clients (subdirs per watch group: upload/{wg_id}/)
pub(crate) static UPLOAD_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/upload"));
/// directory to hold zipped backup files
pub(crate) static BACKUP_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/backup"));
/// cached tile-view thumbnails (subdirs per watch group, mirroring upload/{wg_id}/) -
/// derived data, so deliberately outside `upload/` and therefore out of backups
pub(crate) static THUMBNAIL_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/thumbnail"));
/// legacy rotating monitoring CSVs - only read once, for the import into `monitor.db`
static LEGACY_MONITORING_DIR: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/monitor"));
/// tiered system monitoring (see `monitor/`) - separate from `sqlite.db`, so it stays
/// out of the daily `VACUUM INTO` backups
static MONITOR_DB_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/monitor.db"));
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
    monitor_db: MonitorDb,
    db: ServerDatabase,
    version: &'static str,
    events: Arc<SseRegistry<ServerEventDto>>,
    log_buffer: Arc<LogBuffer>,
    log_events: Arc<SseRegistry<Vec<LogLineDto>>>,
    /// MapTiler API key for the web admin's Locations map, if configured. Meant to
    /// reach the browser (restricted by domain in the MapTiler dashboard), so no
    /// need to proxy tiles through the server.
    maptiler_key: Option<String>,
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
    tokio::spawn(thumbnail::probe_ffmpeg());

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

    let monitor_db = MonitorDb::open(&MONITOR_DB_PATH).await?;
    tokio::spawn(monitor::start(
        monitor_db.clone(),
        LEGACY_MONITORING_DIR.to_path_buf(),
    ));

    let log_events = Arc::new(SseRegistry::new());
    tokio::spawn(logs::flush_pending_periodically(
        log_buffer.clone(),
        log_events.clone(),
    ));

    let maptiler_key = std::env::var("MAPTILER_API_KEY").ok();

    let state = AppState {
        history: Arc::new(history),
        monitor_db,
        db,
        version: env!("CARGO_PKG_VERSION"),
        events: Arc::new(SseRegistry::new()),
        log_buffer,
        log_events,
        maptiler_key,
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
            ServerEndpoint::ApiWatchGroupThumbnail.to_str(),
            get(handler::api_serve_watch_group_thumbnail),
        )
        .route(
            ServerEndpoint::ApiMonitor.to_str(),
            get(monitor::api_get_monitoring),
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
        .route(
            ServerEndpoint::ApiLocations.to_str(),
            get(handler::get_location_points)
                .post(handler::post_location_points)
                .delete(handler::delete_location_points),
        )
        .route(
            ServerEndpoint::ApiMapConfig.to_str(),
            get(|State(state): State<AppState>| async move {
                axum::Json(MapConfigDto {
                    maptiler_key: state.maptiler_key.clone(),
                })
            }),
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
