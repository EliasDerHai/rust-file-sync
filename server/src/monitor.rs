use crate::write::RotatingFileWriter;
use axum::response::{Html, IntoResponse};
use indoc::formatdoc;
use std::sync::{Arc, Mutex};
use sysinfo::System;
use tracing::{error, trace};

const BACKOFF_MS: u64 = 10_000;

pub async fn monitor_sys(writer: Arc<Mutex<RotatingFileWriter>>) {
    let mut system = System::new_all();
    let pid = sysinfo::get_current_pid().expect("Failed to get current PID");
    system.refresh_memory();
    system.refresh_cpu_usage();
    let total_sys_mem = system.total_memory() as f32;
    let backoff = tokio::time::Duration::from_millis(BACKOFF_MS);

    loop {
        system.refresh_all();
        let used_sys_mem_percentage = system.used_memory() as f32 / total_sys_mem * 100f32;
        let used_own_mem_percentage = system
            .process(pid)
            .map(|self_process| self_process.memory() as f32 / total_sys_mem * 100f32);
        let used_sys_cpu_percentage = system.global_cpu_usage();
        let used_own_cpu_percentage = system
            .process(pid)
            .map(|self_process| self_process.cpu_usage());
        trace!(
            "\t
            Total used mem: {}%\t
            App used mem: {}%\t
            Total used cpu: {}%\t
            App used cpu: {}%",
            used_sys_mem_percentage,
            used_own_mem_percentage
                .map(|f| f.to_string())
                .unwrap_or(String::from("unknown")),
            used_sys_cpu_percentage,
            used_own_cpu_percentage
                .map(|f| f.to_string())
                .unwrap_or(String::from("unknown"))
        );
        let x = [
            used_sys_mem_percentage,
            used_own_mem_percentage.unwrap(),
            used_sys_cpu_percentage,
            used_own_cpu_percentage.unwrap(),
        ];
        let csv_line = format!(
            "{};{}",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
            x.iter()
                .map(|f| f.to_string())
                .collect::<Vec<String>>()
                .join(";")
        );
        if let Err(e) = writer.lock().unwrap().write_line(&csv_line) {
            error!("Failed to write monitoring data: {}", e);
        }
        tokio::time::sleep(backoff).await;
    }
}

/// chartjs rendered data
pub async fn get_monitoring(writer: Arc<Mutex<RotatingFileWriter>>) -> impl IntoResponse {
    let csv_content = match writer.lock().unwrap().read_current_file() {
        Ok(content) => content,
        Err(err) => {
            return Html(format!(
                "<html><body><h1>Error reading monitoring data: {}</h1></body></html>",
                err
            ))
        }
    };

    let data_json = csv_to_json(&csv_content);

    Html(formatdoc! {
        r#"<!DOCTYPE html>
                <html>
                <head>
                    <title>System Monitor</title>
                    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
                    <script src="https://cdn.jsdelivr.net/npm/chartjs-adapter-date-fns"></script>
                    <style>
                        body {{ font-family: sans-serif; margin: 20px; background: #1a1a2e; color: #eee; }}
                        .chart-container {{ max-width: 1200px; margin: 0 auto; }}
                        canvas {{ background: #16213e; border-radius: 8px; }}
                        h1 {{ text-align: center; color: #e94560; }}
                    </style>
                </head>
                <body>
                    <h1>System Monitor</h1>
                    <div class="chart-container">
                        <canvas id="chart"></canvas>
                    </div>
                    <script>
                        const rawData = {data_json};
                
                        new Chart(document.getElementById('chart'), {{
                            type: 'line',
                            data: {{
                                datasets: [
                                    {{
                                        label: 'System Memory %',
                                        data: rawData.sys_mem,
                                        borderColor: '#e94560',
                                        backgroundColor: 'rgba(233, 69, 96, 0.1)',
                                        tension: 0.3
                                    }},
                                    {{
                                        label: 'App Memory %',
                                        data: rawData.app_mem,
                                        borderColor: '#0f3460',
                                        backgroundColor: 'rgba(15, 52, 96, 0.1)',
                                        tension: 0.3
                                    }},
                                    {{
                                        label: 'System CPU %',
                                        data: rawData.sys_cpu,
                                        borderColor: '#00b4d8',
                                        backgroundColor: 'rgba(0, 180, 216, 0.1)',
                                        tension: 0.3
                                    }},
                                    {{
                                        label: 'App CPU %',
                                        data: rawData.app_cpu,
                                        borderColor: '#90be6d',
                                        backgroundColor: 'rgba(144, 190, 109, 0.1)',
                                        tension: 0.3
                                    }}
                                ]
                            }},
                            options: {{
                                responsive: true,
                                plugins: {{
                                    legend: {{ labels: {{ color: '#eee' }} }}
                                }},
                                scales: {{
                                    x: {{
                                        type: 'time',
                                        time: {{
                                            displayFormats: {{ hour: 'HH:mm', minute: 'HH:mm', second: 'HH:mm:ss' }}
                                        }},
                                        ticks: {{ color: '#aaa' }},
                                        grid: {{ color: '#333' }}
                                    }},
                                    y: {{
                                        min: 0,
                                        max: 100,
                                        ticks: {{ color: '#aaa' }},
                                        grid: {{ color: '#333' }}
                                    }}
                                }}
                            }}
                        }});
                    </script>
                </body>
                </html>"#,
        data_json = data_json
    })
}

fn csv_to_json(csv: &str) -> String {
    let mut sys_mem = Vec::new();
    let mut app_mem = Vec::new();
    let mut sys_cpu = Vec::new();
    let mut app_cpu = Vec::new();

    for (i, line) in csv.lines().enumerate() {
        if i == 0 {
            continue; // skip header
        }
        let parts: Vec<&str> = line.split(';').collect();
        if parts.len() >= 5 {
            let timestamp = parts[0];
            let sys_mem_val = parts[1].parse::<f32>().unwrap_or(0.0);
            let app_mem_val = parts[2].parse::<f32>().unwrap_or(0.0);
            let sys_cpu_val = parts[3].parse::<f32>().unwrap_or(0.0);
            let app_cpu_val = parts[4].parse::<f32>().unwrap_or(0.0);

            sys_mem.push(format!(r#"{{"x":"{}","y":{}}}"#, timestamp, sys_mem_val));
            app_mem.push(format!(r#"{{"x":"{}","y":{}}}"#, timestamp, app_mem_val));
            sys_cpu.push(format!(r#"{{"x":"{}","y":{}}}"#, timestamp, sys_cpu_val));
            app_cpu.push(format!(r#"{{"x":"{}","y":{}}}"#, timestamp, app_cpu_val));
        }
    }

    format!(
        r#"{{"sys_mem":[{}],"app_mem":[{}],"sys_cpu":[{}],"app_cpu":[{}]}}"#,
        sys_mem.join(","),
        app_mem.join(","),
        sys_cpu.join(","),
        app_cpu.join(",")
    )
}
