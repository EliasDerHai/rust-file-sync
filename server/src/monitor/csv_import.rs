//! One-time import of the legacy `data/monitor/monitor_*.csv` files (10s rows written
//! by the old `RotatingFileWriter`) into `monitor_5s`. The regular rollup + prune then
//! moves anything older than the 5s retention into the 1m/1h tiers like live data.

use super::metrics::{MonitorRow, STAT_METRIC_COUNT, Stat, StatMetric};
use super::sampler::bucket_of;
use super::store::{MonitorDb, Tier};
use chrono::{Local, NaiveDateTime, TimeZone};
use std::path::{Path, PathBuf};
use tracing::info;

const IMPORT_DONE_KEY: &str = "csv_import_done";
const BATCH_SIZE: usize = 5_000;

/// Imports every legacy CSV in `dir` unless that already happened. Returns the number
/// of imported rows (0 when skipped).
pub(crate) async fn import_legacy_csv(db: &MonitorDb, dir: &Path) -> anyhow::Result<usize> {
    if db.get_meta(IMPORT_DONE_KEY).await?.is_some() {
        return Ok(0);
    }
    let files = legacy_files(dir);
    if files.is_empty() {
        db.set_meta(IMPORT_DONE_KEY, "no legacy files").await?;
        return Ok(0);
    }

    let mut total = 0;
    for file in &files {
        let content = tokio::fs::read_to_string(file).await?;
        let rows: Vec<MonitorRow> = content.lines().filter_map(parse_line).collect();
        for batch in rows.chunks(BATCH_SIZE) {
            db.insert_rows(Tier::FiveSeconds, batch).await?;
        }
        total += rows.len();
    }

    db.set_meta(IMPORT_DONE_KEY, &format!("{total} rows"))
        .await?;
    info!(
        "Imported {total} legacy monitoring rows from {} CSV file(s) - {} can be deleted",
        files.len(),
        dir.display()
    );
    Ok(total)
}

/// `monitor_*.csv`, oldest first (the file names are timestamps)
fn legacy_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("monitor") && n.ends_with(".csv"))
        })
        .collect();
    files.sort();
    files
}

/// `Timestamp;sys mem %;app mem %;sys cpu %;app cpu %[;disk used %;disk free GiB]`
/// in server-local time without offset. Header, malformed lines and local times that
/// don't exist (DST gap) yield `None`.
fn parse_line(line: &str) -> Option<MonitorRow> {
    let parts: Vec<&str> = line.split(';').collect();
    if parts.len() < 5 {
        return None;
    }
    let naive = NaiveDateTime::parse_from_str(parts[0], "%Y-%m-%dT%H:%M:%S").ok()?;
    let ts_ms = Local
        .from_local_datetime(&naive)
        .earliest()?
        .timestamp_millis();
    // same 5s grid as live rows
    let bucket_ms = bucket_of(ts_ms, Tier::FiveSeconds.resolution_ms());
    let num = |i: usize| parts.get(i).and_then(|v| v.trim().parse::<f64>().ok());

    let mut stats = [None; STAT_METRIC_COUNT];
    for (metric, column) in [
        (StatMetric::SysMem, 1),
        (StatMetric::AppMem, 2),
        (StatMetric::SysCpu, 3),
        (StatMetric::AppCpu, 4),
    ] {
        stats[metric.index()] = num(column).map(Stat::single);
    }

    Some(MonitorRow {
        bucket_ms,
        sample_count: 1,
        stats,
        disk_used: num(5),
        disk_free_gib: num(6),
        throttled: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_old_and_new_formats() {
        let header =
            "Timestamp;Total used mem in %;App used mem in %;Total used cpu in %;App used cpu in %";
        assert_eq!(parse_line(header), None);
        assert_eq!(parse_line("garbage"), None);
        assert_eq!(parse_line("not-a-date;1;2;3;4"), None);

        let old = parse_line("2026-01-25T22:41:12;35.5;0.15;5.1;1.0").unwrap();
        assert_eq!(old.stat(StatMetric::SysMem), Some(Stat::single(35.5)));
        assert_eq!(old.stat(StatMetric::SysCpu), Some(Stat::single(5.1)));
        assert_eq!(old.disk_used, None);

        let new = parse_line("2026-10-11T15:17:07;34.9;0.15;4.6;0.99;42.75;265.48").unwrap();
        assert_eq!(new.disk_used, Some(42.75));
        assert_eq!(new.disk_free_gib, Some(265.48));
        assert_eq!(new.stat(StatMetric::CpuTemp), None);
        assert!(new.bucket_ms > old.bucket_ms);
    }

    #[tokio::test]
    async fn imports_all_files_once() {
        let dir = tempfile::tempdir().unwrap();
        let header = "Timestamp;a;b;c;d;e;f\n";
        std::fs::write(
            dir.path().join("monitor_2026-01-01T00-00-00.csv"),
            format!("{header}2026-01-01T00:00:00;1;2;3;4\n2026-01-01T00:00:10;1;2;3;4\n"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("monitor_2026-01-02T00-00-00.csv"),
            format!("{header}2026-01-02T00:00:00;1;2;3;4;40;100\nbroken line\n"),
        )
        .unwrap();
        std::fs::write(dir.path().join("unrelated.txt"), "x").unwrap();

        let db = MonitorDb::in_memory().await;
        assert_eq!(import_legacy_csv(&db, dir.path()).await.unwrap(), 3);
        assert_eq!(import_legacy_csv(&db, dir.path()).await.unwrap(), 0);
        assert!(db.earliest_ms(Tier::FiveSeconds).await.unwrap().is_some());
    }
}
