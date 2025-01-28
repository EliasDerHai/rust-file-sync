use std::{path::Path, sync::LazyLock};

use axum::{Json, Router, routing::get};
use axum::extract::Multipart;
use axum::http::StatusCode;
use axum::routing::post;

use read::init_directory;

use crate::read::{FileDescription, get_files_of_dir};

mod read;
mod file_event;
mod client_notification;

static DATA_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data"));

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/scan", get(scan_disk))
        .route("/upload", post(upload_handler));

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

async fn upload_handler(mut multipart: Multipart) -> Result<String, (StatusCode, String)> {
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap_or("unknown").to_string();

        if name == "file" {
            let file_name = field.file_name().unwrap_or("unnamed_file").to_string();
            let file_data = field.bytes().await.unwrap();
            println!("Uploaded file: {} ({}kb)", file_name, file_data.len() / 1024);
        } else {
            let value = field.text().await.unwrap();
            println!("Meta-info: {} = {}", name, value);
        }
    }

    Ok("Upload successful".to_string())
}
