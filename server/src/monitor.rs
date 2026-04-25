use crate::write::RotatingFileWriter;
use axum::response::IntoResponse;
use std::sync::{Arc, Mutex};
use sysinfo::{Disks, System};
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
            .map(|self_process| self_process.memory() as f32 / total_sys_mem * 100f32)
            .unwrap_or(0.0);
        let used_sys_cpu_percentage = system.global_cpu_usage();
        let used_own_cpu_percentage = system
            .process(pid)
            .map(|self_process| self_process.cpu_usage())
            .unwrap_or(0.0);

        let disks = Disks::new_with_refreshed_list();
        let root_disk = disks
            .list()
            .iter()
            .find(|d| d.mount_point() == std::path::Path::new("/"));
        let (disk_used_pct, disk_free_gib) = root_disk
            .map(|d| {
                let total = d.total_space() as f32;
                let available = d.available_space() as f32;
                ((total - available) / total * 100.0, available / (1024.0_f32.powi(3)))
            })
            .unwrap_or((0.0, 0.0));

        trace!(
            "\t
            Total used mem: {}%\t
            App used mem: {}%\t
            Total used cpu: {}%\t
            App used cpu: {}%\t
            Disk used: {}%\t
            Disk free: {:.2} GiB",
            used_sys_mem_percentage,
            used_own_mem_percentage,
            used_sys_cpu_percentage,
            used_own_cpu_percentage,
            disk_used_pct,
            disk_free_gib,
        );
        let csv_line = format!(
            "{};{};{};{};{};{};{}",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
            used_sys_mem_percentage,
            used_own_mem_percentage,
            used_sys_cpu_percentage,
            used_own_cpu_percentage,
            disk_used_pct,
            disk_free_gib,
        );
        if let Err(e) = writer.lock().unwrap().write_line(&csv_line) {
            error!("Failed to write monitoring data: {}", e);
        }
        tokio::time::sleep(backoff).await;
    }
}

/// GET /api/monitor - JSON monitoring data
pub async fn api_get_monitoring(writer: Arc<Mutex<RotatingFileWriter>>) -> impl IntoResponse {
    let csv_content = match writer.lock().unwrap().read_current_file() {
        Ok(content) => content,
        Err(err) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error reading monitoring data: {}", err),
            )
                .into_response();
        }
    };
    let data_json = csv_to_json(&csv_content);
    ([("content-type", "application/json")], data_json).into_response()
}

fn csv_to_json(csv: &str) -> String {
    let mut sys_mem = Vec::new();
    let mut app_mem = Vec::new();
    let mut sys_cpu = Vec::new();
    let mut app_cpu = Vec::new();
    let mut disk_used = Vec::new();
    let mut disk_free = Vec::new();

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
            let disk_used_val = parts.get(5).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            let disk_free_val = parts.get(6).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);

            sys_mem.push(format!(r#"{{"x":"{}","y":{}}}"#, timestamp, sys_mem_val));
            app_mem.push(format!(r#"{{"x":"{}","y":{}}}"#, timestamp, app_mem_val));
            sys_cpu.push(format!(r#"{{"x":"{}","y":{}}}"#, timestamp, sys_cpu_val));
            app_cpu.push(format!(r#"{{"x":"{}","y":{}}}"#, timestamp, app_cpu_val));
            disk_used.push(format!(r#"{{"x":"{}","y":{}}}"#, timestamp, disk_used_val));
            disk_free.push(format!(r#"{{"x":"{}","y":{:.2}}}"#, timestamp, disk_free_val));
        }
    }

    format!(
        r#"{{"sys_mem":[{}],"app_mem":[{}],"sys_cpu":[{}],"app_cpu":[{}],"disk_used":[{}],"disk_free":[{}]}}"#,
        sys_mem.join(","),
        app_mem.join(","),
        sys_cpu.join(","),
        app_cpu.join(","),
        disk_used.join(","),
        disk_free.join(","),
    )
}
