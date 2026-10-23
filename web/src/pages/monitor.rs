use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;

use crate::api;
use crate::components::{Message, ToastSignal};

// Four Chart.js charts sharing one time window. Every stat metric is drawn as an avg
// line plus a translucent min-max band (a `max` dataset filled down to a `min`
// dataset), so short spikes stay visible however coarse the buckets get. Zooming or
// panning syncs the window across all charts and, debounced, asks Rust to refetch
// that window at a matching resolution (`onRange`).
#[wasm_bindgen(module = "/src/pages/monitor.js")]
extern "C" {
    #[wasm_bindgen(js_name = initMonitorCharts)]
    fn init_monitor_charts(on_range: &Closure<dyn FnMut(f64, f64)>);

    #[wasm_bindgen(js_name = updateMonitorCharts)]
    fn update_monitor_charts(
        series_json: &str,
        view_min: f64,
        view_max: f64,
        limit_min: f64,
        limit_max: f64,
    );

    #[wasm_bindgen(js_name = destroyMonitorCharts)]
    fn destroy_monitor_charts();

    #[wasm_bindgen(js_name = monitorChartWidth)]
    fn monitor_chart_width() -> f64;
}

const HOUR_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * HOUR_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonitorRange {
    Minutes,
    Hour,
    Day,
    Week,
    Month,
    Year,
    All,
}

impl MonitorRange {
    fn as_key(self) -> &'static str {
        match self {
            MonitorRange::Minutes => "10min",
            MonitorRange::Hour => "1h",
            MonitorRange::Day => "1d",
            MonitorRange::Week => "7d",
            MonitorRange::Month => "30d",
            MonitorRange::Year => "1y",
            MonitorRange::All => "all",
        }
    }

    /// `None` = everything since the oldest stored data
    fn span_ms(self) -> Option<i64> {
        match self {
            MonitorRange::Minutes => Some(10 * 60 * 1000),
            MonitorRange::Hour => Some(HOUR_MS),
            MonitorRange::Day => Some(DAY_MS),
            MonitorRange::Week => Some(7 * DAY_MS),
            MonitorRange::Month => Some(30 * DAY_MS),
            MonitorRange::Year => Some(365 * DAY_MS),
            MonitorRange::All => None,
        }
    }
}

impl From<&str> for MonitorRange {
    fn from(key: &str) -> Self {
        match key {
            "10min" => MonitorRange::Minutes,
            "1h" => MonitorRange::Hour,
            "1d" => MonitorRange::Day,
            "7d" => MonitorRange::Week,
            "30d" => MonitorRange::Month,
            "1y" => MonitorRange::Year,
            "all" => MonitorRange::All,
            _ => MonitorRange::Day,
        }
    }
}

fn now_ms() -> i64 {
    js_sys::Date::now() as i64
}

#[component]
pub fn MonitorPage() -> impl IntoView {
    let msg = ToastSignal::new();
    let range = RwSignal::new(MonitorRange::Day);
    let earliest = RwSignal::new(None::<i64>);
    let charts_ready = RwSignal::new(false);
    let loading = RwSignal::new(false);
    // only the newest request may draw - a slow wide fetch must not overwrite a newer zoom
    let latest_request = StoredValue::new(0u32);

    // fetch `[from, to)` at one bucket per pixel and show `[view_min, view_max]` of it
    let load = move |from: i64, to: i64, view_min: i64, view_max: i64| {
        let request = latest_request.get_value() + 1;
        latest_request.set_value(request);
        loading.set(true);
        spawn_local(async move {
            let points = monitor_chart_width().max(1.0) as u32;
            let result = api::fetch_monitor(from, to, points).await;
            if latest_request.get_value() != request {
                return;
            }
            loading.set(false);
            match result {
                Ok(series) => {
                    earliest.set(series.earliest_ms);
                    let json = serde_json::to_string(&series).unwrap_or_default();
                    let limit_min = series.earliest_ms.unwrap_or(view_min).min(view_min);
                    update_monitor_charts(
                        &json,
                        view_min as f64,
                        view_max as f64,
                        limit_min as f64,
                        now_ms() as f64,
                    );
                }
                Err(e) => msg.error(format!("Failed to load monitoring data: {e}")),
            }
        });
    };

    let load_range = move |r: MonitorRange| {
        let now = now_ms();
        let from = match r.span_ms() {
            Some(span) => now - span,
            None => earliest.get_untracked().unwrap_or(now - 365 * DAY_MS),
        };
        load(from, now, from, now);
    };

    // create the charts once their canvases exist; destroyed again on unmount
    Effect::new(move |_| {
        request_animation_frame(move || {
            let on_range = Closure::<dyn FnMut(f64, f64)>::new(move |min: f64, max: f64| {
                let (min, max) = (min as i64, max as i64);
                let pad = (max - min) / 2;
                load(min - pad, max + pad, min, max);
            });
            init_monitor_charts(&on_range);
            // the charts are destroyed (and their debounce timer cleared) on unmount,
            // so the closure is never called afterwards
            on_range.forget();
            charts_ready.set(true);
        });
        on_cleanup(destroy_monitor_charts);
    });

    Effect::new(move |_| {
        let r = range.get();
        if charts_ready.get() {
            load_range(r);
        }
    });

    view! {
        <div class="container">
            <h1>"System Monitor"</h1>
            <Message signal=msg />
            <div class="flex gap-2 monitor-toolbar">
                <select
                    class="btn btn-secondary"
                    prop:value=move || range.get().as_key()
                    on:change=move |ev| range.set(MonitorRange::from(event_target_value(&ev).as_str()))
                >
                    <option value="10min">"Last 10 min"</option>
                    <option value="1h">"Last hour"</option>
                    <option value="1d">"Last 24 hours"</option>
                    <option value="7d">"Last 7 days"</option>
                    <option value="30d">"Last 30 days"</option>
                    <option value="1y">"Last year"</option>
                    <option value="all">"All time"</option>
                </select>
                <button class="btn" on:click=move |_| load_range(range.get_untracked())>
                    "Reset zoom"
                </button>
                <Show when=move || loading.get()>
                    <span class="monitor-loading">"Loading…"</span>
                </Show>
            </div>
            <p class="monitor-hint">
                "Lines show averages, shaded bands the min–max range per bucket. Scroll or pinch to zoom, drag to pan - detail is refetched for the visible window."
            </p>
            <h2>"Usage"</h2>
            <div class="chart-wrapper">
                <canvas id="monitor-usage"></canvas>
            </div>
            <h2>"Temperature & load"</h2>
            <div class="chart-wrapper">
                <canvas id="monitor-thermal"></canvas>
            </div>
            <h2>"Network"</h2>
            <div class="chart-wrapper">
                <canvas id="monitor-network"></canvas>
            </div>
            <h2>"Disk"</h2>
            <div class="chart-wrapper">
                <canvas id="monitor-disk"></canvas>
            </div>
        </div>
    }
}
