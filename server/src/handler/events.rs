use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use shared::dtos::ServerEventDto;

use crate::AppState;
use crate::sse::SseConnectionGuard;

/// GET /api/events - SSE stream pushing `ServerEventDto`s to web clients.
pub async fn api_events_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (id, rx) = state.events.register();
    // Round-trips through the registry like any future push would, rather than
    // special-casing the first message.
    state.events.send_to(&id, ServerEventDto::ServerHello);

    let guard = SseConnectionGuard::new(state.events.clone(), id);

    let stream = futures::stream::unfold((rx, guard), |(mut rx, guard)| async move {
        let event = rx.recv().await?;
        let sse_event = Event::default()
            .json_data(event)
            .unwrap_or_else(|_| Event::default());
        Some((Ok(sse_event), (rx, guard)))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
