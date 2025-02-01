use axum::body::Bytes;

use crate::file_event::FileEventType;
use crate::file_event::FileEventType::DeleteEvent;
use crate::matchable_path::MatchablePath;

/// What the client sends upon detecting a change in his file-system
#[derive(Debug, Clone)]
pub struct ClientFileEvent {
    pub utc_millis: u64,
    /// relative path of the file on client side from the tracked root dir
    pub relative_path: MatchablePath,
    pub event_type: FileEventType,
    pub file_bytes: Option<Bytes>,
}

pub struct ClientFileEventDto {
    pub(crate) utc_millis: Option<u64>,
    pub(crate) relative_path: Option<Vec<String>>,
    pub(crate) file_event_type: Option<FileEventType>,
    pub(crate) file_bytes: Option<Bytes>,
}

impl TryFrom<ClientFileEventDto> for ClientFileEvent {
    type Error = String;

    fn try_from(dto: ClientFileEventDto) -> Result<Self, Self::Error> {
        let event = ClientFileEvent {
            utc_millis: dto.utc_millis.ok_or("Missing field 'utc_millis'")?,
            relative_path: dto.relative_path.ok_or("Missing field 'relative_path'")?,
            event_type: dto
                .file_event_type
                .ok_or("Missing field 'file_event_type'")?,
            file_bytes: dto.file_bytes,
        };
        if event.event_type != DeleteEvent && event.file_bytes.is_none() {
            return Err("Missing field 'file'".to_string());
        }
        if event.relative_path.contains("..") {
            println!("Denying relative_path '{}'", &event.relative_path);
            return Err("Forbidden: Attempted directory traversal".to_string());
        }
        if !event.relative_path.starts_with("./") && !event.relative_path.starts_with(".\\") {
            println!("Denying relative_path '{}'", &event.relative_path);
            return Err(format!(
                "Forbidden: '{}' is not a relative path (make sure to prefix with './' or '.\\'",
                event.relative_path
            ));
        }
        Ok(event)
    }
}
