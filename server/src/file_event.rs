use shared::dtos::FileDescription;
use shared::matchable_path::MatchablePath;
use shared::utc_millis::UtcMillis;
use std::fmt::Debug;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FileEventType {
    ChangeEvent,
    DeleteEvent,
}

const CHANGE_STR: &str = "change";
const DELETE_STR: &str = "delete";

impl FileEventType {
    pub fn serialize_to_string(&self) -> String {
        match self {
            FileEventType::ChangeEvent => String::from(CHANGE_STR),
            FileEventType::DeleteEvent => String::from(DELETE_STR),
        }
    }

    pub fn is_delete(&self) -> bool {
        match self {
            FileEventType::ChangeEvent => false,
            FileEventType::DeleteEvent => true,
        }
    }

    pub fn is_change(&self) -> bool {
        !self.is_delete()
    }
}

impl TryFrom<&str> for FileEventType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            CHANGE_STR => Ok(FileEventType::ChangeEvent),
            DELETE_STR => Ok(FileEventType::DeleteEvent),
            _ => Err(format!("Could not parse '{}'", value)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FileEvent {
    /// probably not needed
    pub id: Uuid,
    /// time of event on client side
    pub utc_millis: UtcMillis,
    /// relative path of the file on client side from the tracked root dir
    pub relative_path: MatchablePath,
    pub size_in_bytes: u64,
    pub event_type: FileEventType,
    pub client_host: Option<String>,
    pub watch_group_id: i64,
}

impl From<FileEvent> for FileDescription {
    fn from(val: FileEvent) -> Self {
        let file_name = val.relative_path.tail();
        FileDescription {
            file_name: file_name.clone(),
            relative_path: val.relative_path,
            size_in_bytes: val.size_in_bytes,
            file_type: Path::new(&file_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_string(),
            last_updated_utc_millis: val.utc_millis,
        }
    }
}

impl FileEvent {
    pub fn new(
        id: Uuid,
        utc_millis: UtcMillis,
        relative_path: MatchablePath,
        size_in_bytes: u64,
        event_type: FileEventType,
        client_host: Option<String>,
        watch_group_id: i64,
    ) -> Self {
        FileEvent {
            id,
            utc_millis,
            relative_path,
            size_in_bytes,
            event_type,
            client_host,
            watch_group_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FileEventType::{ChangeEvent, DeleteEvent};
    use super::*;

    #[test]
    fn should_parse_string_to_event_type() {
        assert_eq!(Ok(ChangeEvent), FileEventType::try_from("change"));
        assert_eq!(Ok(DeleteEvent), FileEventType::try_from("delete"));
        assert!(FileEventType::try_from("foobar").is_err());
    }
}
