use axum::body::Bytes;
use uuid::Uuid;
use shared::file_event::{FileEvent, FileEventType};
use shared::file_event::FileEventType::DeleteEvent;
use shared::matchable_path::MatchablePath;

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

impl From<ClientFileEvent> for FileEvent {
    fn from(value: ClientFileEvent) -> Self {
        FileEvent::new(
            Uuid::new_v4(),
            value.utc_millis,
            value.relative_path,
            // deleted files will have size=0 which is fine
            value.file_bytes.map(|b| b.len() as u64).unwrap_or(0),
            value.event_type,
        )
    }
}

impl TryFrom<ClientFileEventDto> for ClientFileEvent {
    type Error = String;

    fn try_from(dto: ClientFileEventDto) -> Result<Self, Self::Error> {
        let event = ClientFileEvent {
            utc_millis: dto.utc_millis.ok_or("Missing field 'utc_millis'")?,
            relative_path: MatchablePath::from(
                dto.relative_path.ok_or("Missing field 'relative_path'")?,
            ),
            event_type: dto
                .file_event_type
                .ok_or("Missing field 'file_event_type'")?,
            file_bytes: dto.file_bytes,
        };
        if event.event_type != DeleteEvent && event.file_bytes.is_none() {
            return Err("Missing field 'file'".to_string());
        }
        Ok(event)
    }
}
