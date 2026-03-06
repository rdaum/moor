// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};
use tracing::warn;

const CPU_SYSFS_ROOT: &str = "/sys/devices/system/cpu";
const MIN_HETEROGENEITY_RATIO: f64 = 0.10;
const PERFORMANCE_THRESHOLD_RATIO: f64 = 0.90;
const TASK_POOL_PINNING_MODE_AUTO: usize = 0;
const TASK_POOL_PINNING_MODE_PERFORMANCE: usize = 1;
const TASK_POOL_PINNING_MODE_NONE: usize = 2;
const UNSET_SERVICE_PERF_CORES: usize = usize::MAX;

#[derive(Debug, Clone)]
pub struct PerformanceCoreSelection {
    pub logical_processor_ids: Vec<usize>,
    pub source: &'static str,
    pub threshold: u32,
    pub min_metric: u32,
    pub max_metric: u32,
    pub metric_tiers: usize,
    pub physical_cores: usize,
    pub logical_processors: usize,
}

#[derive(Debug, Clone)]
pub enum DetectionResult {
    PerformanceCores(PerformanceCoreSelection),
    NoSelection { reason: &'static str },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ThreadClass {
    /// General runtime control-plane work (scheduler, RPC, etc).
    Performance,
    /// Worker/execution-plane threads (task execution pool).
    Worker,
    Efficient,
    Unpinned,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskPoolPinningMode {
    /// Default behavior: use detected performance tier when available, else unpinned.
    #[default]
    Auto,
    /// Force performance-core pinning path (falls back to unpinned if no tier is detected).
    Performance,
    /// Disable task-pool pinning entirely.
    None,
}

impl TaskPoolPinningMode {
    const fn as_usize(self) -> usize {
        match self {
            Self::Auto => TASK_POOL_PINNING_MODE_AUTO,
            Self::Performance => TASK_POOL_PINNING_MODE_PERFORMANCE,
            Self::None => TASK_POOL_PINNING_MODE_NONE,
        }
    }

    const fn from_usize(value: usize) -> Self {
        match value {
            TASK_POOL_PINNING_MODE_PERFORMANCE => Self::Performance,
            TASK_POOL_PINNING_MODE_NONE => Self::None,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct TaskPoolAffinityConfig {
    pub pinning_mode: TaskPoolPinningMode,
    pub service_perf_cores: Option<usize>,
}

#[derive(Debug, Clone)]
struct PhysicalCoreMetrics {
    logical_processor_ids: Vec<usize>,
    capacity: Option<u32>,
    max_freq_khz: Option<u32>,
}

#[derive(Debug)]
struct ThreadPlacement {
    service_performance_core_ids: Vec<usize>,
    worker_performance_core_ids: Vec<usize>,
    efficient_core_ids: Vec<usize>,
    next_service_perf: AtomicUsize,
    next_worker_perf: AtomicUsize,
    next_efficient: AtomicUsize,
}

impl ThreadPlacement {
    fn build() -> Self {
        let all_logical_processor_ids = detect_all_logical_processor_ids();
        let detection = detect_performance_cores();
        let all_performance_core_ids = match detection {
            Ok(DetectionResult::PerformanceCores(selection)) => selection.logical_processor_ids,
            _ => Vec::new(),
        };

        let efficient_core_ids = all_logical_processor_ids
            .iter()
            .copied()
            .filter(|core_id| !all_performance_core_ids.contains(core_id))
            .collect();

        let reserve_service = reserved_service_perf_core_count(all_performance_core_ids.len());
        let split_at = all_performance_core_ids
            .len()
            .saturating_sub(reserve_service);

        let mut worker_performance_core_ids = all_performance_core_ids[..split_at].to_vec();
        let service_performance_core_ids = all_performance_core_ids[split_at..].to_vec();

        // If reservation consumed all performance cores, do not reserve and keep all for workers.
        if worker_performance_core_ids.is_empty() {
            worker_performance_core_ids = all_performance_core_ids;
        }

        Self {
            service_performance_core_ids,
            worker_performance_core_ids,
            efficient_core_ids,
            next_service_perf: AtomicUsize::new(0),
            next_worker_perf: AtomicUsize::new(0),
            next_efficient: AtomicUsize::new(0),
        }
    }
}

static THREAD_PLACEMENT: OnceLock<ThreadPlacement> = OnceLock::new();
static TASK_WORKER_COUNT: AtomicUsize = AtomicUsize::new(0);
static TASK_POOL_PINNING_MODE: AtomicUsize = AtomicUsize::new(TaskPoolPinningMode::Auto.as_usize());
static TASK_POOL_SERVICE_PERF_CORES: AtomicUsize = AtomicUsize::new(UNSET_SERVICE_PERF_CORES);

thread_local! {
    static TASK_WORKER_INDEX: Cell<Option<usize>> = const { Cell::new(None) };
}

fn thread_placement() -> &'static ThreadPlacement {
    THREAD_PLACEMENT.get_or_init(ThreadPlacement::build)
}

pub fn task_pool_affinity_config() -> TaskPoolAffinityConfig {
    let service_perf_cores = match TASK_POOL_SERVICE_PERF_CORES.load(Ordering::Relaxed) {
        UNSET_SERVICE_PERF_CORES => None,
        value => Some(value),
    };
    TaskPoolAffinityConfig {
        pinning_mode: TaskPoolPinningMode::from_usize(
            TASK_POOL_PINNING_MODE.load(Ordering::Relaxed),
        ),
        service_perf_cores,
    }
}

pub fn set_task_pool_affinity_config(config: TaskPoolAffinityConfig) {
    if THREAD_PLACEMENT.get().is_some() && task_pool_affinity_config() != config {
        warn!(
            ?config,
            "Task pool affinity config changed after thread placement initialization; existing placement is unchanged"
        );
    }
    TASK_POOL_PINNING_MODE.store(config.pinning_mode.as_usize(), Ordering::Relaxed);
    TASK_POOL_SERVICE_PERF_CORES.store(
        config
            .service_perf_cores
            .unwrap_or(UNSET_SERVICE_PERF_CORES),
        Ordering::Relaxed,
    );
}

pub fn set_task_worker_count(count: usize) {
    TASK_WORKER_COUNT.store(count, Ordering::Relaxed);
}

pub fn task_worker_count() -> usize {
    TASK_WORKER_COUNT.load(Ordering::Relaxed)
}

pub fn set_current_task_worker_index(index: usize) {
    TASK_WORKER_INDEX.with(|slot| slot.set(Some(index)));
}

pub fn current_task_worker_index() -> Option<usize> {
    TASK_WORKER_INDEX.with(|slot| slot.get())
}

pub fn spawn_perf<T, F>(name: impl Into<String>, f: F) -> io::Result<std::thread::JoinHandle<T>>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    spawn_with_class(name, ThreadClass::Performance, f)
}

pub fn spawn_efficient<T, F>(
    name: impl Into<String>,
    f: F,
) -> io::Result<std::thread::JoinHandle<T>>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    spawn_with_class(name, ThreadClass::Efficient, f)
}

pub fn spawn_worker_perf<T, F>(
    name: impl Into<String>,
    f: F,
) -> io::Result<std::thread::JoinHandle<T>>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    spawn_with_class(name, ThreadClass::Worker, f)
}

pub fn spawn_with_class<T, F>(
    name: impl Into<String>,
    class: ThreadClass,
    f: F,
) -> io::Result<std::thread::JoinHandle<T>>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let name = name.into();
    let assigned_core_id = next_core_for_class(class);

    std::thread::Builder::new().name(name).spawn(move || {
        if let Some(core_id) = assigned_core_id
            && let Err(e) = pin_current_thread_to_core(core_id)
        {
            warn!(core_id, error = ?e, "Failed to pin thread to core");
        }
        f()
    })
}

pub fn set_current_thread_background_priority() -> io::Result<()> {
    #[cfg(unix)]
    {
        // Nice +10 is less aggressive than maximum demotion and usually succeeds unprivileged.
        let result = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 10) };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

pub fn pin_current_thread_to_core(core_id: usize) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        // We bind each worker to a single logical CPU to avoid migration churn.
        let mut cpuset: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::CPU_ZERO(&mut cpuset);
            libc::CPU_SET(core_id, &mut cpuset);
            let thread = libc::pthread_self();
            let result = libc::pthread_setaffinity_np(
                thread,
                std::mem::size_of::<libc::cpu_set_t>(),
                &cpuset,
            );
            if result != 0 {
                return Err(io::Error::from_raw_os_error(result));
            }
        }
    }

    Ok(())
}

/// Reset current thread affinity to all detected logical CPUs.
/// Useful when a parent thread is pinned and child threads should be truly unpinned.
pub fn unpin_current_thread() -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let mut cpuset: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::CPU_ZERO(&mut cpuset);
            for core_id in detect_all_logical_processor_ids() {
                libc::CPU_SET(core_id, &mut cpuset);
            }
            let thread = libc::pthread_self();
            let result = libc::pthread_setaffinity_np(
                thread,
                std::mem::size_of::<libc::cpu_set_t>(),
                &cpuset,
            );
            if result != 0 {
                return Err(io::Error::from_raw_os_error(result));
            }
        }
    }

    Ok(())
}

pub fn pin_current_thread_to_class(class: ThreadClass) -> io::Result<Option<usize>> {
    let Some(core_id) = next_core_for_class(class) else {
        return Ok(None);
    };
    pin_current_thread_to_core(core_id)?;
    Ok(Some(core_id))
}

pub fn detect_performance_cores() -> io::Result<DetectionResult> {
    #[cfg(not(target_os = "linux"))]
    {
        Ok(DetectionResult::NoSelection {
            reason: "cpu-tier pinning is only implemented for linux",
        })
    }

    #[cfg(target_os = "linux")]
    {
        let cores = read_physical_core_metrics()?;
        if cores.is_empty() {
            return Ok(DetectionResult::NoSelection {
                reason: "no CPU topology entries discovered in sysfs",
            });
        }

        if let Some(selection) =
            select_performance_cores_by_metric(&cores, |core| core.capacity, "cpu_capacity")
        {
            return Ok(DetectionResult::PerformanceCores(selection));
        }

        if let Some(selection) =
            select_performance_cores_by_metric(&cores, |core| core.max_freq_khz, "cpuinfo_max_freq")
        {
            return Ok(DetectionResult::PerformanceCores(selection));
        }

        Ok(DetectionResult::NoSelection {
            reason: "cpu_capacity and cpuinfo_max_freq did not provide a clear high-performance tier",
        })
    }
}

pub fn logical_core_count() -> usize {
    detect_all_logical_processor_ids().len()
}

pub fn physical_core_count() -> Option<usize> {
    #[cfg(target_os = "linux")]
    {
        read_physical_core_metrics().ok().map(|cores| cores.len())
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

pub fn worker_performance_core_ids() -> Vec<usize> {
    thread_placement().worker_performance_core_ids.clone()
}

pub fn service_performance_core_ids() -> Vec<usize> {
    thread_placement().service_performance_core_ids.clone()
}

pub fn worker_performance_core_ids_ref() -> &'static [usize] {
    &thread_placement().worker_performance_core_ids
}

pub fn service_performance_core_ids_ref() -> &'static [usize] {
    &thread_placement().service_performance_core_ids
}

fn next_core_for_class(class: ThreadClass) -> Option<usize> {
    let placement = thread_placement();
    match class {
        ThreadClass::Performance => next_round_robin(
            &placement.service_performance_core_ids,
            &placement.efficient_core_ids,
            &placement.next_service_perf,
        ),
        ThreadClass::Worker => next_round_robin(
            &placement.worker_performance_core_ids,
            &placement.service_performance_core_ids,
            &placement.next_worker_perf,
        ),
        ThreadClass::Efficient => next_round_robin(
            &placement.efficient_core_ids,
            &placement.service_performance_core_ids,
            &placement.next_efficient,
        ),
        ThreadClass::Unpinned => None,
    }
}

fn next_round_robin(
    primary_core_ids: &[usize],
    secondary_core_ids: &[usize],
    next_index: &AtomicUsize,
) -> Option<usize> {
    if !primary_core_ids.is_empty() {
        let index = next_index.fetch_add(1, Ordering::Relaxed);
        return Some(primary_core_ids[index % primary_core_ids.len()]);
    }

    if secondary_core_ids.is_empty() {
        return None;
    }

    let index = next_index.fetch_add(1, Ordering::Relaxed);
    Some(secondary_core_ids[index % secondary_core_ids.len()])
}

pub fn task_pool_pinning_mode() -> TaskPoolPinningMode {
    task_pool_affinity_config().pinning_mode
}

fn reserved_service_perf_core_count(total_perf_cores: usize) -> usize {
    if let Some(requested) = task_pool_affinity_config().service_perf_cores {
        return clamp_service_reservation(requested, total_perf_cores);
    }

    match total_perf_cores {
        0..=2 => 0,
        3..=7 => 1,
        _ => 2,
    }
}

fn clamp_service_reservation(requested: usize, total_perf_cores: usize) -> usize {
    if total_perf_cores <= 1 {
        return 0;
    }

    // Keep at least one performance core available for worker threads by default.
    requested.min(total_perf_cores - 1)
}

fn detect_all_logical_processor_ids() -> Vec<usize> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(logical_processor_ids) = read_logical_processor_ids()
            && !logical_processor_ids.is_empty()
        {
            return logical_processor_ids;
        }
    }

    let fallback_threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1);
    (0..fallback_threads).collect()
}

#[cfg(target_os = "linux")]
fn read_physical_core_metrics() -> io::Result<Vec<PhysicalCoreMetrics>> {
    let mut physical_core_map: HashMap<(usize, usize), PhysicalCoreMetrics> = HashMap::new();
    for logical_processor_id in read_logical_processor_ids()? {
        let topology_path = cpu_path(logical_processor_id).join("topology");
        let package_id = read_u32(topology_path.join("physical_package_id"))
            .ok()
            .map_or(0usize, |v| v as usize);
        let core_id = read_u32(topology_path.join("core_id"))
            .ok()
            .map_or(logical_processor_id, |v| v as usize);
        let key = (package_id, core_id);

        let entry = physical_core_map
            .entry(key)
            .or_insert_with(|| PhysicalCoreMetrics {
                logical_processor_ids: Vec::new(),
                capacity: None,
                max_freq_khz: None,
            });

        entry.logical_processor_ids.push(logical_processor_id);

        if let Ok(capacity) = read_u32(cpu_path(logical_processor_id).join("cpu_capacity")) {
            entry.capacity = Some(
                entry
                    .capacity
                    .map_or(capacity, |current| current.max(capacity)),
            );
        }

        if let Ok(max_freq_khz) = read_u32(
            cpu_path(logical_processor_id)
                .join("cpufreq")
                .join("cpuinfo_max_freq"),
        ) {
            entry.max_freq_khz = Some(
                entry
                    .max_freq_khz
                    .map_or(max_freq_khz, |current| current.max(max_freq_khz)),
            );
        }
    }

    let mut cores: Vec<_> = physical_core_map.into_values().collect();
    for core in &mut cores {
        core.logical_processor_ids.sort_unstable();
    }
    cores.sort_by(|left, right| {
        left.logical_processor_ids
            .first()
            .cmp(&right.logical_processor_ids.first())
    });

    Ok(cores)
}

#[cfg(target_os = "linux")]
fn read_logical_processor_ids() -> io::Result<Vec<usize>> {
    let mut logical_processor_ids = Vec::new();
    for entry in fs::read_dir(CPU_SYSFS_ROOT)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(cpu_suffix) = name.strip_prefix("cpu") else {
            continue;
        };
        let Ok(lp_id) = cpu_suffix.parse::<usize>() else {
            continue;
        };
        let topology_path = entry.path().join("topology");
        if topology_path.exists() {
            logical_processor_ids.push(lp_id);
        }
    }
    logical_processor_ids.sort_unstable();
    Ok(logical_processor_ids)
}

#[cfg(target_os = "linux")]
fn select_performance_cores_by_metric(
    cores: &[PhysicalCoreMetrics],
    metric: impl Fn(&PhysicalCoreMetrics) -> Option<u32>,
    source: &'static str,
) -> Option<PerformanceCoreSelection> {
    let mut values = Vec::new();
    for core in cores {
        if let Some(value) = metric(core) {
            values.push(value);
        }
    }

    if values.len() < 2 {
        return None;
    }

    values.sort_unstable();
    values.dedup();

    if values.len() < 2 {
        return None;
    }

    let min_metric = *values.first()?;
    let max_metric = *values.last()?;
    if max_metric == 0 {
        return None;
    }

    let heterogeneity_ratio = (max_metric - min_metric) as f64 / max_metric as f64;
    if heterogeneity_ratio < MIN_HETEROGENEITY_RATIO {
        return None;
    }

    let threshold = (max_metric as f64 * PERFORMANCE_THRESHOLD_RATIO).round() as u32;
    let mut logical_processor_ids = Vec::new();
    let mut selected_cores = 0usize;
    for core in cores {
        let Some(core_metric) = metric(core) else {
            continue;
        };
        if core_metric < threshold {
            continue;
        }
        selected_cores += 1;
        logical_processor_ids.extend_from_slice(&core.logical_processor_ids);
    }

    if logical_processor_ids.is_empty() || selected_cores == cores.len() {
        return None;
    }

    logical_processor_ids.sort_unstable();
    logical_processor_ids.dedup();

    Some(PerformanceCoreSelection {
        logical_processors: logical_processor_ids.len(),
        logical_processor_ids,
        source,
        threshold,
        min_metric,
        max_metric,
        metric_tiers: values.len(),
        physical_cores: cores.len(),
    })
}

fn read_u32(path: PathBuf) -> io::Result<u32> {
    let value = fs::read_to_string(&path)?;
    value
        .trim()
        .parse::<u32>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{path:?}: {e}")))
}

fn cpu_path(logical_processor_id: usize) -> PathBuf {
    Path::new(CPU_SYSFS_ROOT).join(format!("cpu{logical_processor_id}"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{
        PhysicalCoreMetrics, TaskPoolAffinityConfig, TaskPoolPinningMode,
        clamp_service_reservation, select_performance_cores_by_metric,
        set_task_pool_affinity_config, task_pool_affinity_config,
    };
    use std::sync::{Mutex, OnceLock};

    fn config_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn selects_high_tier_capacity_cores() {
        let cores = vec![
            PhysicalCoreMetrics {
                logical_processor_ids: vec![0],
                capacity: Some(718),
                max_freq_khz: None,
            },
            PhysicalCoreMetrics {
                logical_processor_ids: vec![1],
                capacity: Some(731),
                max_freq_khz: None,
            },
            PhysicalCoreMetrics {
                logical_processor_ids: vec![2],
                capacity: Some(997),
                max_freq_khz: None,
            },
            PhysicalCoreMetrics {
                logical_processor_ids: vec![3],
                capacity: Some(1024),
                max_freq_khz: None,
            },
        ];

        let selection =
            select_performance_cores_by_metric(&cores, |core| core.capacity, "cpu_capacity")
                .expect("expected high-tier selection");

        assert_eq!(selection.logical_processor_ids, vec![2, 3]);
    }

    #[test]
    fn task_pool_affinity_config_roundtrips() {
        let _g = config_test_lock();
        let old = task_pool_affinity_config();
        let new = TaskPoolAffinityConfig {
            pinning_mode: TaskPoolPinningMode::Performance,
            service_perf_cores: Some(2),
        };
        set_task_pool_affinity_config(new);
        assert_eq!(task_pool_affinity_config(), new);
        set_task_pool_affinity_config(old);
    }

    #[test]
    fn service_core_reservation_keeps_worker_capacity() {
        assert_eq!(clamp_service_reservation(4, 1), 0);
        assert_eq!(clamp_service_reservation(4, 2), 1);
        assert_eq!(clamp_service_reservation(4, 8), 4);
    }
}
