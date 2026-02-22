use shared::get_files_of_directory::FileDescription;
use std::collections::HashMap;
use std::ops::Add;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::Instant;
use tracing::trace;
use tracing_subscriber::EnvFilter;

mod config;
mod execute;
mod setup;

struct ClientState {
    pub server_url: String,
    pub min_poll_interval_in_ms: u16,
    pub watch_groups: HashMap<i64, WatchGroup>,
}

struct WatchGroup {
    pub name: String,
    pub path_to_monitor: PathBuf,
    pub exclude_dirs: Vec<String>,
    pub exclude_dot_dirs: bool,
}

#[tokio::main]
async fn main() {
    let log_level = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(log_level).init();

    let (config, client) = setup::setup().await;
    let mut last_scans: HashMap<i64, Vec<FileDescription>> = HashMap::new();

    loop {
        let loop_start = Instant::now();

        for (wg_id, wg) in &config.watch_groups {
            let last_scan = last_scans.remove(wg_id);
            let next_scan =
                execute::loop_scan(&config.server_url, *wg_id, wg, &client, last_scan).await;
            // last_scan state should only be updated when everything runs through otherwise we
            // risk losing information (delete)
            last_scans.insert(*wg_id, next_scan);
        }

        trace!("Loop took {:?}", Instant::now().duration_since(loop_start));
        tokio::time::sleep_until(
            loop_start.add(Duration::from_millis(config.min_poll_interval_in_ms as u64)),
        )
        .await;
    }
}

// LOOP -----------------------------------------------------------------------
