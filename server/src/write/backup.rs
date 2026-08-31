use crate::db::ServerDatabase;
use anyhow::Context;
use chrono::{Local, NaiveTime};
use flate2::{Compression, write::GzEncoder};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tokio::time::{Instant, sleep_until};
use tracing::{error, info};

const BACKUP_FILE_PREFIX: &str = "backup";
const DB_SNAPSHOT_PREFIX: &str = "sqlite_snapshot_";
const MAX_BACKUP_FILES: usize = 7;

pub async fn schedule_data_backups(data_path: &Path, backup_path: &Path, db: ServerDatabase) {
    info!("Scheduling backups");
    loop {
        let backup_time = NaiveTime::from_hms_opt(2, 0, 0).unwrap();
        let now = Local::now().naive_local();
        let today = Local::now().date_naive();

        let next_run = if now.time() < backup_time {
            today.and_time(backup_time)
        } else {
            today.succ_opt().unwrap().and_time(backup_time)
        };

        let next_run_duration = next_run
            .and_utc()
            .signed_duration_since(Local::now().with_timezone(&chrono::Utc))
            .to_std()
            .unwrap();

        info!(
            "Next backup scheduled for: {} (in {:?})",
            next_run, next_run_duration
        );

        sleep_until(Instant::now() + next_run_duration).await;

        let backup_start = Instant::now();
        match perform_backup(data_path, backup_path, &db).await {
            Ok(()) => info!("Backup completed successfully in {}s", backup_start.elapsed().as_secs()),
            Err(err) => error!("Backup failed, will retry at next scheduled run: {err:#}"),
        }
    }
}

/// Backs up `data_path` (the synced-files tree) and a consistent snapshot of the
/// server DB into a single `.tar.gz` archive under `backup_path`, then prunes old
/// archives beyond `MAX_BACKUP_FILES`. Never leaves a corrupt/partial archive behind
/// under the final `backup_*.tar.gz` name, and never touches existing good backups
/// on failure.
async fn perform_backup(
    data_path: &Path,
    backup_path: &Path,
    db: &ServerDatabase,
) -> anyhow::Result<()> {
    info!("Executing daily backup...");
    cleanup_stale_temp_files(backup_path);

    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S-%3f").to_string();
    let db_snapshot_path = backup_path.join(format!("{DB_SNAPSHOT_PREFIX}{timestamp}.db"));
    let tmp_archive_path = backup_path.join(format!("{BACKUP_FILE_PREFIX}_{timestamp}.tar.gz.tmp"));
    let final_archive_path = backup_path.join(format!("{BACKUP_FILE_PREFIX}_{timestamp}.tar.gz"));

    // VACUUM INTO errors if db_snapshot_path already exists (timestamping)
    db.vacuum_into(&db_snapshot_path)
        .await
        .context("VACUUM INTO failed while snapshotting the database")?;

    // Build the tar.gz off the async runtime
    let build_result = {
        let data_path = data_path.to_path_buf();
        let db_snapshot_path = db_snapshot_path.clone();
        let tmp_archive_path = tmp_archive_path.clone();
        tokio::task::spawn_blocking(move || {
            build_archive_blocking(&data_path, &db_snapshot_path, &tmp_archive_path)
        })
        .await
    };

    // The DB snapshot is single-purpose scratch space: remove it now regardless of
    // whether archive-building succeeded or failed.
    if let Err(err) = tokio::fs::remove_file(&db_snapshot_path).await {
        error!("Failed to remove temporary DB snapshot {db_snapshot_path:?}: {err}");
    }

    let build_result = build_result.context("archive-building task panicked")?;
    if let Err(err) = build_result {
        let _ = tokio::fs::remove_file(&tmp_archive_path).await; // best-effort cleanup of partial archive
        return Err(err).context("failed to build backup archive");
    }

    tokio::fs::rename(&tmp_archive_path, &final_archive_path)
        .await
        .context("failed to rename temp archive into place")?;
    info!("Backup written to {final_archive_path:?}");

    // FIFO-prune old backups
    if let Err(err) = prune_old_backups(backup_path, MAX_BACKUP_FILES) {
        error!("Failed to prune old backups: {err}");
    }

    Ok(())
}

/// Synchronous: builds a gzip-compressed tar archive containing `data_path`'s
/// contents (under an `upload/` entry prefix) and `db_snapshot_path` (as `sqlite.db`
/// at the archive root). Entry names are chosen so restoring is a single
/// `tar -xzf backup.tar.gz -C data/`. Must run inside `spawn_blocking`.
fn build_archive_blocking(
    data_path: &Path,
    db_snapshot_path: &Path,
    tmp_archive_path: &Path,
) -> io::Result<()> {
    let file = fs::File::create(tmp_archive_path)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(encoder);

    // Store symlinks as symlink entries; never dereference them - avoids archive
    // bloat, infinite loops, and pulling in data from outside data_path.
    builder.follow_symlinks(false);

    if data_path.is_dir() {
        builder.append_dir_all("upload", data_path)?;
    }
    builder.append_path_with_name(db_snapshot_path, "sqlite.db")?;

    let encoder = builder.into_inner()?; // finalizes the tar stream
    let file = encoder.finish()?; // finalizes the gzip stream
    file.sync_all()?; // fsync before the caller renames it into place
    Ok(())
}

/// Removes leftover temp files (partial archives, DB snapshots) from a run that
/// crashed before completing. Best-effort: logs and continues on any error.
fn cleanup_stale_temp_files(backup_path: &Path) {
    let entries = match fs::read_dir(backup_path) {
        Ok(entries) => entries,
        Err(err) => {
            error!("Failed to scan backup dir for stale temp files: {err}");
            return;
        }
    };
    for path in entries.filter_map(|e| e.ok()).map(|e| e.path()) {
        let is_stale = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".tar.gz.tmp") || n.starts_with(DB_SNAPSHOT_PREFIX))
            .unwrap_or(false);
        if is_stale {
            match fs::remove_file(&path) {
                Ok(()) => info!("Removed stale temp backup file from a previous run: {path:?}"),
                Err(err) => error!("Failed to remove stale temp backup file {path:?}: {err}"),
            }
        }
    }
}

/// Lists existing `backup_*.tar.gz` files (sorted oldest to newest by filename,
/// since the embedded timestamp sorts lexicographically) and FIFO-deletes the
/// oldest ones beyond `max_files`.
fn prune_old_backups(backup_path: &Path, max_files: usize) -> io::Result<()> {
    let mut files: Vec<PathBuf> = fs::read_dir(backup_path)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(BACKUP_FILE_PREFIX) && n.ends_with(".tar.gz"))
                    .unwrap_or(false)
        })
        .collect();

    files.sort();

    let files_to_delete = files.len().saturating_sub(max_files);
    for path in files.into_iter().take(files_to_delete) {
        fs::remove_file(&path)?;
        info!("Pruned old backup: {path:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::setup_test_db_file;
    use std::io::Write as _;

    #[tokio::test]
    async fn perform_backup_writes_expected_entries_and_enforces_retention() {
        let source_dir = tempfile::tempdir().unwrap();
        let backup_dir = tempfile::tempdir().unwrap();
        let db_dir = tempfile::tempdir().unwrap();

        let watch_group_dir = source_dir.path().join("watch_group_1");
        fs::create_dir_all(&watch_group_dir).unwrap();
        fs::File::create(watch_group_dir.join("hello.txt"))
            .unwrap()
            .write_all(b"hello world")
            .unwrap();

        let db = setup_test_db_file(&db_dir.path().join("sqlite.db")).await;

        for _ in 0..9 {
            perform_backup(source_dir.path(), backup_dir.path(), &db)
                .await
                .expect("backup should succeed");
        }

        let archives: Vec<PathBuf> = fs::read_dir(backup_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(BACKUP_FILE_PREFIX) && n.ends_with(".tar.gz"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(
            archives.len(),
            MAX_BACKUP_FILES,
            "retention should keep exactly {MAX_BACKUP_FILES} archives"
        );

        let leftovers: Vec<PathBuf> = fs::read_dir(backup_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".tmp") || n.starts_with(DB_SNAPSHOT_PREFIX))
                    .unwrap_or(false)
            })
            .collect();
        assert!(leftovers.is_empty(), "no temp files should remain: {leftovers:?}");

        let newest = archives.iter().max().unwrap();
        let decoder = flate2::read::GzDecoder::new(fs::File::open(newest).unwrap());
        let mut archive = tar::Archive::new(decoder);
        let entry_paths: Vec<String> = archive
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned())
            .collect();

        assert!(entry_paths.iter().any(|p| p == "sqlite.db"));
        assert!(
            entry_paths
                .iter()
                .any(|p| p.starts_with("upload/watch_group_1/hello.txt"))
        );
    }
}
