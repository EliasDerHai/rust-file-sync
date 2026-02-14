use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../web/dist/"]
struct AppAssets;

pub async fn serve_embedded_app(uri: axum::http::Uri) -> axum::response::Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match AppAssets::get(path) {
        Some(file) => (
            [(header::CONTENT_TYPE, file.metadata.mimetype())],
            file.data,
        )
            .into_response(),
        // SPA fallback: serve index.html for unknown paths
        None => match AppAssets::get("index.html") {
            Some(file) => (
                [(header::CONTENT_TYPE, "text/html")],
                file.data,
            )
                .into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}
