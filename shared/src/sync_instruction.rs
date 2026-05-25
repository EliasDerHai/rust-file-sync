use crate::content_hash::ContentHash;
use crate::matchable_path::MatchablePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum SyncInstruction {
    Upload(MatchablePath),
    Download(MatchablePath, ContentHash),
    Delete(MatchablePath),
}
