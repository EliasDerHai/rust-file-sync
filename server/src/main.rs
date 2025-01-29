use std::{path::Path, sync::LazyLock};
use std::sync::Arc;

use axum::{Router, routing::get};
use axum::routing::post;
use read::init_directory;

use crate::file_history::InMemoryFileHistory;

mod read;
mod file_event;
mod client_file_event;
mod handler;
mod file_history;

pub static DATA_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data"));

#[derive(Clone)]
struct AppState {
    // history: Arc<dyn FileHistory>, // todo figure out how to use trait objects 😭
    history: Arc<InMemoryFileHistory>,
}

#[tokio::main]
async fn main() {
    tokio::spawn(async { init_directory(&DATA_PATH) });

    // todo - init from persistence aka file
    let state = AppState {
        history: Arc::new(InMemoryFileHistory::from(vec![])),
    };

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/scan", get(handler::scan_disk))
        .route("/upload", post(handler::upload_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();


    axum::serve(listener, app).await.unwrap();
}