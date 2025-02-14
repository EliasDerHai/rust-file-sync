use std::path::{Path, PathBuf};
use axum::extract::Multipart;
use axum::http::StatusCode;
use shared::file_event::FileEventType;
use tracing::error;
use uuid::Uuid;
use crate::client_file_event::ClientFileEventDto;
use crate::write::write_all_chunks_of_field;

pub async fn parse_multipart_request(upload_root_tmp_path: &Path, multipart: &mut Multipart) -> Result<ClientFileEventDto, (StatusCode, String)> {
    let mut utc_millis: Option<u64> = None;
    let mut relative_path: Option<Vec<String>> = None;
    let mut file_event_type: Option<FileEventType> = None;
    let mut temp_file_path: Option<PathBuf> = None;
    let mut content_size: Option<usize> = None;

    while let Some(field) = multipart.next_field().await.unwrap() {
        match field.name() {
            None => error!("No field name in upload handler!"),
            Some("utc_millis") => {
                utc_millis = field
                    .text()
                    .await
                    .map(|t| t.parse::<u64>().ok())
                    .ok()
                    .flatten();
            }
            Some("relative_path") => {
                relative_path = field
                    .text()
                    .await
                    .map(|t| t.split("/").map(|str| str.to_string()).collect())
                    .ok();
            }
            Some("event_type") => {
                file_event_type = field
                    .text()
                    .await
                    .map(|t| FileEventType::try_from(t.as_str()).ok())
                    .ok()
                    .flatten()
            }
            Some("file") => {
                let random_uuid = Uuid::new_v4(); // avoid collision
                let original_file_name = field.file_name().unwrap_or("unknown_file");
                let temp_path = upload_root_tmp_path.join(Path::new(
                    format!("./{}_{}", random_uuid, original_file_name).as_str(),
                ));
                let s = write_all_chunks_of_field(temp_path.as_path(), field)
                    .await
                    // NOTE - generic: Err("Error parsing `multipart/form-data` request") will be
                    // returned if the axum body limit is exceeded - make sure to adjust the limit
                    // in main.rs
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Error writing multipart chunks - {}", e),
                        )
                    })?;
                temp_file_path = Some(temp_path);
                content_size = Some(s);
            }
            Some(other) => error!("Unknown field name '{other}' in upload handler"),
        }
    }
    Ok(ClientFileEventDto {
        utc_millis,
        relative_path,
        file_event_type,
        temp_file_path,
        content_size,
    })
}