use std::fmt::Debug;

use uuid::Uuid;

use crate::{client_file_event::ClientFileEvent, matchable_path::MatchablePath};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FileEventType {
    CreateEvent,
    UpdateEvent,
    DeleteEvent,
    // I don't think we need a 'rename' or 'move' event as both can be represented as a combination of
    // delete and create.
}

impl FileEventType {
    fn serialize_to_string(&self) -> String {
        match self {
            FileEventType::CreateEvent => String::from("create"),
            FileEventType::UpdateEvent => String::from("update"),
            FileEventType::DeleteEvent => String::from("delete"),
        }
    }
}

impl TryFrom<&str> for FileEventType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "create" => Ok(FileEventType::CreateEvent),
            "update" => Ok(FileEventType::UpdateEvent),
            "delete" => Ok(FileEventType::DeleteEvent),
            _ => Err(format!("Could not parse '{}'", value)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FileEvent {
    /// probably not needed
    pub id: Uuid,
    /// time of event on client side
    pub utc_millis: u64,
    /// relative path of the file on client side from the tracked root dir
    pub relative_path: MatchablePath,
    pub size_in_bytes: u64,
    pub event_type: FileEventType,
}

impl FileEvent {
    /// produces csv line with ; as separator
    pub fn serialize_to_csv_line(&self) -> String {
        let parts = vec![
            self.id.to_string(),
            self.utc_millis.to_string(),
            self.relative_path.0.join("\\"),
            self.size_in_bytes.to_string(),
            self.event_type.serialize_to_string(),
        ];

        parts.join(";")
    }

    pub fn new(
        id: Uuid,
        utc_millis: u64,
        relative_path: MatchablePath,
        size_in_bytes: u64,
        event_type: FileEventType,
    ) -> Self {
        FileEvent {
            id,
            utc_millis,
            relative_path,
            size_in_bytes,
            event_type,
        }
    }
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

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::FileEventType::{CreateEvent, DeleteEvent, UpdateEvent};
    use super::*;

    #[test]
    fn should_serialize_event_to_csv_line() {
        let uuid = Uuid::new_v4();
        let millis = Utc::now().timestamp_millis() as u64;
        let create = FileEvent::new(
            uuid,
            millis,
            MatchablePath::from(vec!["foo", "bar", "file.txt"]),
            1024 * 1024 * 1024,
            CreateEvent,
        );

        let expected = format!("{uuid};{millis};./foo/bar/file.txt;1073741824;create");
        assert_eq!(expected, create.serialize_to_csv_line());
    }

    #[test]
    fn should_parse_string_to_event_type() {
        assert_eq!(Ok(CreateEvent), FileEventType::try_from("create"));
        assert_eq!(Ok(UpdateEvent), FileEventType::try_from("update"));
        assert_eq!(Ok(DeleteEvent), FileEventType::try_from("delete"));
        assert!(FileEventType::try_from("foobar").is_err());
    }
}
