use std::fmt::Debug;

use crate::matchable_path::MatchablePath;
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FileEventType {
    ChangeEvent,
    DeleteEvent,
    // I don't think we need a 'rename' or 'move' event as both can be represented as a combination of
    // delete and create.
}

const CHANGE_STR: &'static str = "change";
const DELETE_STR: &'static str = "delete";

impl FileEventType {
    pub fn serialize_to_string(&self) -> String {
        match self {
            FileEventType::ChangeEvent => String::from(CHANGE_STR),
            FileEventType::DeleteEvent => String::from(DELETE_STR),
        }
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
            self.relative_path.get().join("\\"),
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

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::FileEventType::{ChangeEvent, DeleteEvent};
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
            ChangeEvent,
        );

        let expected = format!("{uuid};{millis};./foo/bar/file.txt;1073741824;create");
        assert_eq!(expected, create.serialize_to_csv_line());
    }

    #[test]
    fn should_parse_string_to_event_type() {
        assert_eq!(Ok(ChangeEvent), FileEventType::try_from("change"));
        assert_eq!(Ok(DeleteEvent), FileEventType::try_from("delete"));
        assert!(FileEventType::try_from("foobar").is_err());
    }
}
