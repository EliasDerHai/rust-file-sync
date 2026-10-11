//! System monitoring: 1s samples stored as 5s min/avg/max rows (kept 1 month), rolled
//! up into 1-minute (3 months) and 1-hour (forever) tiers in `data/monitor.db`.
//!
//! - `sampler`: reads the system once per second, writes 5s rows once a minute
//! - `store`: the tiered SQLite storage, rollups, retention and range queries
//! - `csv_import`: one-time import of the legacy rotating CSV files

mod csv_import;
mod metrics;
mod sampler;
mod store;

pub(crate) use store::MonitorDb;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use metrics::{MonitorRow, Stat, StatMetric};
use shared::dtos::{MonitorPointDto, MonitorSeriesDto, MonitorStatDto};
use std::path::PathBuf;
use std::time::Duration;
use store::Tier;
use tracing::{error, info};

use crate::AppState;

const DEFAULT_RANGE_MS: i64 = 24 * 60 * 60 * 1000;
const DEFAULT_POINTS: i64 = 600;
const MAX_POINTS: i64 = 2_000;
const MIN_POINTS: i64 = 50;
const ROLLUP_INTERVAL: Duration = Duration::from_secs(60);
/// rows land up to a minute late (sampler flush), so roll up a little further back
const ROLLUP_LOOKBACK_MS: i64 = 5 * 60 * 1000;
const PRUNE_EVERY_N_ROLLUPS: u32 = 60;

/// Imports the legacy CSVs (once), catches up rollups for any downtime, then runs the
/// sampler and the rollup/retention loop in the background.
pub(crate) async fn start(db: MonitorDb, legacy_csv_dir: PathBuf) {
    if let Err(e) = csv_import::import_legacy_csv(&db, &legacy_csv_dir).await {
        error!("Legacy monitoring CSV import failed: {e}");
    }
    match db.earliest_ms(Tier::FiveSeconds).await {
        Ok(Some(earliest)) => {
            let now = now_ms();
            if let Err(e) = db.rollup(earliest, now).await {
                error!("Monitoring catch-up rollup failed: {e}");
            }
            prune(&db, now).await;
        }
        Ok(None) => {}
        Err(e) => error!("Failed to read monitoring data: {e}"),
    }

    tokio::spawn(sampler::run_sampler(db.clone()));
    tokio::spawn(maintain(db));
}

async fn maintain(db: MonitorDb) {
    let mut interval = tokio::time::interval(ROLLUP_INTERVAL);
    let mut runs = 0u32;
    loop {
        interval.tick().await;
        let now = now_ms();
        if let Err(e) = db.rollup(now - ROLLUP_LOOKBACK_MS, now).await {
            error!("Monitoring rollup failed: {e}");
        }
        runs += 1;
        if runs.is_multiple_of(PRUNE_EVERY_N_ROLLUPS) {
            prune(&db, now).await;
        }
    }
}

async fn prune(db: &MonitorDb, now: i64) {
    match db.prune(now).await {
        Ok(0) => {}
        Ok(n) => info!("Pruned {n} monitoring rows past retention"),
        Err(e) => error!("Monitoring prune failed: {e}"),
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[derive(serde::Deserialize, Default)]
pub struct MonitorQuery {
    /// UTC epoch ms, default: 24h before `to`
    pub from: Option<i64>,
    /// UTC epoch ms, default: now
    pub to: Option<i64>,
    /// roughly how many buckets to return
    pub points: Option<i64>,
}

/// GET /api/monitor?from=&to=&points= - min/avg/max buckets from the best-fitting tier
pub(crate) async fn api_get_monitoring(
    State(state): State<AppState>,
    Query(q): Query<MonitorQuery>,
) -> Result<Json<MonitorSeriesDto>, (StatusCode, String)> {
    let now = now_ms();
    let to = q.to.unwrap_or(now);
    let from = q.from.unwrap_or(to - DEFAULT_RANGE_MS);
    if from >= to {
        return Err((StatusCode::BAD_REQUEST, "`from` must be before `to`".into()));
    }
    let points = q
        .points
        .unwrap_or(DEFAULT_POINTS)
        .clamp(MIN_POINTS, MAX_POINTS);

    let series = state
        .monitor_db
        .query(from, to, points, now)
        .await
        .map_err(|e| {
            error!("Monitoring query failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    Ok(Json(MonitorSeriesDto {
        resolution_ms: series.resolution_ms,
        earliest_ms: series.earliest_ms,
        points: series.rows.iter().map(to_dto).collect(),
    }))
}

fn to_dto(row: &MonitorRow) -> MonitorPointDto {
    let stat = |m: StatMetric| {
        row.stat(m).map(|Stat { min, avg, max }| MonitorStatDto {
            min: min as f32,
            avg: avg as f32,
            max: max as f32,
        })
    };
    MonitorPointDto {
        t: row.bucket_ms,
        n: row.sample_count.max(0) as u32,
        sys_cpu: stat(StatMetric::SysCpu),
        app_cpu: stat(StatMetric::AppCpu),
        sys_mem: stat(StatMetric::SysMem),
        app_mem: stat(StatMetric::AppMem),
        swap: stat(StatMetric::Swap),
        load1: stat(StatMetric::Load1),
        cpu_temp: stat(StatMetric::CpuTemp),
        net_rx: stat(StatMetric::NetRx),
        net_tx: stat(StatMetric::NetTx),
        app_read: stat(StatMetric::AppRead),
        app_write: stat(StatMetric::AppWrite),
        disk_used: row.disk_used.map(|v| v as f32),
        disk_free_gib: row.disk_free_gib.map(|v| v as f32),
        throttled: row.throttled.map(|v| (v & 0xF) as u8),
    }
}
