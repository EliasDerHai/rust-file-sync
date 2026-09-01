use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use uuid::Uuid;

/// Channel capacity per connection. Small on purpose - if a client can't keep up,
/// dropping it is preferable to buffering unbounded events in memory.
const CONNECTION_CHANNEL_CAPACITY: usize = 16;

/// Identifies one open SSE connection registered with an `SseRegistry`. Not a "web
/// client" id - a single browser tab can hold more than one SSE connection at once
/// (e.g. the app-wide hello stream and the page-scoped logs stream), each getting
/// its own `SseSubscriberId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SseSubscriberId(Uuid);

/// In-memory registry of currently open SSE connections to web clients, keyed by a
/// per-connection id. Lets the server push `T`s to one or all of them. Generic so
/// unrelated push channels (e.g. the app-wide hello/status stream vs. the live logs
/// stream) each get their own registry instance without duplicating this logic.
pub(crate) struct SseRegistry<T> {
    connections: Mutex<HashMap<SseSubscriberId, mpsc::Sender<T>>>,
}

impl<T: Clone> SseRegistry<T> {
    pub(crate) fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }

    /// Registers a new connection and returns its id plus the receiving end of its channel.
    pub(crate) fn register(&self) -> (SseSubscriberId, mpsc::Receiver<T>) {
        let id = SseSubscriberId(Uuid::new_v4());
        let (tx, rx) = mpsc::channel(CONNECTION_CHANNEL_CAPACITY);
        self.connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, tx);
        (id, rx)
    }

    /// Removes a connection, e.g. once its SSE stream has ended.
    pub(crate) fn unregister(&self, id: &SseSubscriberId) {
        self.connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(id);
    }

    /// Sends an event to one specific connection, if it's still registered.
    pub(crate) fn send_to(&self, id: &SseSubscriberId, event: T) {
        let sender = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(id)
            .cloned();
        if let Some(sender) = sender {
            let _ = sender.try_send(event);
        }
    }

    /// Sends an event to every currently registered connection.
    pub(crate) fn broadcast(&self, event: T) {
        let senders: Vec<_> = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .cloned()
            .collect();
        for sender in senders {
            let _ = sender.try_send(event.clone());
        }
    }
}

/// Unregisters a connection from an `SseRegistry` once its SSE stream is dropped
/// (client disconnected, tab closed, navigated away, ...).
pub(crate) struct SseConnectionGuard<T: Clone> {
    registry: Arc<SseRegistry<T>>,
    id: SseSubscriberId,
}

impl<T: Clone> SseConnectionGuard<T> {
    pub(crate) fn new(registry: Arc<SseRegistry<T>>, id: SseSubscriberId) -> Self {
        Self { registry, id }
    }
}

impl<T: Clone> Drop for SseConnectionGuard<T> {
    fn drop(&mut self) {
        self.registry.unregister(&self.id);
    }
}
