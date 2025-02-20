use crate::file_history::InMemoryFileHistory;
use crate::write::{
    create_all_files_if_not_exist, create_all_paths_if_not_exist, schedule_data_backups,
};
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::routing::post;
use axum::{routing::get, Router};
use std::sync::Arc;
use std::{path::Path, sync::LazyLock};
use tracing::error;
use tracing_subscriber::EnvFilter;

mod client_file_event;
mod file_history;
mod handler;
mod init_directories;
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
        create_all_files_if_not_exist(vec![
            HISTORY_CSV_PATH.iter().as_path(),
            MONITORING_CSV_PATH.iter().as_path(),
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
        .route("/ping", get(|| async { "pong" }))
        .route("/scan", get(|| handler::scan_disk(&UPLOAD_PATH)))
        .route(
            "/monitor",
            get(|| handler::get_monitoring(&MONITORING_CSV_PATH)),
        )
        .route(
            "/upload",
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
        .route("/sync", post(handler::sync_handler))
        .route(
            "/download",
            get(|payload: String| handler::download(&UPLOAD_PATH, payload)),
        )
        .route(
            "/delete",
            post(|state: State<AppState>, payload: String| {
                handler::delete(&UPLOAD_PATH, payload, state)
            }),
        )
        // .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
