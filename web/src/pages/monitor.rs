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
                { label: 'App Memory %', data: rawData.app_mem, borderColor: '#0f3460', backgroundColor: 'rgba(15,52,96,0.1)', tension: 0.3 },
                { label: 'System CPU %', data: rawData.sys_cpu, borderColor: '#00b4d8', backgroundColor: 'rgba(0,180,216,0.1)', tension: 0.3 },
                { label: 'App CPU %', data: rawData.app_cpu, borderColor: '#90be6d', backgroundColor: 'rgba(144,190,109,0.1)', tension: 0.3 }
            ]
        },
        options: {
            responsive: true,
            plugins: { legend: { labels: { color: '#eee' } } },
            scales: {
                x: { type: 'time', time: { displayFormats: { hour: 'HH:mm', minute: 'HH:mm', second: 'HH:mm:ss' } }, ticks: { color: '#aaa' }, grid: { color: '#333' } },
                y: { min: 0, max: 100, ticks: { color: '#aaa' }, grid: { color: '#333' } }
            }
        }
    });
    ctx._chartInstance = chart;
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = renderChart)]
    fn render_chart(canvas_id: &str, data_json: &str);
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
                            // Use request_animation_frame to ensure canvas is in DOM
                            request_animation_frame(move || {
                                render_chart("monitor-chart", &json);
                            });
                            view! {
                                <div class="chart-wrapper">
                                    <canvas id="monitor-chart"></canvas>
                                </div>
                            }.into_any()
                        }
                        Err(e) => view! { <div class="message message-error">"Error: " {e}</div> }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
