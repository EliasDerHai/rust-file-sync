use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use shared::dtos::ServerEventDto;
use uuid::Uuid;

use crate::AppState;
use crate::events::EventRegistry;

/// Unregisters a connection from the `EventRegistry` once its SSE stream is dropped
/// (client disconnected, tab closed, ...).
struct Cleanup {
    events: Arc<EventRegistry>,
    id: Uuid,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.events.unregister(&self.id);
    }
}

/// GET /api/events - SSE stream pushing `ServerEventDto`s to web clients.
pub async fn api_events_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (id, rx) = state.events.register();
    // Round-trips through the registry like any future push would, rather than
    // special-casing the first message.
    state.events.send_to(&id, ServerEventDto::ServerHello);

    let cleanup = Cleanup {
        events: state.events.clone(),
        id,
    };

    let stream = futures::stream::unfold((rx, cleanup), |(mut rx, cleanup)| async move {
        let event = rx.recv().await?;
        let sse_event = Event::default()
            .json_data(event)
            .unwrap_or_else(|_| Event::default());
        Some((Ok(sse_event), (rx, cleanup)))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
