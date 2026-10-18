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
#[wasm_bindgen(inline_js = r#"
const CHARTS = [
    { id: 'monitor-usage', axes: { y: { min: 0, suggestedMax: 100, fmt: v => v.toFixed(1) + ' %' } }, series: [
        { key: 'sys_cpu',   label: 'System CPU',    color: '#00b4d8' },
        { key: 'app_cpu',   label: 'App CPU',       color: '#90be6d' },
        { key: 'sys_mem',   label: 'System memory', color: '#e94560' },
        { key: 'app_mem',   label: 'App memory',    color: '#b388ff' },
        { key: 'swap',      label: 'Swap',          color: '#ffd166' },
        { key: 'disk_used', label: 'Disk used',     color: '#f4a261', plain: true },
    ] },
    { id: 'monitor-thermal', axes: {
        y:  { fmt: v => v.toFixed(1) + ' °C' },
        y1: { min: 0, position: 'right', fmt: v => v.toFixed(2) },
    }, series: [
        { key: 'cpu_temp', label: 'CPU temperature', color: '#ff6b6b' },
        { key: 'load1',    label: 'Load (1 min)',    color: '#4cc9f0', axis: 'y1' },
    ] },
    { id: 'monitor-network', axes: { y: { min: 0, fmt: fmtRate } }, series: [
        { key: 'net_rx', label: 'Received', color: '#06d6a0' },
        { key: 'net_tx', label: 'Sent',     color: '#ef476f' },
    ] },
    { id: 'monitor-disk', axes: {
        y:  { fmt: v => v.toFixed(1) + ' GiB' },
        y1: { min: 0, position: 'right', fmt: fmtRate },
    }, series: [
        { key: 'disk_free_gib', label: 'Disk free', color: '#f4a261', plain: true },
        { key: 'app_read',      label: 'App read',  color: '#118ab2', axis: 'y1' },
        { key: 'app_write',     label: 'App write', color: '#e76f51', axis: 'y1' },
    ] },
];

const _charts = {};
let _debounce = null;

function fmtRate(v) {
    const units = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
    let i = 0;
    while (Math.abs(v) >= 1024 && i < units.length - 1) { v /= 1024; i++; }
    return v.toFixed(i === 0 ? 0 : 1) + ' ' + units[i];
}

// a bucket with gaps on both sides draws no line segment - give it a dot instead
function isolatedDot(ctx) {
    const d = ctx.dataset.data, i = ctx.dataIndex;
    const missing = p => !p || p.y === null;
    return !missing(d[i]) && missing(d[i - 1]) && missing(d[i + 1]) ? 2 : 0;
}

function hexAlpha(hex, alpha) {
    const n = parseInt(hex.slice(1), 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${alpha})`;
}

// datasets per series: [max, min, avg] for stat metrics (band = max filled to min),
// [value] for plain ones. `_group` ties a band to its line for legend toggling.
function datasetsFor(cfg) {
    const out = [];
    for (const s of cfg.series) {
        const axis = s.axis || 'y';
        const common = { yAxisID: axis, pointRadius: 0, borderWidth: 1.5, tension: 0.2, spanGaps: false, _group: s.key };
        if (!s.plain) {
            out.push({ ...common, label: s.label + ' max', data: [], borderWidth: 0, fill: '+1', backgroundColor: hexAlpha(s.color, 0.15), _band: 'max' });
            out.push({ ...common, label: s.label + ' min', data: [], borderWidth: 0, fill: false, _band: 'min' });
        }
        out.push({ ...common, label: s.label, data: [], borderColor: s.color, backgroundColor: s.color, fill: false, pointRadius: isolatedDot, _fmt: cfg.axes[axis].fmt });
    }
    return out;
}

function syncWindow(source, min, max) {
    for (const chart of Object.values(_charts)) {
        if (chart === source) continue;
        chart.options.scales.x.min = min;
        chart.options.scales.x.max = max;
        chart.update('none');
    }
}

export function initMonitorCharts(onRange) {
    destroyMonitorCharts();
    const onWindowChange = ({ chart }) => {
        const { min, max } = chart.scales.x;
        syncWindow(chart, min, max);
        clearTimeout(_debounce);
        _debounce = setTimeout(() => onRange(min, max), 300);
    };
    // phones: less chrome around the plot (the secondary axis is dropped, tooltips still show its units)
    const narrow = window.matchMedia('(max-width: 640px)').matches;
    const tickFont = { size: narrow ? 10 : 12 };
    for (const cfg of CHARTS) {
        const canvas = document.getElementById(cfg.id);
        if (!canvas) continue;
        const scales = {
            x: { type: 'time', time: { displayFormats: { hour: 'HH:mm', minute: 'HH:mm', second: 'HH:mm:ss' } }, ticks: { color: '#aaa', maxRotation: 0, font: tickFont, maxTicksLimit: narrow ? 4 : undefined }, grid: { color: '#333' } },
        };
        for (const [id, a] of Object.entries(cfg.axes)) {
            scales[id] = { min: a.min, suggestedMax: a.suggestedMax, position: a.position || 'left', display: !(narrow && id !== 'y'), ticks: { color: '#aaa', callback: a.fmt, font: tickFont, maxTicksLimit: narrow ? 5 : undefined }, grid: { color: id === 'y' ? '#333' : 'transparent' } };
        }
        _charts[cfg.id] = new Chart(canvas, {
            type: 'line',
            data: { datasets: datasetsFor(cfg) },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                animation: false,
                parsing: false,
                normalized: true,
                interaction: { mode: 'index', intersect: false },
                scales,
                plugins: {
                    legend: {
                        position: narrow ? 'bottom' : 'top',
                        labels: { color: '#eee', boxWidth: narrow ? 12 : 40, font: { size: narrow ? 11 : 12 }, filter: (item, data) => !data.datasets[item.datasetIndex]._band },
                        onClick: (e, item, legend) => {
                            const chart = legend.chart;
                            const group = chart.data.datasets[item.datasetIndex]._group;
                            const visible = chart.isDatasetVisible(item.datasetIndex);
                            chart.data.datasets.forEach((d, i) => { if (d._group === group) chart.setDatasetVisibility(i, !visible); });
                            chart.update('none');
                        },
                    },
                    tooltip: {
                        filter: item => !item.dataset._band,
                        callbacks: {
                            title: items => items.length ? new Date(items[0].parsed.x).toLocaleString() : '',
                            label: ctx => {
                                const fmt = ctx.dataset._fmt || (v => v);
                                const r = ctx.raw;
                                if (r.y === null) return null;
                                const range = r.min !== undefined && r.min !== r.max ? ` (${fmt(r.min)} – ${fmt(r.max)})` : '';
                                return `${ctx.dataset.label}: ${fmt(r.y)}${range}`;
                            },
                        },
                    },
                    zoom: {
                        zoom: { wheel: { enabled: true }, pinch: { enabled: true }, mode: 'x', onZoomComplete: onWindowChange },
                        pan: { enabled: true, mode: 'x', onPanComplete: onWindowChange },
                        limits: { x: { minRange: 60 * 1000 } },
                    },
                },
            },
        });
    }
}

// `seriesJson`: MonitorSeriesDto. Shows [viewMin, viewMax] (the data may cover more,
// so panning has something to show before the refetch lands) and limits zoom/pan to
// [limitMin, limitMax].
export function updateMonitorCharts(seriesJson, viewMin, viewMax, limitMin, limitMax) {
    const series = JSON.parse(seriesJson);
    const gap = series.resolution_ms * 2;
    for (const cfg of CHARTS) {
        const chart = _charts[cfg.id];
        if (!chart) continue;
        let i = 0;
        for (const s of cfg.series) {
            const max = [], min = [], avg = [];
            let prevT = null;
            for (const p of series.points) {
                // a missing bucket means the server was down: break the line
                if (prevT !== null && p.t - prevT > gap) {
                    const x = prevT + series.resolution_ms;
                    max.push({ x, y: null }); min.push({ x, y: null }); avg.push({ x, y: null });
                }
                prevT = p.t;
                const v = p[s.key];
                if (s.plain) {
                    avg.push({ x: p.t, y: v ?? null });
                } else {
                    max.push({ x: p.t, y: v ? v.max : null });
                    min.push({ x: p.t, y: v ? v.min : null });
                    avg.push(v ? { x: p.t, y: v.avg, min: v.min, max: v.max } : { x: p.t, y: null });
                }
            }
            if (!s.plain) {
                chart.data.datasets[i++].data = max;
                chart.data.datasets[i++].data = min;
            }
            chart.data.datasets[i++].data = avg;
        }
        chart.options.scales.x.min = viewMin;
        chart.options.scales.x.max = viewMax;
        chart.options.plugins.zoom.limits.x.min = limitMin;
        chart.options.plugins.zoom.limits.x.max = limitMax;
        chart.update('none');
    }
}

export function destroyMonitorCharts() {
    clearTimeout(_debounce);
    for (const [id, chart] of Object.entries(_charts)) {
        chart.destroy();
        delete _charts[id];
    }
}

// plot width in px - one bucket per pixel is as much detail as can be shown
export function monitorChartWidth() {
    const canvas = document.getElementById(CHARTS[0].id);
    return canvas ? canvas.clientWidth : 800;
}
"#)]
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
    Day,
    Week,
    Month,
    Year,
    All,
}

impl MonitorRange {
    fn as_key(self) -> &'static str {
        match self {
            MonitorRange::Day => "24h",
            MonitorRange::Week => "7d",
            MonitorRange::Month => "30d",
            MonitorRange::Year => "1y",
            MonitorRange::All => "all",
        }
    }

    /// `None` = everything since the oldest stored data
    fn span_ms(self) -> Option<i64> {
        match self {
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
                    <option value="24h">"Last 24 hours"</option>
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
