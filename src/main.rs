use std::{path::Path, sync::LazyLock};
use std::sync::Arc;

use axum::{Router, routing::get};
use axum::routing::post;

use read::init_directory;

use crate::file_event::{FileHistory, InMemoryFileHistory};

mod read;
mod file_event;
mod client_notification;

pub static DATA_PATH: LazyLock<&Path> = LazyLock::new(|| Path::new("./data"));

#[derive(Clone)]
struct AppState {
    history: Arc<dyn FileHistory>,
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
        .route("/scan", get(handlers::scan_disk))
        .route("/upload", post(handlers::upload_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();


    axum::serve(listener, app).await.unwrap();
}

mod handlers {
    use axum::body::Bytes;
    use axum::extract::Multipart;
    use axum::http::StatusCode;
    use axum::Json;

    use crate::client_notification::{ClientFileEvent, ClientFileEventDto};
    use crate::DATA_PATH;
    use crate::file_event::FileEventType;
    use crate::read::{FileDescription, get_files_of_dir};

    /// expecting no payload
    /// returning list of file meta infos
    pub async fn scan_disk() -> Result<Json<Vec<FileDescription>>, StatusCode> {
        match get_files_of_dir(&DATA_PATH) {
            Ok(descriptions) => Ok(Json(descriptions)),
            Err(err) => {
                eprintln!("IO Failure - {}", err);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    /// expecting payload like
    /// {
    ///   utc_millis: 42,
    ///   relative_path: "./directory/file.txt",
    ///   event_type: "create",
    ///   file: @File
    /// }
    pub async fn upload_handler(mut multipart: Multipart) -> Result<String, (StatusCode, String)> {
        let mut utc_millis: Option<u64> = None;
        let mut relative_path: Option<String> = None;
        let mut file_event_type: Option<FileEventType> = None;
        let mut file_bytes: Option<Bytes> = None;

        while let Some(field) = multipart.next_field().await.unwrap() {
            match field.name() {
                None => eprintln!("No field name in upload handler!"),
                Some("utc_millis") => utc_millis = field.text()
                    .await.map(|t| t.parse::<u64>().ok()).ok().flatten(),
                Some("relative_path") => relative_path = field.text()
                    .await.map(|t| t.to_string()).ok(),
                Some("event_type") => file_event_type = field.text()
                    .await.map(|t| FileEventType::try_from(t.as_str()).ok()).ok().flatten(),
                Some("file") => file_bytes = field.bytes()
                    .await.ok(),
                Some(other) => eprintln!("Unknown field name '{other}' in upload handler"),
            }
        }

        let event: Result<ClientFileEvent, (StatusCode, String)> = ClientFileEvent::try_from(ClientFileEventDto {
            utc_millis,
            relative_path,
            file_event_type,
            file_bytes,
        }).map_err(|e| (StatusCode::BAD_REQUEST, e));

        event.map(|_| "Upload successful".to_string())
    }
}
