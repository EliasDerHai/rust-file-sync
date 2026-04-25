use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::api;
use crate::components::Loading;

#[wasm_bindgen(inline_js = r#"
export function renderChart(canvasId, dataJson) {
    const rawData = JSON.parse(dataJson);
    const ctx = document.getElementById(canvasId);
    if (!ctx) return;
    if (ctx._chartInstance) ctx._chartInstance.destroy();
    const chart = new Chart(ctx, {
        type: 'line',
        data: {
            datasets: [
                { label: 'System Memory %', data: rawData.sys_mem, borderColor: '#e94560', backgroundColor: 'rgba(233,69,96,0.1)', tension: 0.3 },
                { label: 'App Memory %',    data: rawData.app_mem, borderColor: '#0f3460', backgroundColor: 'rgba(15,52,96,0.1)',  tension: 0.3 },
                { label: 'System CPU %',    data: rawData.sys_cpu, borderColor: '#00b4d8', backgroundColor: 'rgba(0,180,216,0.1)', tension: 0.3 },
                { label: 'App CPU %',       data: rawData.app_cpu, borderColor: '#90be6d', backgroundColor: 'rgba(144,190,109,0.1)', tension: 0.3 },
                { label: 'Disk Used %',     data: rawData.disk_used, borderColor: '#f4a261', backgroundColor: 'rgba(244,162,97,0.1)', tension: 0.3 }
            ]
        },
        options: {
            responsive: true,
            plugins: {
                legend: { labels: { color: '#eee' } },
                zoom: {
                    zoom:   { wheel: { enabled: true }, pinch: { enabled: true }, mode: 'x' },
                    pan:    { enabled: true, mode: 'x' },
                    limits: { x: { min: 'original', max: 'original' } }
                }
            },
            scales: {
                x: { type: 'time', time: { displayFormats: { hour: 'HH:mm', minute: 'HH:mm', second: 'HH:mm:ss' } }, ticks: { color: '#aaa' }, grid: { color: '#333' } },
                y: { min: 0, max: 100, ticks: { color: '#aaa' }, grid: { color: '#333' } }
            }
        }
    });
    ctx._chartInstance = chart;
}

export function renderDiskFreeChart(canvasId, dataJson) {
    const rawData = JSON.parse(dataJson);
    const ctx = document.getElementById(canvasId);
    if (!ctx) return;
    if (ctx._chartInstance) ctx._chartInstance.destroy();
    const chart = new Chart(ctx, {
        type: 'line',
        data: {
            datasets: [
                { label: 'Disk Free GiB', data: rawData.disk_free, borderColor: '#f4a261', backgroundColor: 'rgba(244,162,97,0.1)', tension: 0.3 }
            ]
        },
        options: {
            responsive: true,
            plugins: {
                legend: { labels: { color: '#eee' } },
                zoom: {
                    zoom:   { wheel: { enabled: true }, pinch: { enabled: true }, mode: 'x' },
                    pan:    { enabled: true, mode: 'x' },
                    limits: { x: { min: 'original', max: 'original' } }
                }
            },
            scales: {
                x: { type: 'time', time: { displayFormats: { hour: 'HH:mm', minute: 'HH:mm', second: 'HH:mm:ss' } }, ticks: { color: '#aaa' }, grid: { color: '#333' } },
                y: { min: 0, ticks: { color: '#aaa', callback: v => v + ' GiB' }, grid: { color: '#333' } }
            }
        }
    });
    ctx._chartInstance = chart;
}

export function resetChartZoom(canvasId) {
    const ctx = document.getElementById(canvasId);
    if (ctx && ctx._chartInstance) ctx._chartInstance.resetZoom();
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = renderChart)]
    fn render_chart(canvas_id: &str, data_json: &str);

    #[wasm_bindgen(js_name = renderDiskFreeChart)]
    fn render_disk_free_chart(canvas_id: &str, data_json: &str);

    #[wasm_bindgen(js_name = resetChartZoom)]
    fn reset_chart_zoom(canvas_id: &str);
}

#[component]
pub fn MonitorPage() -> impl IntoView {
    let monitor_data = LocalResource::new(api::fetch_monitor_data);

    view! {
        <div class="container">
            <h1>"System Monitor"</h1>
            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    match monitor_data.await {
                        Ok(data) => {
                            let json = serde_json::to_string(&data).unwrap_or_default();
                            let json2 = json.clone();
                            request_animation_frame(move || {
                                render_chart("monitor-chart", &json);
                                render_disk_free_chart("disk-free-chart", &json2);
                            });
                            view! {
                                <div class="chart-wrapper">
                                    <canvas id="monitor-chart"></canvas>
                                </div>
                                <button
                                    class="btn"
                                    on:click=|_| reset_chart_zoom("monitor-chart")
                                >"Reset Zoom"</button>
                                <h2>"Disk Free Space"</h2>
                                <div class="chart-wrapper">
                                    <canvas id="disk-free-chart"></canvas>
                                </div>
                                <button
                                    class="btn"
                                    on:click=|_| reset_chart_zoom("disk-free-chart")
                                >"Reset Zoom"</button>
                            }.into_any()
                        }
                        Err(e) => view! { <div class="message message-error">"Error: " {e}</div> }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
