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
    pub temp_file_path: Option<PathBuf>,
    /// the size of the uploaded file or 0 for delete events
    pub content_size: usize,
    pub watch_group_id: i64,
}

pub struct ClientFileEventDto {
    pub utc_millis: Option<UtcMillis>,
    pub relative_path: Option<Vec<String>>,
    pub temp_file_path: Option<PathBuf>,
    pub content_size: Option<usize>,
    pub watch_group_id: i64,
}

impl From<ClientFileEvent> for FileEvent {
    fn from(value: ClientFileEvent) -> Self {
        FileEvent::new(
            Uuid::new_v4(),
            value.utc_millis,
            value.relative_path,
            value.content_size as u64,
            FileEventType::ChangeEvent,
            None,
            value.watch_group_id,
        )
    }
}

impl TryFrom<ClientFileEventDto> for ClientFileEvent {
    type Error = String;

    fn try_from(dto: ClientFileEventDto) -> Result<Self, Self::Error> {
        Ok(ClientFileEvent {
            utc_millis: dto.utc_millis.ok_or("Missing field 'utc_millis'")?,
            relative_path: MatchablePath::from(
                dto.relative_path.ok_or("Missing field 'relative_path'")?,
            ),
            temp_file_path: dto.temp_file_path,
            content_size: dto.content_size.unwrap_or(0),
            watch_group_id: dto.watch_group_id,
        })
    }
}
