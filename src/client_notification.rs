use std::fs::File;

use crate::file_event::FileEventType;

pub struct Notification {
    utc_millis: u64,
    /// relative path of the file on client side from the tracked root dir
    relative_path: String,
    event_type: FileEventType,
    file: Option<File>,
}