use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use shared::dtos::LogLineDto;
use shared::log_level::LogLevel;
use shared::utc_millis::UtcMillis;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use crate::sse::SseRegistry;
use std::sync::Arc;

/// Cap on the in-memory backlog. Bounded on purpose - logs are ephemeral, not
/// `data/sqlite.db`-worthy data, so we keep a fixed amount of recent history
/// in memory rather than persisting anything to disk.
pub(crate) const MAX_LOG_LINES: usize = 2000;

/// How often accumulated log lines are flushed to connected web clients as one batch,
/// rather than pushing an SSE message per line (see `flush_pending_periodically`).
const FLUSH_INTERVAL: Duration = Duration::from_millis(400);

struct LogBufferInner {
    /// Recent history, capped at `MAX_LOG_LINES`, oldest evicted first.
    ring: VecDeque<LogLineDto>,
    /// Lines captured since the last flush (SSE stream), drained by `drain_pending`.
    pending: Vec<LogLineDto>,
    next_seq: u64,
}

/// In-memory backlog + staging area for captured log lines. Populated by
/// `LogCaptureLayer::on_event`, read by the `/api/logs` backlog endpoint
/// (`snapshot`) and the periodic flush task (`drain_pending`).
pub(crate) struct LogBuffer(Mutex<LogBufferInner>);

impl LogBuffer {
    pub(crate) fn new() -> Self {
        Self(Mutex::new(LogBufferInner {
            ring: VecDeque::with_capacity(MAX_LOG_LINES),
            pending: Vec::new(),
            next_seq: 0,
        }))
    }

    fn push(&self, level: LogLevel, target: String, message: String) {
        let mut inner = self.inner();
        let line = LogLineDto {
            seq: inner.next_seq,
            timestamp: UtcMillis::now(),
            level,
            target,
            message,
        };
        inner.next_seq += 1;
        if inner.ring.len() >= MAX_LOG_LINES {
            inner.ring.pop_front();
        }
        inner.ring.push_back(line.clone());
        inner.pending.push(line);
    }

    /// Last `tail` lines currently in the backlog, for the `/api/logs` snapshot endpoint.
    pub(crate) fn snapshot(&self, tail: usize) -> Vec<LogLineDto> {
        let inner = self.inner();
        let skip = inner.ring.len().saturating_sub(tail);
        inner.ring.iter().skip(skip).cloned().collect()
    }

    /// All buffered lines with `seq` greater than `since`, for resuming after a pause.
    /// Naturally capped at whatever the ring buffer still holds - if paused long enough
    /// that more than `MAX_LOG_LINES` lines were produced, the oldest of the missed
    /// ones are gone. Same bounded-retention trade-off `snapshot` already has today.
    pub(crate) fn snapshot_since(&self, since: u64) -> Vec<LogLineDto> {
        self.inner().ring.iter().filter(|l| l.seq > since).cloned().collect()
    }

    /// Takes the batch accumulated since the last call, leaving the ring buffer intact.
    fn drain_pending(&self) -> Vec<LogLineDto> {
        std::mem::take(&mut self.inner().pending)
    }

    fn inner(&self) -> MutexGuard<'_, LogBufferInner> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// A `tracing_subscriber::Layer` that captures every event into a `LogBuffer`,
/// alongside (not instead of) the existing stdout `fmt` layer.
///
/// IMPORTANT: nothing reachable from `on_event` - this layer, `LogBuffer`, or the
/// flush task below - may call `tracing::*`. Doing so would re-enter this same
/// layer and recurse. Use `eprintln!` if an internal error ever needs surfacing.
pub(crate) struct LogCaptureLayer {
    buffer: Arc<LogBuffer>,
}

impl LogCaptureLayer {
    pub(crate) fn new(buffer: Arc<LogBuffer>) -> Self {
        Self { buffer }
    }
}

impl<S: Subscriber> Layer<S> for LogCaptureLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        self.buffer.push(
            to_log_level(event.metadata().level()),
            event.metadata().target().to_string(),
            visitor.message,
        );
    }
}

/// `LogLevel` is defined in `shared` with no `tracing` dependency of its own, so this
/// conversion lives here rather than as a `From` impl - implementing `From<&tracing::Level>
/// for LogLevel` from this crate would violate the orphan rule (neither type is local here).
fn to_log_level(level: &tracing::Level) -> LogLevel {
    match *level {
        tracing::Level::TRACE => LogLevel::Trace,
        tracing::Level::DEBUG => LogLevel::Debug,
        tracing::Level::INFO => LogLevel::Info,
        tracing::Level::WARN => LogLevel::Warn,
        tracing::Level::ERROR => LogLevel::Error,
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        }
    }
}

/// Periodically broadcasts whatever log lines accumulated since the last tick to
/// every connected `/api/logs/stream` client, as one batch rather than one message
/// per line - smooths bursts and fits the registry's bounded per-connection channel.
pub(crate) async fn flush_pending_periodically(
    buffer: Arc<LogBuffer>,
    events: Arc<SseRegistry<Vec<LogLineDto>>>,
) {
    let mut interval = tokio::time::interval(FLUSH_INTERVAL);
    loop {
        interval.tick().await;
        let batch = buffer.drain_pending();
        if !batch.is_empty() {
            events.broadcast(batch);
        }
    }
}
