use std::sync::Arc;
use std::{path::Path, sync::LazyLock};

use crate::file_history::InMemoryFileHistory;
use crate::write::schedule_data_backups;
use axum::extract::{Multipart, State};
use axum::routing::post;
use axum::{routing::get, Router};
use tracing::log::warn;
use tracing::{error, info};

mod client_file_event;
mod file_history;
mod handler;
mod init_directories;
mod write;

/// base directory for files synced from clients
static UPLOAD_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/upload"));
/// directory to hold zipped backup files
static BACKUP_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/backup"));
/// file to persist the in-mem state ([`InMemoryFileHistory`])
static HISTORY_CSV_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/history.csv"));

#[derive(Clone)]
struct AppState {
    history: Arc<InMemoryFileHistory>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    tokio::spawn(async {
        if !UPLOAD_PATH.exists() {
            std::fs::create_dir_all(UPLOAD_PATH.iter().as_path())?;
        }
        if !BACKUP_PATH.exists() {
            std::fs::create_dir_all(BACKUP_PATH.iter().as_path())?;
        }
        if !HISTORY_CSV_PATH.is_file() {
            std::fs::write(HISTORY_CSV_PATH.iter().as_path(), b"")?;
        }
        Ok::<(), std::io::Error>(())
    });
    tokio::spawn(schedule_data_backups(&UPLOAD_PATH, &BACKUP_PATH));

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
            "/upload",
            post(|state: State<AppState>, multipart: Multipart| {
                handler::upload_handler(&UPLOAD_PATH, state, multipart, &HISTORY_CSV_PATH)
            }),
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
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
