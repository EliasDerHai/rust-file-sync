use serde::{Deserialize, Serialize};

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
