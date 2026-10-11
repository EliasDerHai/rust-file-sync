//! The single list of monitored metrics. Every SQL column list in `store.rs` is
//! generated from `StatMetric::ALL`, so adding a metric means one variant here plus
//! one migration in `migrations_monitor/`.

/// Metrics stored as min/avg/max per bucket (`{column}_min`, `_avg`, `_max`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatMetric {
    /// %
    SysCpu,
    /// %
    AppCpu,
    /// % of total memory
    SysMem,
    /// % of total memory
    AppMem,
    /// % of total swap
    Swap,
    /// 1-minute load average
    Load1,
    /// °C
    CpuTemp,
    /// bytes/s, all interfaces except loopback
    NetRx,
    /// bytes/s, all interfaces except loopback
    NetTx,
    /// bytes/s read by this process
    AppRead,
    /// bytes/s written by this process
    AppWrite,
}

pub(crate) const STAT_METRIC_COUNT: usize = 11;

impl StatMetric {
    pub(crate) const ALL: [StatMetric; STAT_METRIC_COUNT] = [
        StatMetric::SysCpu,
        StatMetric::AppCpu,
        StatMetric::SysMem,
        StatMetric::AppMem,
        StatMetric::Swap,
        StatMetric::Load1,
        StatMetric::CpuTemp,
        StatMetric::NetRx,
        StatMetric::NetTx,
        StatMetric::AppRead,
        StatMetric::AppWrite,
    ];

    pub(crate) fn column(self) -> &'static str {
        match self {
            StatMetric::SysCpu => "sys_cpu",
            StatMetric::AppCpu => "app_cpu",
            StatMetric::SysMem => "sys_mem",
            StatMetric::AppMem => "app_mem",
            StatMetric::Swap => "swap",
            StatMetric::Load1 => "load1",
            StatMetric::CpuTemp => "cpu_temp",
            StatMetric::NetRx => "net_rx",
            StatMetric::NetTx => "net_tx",
            StatMetric::AppRead => "app_read",
            StatMetric::AppWrite => "app_write",
        }
    }

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Stat {
    pub min: f64,
    pub avg: f64,
    pub max: f64,
}

impl Stat {
    pub(crate) fn single(value: f64) -> Self {
        Stat {
            min: value,
            avg: value,
            max: value,
        }
    }
}

/// One stored bucket in any tier.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MonitorRow {
    /// bucket start, UTC epoch ms
    pub bucket_ms: i64,
    pub sample_count: i64,
    /// indexed by `StatMetric::index`
    pub stats: [Option<Stat>; STAT_METRIC_COUNT],
    pub disk_used: Option<f64>,
    pub disk_free_gib: Option<f64>,
    /// Pi `get_throttled` bits 0-3, OR-ed over the bucket
    pub throttled: Option<i64>,
}

impl MonitorRow {
    pub(crate) fn stat(&self, metric: StatMetric) -> Option<Stat> {
        self.stats[metric.index()]
    }
}

/// One 1s reading. `None` = not available on this machine (e.g. no thermal zone).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Sample {
    pub values: [Option<f64>; STAT_METRIC_COUNT],
    pub disk_used: Option<f64>,
    pub disk_free_gib: Option<f64>,
    pub throttled: Option<i64>,
}

impl Sample {
    pub(crate) fn set(&mut self, metric: StatMetric, value: Option<f64>) {
        self.values[metric.index()] = value;
    }
}

/// Folds the 1s samples of one bucket into a stored row: min/avg/max per metric over
/// the samples that have a value, plain mean for disk, OR for the throttle bits.
pub(crate) fn aggregate(bucket_ms: i64, samples: &[Sample]) -> MonitorRow {
    let mut stats = [None; STAT_METRIC_COUNT];
    for metric in StatMetric::ALL {
        let values: Vec<f64> = samples
            .iter()
            .filter_map(|s| s.values[metric.index()])
            .collect();
        stats[metric.index()] = stat_of(&values);
    }

    let mean = |values: Vec<f64>| stat_of(&values).map(|s| s.avg);
    let throttled = samples
        .iter()
        .filter_map(|s| s.throttled)
        .reduce(|a, b| a | b);

    MonitorRow {
        bucket_ms,
        sample_count: samples.len() as i64,
        stats,
        disk_used: mean(samples.iter().filter_map(|s| s.disk_used).collect()),
        disk_free_gib: mean(samples.iter().filter_map(|s| s.disk_free_gib).collect()),
        throttled,
    }
}

fn stat_of(values: &[f64]) -> Option<Stat> {
    if values.is_empty() {
        return None;
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let avg = values.iter().sum::<f64>() / values.len() as f64;
    Some(Stat { min, avg, max })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(cpu: Option<f64>, throttled: Option<i64>) -> Sample {
        let mut s = Sample {
            throttled,
            disk_used: Some(40.0),
            ..Default::default()
        };
        s.set(StatMetric::SysCpu, cpu);
        s
    }

    #[test]
    fn index_matches_position_in_all() {
        for (i, metric) in StatMetric::ALL.iter().enumerate() {
            assert_eq!(metric.index(), i);
        }
    }

    #[test]
    fn aggregate_computes_min_avg_max_and_ors_throttle_bits() {
        let samples = [
            sample(Some(10.0), Some(0b0001)),
            sample(Some(90.0), Some(0)),
            sample(Some(20.0), Some(0b0100)),
        ];
        let row = aggregate(5_000, &samples);

        assert_eq!(row.bucket_ms, 5_000);
        assert_eq!(row.sample_count, 3);
        assert_eq!(
            row.stat(StatMetric::SysCpu),
            Some(Stat {
                min: 10.0,
                avg: 40.0,
                max: 90.0
            })
        );
        assert_eq!(row.throttled, Some(0b0101));
        assert_eq!(row.disk_used, Some(40.0));
    }

    #[test]
    fn aggregate_skips_missing_values() {
        let samples = [sample(None, None), sample(Some(30.0), None)];
        let row = aggregate(0, &samples);

        assert_eq!(row.stat(StatMetric::SysCpu), Some(Stat::single(30.0)));
        assert_eq!(row.stat(StatMetric::CpuTemp), None);
        assert_eq!(row.throttled, None);
    }
}
