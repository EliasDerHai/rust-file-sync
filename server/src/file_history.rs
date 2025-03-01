use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use shared::file_event::{FileEvent, FileEventType};
use shared::matchable_path::MatchablePath;
use tokio::time::Instant;
use tracing::{info, warn};

pub trait FileHistory: Send + Sync {
    /// add new event (insert at end of nested vec)
    fn add(&self, event: FileEvent);
    /// get all events of a path (chronologically = oldest first, latest last)
    fn get_events(&self, path: &MatchablePath) -> Option<Vec<FileEvent>>;
    /// get the latest event of one specific path
    fn get_latest_event(&self, path: &MatchablePath) -> Option<FileEvent>;
    /// get the latest event of every path
    fn get_latest_events(&self) -> Vec<FileEvent>;
    /// check if compliant with rules (chronologically sorted + grouped by path) - may panic
    fn sanity_check(&self);
}

/// multiple [`FileEvent`]s represent a history which allows to draw conclusions for synchronization of clients
#[derive(Default, Clone)]
pub struct InMemoryFileHistory {
    /// key = rel. file path - value = events (chronological) of given path
    store: Arc<Mutex<HashMap<MatchablePath, Vec<FileEvent>>>>,
}

impl From<Vec<FileEvent>> for InMemoryFileHistory {
    fn from(mut value: Vec<FileEvent>) -> Self {
        let i = Instant::now();
        if !value.is_sorted_by(|a, b| a.utc_millis < b.utc_millis) {
            warn!("History not chronological - correcting order...");
            value.sort_by_key(|e| e.utc_millis.clone());
        }
        let inner = value.into_iter().fold(HashMap::new(), |mut acc, curr| {
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

        let history = InMemoryFileHistory {
            store: Arc::new(Mutex::new(inner)),
        };
        history.sanity_check();
        info!(
            "History successfully initialized - took {}ms",
            i.elapsed().as_millis()
        );
        history
    }
}

impl TryFrom<&Path> for InMemoryFileHistory {
    type Error = std::io::Error;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        let content = fs::read_to_string(value)?;

        let events: Vec<FileEvent> = content
            .lines()
            .skip(1) // skip header
            .filter_map(|line| {
                let result = FileEvent::try_from(line);

                if result.is_err() {
                    warn!(
                        "Deserialization error while parsing event history: {:?}",
                        result
                    );
                }

                result.ok()
            })
            .collect();

        Ok(InMemoryFileHistory::from(events))
    }
}

impl FileHistory for InMemoryFileHistory {
    fn add(&self, event: FileEvent) {
        let mut guard = self.store.lock().unwrap();
        match guard.get_mut(&event.relative_path) {
            None => {
                guard.insert(event.relative_path.clone(), vec![event]);
            }
            Some(vec) => vec.push(event),
        }
    }

    fn get_events(&self, path: &MatchablePath) -> Option<Vec<FileEvent>> {
        self.store.lock().unwrap().get(path).cloned()
    }

    fn get_latest_event(&self, path: &MatchablePath) -> Option<FileEvent> {
        self.get_events(path)
            .map(|vec| vec.get(vec.len() - 1).cloned())
            .flatten()
    }

    fn get_latest_events(&self) -> Vec<FileEvent> {
        self.store
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(_, events)| {
                events.get(events.len() - 1).map_or(None, |e| {
                    if e.event_type != FileEventType::DeleteEvent {
                        Some(e.clone())
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// might panic if there is a programmatic error (sorting / grouping)
    fn sanity_check(&self) {
        for (key, value) in self.store.lock().unwrap().iter() {
            if let Some(false_path) = value
                .into_iter()
                .find(|e| &e.relative_path != key)
                .map(|e| e.relative_path.clone())
            {
                panic!(
                    "History invalid - should be grouped by relative_path - key: {:?} - found: {:?}",
                    key.get(), false_path
                );
            }
            if !value.is_sorted_by_key(|e| &e.utc_millis) {
                panic!(
                    "History invalid - should be sorted by time - key: {:?} ",
                    key
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::file_event::FileEvent;
    use shared::file_event::FileEventType::ChangeEvent;
    use shared::utc_millis::UtcMillis;
    use uuid::Uuid;

    #[test]
    fn should_get_latest() {
        let history = InMemoryFileHistory::from(Vec::new());

        let e1 = FileEvent::new(
            Uuid::new_v4(),
            UtcMillis::from(100),
            MatchablePath::from(vec!["dir", "file.txt"]),
            1024,
            ChangeEvent,
        );
        let e2 = FileEvent::new(
            Uuid::new_v4(),
            UtcMillis::from(200),
            MatchablePath::from(vec!["dir", "file.txt"]),
            1024,
            ChangeEvent,
        );

        history.add(e1);
        history.add(e2.clone());

        let latest = history.get_latest_event(&MatchablePath::from(vec!["dir", "file.txt"]));
        assert_eq!(Some(e2), latest);
        assert_eq!(
            2,
            history
                .get_events(&MatchablePath::from(vec!["dir", "file.txt"]))
                .unwrap()
                .len()
        );
    }

    #[test]
    fn should_build_history() {
        // arrange
        let matchable_path = MatchablePath::from(vec!["foo", "bar", "file.txt"]);
        let events: Vec<FileEvent> = (0..500)
            .map(|i: u64| {
                FileEvent::new(
                    Uuid::new_v4(),
                    UtcMillis::from(i),
                    matchable_path.clone(),
                    1024 * 1024 * 1024,
                    ChangeEvent,
                )
            })
            .collect();

        // act
        let history = InMemoryFileHistory::from(events);

        // assert
        let expected_latest_utc_millis = 499; // 500 elements but starts at 0
        assert_eq!(
            UtcMillis::from(expected_latest_utc_millis),
            history
                .get_latest_event(&matchable_path)
                .unwrap()
                .utc_millis
        );

        let events_in_history = history
            .store
            .lock()
            .unwrap()
            .get(&MatchablePath::from(vec!["foo", "bar", "file.txt"]))
            .unwrap()
            .clone();
        assert_eq!(500, events_in_history.len());
    }

    #[test]
    fn should_correct_bad_order_when_building_history() {
        // arrange
        let matchable_path = MatchablePath::from(vec!["foo", "bar", "file.txt"]);
        let events: Vec<FileEvent> = (0..500)
            .rev() // effectively different from `should_build_history` test
            .map(|i: u64| {
                FileEvent::new(
                    Uuid::new_v4(),
                    UtcMillis::from(i),
                    matchable_path.clone(),
                    1024 * 1024 * 1024,
                    ChangeEvent,
                )
            })
            .collect();

        // act
        let history = InMemoryFileHistory::from(events);

        // assert
        assert_eq!(
            UtcMillis::from(499),
            history
                .get_latest_event(&matchable_path)
                .unwrap()
                .utc_millis
        );
    }
}
