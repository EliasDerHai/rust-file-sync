use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
