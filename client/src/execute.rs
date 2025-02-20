use crate::endpoints::ServerEndpoint;
use reqwest::multipart::Form;
use reqwest::Client;
use shared::file_event::FileEventType;
use shared::get_files_of_directory::get_file_description;
use shared::sync_instruction::SyncInstruction;
use std::path::Path;
use tokio::fs;
use tokio::fs::{create_dir_all, remove_file};

/// executes an instruction of the server (see [`SyncInstruction`])
pub async fn execute(
    client: &Client,
    instruction: SyncInstruction,
    root: &Path,
    base: &str,
) -> Result<String, String> {
    match instruction {
        SyncInstruction::Upload(p) => {
            let file_path = p.resolve(root);
            let description = get_file_description(file_path.as_path(), root)?;
            let relative_path_to_send = description.relative_path.get().join("/");
            let form: Form = Form::new()
                .text(
                    "utc_millis",
                    description.last_updated_utc_millis.to_string(),
                )
                .text("relative_path", relative_path_to_send)
                .text(
                    "event_type",
                    FileEventType::ChangeEvent.serialize_to_string(),
                )
                .file("file", file_path)
                .await
                .map_err(|e| e.to_string())?;

            client
                .post(ServerEndpoint::Upload.to_uri(base))
                .multipart(form)
                .send()
                .await
                .map_err(|e| format!("Upload failed - {}", e.to_string()))?
                .text()
                .await
                .map_err(|_e| "?".to_string()) // just the error of response.text()
                .map(|response| format!("Upload successful - server replied with '{}'", response))
        }

        SyncInstruction::Download(p) => {
            let file_path = p.resolve(root);

            let response = client
                .get(ServerEndpoint::Download.to_uri(base))
                .body(p.to_serialized_string())
                .send()
                .await
                .map_err(|e| format!("Download request failed - {}", e.to_string()))?;

            let bytes = response.bytes().await.map_err(|e| {
                format!(
                    "Download failed - cannot read response body - {}",
                    e.to_string()
                )
            })?;

            create_dir_all(file_path.parent().unwrap())
                .await
                .expect(&format!(
                    "Should be able to create parent directory of file ({:?})",
                    &file_path
                ));

            fs::write(&file_path, bytes).await.map_err(|e| {
                format!(
                    "Could not save downloaded file ({:?}): {}",
                    &file_path,
                    e.to_string()
                )
            })?;

            Ok(format!(
                "Downloaded {} successfully",
                file_path
                    .file_name()
                    .map(|osstr| osstr.to_string_lossy().to_string())
                    .unwrap_or_else(|| "?".to_string())
            ))
        }

        SyncInstruction::Delete(p) => {
            let file_path = p.resolve(root);

            remove_file(&file_path)
                .await
                .map_err(|e| format!("Deleting file failed - {}", e.to_string()))
                .map(|_| {
                    format!(
                        "Deleted file '{}'",
                        &file_path
                            .file_name()
                            .map(|osstr| osstr.to_string_lossy().to_string())
                            .unwrap_or("?".to_string())
                    )
                })
        }
    }
}
