use serde::{Deserialize, Serialize};
use crate::matchable_path::MatchablePath;

#[derive(Debug, Deserialize, Serialize)]
pub enum SyncInstruction {
    Download(MatchablePath),
    Delete(MatchablePath),
}
