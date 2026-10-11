//! `data/monitor.db`: three tiers of identically shaped tables (see
//! `migrations_monitor/`). Queries are runtime-checked on purpose - table and column
//! names are generated from `StatMetric::ALL`, and this keeps the monitoring schema
//! out of `comptime.db` / `.sqlx`. The tests below are what cover the SQL.

use super::metrics::{MonitorRow, STAT_METRIC_COUNT, Stat, StatMetric};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteRow, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use std::path::Path;

static MONITOR_MIGRATOR: Migrator = sqlx::migrate!("./migrations_monitor");

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tier {
    FiveSeconds,
    Minute,
    Hour,
}

impl Tier {
    /// finest first
    const ALL: [Tier; 3] = [Tier::FiveSeconds, Tier::Minute, Tier::Hour];

    pub(crate) fn table(self) -> &'static str {
        match self {
            Tier::FiveSeconds => "monitor_5s",
            Tier::Minute => "monitor_1m",
            Tier::Hour => "monitor_1h",
        }
    }

    pub(crate) fn resolution_ms(self) -> i64 {
        match self {
            Tier::FiveSeconds => 5_000,
            Tier::Minute => MINUTE_MS,
            Tier::Hour => HOUR_MS,
        }
    }

    /// `None` = kept forever
    fn retention_ms(self) -> Option<i64> {
        match self {
            Tier::FiveSeconds => Some(31 * DAY_MS),
            Tier::Minute => Some(92 * DAY_MS),
            Tier::Hour => None,
        }
    }
}

/// Picks the tier to read `[from, to)` from and the bucket size to re-aggregate it to,
/// so the result has roughly `points` rows: the coarsest tier that is still at least
/// as fine as the target bucket, among the tiers whose retention still covers `from`.
pub(crate) fn choose_tier(from_ms: i64, to_ms: i64, points: i64, now_ms: i64) -> (Tier, i64) {
    let target = ((to_ms - from_ms) / points.max(1)).max(1);
    let covering: Vec<Tier> = Tier::ALL
        .into_iter()
        .filter(|t| t.retention_ms().is_none_or(|r| from_ms >= now_ms - r))
        .collect();
    let tier = covering
        .iter()
        .rev()
        .find(|t| t.resolution_ms() <= target)
        .or(covering.first())
        .copied()
        .unwrap_or(Tier::Hour);
    let res = tier.resolution_ms();
    (tier, (target / res * res).max(res))
}

pub(crate) struct MonitorSeries {
    pub resolution_ms: i64,
    /// oldest data point at all (any range), for the SPA's "all" range and zoom limits
    pub earliest_ms: Option<i64>,
    pub rows: Vec<MonitorRow>,
}

/// `bucket_ms, sample_count, {m}_min, {m}_avg, {m}_max..., disk_used, disk_free_gib, throttled`
fn all_columns() -> Vec<String> {
    let mut cols = vec!["bucket_ms".to_string(), "sample_count".to_string()];
    for metric in StatMetric::ALL {
        let c = metric.column();
        cols.extend([format!("{c}_min"), format!("{c}_avg"), format!("{c}_max")]);
    }
    cols.extend(["disk_used", "disk_free_gib", "throttled"].map(String::from));
    cols
}

/// count-weighted average that ignores buckets without a value
fn weighted_avg(col: &str) -> String {
    format!("SUM({col} * sample_count) / SUM(CASE WHEN {col} IS NOT NULL THEN sample_count END)")
}

/// Re-aggregates rows of `source` in `[?1, ?2)` into buckets of `bucket` ms, producing
/// the columns of `all_columns()` in order. SQLite has no BIT_OR, hence the per-bit MAX.
fn aggregate_select(source: &str, bucket: i64) -> String {
    let mut exprs = vec![
        format!("(bucket_ms / {bucket}) * {bucket} AS bucket_ms"),
        "SUM(sample_count) AS sample_count".to_string(),
    ];
    for metric in StatMetric::ALL {
        let c = metric.column();
        exprs.push(format!("MIN({c}_min) AS {c}_min"));
        exprs.push(format!("{} AS {c}_avg", weighted_avg(&format!("{c}_avg"))));
        exprs.push(format!("MAX({c}_max) AS {c}_max"));
    }
    exprs.push(format!("{} AS disk_used", weighted_avg("disk_used")));
    exprs.push(format!(
        "{} AS disk_free_gib",
        weighted_avg("disk_free_gib")
    ));
    exprs.push(
        "MAX(throttled & 1) | MAX(throttled & 2) | MAX(throttled & 4) | MAX(throttled & 8) AS throttled"
            .to_string(),
    );
    format!(
        "SELECT {} FROM {source} WHERE bucket_ms >= ?1 AND bucket_ms < ?2 GROUP BY 1",
        exprs.join(", ")
    )
}

fn row_from_sqlite(row: &SqliteRow) -> sqlx::Result<MonitorRow> {
    let mut stats = [None; STAT_METRIC_COUNT];
    for metric in StatMetric::ALL {
        let c = metric.column();
        let min: Option<f64> = row.try_get(format!("{c}_min").as_str())?;
        let avg: Option<f64> = row.try_get(format!("{c}_avg").as_str())?;
        let max: Option<f64> = row.try_get(format!("{c}_max").as_str())?;
        stats[metric.index()] = match (min, avg, max) {
            (Some(min), Some(avg), Some(max)) => Some(Stat { min, avg, max }),
            _ => None,
        };
    }
    Ok(MonitorRow {
        bucket_ms: row.try_get("bucket_ms")?,
        sample_count: row.try_get("sample_count")?,
        stats,
        disk_used: row.try_get("disk_used")?,
        disk_free_gib: row.try_get("disk_free_gib")?,
        throttled: row.try_get("throttled")?,
    })
}

#[derive(Clone)]
pub(crate) struct MonitorDb {
    pool: SqlitePool,
}

impl MonitorDb {
    pub(crate) async fn open(path: &Path) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            // one write per minute plus rollups - WAL + NORMAL keeps fsyncs (SD card) down
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
        let pool = SqlitePool::connect_with(opts).await?;
        MONITOR_MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    #[cfg(test)]
    pub(crate) async fn in_memory() -> Self {
        // single connection: every `:memory:` connection is its own empty database
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        MONITOR_MIGRATOR.run(&pool).await.unwrap();
        Self { pool }
    }

    /// Upserts rows into `tier` in one transaction.
    pub(crate) async fn insert_rows(&self, tier: Tier, rows: &[MonitorRow]) -> sqlx::Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let cols = all_columns();
        let sql = format!(
            "INSERT OR REPLACE INTO {} ({}) VALUES ({})",
            tier.table(),
            cols.join(", "),
            vec!["?"; cols.len()].join(", ")
        );
        let mut tx = self.pool.begin().await?;
        for row in rows {
            let mut q = sqlx::query(&sql).bind(row.bucket_ms).bind(row.sample_count);
            for stat in &row.stats {
                q = q
                    .bind(stat.map(|s| s.min))
                    .bind(stat.map(|s| s.avg))
                    .bind(stat.map(|s| s.max));
            }
            q.bind(row.disk_used)
                .bind(row.disk_free_gib)
                .bind(row.throttled)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await
    }

    /// Recomputes the 1m tier from 5s rows, then the 1h tier from 1m rows, for every
    /// bucket touching `[from_ms, to_ms)`. Idempotent, so partial (current) buckets and
    /// overlapping windows are fine - they're simply replaced on the next run.
    pub(crate) async fn rollup(&self, from_ms: i64, to_ms: i64) -> sqlx::Result<()> {
        for (source, target) in [
            (Tier::FiveSeconds, Tier::Minute),
            (Tier::Minute, Tier::Hour),
        ] {
            let res = target.resolution_ms();
            let start = from_ms.div_euclid(res) * res;
            let end = (to_ms.div_euclid(res) + 1) * res;
            let sql = format!(
                "INSERT OR REPLACE INTO {} ({}) {}",
                target.table(),
                all_columns().join(", "),
                aggregate_select(source.table(), res)
            );
            sqlx::query(&sql)
                .bind(start)
                .bind(end)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Drops rows that fell out of their tier's retention.
    pub(crate) async fn prune(&self, now_ms: i64) -> sqlx::Result<u64> {
        let mut deleted = 0;
        for tier in Tier::ALL {
            if let Some(retention) = tier.retention_ms() {
                let sql = format!("DELETE FROM {} WHERE bucket_ms < ?", tier.table());
                deleted += sqlx::query(&sql)
                    .bind(now_ms - retention)
                    .execute(&self.pool)
                    .await?
                    .rows_affected();
            }
        }
        Ok(deleted)
    }

    pub(crate) async fn earliest_ms(&self, tier: Tier) -> sqlx::Result<Option<i64>> {
        let sql = format!("SELECT MIN(bucket_ms) FROM {}", tier.table());
        sqlx::query_scalar(&sql).fetch_one(&self.pool).await
    }

    /// About `points` rows covering `[from_ms, to_ms)` from the best-fitting tier.
    pub(crate) async fn query(
        &self,
        from_ms: i64,
        to_ms: i64,
        points: i64,
        now_ms: i64,
    ) -> sqlx::Result<MonitorSeries> {
        let (tier, bucket) = choose_tier(from_ms, to_ms, points, now_ms);
        let sql = format!("{} ORDER BY 1", aggregate_select(tier.table(), bucket));
        let rows = sqlx::query(&sql)
            .bind(from_ms)
            .bind(to_ms)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(row_from_sqlite)
            .collect::<sqlx::Result<Vec<_>>>()?;
        Ok(MonitorSeries {
            resolution_ms: bucket,
            earliest_ms: self.earliest_ms(Tier::Hour).await?,
            rows,
        })
    }

    pub(crate) async fn get_meta(&self, key: &str) -> sqlx::Result<Option<String>> {
        sqlx::query_scalar("SELECT value FROM monitor_meta WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
    }

    pub(crate) async fn set_meta(&self, key: &str, value: &str) -> sqlx::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO monitor_meta (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(bucket_ms: i64, count: i64, cpu: Stat, throttled: Option<i64>) -> MonitorRow {
        let mut stats = [None; STAT_METRIC_COUNT];
        stats[StatMetric::SysCpu.index()] = Some(cpu);
        MonitorRow {
            bucket_ms,
            sample_count: count,
            stats,
            disk_used: Some(50.0),
            disk_free_gib: None,
            throttled,
        }
    }

    async fn rows_of(db: &MonitorDb, tier: Tier) -> Vec<MonitorRow> {
        let sql = format!("SELECT * FROM {} ORDER BY bucket_ms", tier.table());
        sqlx::query(&sql)
            .fetch_all(&db.pool)
            .await
            .unwrap()
            .iter()
            .map(|r| row_from_sqlite(r).unwrap())
            .collect()
    }

    #[test]
    fn choose_tier_by_span_and_retention() {
        let now = 400 * DAY_MS;
        let cases = [
            // (span, expected tier, expected bucket)
            (10 * MINUTE_MS, Tier::FiveSeconds, 5_000), // finer than any tier -> finest
            (2 * HOUR_MS, Tier::FiveSeconds, 10_000),
            (DAY_MS, Tier::Minute, 2 * MINUTE_MS),
            (7 * DAY_MS, Tier::Minute, 16 * MINUTE_MS),
            (30 * DAY_MS, Tier::Hour, HOUR_MS),
            (365 * DAY_MS, Tier::Hour, 14 * HOUR_MS),
        ];
        for (span, tier, bucket) in cases {
            assert_eq!(
                choose_tier(now - span, now, 600, now),
                (tier, bucket),
                "span {span}"
            );
        }
        // a short window 2 months back: the 5s tier no longer covers it
        let from = now - 60 * DAY_MS;
        assert_eq!(
            choose_tier(from, from + HOUR_MS, 600, now),
            (Tier::Minute, MINUTE_MS)
        );
        // a short window a year back: only hourly data is left
        let from = now - 365 * DAY_MS;
        assert_eq!(
            choose_tier(from, from + HOUR_MS, 600, now),
            (Tier::Hour, HOUR_MS)
        );
    }

    #[tokio::test]
    async fn rollup_weights_averages_and_keeps_extremes() {
        let db = MonitorDb::in_memory().await;
        db.insert_rows(
            Tier::FiveSeconds,
            &[
                row(
                    0,
                    5,
                    Stat {
                        min: 1.0,
                        avg: 2.0,
                        max: 3.0,
                    },
                    Some(0b0001),
                ),
                row(
                    5_000,
                    1,
                    Stat {
                        min: 5.0,
                        avg: 8.0,
                        max: 99.0,
                    },
                    Some(0),
                ),
                row(
                    10_000,
                    4,
                    Stat {
                        min: 0.5,
                        avg: 5.0,
                        max: 6.0,
                    },
                    None,
                ),
                // next minute
                row(MINUTE_MS, 5, Stat::single(10.0), Some(0b0100)),
            ],
        )
        .await
        .unwrap();

        db.rollup(0, 2 * MINUTE_MS).await.unwrap();

        let minutes = rows_of(&db, Tier::Minute).await;
        assert_eq!(minutes.len(), 2);
        let first = &minutes[0];
        assert_eq!(first.bucket_ms, 0);
        assert_eq!(first.sample_count, 10);
        let cpu = first.stat(StatMetric::SysCpu).unwrap();
        assert_eq!((cpu.min, cpu.max), (0.5, 99.0));
        // (2*5 + 8*1 + 5*4) / 10
        assert!((cpu.avg - 3.8).abs() < 1e-9);
        assert_eq!(first.throttled, Some(0b0001));
        assert_eq!(first.stat(StatMetric::CpuTemp), None);

        let hours = rows_of(&db, Tier::Hour).await;
        assert_eq!(hours.len(), 1);
        assert_eq!(hours[0].sample_count, 15);
        assert_eq!(hours[0].throttled, Some(0b0101));
        let cpu = hours[0].stat(StatMetric::SysCpu).unwrap();
        assert!((cpu.avg - (3.8 * 10.0 + 10.0 * 5.0) / 15.0).abs() < 1e-9);

        // idempotent
        db.rollup(0, 2 * MINUTE_MS).await.unwrap();
        assert_eq!(rows_of(&db, Tier::Minute).await, minutes);
        assert_eq!(rows_of(&db, Tier::Hour).await, hours);
    }

    #[tokio::test]
    async fn prune_respects_tier_retention() {
        let db = MonitorDb::in_memory().await;
        let now = 200 * DAY_MS;
        let old = now - 40 * DAY_MS; // past 5s retention, within 1m retention
        let ancient = now - 100 * DAY_MS; // past 1m retention
        for tier in Tier::ALL {
            db.insert_rows(
                tier,
                &[
                    row(ancient, 1, Stat::single(1.0), None),
                    row(old, 1, Stat::single(1.0), None),
                    row(now, 1, Stat::single(1.0), None),
                ],
            )
            .await
            .unwrap();
        }

        db.prune(now).await.unwrap();

        let buckets = |rows: Vec<MonitorRow>| rows.iter().map(|r| r.bucket_ms).collect::<Vec<_>>();
        assert_eq!(buckets(rows_of(&db, Tier::FiveSeconds).await), [now]);
        assert_eq!(buckets(rows_of(&db, Tier::Minute).await), [old, now]);
        assert_eq!(buckets(rows_of(&db, Tier::Hour).await), [ancient, old, now]);
    }

    #[tokio::test]
    async fn query_rebuckets_to_requested_point_count() {
        let db = MonitorDb::in_memory().await;
        let now = 10 * DAY_MS;
        let rows: Vec<MonitorRow> = (0..720)
            .map(|i| row(now - HOUR_MS + i * 5_000, 5, Stat::single(i as f64), None))
            .collect();
        db.insert_rows(Tier::FiveSeconds, &rows).await.unwrap();
        db.rollup(now - HOUR_MS, now).await.unwrap();

        let series = db.query(now - HOUR_MS, now, 60, now).await.unwrap();

        assert_eq!(series.resolution_ms, MINUTE_MS);
        assert_eq!(series.rows.len(), 60);
        // each 1-minute bucket spans 12 rows: max of the first is row 11
        assert_eq!(series.rows[0].stat(StatMetric::SysCpu).unwrap().max, 11.0);
        assert_eq!(series.earliest_ms, Some(now - HOUR_MS));
    }

    #[tokio::test]
    async fn meta_roundtrip() {
        let db = MonitorDb::in_memory().await;
        assert_eq!(db.get_meta("k").await.unwrap(), None);
        db.set_meta("k", "v").await.unwrap();
        assert_eq!(db.get_meta("k").await.unwrap().as_deref(), Some("v"));
    }
}
