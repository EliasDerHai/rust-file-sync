use std::collections::HashMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::content_hash::ContentHash;
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
    pub disk_used: Vec<DataPoint>,
    pub disk_free: Vec<DataPoint>,
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

// file type helpers

/// How a file should be presented in the web UI, derived from its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Audio,
    Text,
    Other,
}

/// Classifies a file extension (with or without a leading dot, any case).
///
/// The listed formats are the ones browsers can render natively — no
/// transcoding or thumbnailing happens anywhere in the stack.
pub fn media_kind(ext: &str) -> MediaKind {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "jfif" | "png" | "apng" | "gif" | "webp" | "avif" | "svg" | "bmp"
        | "ico" | "heic" | "heif" => MediaKind::Image,
        "mp4" | "m4v" | "webm" | "mov" | "mkv" | "ogv" | "3gp" => MediaKind::Video,
        "mp3" | "m4a" | "aac" | "wav" | "flac" | "ogg" | "oga" | "opus" | "weba" => {
            MediaKind::Audio
        }
        "txt" | "md" | "rs" | "toml" | "json" | "yaml" | "yml" | "sh" | "log" => MediaKind::Text,
        _ => MediaKind::Other,
    }
}

pub fn is_image(ext: &str) -> bool {
    media_kind(ext) == MediaKind::Image
}

/// True for anything the gallery can display: image, video or audio.
pub fn is_media(ext: &str) -> bool {
    matches!(
        media_kind(ext),
        MediaKind::Image | MediaKind::Video | MediaKind::Audio
    )
}

/// Content type to serve a file with, or `None` to let the server fall back to
/// guessing. Explicit because mime guessing misses several of these (`heic`,
/// `opus`, `rs`, `log`) and would resolve `ogg` to video rather than audio.
pub fn content_type_for(ext: &str) -> Option<&'static str> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let content_type = match ext.as_str() {
        // image
        "jpg" | "jpeg" | "jfif" => "image/jpeg",
        "png" => "image/png",
        "apng" => "image/apng",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "heic" => "image/heic",
        "heif" => "image/heif",
        // video
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "ogv" => "video/ogg",
        "3gp" => "video/3gpp",
        // audio
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/opus",
        "weba" => "audio/webm",
        // text
        "txt" | "md" | "rs" | "toml" | "json" | "yaml" | "yml" | "sh" | "log" => {
            "text/plain; charset=utf-8"
        }
        _ => return None,
    };
    Some(content_type)
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

#[cfg(test)]
mod tests {
    use super::*;

    const IMAGES: [&str; 13] = [
        "jpg", "jpeg", "jfif", "png", "apng", "gif", "webp", "avif", "svg", "bmp", "ico", "heic",
        "heif",
    ];
    const VIDEOS: [&str; 7] = ["mp4", "m4v", "webm", "mov", "mkv", "ogv", "3gp"];
    const AUDIOS: [&str; 9] = [
        "mp3", "m4a", "aac", "wav", "flac", "ogg", "oga", "opus", "weba",
    ];
    const TEXTS: [&str; 9] = [
        "txt", "md", "rs", "toml", "json", "yaml", "yml", "sh", "log",
    ];

    #[test]
    fn classifies_every_kind() {
        for ext in IMAGES {
            assert_eq!(media_kind(ext), MediaKind::Image, "{ext}");
        }
        for ext in VIDEOS {
            assert_eq!(media_kind(ext), MediaKind::Video, "{ext}");
        }
        for ext in AUDIOS {
            assert_eq!(media_kind(ext), MediaKind::Audio, "{ext}");
        }
        for ext in TEXTS {
            assert_eq!(media_kind(ext), MediaKind::Text, "{ext}");
        }
        for ext in ["", "avi", "exe", "zip", "docx"] {
            assert_eq!(media_kind(ext), MediaKind::Other, "{ext}");
        }
    }

    #[test]
    fn ignores_case_and_leading_dot() {
        // phones and cameras like to shout: IMG_0001.JPG, IMG_0002.MOV
        assert_eq!(media_kind("MOV"), MediaKind::Video);
        assert_eq!(media_kind(".MP4"), MediaKind::Video);
        assert_eq!(media_kind("JPG"), MediaKind::Image);
        assert_eq!(media_kind(".Png"), MediaKind::Image);
        assert_eq!(content_type_for("MP4"), Some("video/mp4"));
    }

    #[test]
    fn is_media_covers_image_video_audio_only() {
        assert!(is_media("png"));
        assert!(is_media("mp4"));
        assert!(is_media("flac"));
        assert!(!is_media("md"));
        assert!(!is_media("zip"));

        assert!(is_image("gif"));
        assert!(!is_image("mp4"));
    }

    #[test]
    fn every_known_extension_has_a_content_type() {
        for ext in IMAGES.iter().chain(&VIDEOS).chain(&AUDIOS).chain(&TEXTS) {
            let ct = content_type_for(ext).unwrap_or_else(|| panic!("no content type for {ext}"));
            let expected_prefix = match media_kind(ext) {
                MediaKind::Image => "image/",
                MediaKind::Video => "video/",
                MediaKind::Audio => "audio/",
                MediaKind::Text => "text/",
                MediaKind::Other => unreachable!("{ext} should be classified"),
            };
            assert!(
                ct.starts_with(expected_prefix),
                "{ext} -> {ct}, expected {expected_prefix}*"
            );
        }
        assert_eq!(content_type_for("zip"), None);
    }
}
