use shared::dtos::FileDescription;
use std::collections::HashMap;
use std::ops::Add;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::Instant;
use tracing::trace;
use tracing_subscriber::EnvFilter;

use crate::execute::loop_scan;
use crate::init::{config, load, state};

mod execute;
mod init;

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

    let (mut state, client, mut persisted_state) = load().await;
    let mut last_scans: HashMap<i64, Vec<FileDescription>> = HashMap::new();

    loop {
        let loop_start = Instant::now();

        let dto = config::fetch_watch_config(&client, &state.server_url).await;
        state.watch_groups = init::to_watch_group(dto.watch_groups);
        state.min_poll_interval_in_ms = dto.min_poll_interval_in_ms;

        for (wg_id, wg) in &state.watch_groups {
            let last_scan = last_scans.remove(wg_id);
            let synced = persisted_state.synced_hashes.entry(*wg_id).or_default();
            let next_scan =
                loop_scan(&state.server_url, *wg_id, wg, &client, last_scan, synced).await;
            // last_scan state should only be updated when everything runs through otherwise we
            // risk losing information (delete)
            last_scans.insert(*wg_id, next_scan);
        }

        state::save(&persisted_state);

        trace!("Loop took {:?}", Instant::now().duration_since(loop_start));
        tokio::time::sleep_until(
            loop_start.add(Duration::from_millis(state.min_poll_interval_in_ms as u64)),
        )
        .await;
    }
}
