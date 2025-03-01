use crate::file_history::InMemoryFileHistory;
use crate::write::{
    create_all_csv_files_if_not_exist, create_all_paths_if_not_exist, schedule_data_backups,
};
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::routing::post;
use axum::{routing::get, Router};
use shared::endpoint::ServerEndpoint;
use std::sync::Arc;
use std::{path::Path, sync::LazyLock};
use tracing::error;
use tracing_subscriber::EnvFilter;

mod client_file_event;
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
static MONITORING_CSV_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/monitor.csv"));
/// dir to which multipart-files can be saved to, before being moved to the actual 'mirrored path'
/// temporary and might be cleaned upon encountering errors or on scheduled intervals
static UPLOAD_TMP_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/upload_in_progress"));

#[derive(Clone)]
struct AppState {
    history: Arc<InMemoryFileHistory>,
}

#[tokio::main]
async fn main() {
    let log_level = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(log_level).init();

    tokio::spawn(async {
        create_all_paths_if_not_exist(vec![
            UPLOAD_PATH.iter().as_path(),
            UPLOAD_TMP_PATH.iter().as_path(),
            BACKUP_PATH.iter().as_path(),
        ])?;
        create_all_csv_files_if_not_exist(vec![
            (
                MONITORING_CSV_PATH.iter().as_path(),
                Some(vec![
                    "Timestamp".to_string(),
                    "Total used mem in %".to_string(),
                    "App used mem in %".to_string(),
                    "Total used cpu in %".to_string(),
                    "App used cpu in %".to_string(),
                ]),
            ),
            (
                HISTORY_CSV_PATH.iter().as_path(),
                Some(vec![
                    "id".to_string(),
                    "utc_millis".to_string(),
                    "relative_path".to_string(),
                    "size_in_bytes".to_string(),
                    "event_type".to_string(),
                ]),
            ),
        ])?;
        Ok::<(), std::io::Error>(())
    });
    tokio::spawn(schedule_data_backups(&UPLOAD_PATH, &BACKUP_PATH));
    tokio::spawn(monitor::monitor_sys(&MONITORING_CSV_PATH));

    let history =
        InMemoryFileHistory::try_from(HISTORY_CSV_PATH.iter().as_path()).unwrap_or_else(|err| {
            error!("Failed to load history: {}", err);
            InMemoryFileHistory::from(Vec::new())
        });
    let state = AppState {
        history: Arc::new(history),
    };

    let app = Router::new()
        .route(ServerEndpoint::Ping.to_str(), get(|| async { "pong" }))
        .route(
            ServerEndpoint::Scan.to_str(),
            get(|| handler::scan_disk(&UPLOAD_PATH)),
        )
        .route(
            ServerEndpoint::Monitor.to_str(),
            get(|| handler::get_monitoring(&MONITORING_CSV_PATH)),
        )
        .route(
            ServerEndpoint::Upload.to_str(),
            post(|state: State<AppState>, multipart: Multipart| {
                handler::upload_handler(
                    &UPLOAD_PATH,
                    &UPLOAD_TMP_PATH,
                    &HISTORY_CSV_PATH,
                    state,
                    multipart,
                )
            })
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
            post(|state: State<AppState>, payload: String| {
                handler::delete(&UPLOAD_PATH, &HISTORY_CSV_PATH, payload, state)
            }),
        )
        // .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
