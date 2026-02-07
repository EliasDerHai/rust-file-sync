use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use rust_embed::RustEmbed;
use tracing::info;

#[derive(RustEmbed)]
#[folder = "pwa/"]
struct PwaAssets;

pub async fn serve_embedded_pwa(uri: axum::http::Uri) -> axum::response::Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    info!("serve_embedded_pwa's path: {}", path);
    match PwaAssets::get(path) {
        Some(file) => (
            [(header::CONTENT_TYPE, file.metadata.mimetype())],
            file.data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
