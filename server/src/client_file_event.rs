use shared::file_event::FileEventType::DeleteEvent;
use shared::file_event::{FileEvent, FileEventType};
use shared::matchable_path::MatchablePath;
use shared::utc_millis::UtcMillis;
use std::path::PathBuf;
use uuid::Uuid;

/// What the client sends upon detecting a change in his file-system
#[derive(Debug, Clone)]
pub struct ClientFileEvent {
    pub utc_millis: UtcMillis,
    /// relative path of the file on client side from the tracked root dir
    pub relative_path: MatchablePath,
    pub event_type: FileEventType,
    pub temp_file_path: Option<PathBuf>,
    /// the size of the uploaded file or 0 for delete events
    pub content_size: usize,
}

pub struct ClientFileEventDto {
    pub utc_millis: Option<UtcMillis>,
    pub relative_path: Option<Vec<String>>,
    pub file_event_type: Option<FileEventType>,
    pub temp_file_path: Option<PathBuf>,
    pub content_size: Option<usize>,
}

impl From<ClientFileEvent> for FileEvent {
    fn from(value: ClientFileEvent) -> Self {
        FileEvent::new(
            Uuid::new_v4(),
            value.utc_millis,
            value.relative_path,
            value.content_size as u64,
            value.event_type,
            None,
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
            temp_file_path: dto.temp_file_path,
            content_size: dto.content_size.unwrap_or(0),
        };
        if event.event_type != DeleteEvent && event.temp_file_path.is_none() {
            return Err("Missing field 'file'".to_string());
        }
        Ok(event)
    }
}
