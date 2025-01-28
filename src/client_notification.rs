use axum::body::Bytes;

use crate::file_event::FileEventType;
use crate::file_event::FileEventType::DeleteEvent;

/// What the client sends upon detecting a change in his file-system
#[derive(Debug)]
pub struct ClientFileNotification {
    utc_millis: u64,
    /// relative path of the file on client side from the tracked root dir
    relative_path: String,
    event_type: FileEventType,
    file_bytes: Option<Bytes>,
}

pub struct ClientFileNotificationDto {
    pub(crate) utc_millis: Option<u64>,
    pub(crate) relative_path: Option<String>,
    pub(crate) file_event_type: Option<FileEventType>,
    pub(crate) file_bytes: Option<Bytes>,
}

impl TryFrom<ClientFileNotificationDto> for ClientFileNotification {
    type Error = String;

    fn try_from(dto: ClientFileNotificationDto) -> Result<Self, Self::Error> {
        let notification = ClientFileNotification {
            utc_millis: dto.utc_millis.ok_or("Missing field 'utc_millis'")?,
            relative_path: dto.relative_path.ok_or("Missing field 'relative_path'")?,
            event_type: dto.file_event_type.ok_or("Missing field 'file_event_type'")?,
            file_bytes: dto.file_bytes,
        };
        if notification.event_type != DeleteEvent && notification.file_bytes.is_none() {
            return Err("Missing field 'file'".to_string());
        }
        Ok(notification)
    }
}
