
/// How a file should be presented in the web UI, derived from its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Audio,
    Text,
    Other,
}

impl MediaKind {

/// Classifies a file extension (with or without a leading dot, any case).
///
/// The listed formats are the ones browsers can render natively — no
/// transcoding or thumbnailing happens anywhere in the stack.
pub fn from(ext: &str) -> Self {
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

pub fn is_image(&self) -> bool {
    *self == MediaKind::Image
}

/// True for anything the gallery can display: image, video or audio.
pub fn is_media(&self) -> bool {
    matches!(
        self,
        MediaKind::Image | MediaKind::Video | MediaKind::Audio
    )
}
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
            assert_eq!(MediaKind::from(ext), MediaKind::Image, "{ext}");
        }
        for ext in VIDEOS {
            assert_eq!(MediaKind::from(ext), MediaKind::Video, "{ext}");
        }
        for ext in AUDIOS {
            assert_eq!(MediaKind::from(ext), MediaKind::Audio, "{ext}");
        }
        for ext in TEXTS {
            assert_eq!(MediaKind::from(ext), MediaKind::Text, "{ext}");
        }
        for ext in ["", "avi", "exe", "zip", "docx"] {
            assert_eq!(MediaKind::from(ext), MediaKind::Other, "{ext}");
        }
    }

    #[test]
    fn ignores_case_and_leading_dot() {
        // phones and cameras like to shout: IMG_0001.JPG, IMG_0002.MOV
        assert_eq!(MediaKind::from("MOV"), MediaKind::Video);
        assert_eq!(MediaKind::from(".MP4"), MediaKind::Video);
        assert_eq!(MediaKind::from("JPG"), MediaKind::Image);
        assert_eq!(MediaKind::from(".Png"), MediaKind::Image);
        assert_eq!(content_type_for("MP4"), Some("video/mp4"));
    }

    #[test]
    fn is_media_covers_image_video_audio_only() {
        assert!(MediaKind::from("png").is_media());
        assert!(MediaKind::from("mp4").is_media());
        assert!(MediaKind::from("flac").is_media());
        assert!(!MediaKind::from("md").is_media());
        assert!(!MediaKind::from("zip").is_media());
        assert!(MediaKind::from("gif").is_image());
        assert!(!MediaKind::from("mp4").is_image());
    }

    #[test]
    fn every_known_extension_has_a_content_type() {
        for ext in IMAGES.iter().chain(&VIDEOS).chain(&AUDIOS).chain(&TEXTS) {
            let ct = content_type_for(ext).unwrap_or_else(|| panic!("no content type for {ext}"));
            let expected_prefix = match MediaKind::from(ext) {
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
