use std::collections::HashMap;
use std::sync::Mutex;

use shared::dtos::ServerEventDto;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Channel capacity per connection. Small on purpose - if a client can't keep up,
/// dropping it is preferable to buffering unbounded events in memory.
const CONNECTION_CHANNEL_CAPACITY: usize = 16;

/// In-memory registry of currently open SSE connections to web clients, keyed by a
/// per-connection id. Lets the server push `ServerEventDto`s to one or all of them.
pub(crate) struct EventRegistry {
    connections: Mutex<HashMap<Uuid, mpsc::Sender<ServerEventDto>>>,
}

impl EventRegistry {
    pub(crate) fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }

    /// Registers a new connection and returns its id plus the receiving end of its channel.
    pub(crate) fn register(&self) -> (Uuid, mpsc::Receiver<ServerEventDto>) {
        let id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel(CONNECTION_CHANNEL_CAPACITY);
        self.connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, tx);
        (id, rx)
    }

    /// Removes a connection, e.g. once its SSE stream has ended.
    pub(crate) fn unregister(&self, id: &Uuid) {
        self.connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(id);
    }

    /// Sends an event to one specific connection, if it's still registered.
    pub(crate) fn send_to(&self, id: &Uuid, event: ServerEventDto) {
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

    /// Sends an event to every currently registered connection. Not called anywhere yet -
    /// the natural entry point once there's a real event to fan out to all web clients.
    #[allow(dead_code)]
    pub(crate) fn broadcast(&self, event: ServerEventDto) {
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
