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

// sys config (client ↔ server)

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

// api - clients

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientDto {
    pub id: String,
    pub host_name: String,
    pub min_poll_interval_in_ms: u16,
}

/// PUT /api/clients/{id}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientUpdateDto {
    pub min_poll_interval_in_ms: u16,
}

// api - client watch group assignments

/// GET /api/clients/{id}/watch-groups → Vec<ClientWatchGroupDto>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientWatchGroupDto {
    pub server_watch_group_id: i64,
    pub server_watch_group_name: String,
    pub path_to_monitor: String,
    pub exclude_dirs: Vec<String>,
    pub exclude_dot_dirs: bool,
}

/// POST /api/clients/{id}/watch-groups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientWatchGroupCreateDto {
    pub server_watch_group_id: i64,
    pub path_to_monitor: String,
    pub exclude_dirs: Vec<String>,
    pub exclude_dot_dirs: bool,
}

/// PUT /api/clients/{id}/watch-groups/{wg_id}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientWatchGroupUpdateDto {
    pub path_to_monitor: String,
    pub exclude_dirs: Vec<String>,
    pub exclude_dot_dirs: bool,
}

// api - server watch groups

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerWatchGroup {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkCreateDto {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkDto {
    pub url: String,
    pub title: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkTagCreateDto {
    pub url: String,
    pub tag: String,
}
