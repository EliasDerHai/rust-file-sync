use serde::{Deserialize, Serialize};

/// DTO for client registration / config migration
/// Sent from client to server to register the client's config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterClientRequest {
    pub path_to_monitor: String,
    pub exclude_dirs: Vec<String>,
    #[serde(default = "default_exclude_dot_dirs")]
    pub exclude_dot_dirs: bool,
    pub min_poll_interval_in_ms: u16,
}

fn default_exclude_dot_dirs() -> bool {
    true
}
