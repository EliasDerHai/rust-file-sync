use std::sync::Arc;
use std::{path::Path, sync::LazyLock};

use axum::extract::{Multipart, State};
use axum::routing::post;
use axum::{routing::get, Router};

use read::init_directories;

use crate::file_history::InMemoryFileHistory;
use crate::write::schedule_data_backups;

mod client_file_event;
mod file_event;
mod file_history;
mod handler;
mod read;
mod write;

/// base directory of all runtime data
static DATA_ROOT_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data"));
/// base directory for files synced from clients
static UPLOAD_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/upload"));
/// directory to hold zipped backup files
static BACKUP_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/backup"));
/// file to persist the in-mem state ([`InMemoryFileHistory`])
static HISTORY_CSV_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data/history.csv"));

#[derive(Clone)]
struct AppState {
    // history: Arc<dyn FileHistory>, // todo figure out how to use trait objects 😭
    history: Arc<InMemoryFileHistory>,
}

#[tokio::main]
async fn main() {
    tokio::spawn(async { init_directories(&UPLOAD_PATH, &BACKUP_PATH, &HISTORY_CSV_PATH) });
    tokio::spawn(async { schedule_data_backups(&UPLOAD_PATH, &BACKUP_PATH) });

    // todo - init from persistence aka file
    let state = AppState {
        history: Arc::new(InMemoryFileHistory::from(vec![])),
    };

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/scan", get(|| handler::scan_disk(&UPLOAD_PATH)))
        .route(
            "/upload",
            post(|state: State<AppState>, multipart: Multipart| {
                handler::upload_handler(&UPLOAD_PATH, state, multipart)
            }),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
