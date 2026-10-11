//! On-demand thumbnails for the SPA's tile view, cached on disk under
//! `data/thumbnail/{wg_id}/{rel}.jpg`.
//!
//! Rasters are decoded with the pure-Rust `image` crate (keeps the static musl
//! build free of C deps and honors EXIF orientation); videos shell out to the
//! system `ffmpeg` binary. Without ffmpeg, video thumbnails are simply
//! unavailable and the SPA falls back to an icon.

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageDecoder, ImageReader};
use shared::media::MediaKind;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{LazyLock, OnceLock};
use std::time::{Duration, SystemTime};
use tokio::process::Command;
use tokio::sync::Semaphore;
use tracing::{info, warn};

/// longest side in px - 2x the ~200px tile for high-dpi screens
const MAX_DIM: u32 = 400;
const JPEG_QUALITY: u8 = 75;
/// thumbnail generation is CPU heavy - leave the Pi room for syncing
const MAX_CONCURRENT: usize = 2;
const FFMPEG_TIMEOUT: Duration = Duration::from_secs(30);

static GENERATION_PERMITS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(MAX_CONCURRENT));
static FFMPEG_AVAILABLE: OnceLock<bool> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Strategy {
    /// decoded + downscaled with the `image` crate
    Raster,
    /// first frame grabbed with ffmpeg
    Video,
    /// browsers render these natively and they can't be decoded here - serve the original
    Passthrough,
    Unsupported,
}

impl Strategy {
    pub(crate) fn for_ext(ext: &str) -> Self {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        match ext.as_str() {
            "jpg" | "jpeg" | "jfif" | "png" | "apng" | "gif" | "webp" | "bmp" | "ico" => {
                Strategy::Raster
            }
            "svg" | "avif" => Strategy::Passthrough,
            _ if MediaKind::from(&ext) == MediaKind::Video => Strategy::Video,
            _ => Strategy::Unsupported,
        }
    }
}

/// Checks once at startup whether `ffmpeg` is on the PATH.
pub(crate) async fn probe_ffmpeg() {
    let available = Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success());
    if available {
        info!("ffmpeg found - video thumbnails enabled");
    } else {
        warn!(
            "ffmpeg not found on PATH - video thumbnails disabled (install with `sudo apt install ffmpeg`)"
        );
    }
    let _ = FFMPEG_AVAILABLE.set(available);
}

fn ffmpeg_available() -> bool {
    FFMPEG_AVAILABLE.get().copied().unwrap_or(false)
}

/// `a/clip.mp4` -> `{base}/{wg_id}/a/clip.mp4.jpg` (full name kept so `a.png` and `a.mp4` don't collide)
pub(crate) fn thumbnail_path(base: &Path, wg_id: i64, rel: &Path) -> PathBuf {
    with_suffix(&base.join(wg_id.to_string()).join(rel), ".jpg")
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = OsString::from(path.as_os_str());
    s.push(suffix);
    PathBuf::from(s)
}

/// negative cache, so a file that can't be thumbnailed isn't retried on every view
fn failed_marker(thumb: &Path) -> PathBuf {
    with_suffix(thumb, ".failed")
}

/// fresh = written after the source was last modified
async fn is_fresh(candidate: &Path, source_mtime: SystemTime) -> bool {
    match tokio::fs::metadata(candidate)
        .await
        .and_then(|m| m.modified())
    {
        Ok(mtime) => mtime >= source_mtime,
        Err(_) => false,
    }
}

/// Returns the cached thumbnail for `source`, generating it first if missing or stale.
/// `None` means no thumbnail can be produced (unsupported, broken file, no ffmpeg).
pub(crate) async fn get_or_create(
    base: &Path,
    wg_id: i64,
    rel: &Path,
    source: &Path,
    strategy: Strategy,
) -> io::Result<Option<PathBuf>> {
    if !matches!(strategy, Strategy::Raster | Strategy::Video) {
        return Ok(None);
    }
    if strategy == Strategy::Video && !ffmpeg_available() {
        // no failed marker - installing ffmpeg later should just work
        return Ok(None);
    }

    let source_mtime = tokio::fs::metadata(source).await?.modified()?;
    let thumb = thumbnail_path(base, wg_id, rel);
    let marker = failed_marker(&thumb);

    if is_fresh(&thumb, source_mtime).await {
        return Ok(Some(thumb));
    }
    if is_fresh(&marker, source_mtime).await {
        return Ok(None);
    }

    let _permit = GENERATION_PERMITS
        .acquire()
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    // a concurrent request for the same file may have finished while we waited
    if is_fresh(&thumb, source_mtime).await {
        return Ok(Some(thumb));
    }

    if let Some(parent) = thumb.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = with_suffix(&thumb, ".tmp");
    let result = match strategy {
        Strategy::Raster => generate_raster(source.to_path_buf(), tmp.clone()).await,
        _ => generate_video(source, &tmp).await,
    };

    match result {
        Ok(()) => {
            // rename is atomic, so readers never see a half-written thumbnail
            tokio::fs::rename(&tmp, &thumb).await?;
            let _ = tokio::fs::remove_file(&marker).await;
            Ok(Some(thumb))
        }
        Err(e) => {
            warn!("Thumbnail generation failed for {}: {e}", source.display());
            let _ = tokio::fs::remove_file(&tmp).await;
            tokio::fs::write(&marker, b"").await?;
            Ok(None)
        }
    }
}

/// Best-effort removal of a deleted file's thumbnail and failure marker.
pub(crate) async fn remove(base: &Path, wg_id: i64, rel: &Path) {
    let thumb = thumbnail_path(base, wg_id, rel);
    let _ = tokio::fs::remove_file(failed_marker(&thumb)).await;
    let _ = tokio::fs::remove_file(thumb).await;
}

async fn generate_raster(source: PathBuf, out: PathBuf) -> io::Result<()> {
    tokio::task::spawn_blocking(move || {
        let mut decoder = ImageReader::open(&source)?
            .with_guessed_format()?
            .into_decoder()
            .map_err(io::Error::other)?;
        let orientation = decoder.orientation().map_err(io::Error::other)?;
        let mut img = DynamicImage::from_decoder(decoder).map_err(io::Error::other)?;
        img.apply_orientation(orientation);
        // `thumbnail` would also upscale small images
        if img.width() > MAX_DIM || img.height() > MAX_DIM {
            img = img.thumbnail(MAX_DIM, MAX_DIM);
        }
        let thumb = img.to_rgb8();
        let file = std::fs::File::create(&out)?;
        thumb
            .write_with_encoder(JpegEncoder::new_with_quality(
                io::BufWriter::new(file),
                JPEG_QUALITY,
            ))
            .map_err(io::Error::other)
    })
    .await
    .map_err(io::Error::other)?
}

async fn generate_video(source: &Path, out: &Path) -> io::Result<()> {
    // 1s skips black fade-ins; clips shorter than that produce no frame, so retry at 0
    for seek in ["1", "0"] {
        let _ = tokio::fs::remove_file(out).await;
        let run = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-ss",
                seek,
                "-i",
            ])
            .arg(source)
            .args(["-frames:v", "1", "-vf"])
            // min() keeps small videos from being upscaled
            .arg(format!(
                "scale='min({MAX_DIM},iw)':'min({MAX_DIM},ih)':force_original_aspect_ratio=decrease"
            ))
            .args(["-q:v", "5", "-f", "image2", "-y"])
            .arg(out)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(FFMPEG_TIMEOUT, run)
            .await
            .map_err(|_| io::Error::other("ffmpeg timed out"))??;
        let produced = tokio::fs::metadata(out).await.is_ok_and(|m| m.len() > 0);
        if output.status.success() && produced {
            return Ok(());
        }
        if seek == "0" {
            return Err(io::Error::other(format!(
                "ffmpeg produced no frame: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
    }
    unreachable!("loop returns on its last iteration")
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbImage};

    #[test]
    fn strategy_by_extension() {
        assert_eq!(Strategy::for_ext("JPG"), Strategy::Raster);
        assert_eq!(Strategy::for_ext(".webp"), Strategy::Raster);
        assert_eq!(Strategy::for_ext("mp4"), Strategy::Video);
        assert_eq!(Strategy::for_ext("MOV"), Strategy::Video);
        assert_eq!(Strategy::for_ext("svg"), Strategy::Passthrough);
        assert_eq!(Strategy::for_ext("avif"), Strategy::Passthrough);
        assert_eq!(Strategy::for_ext("heic"), Strategy::Unsupported);
        assert_eq!(Strategy::for_ext("mp3"), Strategy::Unsupported);
        assert_eq!(Strategy::for_ext("txt"), Strategy::Unsupported);
    }

    #[test]
    fn thumbnail_path_keeps_full_file_name() {
        let p = thumbnail_path(Path::new("/thumbs"), 7, Path::new("a/clip.mp4"));
        assert_eq!(p, PathBuf::from("/thumbs/7/a/clip.mp4.jpg"));
    }

    fn write_png(path: &Path, w: u32, h: u32) {
        RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]))
            .save_with_format(path, ImageFormat::Png)
            .unwrap();
    }

    #[tokio::test]
    async fn raster_thumbnail_is_downscaled_jpeg() {
        let src_dir = tempfile::tempdir().unwrap();
        let thumb_dir = tempfile::tempdir().unwrap();
        let source = src_dir.path().join("wide.png");
        write_png(&source, 1000, 500);

        let thumb = get_or_create(
            thumb_dir.path(),
            1,
            Path::new("wide.png"),
            &source,
            Strategy::Raster,
        )
        .await
        .unwrap()
        .expect("thumbnail");

        let reader = ImageReader::open(&thumb)
            .unwrap()
            .with_guessed_format()
            .unwrap();
        assert_eq!(reader.format(), Some(ImageFormat::Jpeg));
        let img = reader.decode().unwrap();
        assert_eq!((img.width(), img.height()), (400, 200));
    }

    #[tokio::test]
    async fn stale_thumbnail_is_regenerated() {
        let src_dir = tempfile::tempdir().unwrap();
        let thumb_dir = tempfile::tempdir().unwrap();
        let source = src_dir.path().join("pic.png");
        let rel = Path::new("pic.png");
        write_png(&source, 100, 100);
        get_or_create(thumb_dir.path(), 1, rel, &source, Strategy::Raster)
            .await
            .unwrap()
            .unwrap();

        // overwrite the source with a newer, differently sized image
        std::thread::sleep(Duration::from_millis(20));
        write_png(&source, 300, 100);
        let thumb = get_or_create(thumb_dir.path(), 1, rel, &source, Strategy::Raster)
            .await
            .unwrap()
            .unwrap();

        let img = image::open(thumb).unwrap();
        assert_eq!((img.width(), img.height()), (300, 100));
    }

    #[tokio::test]
    async fn broken_file_writes_failed_marker_and_short_circuits() {
        let src_dir = tempfile::tempdir().unwrap();
        let thumb_dir = tempfile::tempdir().unwrap();
        let source = src_dir.path().join("broken.jpg");
        std::fs::write(&source, b"not an image").unwrap();
        let rel = Path::new("broken.jpg");

        let first = get_or_create(thumb_dir.path(), 1, rel, &source, Strategy::Raster)
            .await
            .unwrap();
        assert!(first.is_none());
        let marker = failed_marker(&thumbnail_path(thumb_dir.path(), 1, rel));
        assert!(marker.exists());

        let second = get_or_create(thumb_dir.path(), 1, rel, &source, Strategy::Raster)
            .await
            .unwrap();
        assert!(second.is_none());

        remove(thumb_dir.path(), 1, rel).await;
        assert!(!marker.exists());
    }

    #[tokio::test]
    async fn video_thumbnail_via_ffmpeg() {
        probe_ffmpeg().await;
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed");
            return;
        }
        let src_dir = tempfile::tempdir().unwrap();
        let thumb_dir = tempfile::tempdir().unwrap();
        let source = src_dir.path().join("clip.mp4");
        let status = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=duration=2:size=640x360:rate=10")
            .arg("-y")
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());

        let thumb = get_or_create(
            thumb_dir.path(),
            1,
            Path::new("clip.mp4"),
            &source,
            Strategy::Video,
        )
        .await
        .unwrap()
        .expect("thumbnail");

        let img = image::open(thumb).unwrap();
        assert_eq!((img.width(), img.height()), (400, 225));
    }
}
