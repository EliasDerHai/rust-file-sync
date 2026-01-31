use crate::matchable_path::MatchablePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum SyncInstruction {
    Upload(MatchablePath),
    Download(MatchablePath),
    Delete(MatchablePath),
}
