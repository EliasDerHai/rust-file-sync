use std::{
    path::Path,
    sync::LazyLock,
};

use axum::{Json, Router, routing::get};
use axum::http::StatusCode;

use read::init_directory;

use crate::read::{FileDescription, get_files_of_dir};

mod read;
mod file_event;

static DATA_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data"));

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/scan", get(scan_disk));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tokio::spawn(async { init_directory(&DATA_PATH) });

    axum::serve(listener, app).await.unwrap();
}

async fn scan_disk() -> Result<Json<Vec<FileDescription>>, StatusCode> {
    match get_files_of_dir(&DATA_PATH) {
        Ok(descriptions) => Ok(Json(descriptions)),
        Err(err) => {
            eprintln!("IO Failure - {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
