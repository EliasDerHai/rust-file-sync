use crate::write::RotatingFileWriter;
use axum::response::IntoResponse;
use std::sync::{Arc, Mutex};
use sysinfo::{Disks, System};
use tracing::{error, trace};

const BACKOFF_MS: u64 = 10_000;
pub const DEFAULT_MAX_POINTS: usize = 300;

#[derive(serde::Deserialize, Default)]
pub struct MonitorQuery {
    pub points: Option<usize>,
}

struct CsvRow {
    timestamp: String,
    sys_mem: f32,
    app_mem: f32,
    sys_cpu: f32,
    app_cpu: f32,
    disk_used: f32,
    disk_free: f32,
}

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

/// GET /api/monitor?points=N - JSON monitoring data, downsampled to N points (default 300)
pub async fn api_get_monitoring(
    writer: Arc<Mutex<RotatingFileWriter>>,
    max_points: usize,
) -> impl IntoResponse {
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
    let data_json = csv_to_json(&csv_content, max_points);
    ([("content-type", "application/json")], data_json).into_response()
}

fn parse_csv_rows(csv: &str) -> Vec<CsvRow> {
    csv.lines()
        .enumerate()
        .filter(|(i, _)| *i != 0)
        .filter_map(|(_, line)| {
            let parts: Vec<&str> = line.split(';').collect();
            if parts.len() < 5 {
                return None;
            }
            Some(CsvRow {
                timestamp: parts[0].to_string(),
                sys_mem: parts[1].parse().unwrap_or(0.0),
                app_mem: parts[2].parse().unwrap_or(0.0),
                sys_cpu: parts[3].parse().unwrap_or(0.0),
                app_cpu: parts[4].parse().unwrap_or(0.0),
                disk_used: parts.get(5).and_then(|v| v.parse().ok()).unwrap_or(0.0),
                disk_free: parts.get(6).and_then(|v| v.parse().ok()).unwrap_or(0.0),
            })
        })
        .collect()
}

fn downsample(rows: Vec<CsvRow>, max_points: usize) -> Vec<CsvRow> {
    let n = rows.len();
    if n <= max_points {
        return rows;
    }
    let bucket_size = n as f64 / max_points as f64;
    (0..max_points)
        .map(|i| {
            let start = (i as f64 * bucket_size) as usize;
            let end = ((i + 1) as f64 * bucket_size) as usize;
            let end = end.min(n);
            let bucket = &rows[start..end];
            let count = bucket.len() as f32;
            CsvRow {
                timestamp: bucket[0].timestamp.clone(),
                sys_mem: bucket.iter().map(|r| r.sys_mem).sum::<f32>() / count,
                app_mem: bucket.iter().map(|r| r.app_mem).sum::<f32>() / count,
                sys_cpu: bucket.iter().map(|r| r.sys_cpu).sum::<f32>() / count,
                app_cpu: bucket.iter().map(|r| r.app_cpu).sum::<f32>() / count,
                disk_used: bucket.iter().map(|r| r.disk_used).sum::<f32>() / count,
                disk_free: bucket.iter().map(|r| r.disk_free).sum::<f32>() / count,
            }
        })
        .collect()
}

fn csv_to_json(csv: &str, max_points: usize) -> String {
    let rows = downsample(parse_csv_rows(csv), max_points);

    let mut sys_mem = Vec::with_capacity(rows.len());
    let mut app_mem = Vec::with_capacity(rows.len());
    let mut sys_cpu = Vec::with_capacity(rows.len());
    let mut app_cpu = Vec::with_capacity(rows.len());
    let mut disk_used = Vec::with_capacity(rows.len());
    let mut disk_free = Vec::with_capacity(rows.len());

    for row in &rows {
        sys_mem.push(format!(r#"{{"x":"{}","y":{}}}"#, row.timestamp, row.sys_mem));
        app_mem.push(format!(r#"{{"x":"{}","y":{}}}"#, row.timestamp, row.app_mem));
        sys_cpu.push(format!(r#"{{"x":"{}","y":{}}}"#, row.timestamp, row.sys_cpu));
        app_cpu.push(format!(r#"{{"x":"{}","y":{}}}"#, row.timestamp, row.app_cpu));
        disk_used.push(format!(r#"{{"x":"{}","y":{}}}"#, row.timestamp, row.disk_used));
        disk_free.push(format!(r#"{{"x":"{}","y":{:.2}}}"#, row.timestamp, row.disk_free));
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
