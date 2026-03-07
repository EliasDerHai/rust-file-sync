use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{matchable_path::MatchablePath, utc_millis::UtcMillis};

// sync

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FileDescription {
    // eg. "test.txt"
    pub file_name: String,
    // contains file_name eg. "./dir/test.txt"
    pub relative_path: MatchablePath,
    pub size_in_bytes: u64,
    pub file_type: String,
    pub last_updated_utc_millis: UtcMillis,
}

// web

/// config needed to start watching directories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfigDto {
    pub min_poll_interval_in_ms: u16,
    pub watch_groups: HashMap<i64, WatchGroupConfigDto>,
}

impl Default for WatchConfigDto {
    fn default() -> Self {
        Self {
            min_poll_interval_in_ms: 5000,
            watch_groups: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchGroupConfigDto {
    pub path_to_monitor: String,
    pub exclude_dirs: Vec<String>,
    #[serde(default = "default_exclude_dot_dirs")]
    pub exclude_dot_dirs: bool,
    /// for logging
    pub name: String,
}

fn default_exclude_dot_dirs() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfigUpdateDto {
    pub path_to_monitor: String,
    pub min_poll_interval_in_ms: u16,
    pub exclude_dirs: Vec<String>,
    pub exclude_dot_dirs: bool,
    pub server_watch_group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientWithConfig {
    // client
    pub id: String,
    pub host_name: String,
    pub min_poll_interval_in_ms: u16,

    // client-watch-group
    pub path_to_monitor: String,
    pub exclude_dirs: Vec<String>,
    pub exclude_dot_dirs: bool,

    // server-watch-group
    pub server_watch_group_id: i64,
    pub server_watch_group_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerWatchGroup {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WatchGroupNameDto {
    pub name: String,
}

// monitoring

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub x: String,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorData {
    pub sys_mem: Vec<DataPoint>,
    pub app_mem: Vec<DataPoint>,
    pub sys_cpu: Vec<DataPoint>,
    pub app_cpu: Vec<DataPoint>,
}

// links

#[derive(Deserialize, serde::Serialize)]
pub struct ShareLinkRequest {
    pub url: String,
    pub title: Option<String>,
}
