//! Samples the system once per second and stores one min/avg/max row per 5s bucket.
//! Rows are buffered and written once a minute in a single transaction, which keeps
//! writes to the Pi's SD card rare; a crash loses at most that last minute.

use super::metrics::{Sample, StatMetric, aggregate};
use super::store::{MonitorDb, Tier};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, Pid, ProcessesToUpdate, System};
use tokio::time::MissedTickBehavior;
use tracing::{error, info, warn};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const FLUSH_INTERVAL: Duration = Duration::from_secs(60);
/// disk usage moves slowly and enumerating disks isn't free
const DISK_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
/// rows kept in memory while the DB keeps failing (~1h) before the oldest are dropped
const MAX_PENDING_ROWS: usize = 720;

const THERMAL_ZONES: &str = "/sys/class/thermal";
/// Raspberry Pi firmware: same bitmask as `vcgencmd get_throttled`
const PI_THROTTLED: &str = "/sys/devices/platform/soc/soc:firmware/get_throttled";

/// start of the wall-clock aligned bucket containing `ts_ms`
pub(crate) fn bucket_of(ts_ms: i64, bucket_ms: i64) -> i64 {
    ts_ms.div_euclid(bucket_ms) * bucket_ms
}

pub(crate) async fn run_sampler(db: MonitorDb) {
    let mut probe = Probe::new();
    let mut interval = tokio::time::interval(SAMPLE_INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let bucket_ms = Tier::FiveSeconds.resolution_ms();
    let mut current_bucket: Option<i64> = None;
    let mut samples: Vec<Sample> = Vec::with_capacity(5);
    let mut pending = Vec::new();
    let mut last_flush = Instant::now();

    loop {
        interval.tick().await;
        let bucket = bucket_of(chrono::Utc::now().timestamp_millis(), bucket_ms);
        if current_bucket != Some(bucket) {
            if let Some(closed) = current_bucket
                && !samples.is_empty()
            {
                pending.push(aggregate(closed, &samples));
                samples.clear();
            }
            current_bucket = Some(bucket);
        }
        samples.push(probe.sample());

        if last_flush.elapsed() >= FLUSH_INTERVAL && !pending.is_empty() {
            last_flush = Instant::now();
            match db.insert_rows(Tier::FiveSeconds, &pending).await {
                Ok(()) => pending.clear(),
                Err(e) => {
                    error!("Failed to write monitoring rows: {e}");
                    if pending.len() > MAX_PENDING_ROWS {
                        let excess = pending.len() - MAX_PENDING_ROWS;
                        pending.drain(..excess);
                        warn!("Dropped {excess} buffered monitoring rows");
                    }
                }
            }
        }
    }
}

/// Holds the sysinfo handles between ticks: CPU usage, network and disk I/O are all
/// deltas since the previous refresh, so the handles must be reused, never recreated.
struct Probe {
    system: System,
    networks: Networks,
    pid: Pid,
    last_tick: Instant,
    disk: (Option<f64>, Option<f64>),
    disk_refreshed: Option<Instant>,
    cpu_temp_file: Option<PathBuf>,
}

impl Probe {
    fn new() -> Self {
        let mut system = System::new();
        system.refresh_memory();
        // baseline for the first usage delta
        system.refresh_cpu_usage();
        let pid = sysinfo::get_current_pid().expect("Failed to get current PID");
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        Self {
            system,
            networks: Networks::new_with_refreshed_list(),
            pid,
            last_tick: Instant::now(),
            disk: (None, None),
            disk_refreshed: None,
            cpu_temp_file: find_cpu_thermal_zone(Path::new(THERMAL_ZONES)),
        }
    }

    fn sample(&mut self) -> Sample {
        // targeted refreshes only - `refresh_all` would walk every process each second
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system
            .refresh_processes(ProcessesToUpdate::Some(&[self.pid]), true);
        self.networks.refresh(true);
        let elapsed = self.last_tick.elapsed().as_secs_f64().max(0.001);
        self.last_tick = Instant::now();

        let total_mem = self.system.total_memory() as f64;
        let percent_of_mem =
            |bytes: u64| (total_mem > 0.0).then(|| bytes as f64 / total_mem * 100.0);
        let process = self.system.process(self.pid);

        let mut s = Sample::default();
        s.set(
            StatMetric::SysCpu,
            Some(self.system.global_cpu_usage() as f64),
        );
        // not normalized by core count (can exceed 100%), same as the imported history
        s.set(StatMetric::AppCpu, process.map(|p| p.cpu_usage() as f64));
        s.set(
            StatMetric::SysMem,
            percent_of_mem(self.system.used_memory()),
        );
        s.set(
            StatMetric::AppMem,
            process.and_then(|p| percent_of_mem(p.memory())),
        );
        let total_swap = self.system.total_swap();
        s.set(
            StatMetric::Swap,
            (total_swap > 0).then(|| self.system.used_swap() as f64 / total_swap as f64 * 100.0),
        );
        s.set(StatMetric::Load1, Some(System::load_average().one));
        s.set(
            StatMetric::CpuTemp,
            self.cpu_temp_file.as_deref().and_then(read_cpu_temp),
        );

        let (rx, tx) = self
            .networks
            .iter()
            .filter(|(name, _)| name.as_str() != "lo")
            .fold((0u64, 0u64), |(rx, tx), (_, data)| {
                (rx + data.received(), tx + data.transmitted())
            });
        s.set(StatMetric::NetRx, Some(rx as f64 / elapsed));
        s.set(StatMetric::NetTx, Some(tx as f64 / elapsed));

        let io = process.map(|p| p.disk_usage());
        s.set(
            StatMetric::AppRead,
            io.map(|u| u.read_bytes as f64 / elapsed),
        );
        s.set(
            StatMetric::AppWrite,
            io.map(|u| u.written_bytes as f64 / elapsed),
        );

        (s.disk_used, s.disk_free_gib) = self.root_disk();
        s.throttled = read_throttled(Path::new(PI_THROTTLED));
        s
    }

    /// (used %, free GiB) of the root filesystem, refreshed at most once a minute
    fn root_disk(&mut self) -> (Option<f64>, Option<f64>) {
        if self
            .disk_refreshed
            .is_none_or(|t| t.elapsed() >= DISK_REFRESH_INTERVAL)
        {
            self.disk_refreshed = Some(Instant::now());
            let disks = Disks::new_with_refreshed_list();
            self.disk = disks
                .list()
                .iter()
                .find(|d| d.mount_point() == Path::new("/"))
                .filter(|d| d.total_space() > 0)
                .map(|d| {
                    let total = d.total_space() as f64;
                    let available = d.available_space() as f64;
                    (
                        Some((total - available) / total * 100.0),
                        Some(available / 1024f64.powi(3)),
                    )
                })
                .unwrap_or((None, None));
        }
        self.disk
    }
}

/// The thermal zone that actually measures the CPU (`cpu-thermal` on the Pi,
/// `x86_pkg_temp` on Intel). Other zones such as `acpitz` often report a fixed,
/// meaningless value - storing nothing is better than years of junk.
fn find_cpu_thermal_zone(dir: &Path) -> Option<PathBuf> {
    let mut zones: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("thermal_zone"))
        })
        .collect();
    zones.sort();
    let zone = zones.into_iter().find(|zone| {
        std::fs::read_to_string(zone.join("type")).is_ok_and(|t| {
            let t = t.trim().to_ascii_lowercase();
            t.contains("cpu") || t.contains("x86_pkg") || t.contains("soc")
        })
    });
    match &zone {
        Some(z) => info!("CPU temperature from {}", z.display()),
        None => info!("No CPU thermal zone found - CPU temperature not recorded"),
    }
    zone.map(|z| z.join("temp"))
}

/// millidegrees in sysfs -> °C
fn read_cpu_temp(path: &Path) -> Option<f64> {
    let raw = std::fs::read_to_string(path).ok()?;
    raw.trim().parse::<f64>().ok().map(|m| m / 1000.0)
}

/// Pi firmware bitmask (hex, e.g. `50005`); only the "currently" bits 0-3 are kept -
/// the "since boot" bits 16-19 are redundant when sampling every second
fn read_throttled(path: &Path) -> Option<i64> {
    let raw = std::fs::read_to_string(path).ok()?;
    let hex = raw.trim().trim_start_matches("0x");
    i64::from_str_radix(hex, 16).ok().map(|bits| bits & 0xF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_are_wall_clock_aligned() {
        assert_eq!(bucket_of(0, 5_000), 0);
        assert_eq!(bucket_of(4_999, 5_000), 0);
        assert_eq!(bucket_of(5_000, 5_000), 5_000);
        assert_eq!(bucket_of(12_345, 5_000), 10_000);
    }

    #[test]
    fn parses_sysfs_values() {
        let dir = tempfile::tempdir().unwrap();
        let temp = dir.path().join("temp");
        std::fs::write(&temp, "48312\n").unwrap();
        assert_eq!(read_cpu_temp(&temp), Some(48.312));

        let throttled = dir.path().join("get_throttled");
        std::fs::write(&throttled, "50005\n").unwrap();
        // under-voltage (bit 0) + throttled (bit 2) now; since-boot bits dropped
        assert_eq!(read_throttled(&throttled), Some(0b0101));
        std::fs::write(&throttled, "0x0\n").unwrap();
        assert_eq!(read_throttled(&throttled), Some(0));

        assert_eq!(read_cpu_temp(&dir.path().join("missing")), None);

        // only a cpu-like zone is picked, never e.g. acpitz
        let zones = dir.path().join("thermal");
        for (zone, kind) in [
            ("thermal_zone0", "acpitz"),
            ("thermal_zone1", "cpu-thermal"),
        ] {
            std::fs::create_dir_all(zones.join(zone)).unwrap();
            std::fs::write(zones.join(zone).join("type"), format!("{kind}\n")).unwrap();
        }
        assert_eq!(
            find_cpu_thermal_zone(&zones),
            Some(zones.join("thermal_zone1").join("temp"))
        );
        std::fs::write(zones.join("thermal_zone1").join("type"), "acpitz\n").unwrap();
        assert_eq!(find_cpu_thermal_zone(&zones), None);
        assert_eq!(read_throttled(&dir.path().join("missing")), None);
    }
}
