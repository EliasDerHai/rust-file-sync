use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use shared::file_event::FileEvent;
use shared::matchable_path::MatchablePath;
use tokio::time::Instant;
use tracing::{info, warn};

pub trait FileHistory: Send + Sync {
    /// add new event (insert at end of nested vec)
    fn add(&self, event: FileEvent);
    /// get all events of a path within a watch group (chronologically = oldest first, latest last)
    fn get_events(&self, wg_id: i64, path: &MatchablePath) -> Option<Vec<FileEvent>>;
    /// get the latest event of one specific path within a watch group
    fn get_latest_event(&self, wg_id: i64, path: &MatchablePath) -> Option<FileEvent>;
    /// get the latest event of every path within a watch group
    fn get_latest_events(&self, wg_id: i64) -> Vec<FileEvent>;
    /// check if compliant with rules (chronologically sorted + grouped by path) - may panic
    fn sanity_check(&self);
}

/// outer key = watch_group_id, inner key = rel. file path, value = events (chronological)
type HistoryStore = HashMap<i64, HashMap<MatchablePath, Vec<FileEvent>>>;

/// multiple [`FileEvent`]s represent a history which allows to draw conclusions for synchronization of clients
#[derive(Default, Clone)]
pub struct InMemoryFileHistory {
    store: Arc<Mutex<HistoryStore>>,
}

impl From<Vec<FileEvent>> for InMemoryFileHistory {
    fn from(mut value: Vec<FileEvent>) -> Self {
        let i = Instant::now();
        if !value.is_sorted_by(|a, b| a.utc_millis < b.utc_millis) {
            warn!("History not chronological - correcting order...");
            value.sort_by_key(|e| e.utc_millis.clone());
        }

        let inner: HistoryStore =
            value
                .into_iter()
                .fold(HashMap::new(), |mut outer, curr| {
                    let wg_map = outer.entry(curr.watch_group_id).or_default();
                    wg_map
                        .entry(curr.relative_path.clone())
                        .or_default()
                        .push(curr);
                    outer
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

impl FileHistory for InMemoryFileHistory {
    fn add(&self, event: FileEvent) {
        let mut guard = self.store.lock().unwrap();
        let wg_map = guard.entry(event.watch_group_id).or_default();
        wg_map
            .entry(event.relative_path.clone())
            .or_default()
            .push(event);
    }

    fn get_events(&self, wg_id: i64, path: &MatchablePath) -> Option<Vec<FileEvent>> {
        self.store
            .lock()
            .unwrap()
            .get(&wg_id)
            .and_then(|wg_map| wg_map.get(path).cloned())
    }

    fn get_latest_event(&self, wg_id: i64, path: &MatchablePath) -> Option<FileEvent> {
        self.get_events(wg_id, path)
            .and_then(|vec| vec.last().cloned())
    }

    fn get_latest_events(&self, wg_id: i64) -> Vec<FileEvent> {
        self.store
            .lock()
            .unwrap()
            .get(&wg_id)
            .map(|wg_map| {
                wg_map
                    .iter()
                    .filter_map(|(_, events)| events.last().cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// might panic if there is a programmatic error (sorting / grouping)
    fn sanity_check(&self) {
        for (_, wg_map) in self.store.lock().unwrap().iter() {
            for (key, value) in wg_map.iter() {
                if let Some(false_path) = value
                    .iter()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::file_event::FileEvent;
    use shared::file_event::FileEventType::ChangeEvent;
    use shared::utc_millis::UtcMillis;
    use uuid::Uuid;

    const WG: i64 = 1;

    #[test]
    fn should_get_latest() {
        let history = InMemoryFileHistory::from(Vec::new());

        let e1 = FileEvent::new(
            Uuid::new_v4(),
            UtcMillis::from(100),
            MatchablePath::from(vec!["dir", "file.txt"]),
            1024,
            ChangeEvent,
            None,
            WG,
        );
        let e2 = FileEvent::new(
            Uuid::new_v4(),
            UtcMillis::from(200),
            MatchablePath::from(vec!["dir", "file.txt"]),
            1024,
            ChangeEvent,
            None,
            WG,
        );

        history.add(e1);
        history.add(e2.clone());

        let latest =
            history.get_latest_event(WG, &MatchablePath::from(vec!["dir", "file.txt"]));
        assert_eq!(Some(e2), latest);
        assert_eq!(
            2,
            history
                .get_events(WG, &MatchablePath::from(vec!["dir", "file.txt"]))
                .unwrap()
                .len()
        );
    }

    #[test]
    fn should_build_history() {
        let matchable_path = MatchablePath::from(vec!["foo", "bar", "file.txt"]);
        let events: Vec<FileEvent> = (0..500)
            .map(|i: u64| {
                FileEvent::new(
                    Uuid::new_v4(),
                    UtcMillis::from(i),
                    matchable_path.clone(),
                    1024 * 1024 * 1024,
                    ChangeEvent,
                    None,
                    WG,
                )
            })
            .collect();

        let history = InMemoryFileHistory::from(events);

        assert_eq!(
            UtcMillis::from(499),
            history
                .get_latest_event(WG, &matchable_path)
                .unwrap()
                .utc_millis
        );

        let guard = history.store.lock().unwrap();
        let events_in_history = guard
            .get(&WG)
            .unwrap()
            .get(&MatchablePath::from(vec!["foo", "bar", "file.txt"]))
            .unwrap();
        assert_eq!(500, events_in_history.len());
    }

    #[test]
    fn should_correct_bad_order_when_building_history() {
        let matchable_path = MatchablePath::from(vec!["foo", "bar", "file.txt"]);
        let events: Vec<FileEvent> = (0..500)
            .rev()
            .map(|i: u64| {
                FileEvent::new(
                    Uuid::new_v4(),
                    UtcMillis::from(i),
                    matchable_path.clone(),
                    1024 * 1024 * 1024,
                    ChangeEvent,
                    None,
                    WG,
                )
            })
            .collect();

        let history = InMemoryFileHistory::from(events);

        assert_eq!(
            UtcMillis::from(499),
            history
                .get_latest_event(WG, &matchable_path)
                .unwrap()
                .utc_millis
        );
    }

    #[test]
    fn should_isolate_watch_groups() {
        let history = InMemoryFileHistory::from(Vec::new());
        let path = MatchablePath::from(vec!["file.txt"]);

        let e1 = FileEvent::new(
            Uuid::new_v4(),
            UtcMillis::from(100),
            path.clone(),
            1024,
            ChangeEvent,
            None,
            1,
        );
        let e2 = FileEvent::new(
            Uuid::new_v4(),
            UtcMillis::from(200),
            path.clone(),
            2048,
            ChangeEvent,
            None,
            2,
        );

        history.add(e1.clone());
        history.add(e2.clone());

        assert_eq!(Some(e1), history.get_latest_event(1, &path));
        assert_eq!(Some(e2), history.get_latest_event(2, &path));
        assert_eq!(1, history.get_latest_events(1).len());
        assert_eq!(1, history.get_latest_events(2).len());
        assert_eq!(0, history.get_latest_events(99).len());
    }
}
