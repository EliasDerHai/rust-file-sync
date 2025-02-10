use std::fs;
use std::fs::{create_dir_all, OpenOptions};
use std::path::{Path, PathBuf};

use std::io::prelude::*;

use axum::body::Bytes;
use chrono::{Local, NaiveTime};
use tokio::time::{sleep_until, Instant};
use tracing::{error, info};

pub async fn schedule_data_backups(data_path: &Path, backup_path: &Path) {
    info!("Scheduling backups");
    loop {
        let backup_time = NaiveTime::from_hms_opt(2, 0, 0).unwrap();
        let now = Local::now().naive_local();
        let today = Local::now().date_naive();

        let next_run = if now.time() < backup_time {
            today.and_time(backup_time)
        } else {
            today.succ_opt().unwrap().and_time(backup_time)
        };

        let next_run_duration = next_run
            .and_utc()
            .signed_duration_since(Local::now().with_timezone(&chrono::Utc))
            .to_std()
            .unwrap();

        info!(
            "Next backup scheduled for: {} (in {:?})",
            next_run, next_run_duration
        );

        sleep_until(Instant::now() + next_run_duration).await;

        perform_backup(data_path, backup_path).await;
    }
}

async fn perform_backup(data_path: &Path, backup_path: &Path) {
    info!("Executing daily backup...");
}

pub fn create_all_dir_and_write(path: &PathBuf, bytes: &Bytes) -> Result<(), std::io::Error> {
    create_dir_all(path.parent().unwrap_or(Path::new("./"))).and_then(|()| fs::write(&path, bytes))
}

pub fn append_line(file_path: &Path, line: &str) {
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(file_path)
        .unwrap();

    if let Err(e) = writeln!(file, "{line}") {
        error!("Couldn't append to file: {}", e);
    }
}
