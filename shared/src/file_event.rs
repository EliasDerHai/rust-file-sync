use crate::matchable_path::MatchablePath;
use crate::utc_millis::UtcMillis;
use serde_json::to_string;
use std::fmt::Debug;
use std::path::Path;
use uuid::Uuid;

// TODO consider moving to server (not a client concept)
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
}

impl FileEvent {
    /// produces csv line with ; as separator
    pub fn serialize_to_csv_line(&self) -> String {
        let parts = [
            self.id.to_string(),
            to_string(&self.utc_millis).unwrap(),
            self.relative_path.get().join("/"),
            self.size_in_bytes.to_string(),
            self.event_type.serialize_to_string(),
            self.client_host.clone().unwrap_or_default(),
        ];

        parts.join(";")
    }

    pub fn new(
        id: Uuid,
        utc_millis: UtcMillis,
        relative_path: MatchablePath,
        size_in_bytes: u64,
        event_type: FileEventType,
        client_host: Option<String>,
    ) -> Self {
        FileEvent {
            id,
            utc_millis,
            relative_path,
            size_in_bytes,
            event_type,
            client_host,
        }
    }
}

impl TryFrom<&str> for FileEvent {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.lines().count() > 1 {
            return Err(format!(
                "Parsing error - expected single CSV line but got {} lines: {:?}",
                value.lines().count(),
                value
            ));
        }

        let parts: Vec<&str> = value.split(';').collect();
        if parts.len() != 5 && parts.len() != 6 {
            return Err(format!(
                "Parsing error - expected 5 parts (id;utc_millis;relative_path;size_in_bytes;event_type) or 6 parts (id;utc_millis;relative_path;size_in_bytes;event_type;host_name) but found {} in {:?}",
                parts.len(),
                value
            ));
        }

        let id = Uuid::parse_str(parts[0])
            .map_err(|e| format!("Parsing error - invalid UUID '{}': {}", parts[0], e))?;

        let utc_millis = parts[1]
            .parse::<u64>()
            .map_err(|e| format!("Parsing error - invalid utc_millis '{}': {}", parts[1], e))?;

        let relative_path = MatchablePath::from(Path::new(parts[2]));

        let size_in_bytes = parts[3].parse::<u64>().map_err(|e| {
            format!(
                "Parsing error - invalid size_in_bytes '{}': {}",
                parts[3], e
            )
        })?;

        let event_type = FileEventType::try_from(parts[4])?;

        let client_host = if parts.len() == 6 && !parts[5].is_empty() {
            Some(parts[5].to_string())
        } else {
            None
        };

        Ok(FileEvent {
            id,
            utc_millis: UtcMillis::from(utc_millis),
            relative_path,
            size_in_bytes,
            event_type,
            client_host,
        })
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
        let event = FileEvent::new(
            uuid,
            UtcMillis::from(millis),
            MatchablePath::from(vec!["foo", "bar", "file.txt"]),
            1024 * 1024 * 1024,
            ChangeEvent,
            None,
        );

        let expected = format!("{uuid};{millis};foo/bar/file.txt;1073741824;change;");
        assert_eq!(expected, event.serialize_to_csv_line());
    }
    #[test]
    fn should_serialize_deserialize_round_trip() {
        let original_event = FileEvent {
            id: Uuid::new_v4(),
            utc_millis: UtcMillis::from(1234567890),
            relative_path: MatchablePath::from(vec!["folder", "subfolder", "file.txt"]),
            size_in_bytes: 1024,
            event_type: ChangeEvent,
            client_host: Some("arch".to_string()),
        };

        let csv_line = original_event.serialize_to_csv_line();
        let parsed_event =
            FileEvent::try_from(csv_line.as_str()).expect("Failed to parse valid CSV line");

        assert_eq!(original_event, parsed_event, "Round-trip mismatch!");
    }

    #[test]
    fn should_parse_err_invalid_uuid() {
        let invalid_uuid = "not-a-uuid;1234567;some/path;300;change";
        let result = FileEvent::try_from(invalid_uuid);
        assert!(result.is_err(), "Parsing invalid UUID should fail");
        assert!(
            result.unwrap_err().contains("invalid UUID"),
            "Error should mention invalid UUID"
        );
    }

    #[test]
    fn should_parse_err_invalid_utc_millis() {
        let invalid_utc = format!("{};abc;some/path;300;change", Uuid::new_v4());
        let result = FileEvent::try_from(invalid_utc.as_str());
        assert!(result.is_err(), "Parsing invalid utc_millis should fail");
        assert!(
            result.unwrap_err().contains("invalid utc_millis"),
            "Error should mention invalid utc_millis"
        );
    }

    #[test]
    fn should_parse_err_invalid_size_in_bytes() {
        let invalid_size = format!("{};123456;some/path;not-a-number;change", Uuid::new_v4());
        let result = FileEvent::try_from(invalid_size.as_str());
        assert!(result.is_err(), "Parsing invalid size_in_bytes should fail");
        assert!(
            result.unwrap_err().contains("invalid size_in_bytes"),
            "Error should mention invalid size_in_bytes"
        );
    }

    #[test]
    fn should_parse_err_invalid_event_type() {
        let invalid_event_type = format!("{};1234567;some/path;300;foo-bar", Uuid::new_v4());
        let result = FileEvent::try_from(invalid_event_type.as_str());
        assert!(result.is_err(), "Parsing invalid event_type should fail");
        assert!(
            result.unwrap_err().contains("Could not parse"),
            "Error should mention that event_type couldn't be parsed"
        );
    }

    #[test]
    fn should_parse_err_multiple_lines() {
        let multi_line = format!(
            "{};123456;some/path;300;change\nAnotherLine",
            Uuid::new_v4()
        );
        let result = FileEvent::try_from(multi_line.as_str());
        assert!(result.is_err(), "Multiple lines should fail");
        assert!(
            result.unwrap_err().contains("expected single CSV line"),
            "Error should mention multiple lines"
        );
    }

    #[test]
    fn should_parse_err_incorrect_parts_too_few() {
        let too_few = format!("{};123456;some/path;300", Uuid::new_v4()); // only 4 parts
        let result = FileEvent::try_from(too_few.as_str());
        assert!(result.is_err(), "Fewer than 5 parts should fail");
        assert!(
            result.unwrap_err().contains("expected 5 parts"),
            "Error should mention 5 parts"
        );
    }

    #[test]
    fn should_parse_err_incorrect_parts_too_many() {
        let too_many = format!("{};123456;some/path;300;change;host;other", Uuid::new_v4()); // 6 parts
        let result = FileEvent::try_from(too_many.as_str());
        assert!(result.is_err(), "More than 6 parts should fail");
    }

    #[test]
    fn should_parse_string_to_event_type() {
        assert_eq!(Ok(ChangeEvent), FileEventType::try_from("change"));
        assert_eq!(Ok(DeleteEvent), FileEventType::try_from("delete"));
        assert!(FileEventType::try_from("foobar").is_err());
    }
}
