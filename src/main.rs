use axum::{
    http::StatusCode, routing::get, Json, Router
};

#[tokio::main]
async fn main() {

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/scan", get(scan_disk));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}



async fn scan_disk(
    // State(state): State<AppStateDyn>,
    // Path(id): Path<Uuid>,
) -> Result<Json<()>, StatusCode> {
         Err(StatusCode::NOT_IMPLEMENTED)
}
