use std::collections::HashMap;
use std::fmt::Debug;

use tokio::time::Instant;
use uuid::Uuid;

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
struct FileEvent {
    /// probably not needed
    id: Uuid,
    /// time of event on client side
    utc_millis: u64,
    /// relative path of the file on client side from the tracked root dir
    relative_path: String,
    size_in_bytes: u64,
    event_type: FileEventType,
}

impl FileEvent {
    /// produces csv line with ; as separator
    fn serialize_to_csv_line(&self) -> String {
        let parts = vec![
            self.id.to_string(),
            self.utc_millis.to_string(),
            self.relative_path.clone(),
            self.event_type.serialize_to_string(),
        ];

        parts.join(";")
    }

    fn new(
        id: Uuid,
        utc_millis: u64,
        relative_path: String,
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

/// multiple [`FileEvent`]s represent a history which allows to draw conclusions for synchronization of clients
pub struct FileHistory {
    /// key = rel. file path - value = events (chronological) of given path
    inner: HashMap<String, Vec<FileEvent>>,
}

impl From<Vec<FileEvent>> for FileHistory {
    fn from(mut value: Vec<FileEvent>) -> Self {
        let i = Instant::now();
        if !value.is_sorted_by_key(|e| e.utc_millis) {
            println!("History not chronological - correcting order...");
            value.sort_by_key(|e| e.utc_millis);
        }
        let inner = value
            .into_iter()
            .fold(HashMap::new(), |mut acc, curr| {
                match acc.get_mut(&curr.relative_path) {
                    None => {
                        acc.insert(curr.relative_path.clone(), vec![curr]);
                    }
                    Some(events) => {
                        events.push(curr);
                    }
                }
                acc
            });

        let history = FileHistory { inner };
        history.sanity_check();
        println!("History successfully initialized - took {}ms", i.elapsed().as_millis());
        history
    }
}

impl FileHistory {
    /// might panic if there is a programmatic error (sorting / grouping)
    fn sanity_check(&self) {
        for (key, value) in self.inner.iter() {
            if let Some(false_path) = value.iter()
                .find(|e| &e.relative_path != key)
                .map(|e| e.relative_path.as_str()) {
                panic!("History invalid - should be grouped by relative_path - key: {} - found: {}", key, false_path);
            }
            if !value.is_sorted_by_key(|e| e.utc_millis) {
                panic!("History invalid - should be sorted by time - key: {} ", key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use super::FileEventType::{CreateEvent, DeleteEvent, UpdateEvent};

    #[test]
    fn should_serialize_event_to_csv_line() {
        let uuid = Uuid::new_v4();
        let millis = Utc::now().timestamp_millis() as u64;
        let create = FileEvent::new(
            uuid,
            millis,
            "./foo/bar/file.txt".to_string(),
            1024 * 1024 * 1024,
            CreateEvent,
        );

        let expected = format!("{uuid};{millis};./foo/bar/file.txt;create");
        assert_eq!(expected, create.serialize_to_csv_line());
    }

    #[test]
    fn should_parse_string_to_event_type() {
        assert_eq!(Ok(CreateEvent), FileEventType::try_from("create"));
        assert_eq!(Ok(UpdateEvent), FileEventType::try_from("update"));
        assert_eq!(Ok(DeleteEvent), FileEventType::try_from("delete"));
        assert!(FileEventType::try_from("foobar").is_err());
    }

    #[test]
    fn should_build_history() {
        let events: Vec<FileEvent> = (0..500)
            .map(|i|
                FileEvent::new(
                    Uuid::new_v4(),
                    i,
                    "./foo/bar/file.txt".to_string(),
                    1024 * 1024 * 1024,
                    CreateEvent,
                )
            ).collect();

        let history = FileHistory::from(events);

        assert_eq!(500, history.inner.get("./foo/bar/file.txt").unwrap().len());
    }
}