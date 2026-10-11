use std::collections::HashMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::content_hash::ContentHash;
use crate::log_level::LogLevel;
use crate::{matchable_path::MatchablePath, utc_millis::UtcMillis};

// sync

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize, Clone)]
pub struct FileDescription {
    // eg. "test.txt"
    pub file_name: String,
    // contains file_name eg. "./dir/test.txt"
    pub relative_path: MatchablePath,
    pub size_in_bytes: u64,
    pub file_type: String,
    pub last_updated_utc_millis: UtcMillis,
    pub content_hash: ContentHash,
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
    pub version: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MonitorStatDto {
    pub min: f32,
    pub avg: f32,
    pub max: f32,
}

/// One bucket of `GET /api/monitor`. `None` = metric not available for this bucket
/// (not measured on this machine, or older imported history).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorPointDto {
    /// bucket start, UTC epoch ms
    pub t: i64,
    /// number of 1s samples in the bucket
    pub n: u32,
    /// %
    pub sys_cpu: Option<MonitorStatDto>,
    /// %, not normalized by core count
    pub app_cpu: Option<MonitorStatDto>,
    /// % of total memory
    pub sys_mem: Option<MonitorStatDto>,
    /// % of total memory
    pub app_mem: Option<MonitorStatDto>,
    /// % of total swap
    pub swap: Option<MonitorStatDto>,
    /// 1-minute load average
    pub load1: Option<MonitorStatDto>,
    /// °C
    pub cpu_temp: Option<MonitorStatDto>,
    /// bytes/s
    pub net_rx: Option<MonitorStatDto>,
    /// bytes/s
    pub net_tx: Option<MonitorStatDto>,
    /// bytes/s read by the server process
    pub app_read: Option<MonitorStatDto>,
    /// bytes/s written by the server process
    pub app_write: Option<MonitorStatDto>,
    /// % of the root filesystem
    pub disk_used: Option<f32>,
    pub disk_free_gib: Option<f32>,
    /// Raspberry Pi throttle bits 0-3 (under-voltage, freq capped, throttled, soft temp
    /// limit), OR-ed over the bucket; `None` off-Pi
    pub throttled: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorSeriesDto {
    /// bucket size of `points`, ms
    pub resolution_ms: i64,
    /// oldest stored data at all, for "all time" ranges and zoom limits
    pub earliest_ms: Option<i64>,
    pub points: Vec<MonitorPointDto>,
}

// links
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkCreateDto {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkDeleteDto {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkDto {
    pub url: String,
    pub created_at: NaiveDateTime,
    pub title: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkTagCreateDto {
    pub url: String,
    pub tag: String,
}

// impls

impl LinkDto {
    pub fn link_text(&self, max: usize) -> String {
        match self.title.clone() {
            Some(t) => t,
            None => {
                let s = self
                    .url
                    .strip_prefix("https://")
                    .or_else(|| self.url.strip_prefix("http://"))
                    .unwrap_or(&self.url);

                s.chars().take(max).collect()
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum SyncInstruction {
    Upload(MatchablePath),
    Download(MatchablePath, ContentHash),
    Delete(MatchablePath),
}

// backups

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFileDto {
    /// Position among currently existing backups, newest first (0 = latest). A rank,
    /// not a stable id - shifts as new backups are created and old ones are pruned.
    pub index: u8,
    pub file_name: String,
    pub size_in_bytes: u64,
    pub created_at_utc_millis: UtcMillis,
}

// server-sent events (GET /api/events, web client only)

/// Payload pushed to web clients over the SSE stream. Internally tagged so future
/// variants can carry their own fields without breaking the wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerEventDto {
    ServerHello,
}

// live server logs (GET /api/logs, GET /api/logs/stream, web client only)

/// One captured tracing event, either from the `/api/logs` backlog snapshot or
/// pushed in batches over `/api/logs/stream`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogLineDto {
    /// Monotonic per-process counter. The backlog fetch and the live stream are two
    /// separate HTTP requests, so the client uses this to de-dupe the small overlap/gap
    /// possible at the seam between them, rather than the server engineering it away.
    pub seq: u64,
    pub timestamp: UtcMillis,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
}

// location points (POST /api/locations, bulk upload from the Android app)

/// One recorded GPS point, as sent by the Android app's `LocationPointEntity`. The
/// entity's own local Room `id` is not included - it's a local autoincrement with no
/// meaning outside the device, so the server assigns its own id on insert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationPointCreateDto {
    pub timestamp_epoch_ms: UtcMillis,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_meters: Option<f64>,
    pub accuracy_meters: Option<f32>,
    pub speed_meters_per_second: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationPointUploadResultDto {
    pub inserted: usize,
}

/// One recorded GPS point as returned to the web admin's Locations tab.
/// `is_flagged` is computed fresh on every read (accuracy/implied-speed heuristics in
/// `LocationPointRepository::get_range`), never persisted, so the thresholds stay
/// tunable without a migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationPointDto {
    pub id: i64,
    pub timestamp_epoch_ms: UtcMillis,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_meters: Option<f64>,
    pub accuracy_meters: Option<f32>,
    pub speed_meters_per_second: Option<f32>,
    pub is_flagged: bool,
}

/// DELETE /api/locations body - bulk soft-delete (sets `excluded_at`) of
/// user-confirmed outlier points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationPointDeleteDto {
    pub ids: Vec<i64>,
}

/// GET /api/map-config - hands the server-held MapTiler API key to the web admin's
/// Locations map. MapTiler keys are meant to reach the browser (restricted by domain
/// in the MapTiler dashboard, not treated as secret).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapConfigDto {
    pub maptiler_key: Option<String>,
}
