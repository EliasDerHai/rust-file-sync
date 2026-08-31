use crate::{BACKUP_PATH, write};
use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use shared::dtos::BackupFileDto;
use tokio_util::io::ReaderStream;
use tracing::error;

pub async fn list_backups() -> Result<Json<Vec<BackupFileDto>>, (StatusCode, String)> {
    let backups = write::enumerate_backup_files(&BACKUP_PATH).map_err(|err| {
        error!("Failed to list backups: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    })?;
    Ok(Json(backups))
}

pub async fn download_backup(Path(index): Path<u8>) -> impl IntoResponse {
    let backups = write::enumerate_backup_files(&BACKUP_PATH)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let Some(backup) = backups.get(index as usize) else {
        return Err((StatusCode::NOT_FOUND, "Backup not found".to_string()));
    };

    let path = BACKUP_PATH.join(&backup.file_name);
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let body = axum::body::Body::from_stream(ReaderStream::new(file));

    let headers = [(
        axum::http::header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", backup.file_name),
    )];

    Ok((headers, body))
}
