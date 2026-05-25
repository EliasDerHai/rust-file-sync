use serde::{Deserialize, Serialize};
use shared::content_hash::ContentHash;
use shared::matchable_path::MatchablePath;
use std::collections::HashMap;
use std::fs;
use tracing::warn;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedClientState {
    /// Maps wg_id -> (relative path -> last synced ContentHash).
    /// used for three-way merge conflict detection using a common ancestor/base
    pub synced_hashes: HashMap<i64, HashMap<MatchablePath, ContentHash>>,
}

const STATE_FILE: &str = "./state.json";

pub fn load() -> PersistedClientState {
    match fs::read_to_string(STATE_FILE) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            warn!("Could not parse sync state file, starting fresh: {e}");
            PersistedClientState::default()
        }),
        Err(_) => PersistedClientState::default(),
    }
}

pub fn save(state: &PersistedClientState) {
    match serde_json::to_string(state) {
        Ok(s) => {
            if let Err(e) = fs::write(STATE_FILE, s) {
                warn!("Could not save sync state: {e}");
            }
        }
        Err(e) => warn!("Could not serialize sync state: {e}"),
    }
}
