use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::dtos::LogLineDto;
use shared::endpoint::ServerEndpoint;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{EventSource, MessageEvent};

use crate::api;

/// Cap on how many lines this tab keeps around, independent of the server's own
/// `MAX_LOG_LINES` ring buffer - bounds memory for a tab left open a long time.
const MAX_CLIENT_LINES: usize = 5000;

/// Merges freshly-received lines into the held buffer, de-duplicating by `seq` and
/// re-sorting. The backlog/catch-up fetch and the live stream are independent
/// requests with no ordering guarantee between them, so a plain "append if newer"
/// isn't reliable - this handles either arriving first.
fn merge_lines(current: &mut Vec<LogLineDto>, incoming: Vec<LogLineDto>) {
    for line in incoming {
        if !current.iter().any(|l| l.seq == line.seq) {
            current.push(line);
        }
    }
    current.sort_by_key(|l| l.seq);
    if current.len() > MAX_CLIENT_LINES {
        let excess = current.len() - MAX_CLIENT_LINES;
        current.drain(0..excess);
    }
}

#[component]
pub fn LogsPage() -> impl IntoView {
    let lines = RwSignal::new(Vec::<LogLineDto>::new());
    let error = RwSignal::new(None::<String>);
    let search = RwSignal::new(String::new());
    let streaming = RwSignal::new(true);

    // Backlog: fetched once on mount, merged into `lines` (see `merge_lines` for why
    // this can't just be a plain `.set()` - the live stream below may already have
    // appended lines by the time this resolves).
    spawn_local(async move {
        match api::fetch_logs(MAX_CLIENT_LINES).await {
            Ok(backlog) => lines.update(|current| merge_lines(current, backlog)),
            Err(e) => error.set(Some(e)),
        }
    });

    // Live tail: a dedicated connection, opened only while this page is mounted and
    // `streaming` is on (unlike the app-wide hello/status connection in `crate::sse`,
    // which never closes). Reacting to `streaming` via an Effect - rather than opening
    // the connection once in the component body - means toggling it off and back on
    // reliably closes the old connection (Leptos runs the previous run's `on_cleanup`
    // before re-running the effect) before opening a fresh one.
    Effect::new(move |_| {
        if !streaming.get() {
            return; // no connection while paused - the previous run's on_cleanup already closed it
        }

        // On resume this catches up on everything missed while paused; on the very
        // first run `lines` is still empty (the backlog fetch above hasn't necessarily
        // resolved yet), so there's nothing to catch up on and this is a no-op.
        let last_seq = lines.get_untracked().last().map(|l| l.seq);
        spawn_local(async move {
            if let Some(seq) = last_seq {
                match api::fetch_logs_since(seq).await {
                    Ok(missed) => lines.update(|current| merge_lines(current, missed)),
                    Err(e) => error.set(Some(e)),
                }
            }
        });

        let source = EventSource::new(ServerEndpoint::ApiLogsStream.to_str())
            .expect("failed to open logs SSE connection");

        let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let Some(text) = event.data().as_string() else {
                return;
            };
            match serde_json::from_str::<Vec<LogLineDto>>(&text) {
                Ok(batch) => lines.update(|current| merge_lines(current, batch)),
                Err(err) => leptos::logging::warn!("logs stream: failed to parse batch: {err}"),
            }
        });
        source.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();

        let onerror = Closure::<dyn FnMut()>::new(move || {
            error.set(Some("Live log stream disconnected".to_string()));
        });
        source.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();

        let cleanup_source = source.clone();
        on_cleanup(move || cleanup_source.close());
    });

    let visible = Memo::new(move |_| {
        let query = search.get().to_lowercase();
        lines
            .get()
            .into_iter()
            .filter(|line| {
                query.is_empty()
                    || line.message.to_lowercase().contains(&query)
                    || line.target.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>()
    });

    view! {
        <div class="container">
            <h1>"Server Logs"</h1>

            {move || error.get().map(|e| view! { <div class="message message-error">{e}</div> })}

            <div class="flex gap-2" style="margin-bottom: 1rem;">
                <input
                    type="text"
                    class="form-input"
                    style="flex: 1;"
                    placeholder="Search message or target..."
                    bind:value=search
                />
                <button class="btn" on:click=move |_| streaming.update(|s| *s = !*s)>
                    {move || if streaming.get() { "Pause" } else { "Resume" }}
                </button>
            </div>

            <div class="log-viewer">
                {move || {
                    visible
                        .get()
                        .into_iter()
                        .map(|line| {
                            let level_class = format!("log-line log-line--{}", line.level.as_str());
                            view! {
                                <div class=level_class>
                                    <span class="log-line-timestamp">{line.timestamp.to_string()}</span>
                                    <span class="log-line-level">{line.level.as_str().to_uppercase()}</span>
                                    <span class="log-line-target">{line.target.clone()}</span>
                                    <span class="log-line-message">{line.message.clone()}</span>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}
