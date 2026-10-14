use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use rust_embed::RustEmbed;
use shared::endpoint::{CLIENT_ID_HEADER_KEY, PWA_CLIENT_ID};
use tracing::info;

#[derive(RustEmbed)]
#[folder = "pwa/"]
struct PwaAssets;

pub async fn serve_embedded_pwa(uri: axum::http::Uri) -> axum::response::Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    info!("serve_embedded_pwa's path: {}", path);
    // generated so the PWA's JS shares these values with the Rust crates instead of copying them
    if path == "constants.js" {
        let js = format!(
            "export const CLIENT_ID_HEADER_KEY = '{CLIENT_ID_HEADER_KEY}';\n\
             export const PWA_CLIENT_ID = '{PWA_CLIENT_ID}';\n"
        );
        return ([(header::CONTENT_TYPE, "text/javascript")], js).into_response();
    }
    match PwaAssets::get(path) {
        Some(file) => (
            [(header::CONTENT_TYPE, file.metadata.mimetype())],
            file.data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
