use std::convert::Infallible;

use axum::Json;
use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use shared::dtos::LogLineDto;

use crate::AppState;
use crate::events::ConnectionGuard;
use crate::logs::MAX_LOG_LINES;

#[derive(serde::Deserialize, Default)]
pub struct LogsQuery {
    pub tail: Option<usize>,
    /// When present, returns lines with `seq` greater than this instead of the last
    /// `tail` lines - used to catch up after the live stream was paused and resumed,
    /// rather than re-fetching the whole backlog.
    pub since_seq: Option<u64>,
}

/// GET /api/logs?tail=N or ?since_seq=N - snapshot of captured log lines, either the
/// most recent `tail` (up to `MAX_LOG_LINES`, for the log viewer's initial load) or
/// everything after `since_seq` (for resuming a paused stream with no gaps). Live
/// updates come from `/api/logs/stream`.
pub async fn api_get_logs(
    State(state): State<AppState>,
    Query(q): Query<LogsQuery>,
) -> Json<Vec<LogLineDto>> {
    let lines = match q.since_seq {
        Some(seq) => state.log_buffer.snapshot_since(seq),
        None => state.log_buffer.snapshot(q.tail.unwrap_or(500).min(MAX_LOG_LINES)),
    };
    Json(lines)
}

/// GET /api/logs/stream - SSE stream of live log line batches, flushed periodically
/// (see `logs::flush_pending_periodically`) rather than one message per line.
pub async fn api_logs_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (id, rx) = state.log_events.register();
    let guard = ConnectionGuard::new(state.log_events.clone(), id);

    let stream = futures::stream::unfold((rx, guard), |(mut rx, guard)| async move {
        let batch = rx.recv().await?;
        let sse_event = Event::default()
            .json_data(batch)
            .unwrap_or_else(|_| Event::default());
        Some((Ok(sse_event), (rx, guard)))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
