use crate::write::RotatingFileWriter;
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
            chrono::Local::now(),
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
