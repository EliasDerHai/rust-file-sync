use crate::file_event::{FileEvent, FileEventType};
use crate::file_history::FileHistory;
use crate::thumbnail;
use crate::write::write_all_chunks_of_field;
use crate::{AppState, THUMBNAIL_PATH, UPLOAD_PATH, UPLOAD_TMP_PATH};

pub(crate) static WEB_CLIENT_UUID: LazyLock<Uuid> =
    LazyLock::new(|| Uuid::parse_str(WEB_CLIENT_ID).expect("not a uuid"));

use axum::Json;
use axum::body::Body;
use axum::extract::{Multipart, Query, Request, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::IntoResponse;
use shared::content_hash::ContentHash;
use shared::dtos::{FileDescription, ServerWatchGroup, WatchGroupNameDto};
use shared::endpoint::{CLIENT_ID_HEADER_KEY, WEB_CLIENT_ID};
use shared::matchable_path::MatchablePath;
use shared::media::content_type_for;
use shared::utc_millis::UtcMillis;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tower_http::services::ServeFile;
use tracing::{error, info, warn};
use uuid::Uuid;

/// GET /api/watch-groups
pub async fn api_list_watch_groups(
    State(state): State<AppState>,
) -> Result<Json<Vec<ServerWatchGroup>>, (StatusCode, String)> {
    let groups = state
        .db
        .server_watch_group()
        .get_all_watch_groups()
        .await
        .map_err(|e| {
            error!("Failed to get watch groups: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    Ok(Json(groups))
}

/// POST /api/watch-groups
pub async fn api_create_watch_group(
    State(state): State<AppState>,
    Json(dto): Json<WatchGroupNameDto>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .server_watch_group()
        .insert_watch_group(dto.name.clone())
        .await
        .map_err(|e| {
            error!("Failed to create watch group: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Created watch group '{}'", dto.name);
    Ok(StatusCode::CREATED)
}

/// PUT /api/watch-groups/{id}
pub async fn api_update_watch_group(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(dto): Json<WatchGroupNameDto>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .server_watch_group()
        .rename_watch_group(id, dto.name)
        .await
        .map_err(|e| {
            error!("Failed to rename watch group: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Renamed watch group {}", id);
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/watch-groups/{id}
pub async fn api_delete_watch_group(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let found = state
        .db
        .server_watch_group()
        .delete(id)
        .await
        .map_err(|e| {
            error!("Failed to delete watch group: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    if found {
        info!("Deleted watch group {}", id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "Watch group not found".to_string()))
    }
}

/// GET /api/watch-groups/{id}/file?path=dir/subdir/file.ext — inline file preview
///
/// Served through `ServeFile` so range requests work: `<video>`/`<audio>` in the
/// SPA need 206 responses to show a duration and to seek.
pub async fn api_serve_watch_group_file(
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(params): Query<HashMap<String, String>>,
    request: Request,
) -> impl IntoResponse {
    let (_, _, full_path) = resolve_upload_path(id, &params)?;
    serve_original(&full_path, request).await
}

/// `ServeFile` (range requests, 304s) with our own content type table on top
async fn serve_original(
    full_path: &Path,
    request: Request,
) -> Result<axum::response::Response, (StatusCode, String)> {
    let ext = extension_of(full_path);

    let mut response = ServeFile::new(full_path)
        .try_call(request)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {}", e)))?;

    // our own table wins over mime guessing where we have an entry
    if let Some(content_type) = content_type_for(&ext) {
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    }

    Ok(response.map(Body::new))
}

/// GET /api/watch-groups/{id}/thumbnail?path=dir/file.ext&m=<mtime>
///
/// Small JPEG for the tile view, generated on first request and cached under
/// `data/thumbnail/` (see `crate::thumbnail`). 404 means "no thumbnail" (unsupported
/// format, undecodable file, no ffmpeg) and the SPA falls back to an icon. The `m`
/// param is only a cache-buster, which is what makes the long `Cache-Control` safe.
pub async fn api_serve_watch_group_thumbnail(
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(params): Query<HashMap<String, String>>,
    request: Request,
) -> Result<axum::response::Response, (StatusCode, String)> {
    let (_, rel, full_path) = resolve_upload_path(id, &params)?;
    if !full_path.is_file() {
        return Err((StatusCode::NOT_FOUND, "File not found".to_string()));
    }

    let strategy = thumbnail::Strategy::for_ext(&extension_of(&full_path));
    if strategy == thumbnail::Strategy::Passthrough {
        return serve_original(&full_path, request).await;
    }

    let thumb = thumbnail::get_or_create(&THUMBNAIL_PATH, id, &rel, &full_path, strategy)
        .await
        .map_err(|e| {
            error!("Thumbnail lookup failed for {}: {e}", full_path.display());
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or((StatusCode::NOT_FOUND, "No thumbnail available".to_string()))?;

    let mut response = ServeFile::new(&thumb)
        .try_call(request)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Thumbnail not found: {}", e)))?;
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    Ok(response.map(Body::new))
}

/// `?path=` query -> (matchable path, path relative to the watch group, full path under `upload/{id}/`)
fn resolve_upload_path(
    id: i64,
    params: &HashMap<String, String>,
) -> Result<(MatchablePath, PathBuf, PathBuf), (StatusCode, String)> {
    let path_str = match params.get("path") {
        Some(p) if !p.is_empty() => p,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Missing path parameter".to_string(),
            ));
        }
    };

    let matchable = MatchablePath::from(Path::new(path_str));
    if matchable.get().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Invalid path".to_string()));
    }

    let rel: PathBuf = matchable.get().iter().collect();
    let full_path = UPLOAD_PATH.join(id.to_string()).join(&rel);
    Ok((matchable, rel, full_path))
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

pub async fn api_get_watch_group_files(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<Vec<FileDescription>>, (StatusCode, String)> {
    let events = state
        .history
        .get_latest_events(id)
        .into_iter()
        .filter(|e| e.event_type.is_change())
        .map(FileEvent::into)
        .collect();

    Ok(Json(events))
}

/// POST /api/watch-groups/{id}/files — multipart `file` plus optional `dir` (e.g. `photos/2026`).
///
/// `X-Client-Id` must be the web app client; an existing file is replaced.
pub async fn api_upload_to_watch_group(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<StatusCode, (StatusCode, String)> {
    let (client_id, client_host) = app_client(&headers)?;

    let exists = state
        .db
        .server_watch_group()
        .exists(id)
        .await
        .map_err(|e| {
            error!("Failed to check watch group existence: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    if !exists {
        return Err((StatusCode::NOT_FOUND, format!("Watch group {id} not found")));
    }

    let (tmp_path, path_parts, size, hash) = extract_file(&mut multipart).await?;

    let rel: PathBuf = path_parts.iter().collect();
    let target_path = UPLOAD_PATH.join(id.to_string()).join(&rel);

    if let Some(target_dir) = target_path.parent()
        && let Err(e) = fs::create_dir_all(target_dir)
    {
        let _ = fs::remove_file(&tmp_path);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create directory: {e}"),
        ));
    }

    if let Err(e) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to move file: {e}"),
        ));
    }

    let event = FileEvent::new(
        Uuid::new_v4(),
        UtcMillis::now(),
        MatchablePath::from(path_parts),
        size as u64,
        hash,
        FileEventType::ChangeEvent,
        client_id,
        Some(client_host.to_string()),
        id,
    );

    if let Err(e) = state.db.file_event().insert(&event).await {
        error!("Failed to persist file event for {client_host} upload: {e}");
    }
    state.history.add(event);

    info!(
        "{client_host} uploaded '{}' to watch group {id}",
        rel.display()
    );
    Ok(StatusCode::CREATED)
}

/// Only the web app client may use the web API upload; sync clients go through `/sys/upload`.
fn app_client(headers: &HeaderMap) -> Result<(Uuid, &'static str), (StatusCode, String)> {
    let client_id = headers
        .get(CLIENT_ID_HEADER_KEY)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| Uuid::parse_str(v).ok());
    match client_id {
        Some(id) if id == *WEB_CLIENT_UUID => Ok((id, "web")),
        _ => Err((
            StatusCode::BAD_REQUEST,
            format!("{CLIENT_ID_HEADER_KEY} must be the web client id"),
        )),
    }
}

/// Returns the temp file and the target path parts (`dir` segments + file name).
async fn extract_file(
    multipart: &mut Multipart,
) -> Result<(PathBuf, Vec<String>, usize, ContentHash), (StatusCode, String)> {
    let mut dir = String::new();
    let mut upload: Option<(PathBuf, String, usize, ContentHash)> = None;

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        match field.name() {
            Some("dir") => {
                dir = field.text().await.unwrap_or_default();
            }
            Some("file") if upload.is_none() => {
                let raw_name = field.file_name().unwrap_or("upload").to_string();
                let filename = sanitize_filename(&raw_name)?;

                let tmp_path = UPLOAD_TMP_PATH.join(format!("{}_{}", Uuid::new_v4(), filename));
                let (size, hash) = write_all_chunks_of_field(tmp_path.as_path(), field)
                    .await
                    .map_err(|e| {
                        let _ = fs::remove_file(&tmp_path);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Failed to write upload: {e}"),
                        )
                    })?;
                upload = Some((tmp_path, filename, size, hash));
            }
            _ => {}
        }
    }

    let Some((tmp_path, filename, size, hash)) = upload else {
        return Err((
            StatusCode::BAD_REQUEST,
            "No 'file' field in request".to_string(),
        ));
    };
    let mut parts: Vec<String> = dir
        .split('/')
        .filter(|s| !s.is_empty())
        .map(sanitize_filename)
        .collect::<Result<_, _>>()
        .inspect_err(|_| {
            let _ = fs::remove_file(&tmp_path);
        })?;
    parts.push(filename);
    Ok((tmp_path, parts, size, hash))
}

/// DELETE /api/watch-groups/{id}/file?path=dir/subdir/file.ext
pub async fn api_delete_watch_group_file(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<StatusCode, (StatusCode, String)> {
    let (matchable_path, rel, full_path) = resolve_upload_path(id, &params)?;

    if !full_path.exists() {
        return Err((StatusCode::NOT_FOUND, "File not found".to_string()));
    }

    tokio::fs::remove_file(&full_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    thumbnail::remove(&THUMBNAIL_PATH, id, &rel).await;

    let event = FileEvent::new(
        Uuid::new_v4(),
        UtcMillis::now(),
        matchable_path,
        0,
        ContentHash::unknown(),
        FileEventType::DeleteEvent,
        *WEB_CLIENT_UUID,
        Some("web".to_string()),
        id,
    );

    if let Err(e) = state.db.file_event().insert(&event).await {
        error!("Failed to persist web delete event to DB: {e}");
    }
    state.history.add(event);

    info!(
        "Web UI deleted file '{}' from watch group {id}",
        rel.display()
    );
    Ok(StatusCode::NO_CONTENT)
}

fn sanitize_filename(raw: &str) -> Result<String, (StatusCode, String)> {
    let name = Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .filter(|n| !n.is_empty() && n != "." && n != "..")
        .ok_or_else(|| {
            warn!("Rejected unsafe filename: {:?}", raw);
            (StatusCode::BAD_REQUEST, format!("Invalid filename: {raw}"))
        })?;
    Ok(name)
}
