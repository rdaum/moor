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

use crate::{BenchmarkResult, TableFormatter, add_session_result};
use moor_common::threading::{
    DetectionResult, detect_performance_cores, pin_current_thread_to_core,
};
use moor_common::util::Instant;
use std::{
    hint::black_box,
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[cfg(target_os = "linux")]
use perf_event::{Builder, Group, events::Hardware};
#[cfg(target_os = "linux")]
use std::sync::{Mutex, OnceLock};

const MIN_CHUNK_SIZE: usize = 100_000; // Large enough for reliable timing
const MAX_CHUNK_SIZE: usize = 50_000_000; // Maximum reasonable chunk
const TARGET_CHUNK_DURATION_MS: u64 = 200; // Target 200ms per chunk for accurate timing
const WARM_UP_DURATION_MS: u64 = 1_000; // 1 second warm-up
const MIN_BENCHMARK_DURATION_MS: u64 = 5_000; // At least 5 seconds of actual benchmarking
const MIN_SAMPLES: usize = 20; // More samples for better statistics
const MAX_SAMPLES: usize = 50; // Reasonable upper bound
const MIN_PMU_ACTIVE_PERCENT: f64 = 95.0; // Reject heavily multiplexed PMU runs

type BenchFunction<T> = fn(&mut T, usize, usize);

fn flush_stdout() {
    let _ = io::stdout().flush();
}

fn safe_ratio_f64(numerator: f64, denominator: f64) -> f64 {
    if denominator > 0.0 && denominator.is_finite() && numerator.is_finite() {
        numerator / denominator
    } else {
        0.0
    }
}

fn throughput_ops_per_sec(result: &Results) -> Option<f64> {
    let seconds = result.duration.as_secs_f64();
    if seconds <= 0.0 || !seconds.is_finite() || result.iterations == 0 {
        return None;
    }

    Some(result.iterations as f64 / seconds)
}

fn coefficient_of_variation_percent(samples: &[Results]) -> f64 {
    let throughputs: Vec<f64> = samples.iter().filter_map(throughput_ops_per_sec).collect();
    if throughputs.is_empty() {
        return 0.0;
    }

    let mean = throughputs.iter().sum::<f64>() / throughputs.len() as f64;
    if mean <= 0.0 || !mean.is_finite() {
        return 0.0;
    }

    let variance = throughputs
        .iter()
        .map(|&throughput| (throughput - mean).powi(2))
        .sum::<f64>()
        / throughputs.len() as f64;

    if !variance.is_finite() || variance < 0.0 {
        return 0.0;
    }

    (variance.sqrt() / mean) * 100.0
}

fn scale_multiplexed_count(raw: u64, enabled_ns: u64, running_ns: u64) -> u64 {
    if raw == 0 {
        return 0;
    }

    if enabled_ns == 0 || running_ns == 0 {
        return raw;
    }

    if running_ns >= enabled_ns {
        return raw;
    }

    ((raw as u128 * enabled_ns as u128) / running_ns as u128).min(u64::MAX as u128) as u64
}

fn pmu_active_percent(results: &Results) -> f64 {
    safe_ratio_f64(
        results.pmu_time_running_ns as f64,
        results.pmu_time_enabled_ns as f64,
    ) * 100.0
}

fn enforce_pmu_quality(name: &str, has_perf_counters: bool, results: &Results) {
    if !has_perf_counters || results.pmu_time_enabled_ns == 0 || results.pmu_time_running_ns == 0 {
        return;
    }

    let active_percent = pmu_active_percent(results);
    if active_percent < MIN_PMU_ACTIVE_PERCENT {
        panic!(
            "PMU counters were multiplexed too heavily for benchmark '{name}': active {active_percent:.1}% < {MIN_PMU_ACTIVE_PERCENT:.1}%"
        );
    }
}

#[cfg(target_os = "linux")]
fn perf_issues() -> &'static Mutex<Vec<String>> {
    static PERF_ISSUES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    PERF_ISSUES.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(target_os = "linux")]
fn record_perf_issue(message: impl Into<String>) {
    let message = message.into();
    let lock = perf_issues().lock();
    let mut issues = match lock {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    if issues.iter().any(|existing| existing == &message) {
        return;
    }
    if issues.len() >= 6 {
        return;
    }
    issues.push(message);
}

#[cfg(target_os = "linux")]
fn clear_perf_issues() {
    let lock = perf_issues().lock();
    let mut issues = match lock {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    issues.clear();
}

#[cfg(target_os = "linux")]
fn current_perf_issues() -> Vec<String> {
    let lock = perf_issues().lock();
    match lock {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

#[cfg(target_os = "linux")]
fn linux_perf_hint(has_perf_counters: bool, issues: &[String]) -> Option<String> {
    if has_perf_counters {
        return None;
    }

    let looks_like_perf_access_issue = issues.iter().any(|issue| {
        issue.contains("unusable timing window")
            || issue.contains("Operation not permitted")
            || issue.contains("Permission denied")
    });
    if !looks_like_perf_access_issue {
        return None;
    }

    let paranoid = std::fs::read_to_string("/proc/sys/kernel/perf_event_paranoid")
        .ok()
        .and_then(|value| value.trim().parse::<i32>().ok());

    match paranoid {
        Some(value) if value > 2 => Some(format!(
            "kernel.perf_event_paranoid={value}; lower it to 2 or less (or grant CAP_PERFMON/CAP_SYS_ADMIN) to enable PMU counters"
        )),
        Some(value) => Some(format!(
            "kernel.perf_event_paranoid={value}; PMU still unavailable, likely due to missing CAP_PERFMON/CAP_SYS_ADMIN or container perf_event restrictions"
        )),
        None => Some(
            "PMU still unavailable; check /proc/sys/kernel/perf_event_paranoid and container capabilities (CAP_PERFMON/CAP_SYS_ADMIN)".to_string(),
        ),
    }
}

fn warn_affinity_once(message: impl Into<String>) {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("⚠️  {}", message.into());
    }
}

#[cfg(target_os = "linux")]
fn core_has_usable_pmu(core_id: usize) -> bool {
    if pin_current_thread_to_core(core_id).is_err() {
        return false;
    }

    let mut counter = match Builder::new(Hardware::INSTRUCTIONS).build() {
        Ok(counter) => counter,
        Err(_) => return false,
    };
    if counter.enable().is_err() {
        return false;
    }

    let mut acc = 0_u64;
    for i in 0..100_000 {
        acc = acc.wrapping_add(i);
    }
    black_box(acc);

    let _ = counter.disable();
    match counter.read_count_and_time() {
        Ok(cat) => cat.count > 0 || cat.time_running > 0,
        Err(_) => false,
    }
}

#[cfg(target_os = "linux")]
fn choose_default_pin_core(allowed_core_ids: &[usize]) -> Option<usize> {
    let mut candidates: Vec<usize> = Vec::new();
    if let Ok(DetectionResult::PerformanceCores(selection)) = detect_performance_cores() {
        for core_id in selection.logical_processor_ids {
            if allowed_core_ids.contains(&core_id) && !candidates.contains(&core_id) {
                candidates.push(core_id);
            }
        }
    }
    for core_id in allowed_core_ids {
        if !candidates.contains(core_id) {
            candidates.push(*core_id);
        }
    }

    for core_id in &candidates {
        if core_has_usable_pmu(*core_id) {
            return Some(*core_id);
        }
    }

    candidates.first().copied()
}

#[cfg(target_os = "linux")]
fn capture_current_thread_affinity() -> io::Result<libc::cpu_set_t> {
    // SAFETY: zeroed is valid initialization for cpu_set_t.
    let mut cpuset: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    // SAFETY: pthread_self returns a valid handle for current thread; cpuset pointer is valid.
    let result = unsafe {
        libc::pthread_getaffinity_np(
            libc::pthread_self(),
            std::mem::size_of::<libc::cpu_set_t>(),
            &mut cpuset,
        )
    };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result));
    }
    Ok(cpuset)
}

#[cfg(target_os = "linux")]
fn restore_current_thread_affinity(mask: &libc::cpu_set_t) -> io::Result<()> {
    // SAFETY: pthread_self returns a valid handle; mask pointer is valid.
    let result = unsafe {
        libc::pthread_setaffinity_np(
            libc::pthread_self(),
            std::mem::size_of::<libc::cpu_set_t>(),
            mask,
        )
    };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn core_ids_from_mask(mask: &libc::cpu_set_t) -> Vec<usize> {
    let mut core_ids = Vec::new();
    for core_id in 0..(libc::CPU_SETSIZE as usize) {
        // SAFETY: core_id is in range and mask points to a valid cpu_set_t.
        let is_set = unsafe { libc::CPU_ISSET(core_id, mask) };
        if is_set {
            core_ids.push(core_id);
        }
    }
    core_ids
}

struct BenchAffinityGuard {
    #[cfg(target_os = "linux")]
    restore_mask: Option<libc::cpu_set_t>,
    #[cfg(target_os = "linux")]
    did_pin: bool,
}

impl BenchAffinityGuard {
    fn acquire() -> Self {
        #[cfg(target_os = "linux")]
        {
            let restore_mask = match capture_current_thread_affinity() {
                Ok(mask) => Some(mask),
                Err(error) => {
                    warn_affinity_once(format!(
                        "Could not capture existing benchmark thread affinity: {error}. Continuing with best effort pinning"
                    ));
                    None
                }
            };

            let allowed_core_ids = restore_mask
                .as_ref()
                .map(core_ids_from_mask)
                .filter(|core_ids| !core_ids.is_empty())
                .unwrap_or_else(|| {
                    let count = std::thread::available_parallelism()
                        .map(|p| p.get())
                        .unwrap_or(1);
                    (0..count).collect()
                });

            let requested_core = std::env::var("MOOR_BENCH_PIN_CORE")
                .ok()
                .and_then(|value| value.parse::<usize>().ok());
            let core_to_pin = requested_core
                .filter(|core_id| allowed_core_ids.contains(core_id))
                .or_else(|| choose_default_pin_core(&allowed_core_ids));

            let Some(core_id) = core_to_pin else {
                warn_affinity_once(
                    "No logical cores detected for pinning; benchmark will run without CPU pinning",
                );
                return Self {
                    restore_mask: None,
                    did_pin: false,
                };
            };

            if let Err(error) = pin_current_thread_to_core(core_id) {
                warn_affinity_once(format!(
                    "Could not pin benchmark thread to core {core_id}: {error}. Continuing without pinning"
                ));
                return Self {
                    restore_mask: None,
                    did_pin: false,
                };
            }

            Self {
                restore_mask,
                did_pin: true,
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            Self {}
        }
    }
}

impl Drop for BenchAffinityGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if !self.did_pin {
                return;
            }

            let Some(mask) = &self.restore_mask else {
                return;
            };

            if let Err(error) = restore_current_thread_affinity(mask) {
                warn_affinity_once(format!(
                    "Could not restore benchmark thread affinity after run: {error}"
                ));
            }
        }
    }
}

#[cfg(target_os = "linux")]
struct PerfGroupCounters {
    group: Group,
    instructions: Option<perf_event::Counter>,
    branches: Option<perf_event::Counter>,
    branch_misses: Option<perf_event::Counter>,
    cache_misses: Option<perf_event::Counter>,
}

#[cfg(target_os = "linux")]
fn try_add_group_counter(
    group: &mut Group,
    event: Hardware,
    name: &str,
) -> Option<perf_event::Counter> {
    match group.add(&Builder::new(event)) {
        Ok(counter) => Some(counter),
        Err(error) => {
            record_perf_issue(format!("perf event '{name}' unavailable: {error}"));
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn build_perf_counter_group() -> Option<PerfGroupCounters> {
    let mut group = match Group::new() {
        Ok(group) => group,
        Err(error) => {
            record_perf_issue(format!("perf group unavailable: {error}"));
            return None;
        }
    };

    let instructions = try_add_group_counter(&mut group, Hardware::INSTRUCTIONS, "instructions");
    let branches = try_add_group_counter(&mut group, Hardware::BRANCH_INSTRUCTIONS, "branches");
    let branch_misses = try_add_group_counter(&mut group, Hardware::BRANCH_MISSES, "branch-misses");
    let cache_misses = try_add_group_counter(&mut group, Hardware::CACHE_MISSES, "cache-misses");

    if instructions.is_none()
        && branches.is_none()
        && branch_misses.is_none()
        && cache_misses.is_none()
    {
        record_perf_issue("no perf events could be added to perf group".to_string());
        return None;
    }

    Some(PerfGroupCounters {
        group,
        instructions,
        branches,
        branch_misses,
        cache_misses,
    })
}

#[derive(Clone, Default)]
pub struct Results {
    pub instructions: u64,
    pub branches: u64,
    pub branch_misses: u64,
    pub cache_misses: u64,
    pub pmu_time_enabled_ns: u64,
    pub pmu_time_running_ns: u64,
    pub duration: Duration,
    pub iterations: u64,
    pub chunks_executed: u64,
}

impl Results {
    pub fn add(&mut self, other: &Results) {
        self.instructions += other.instructions;
        self.branches += other.branches;
        self.branch_misses += other.branch_misses;
        self.cache_misses += other.cache_misses;
        self.pmu_time_enabled_ns += other.pmu_time_enabled_ns;
        self.pmu_time_running_ns += other.pmu_time_running_ns;
        self.duration += other.duration;
        self.iterations += other.iterations;
        self.chunks_executed += other.chunks_executed;
    }

    pub fn divide(&mut self, divisor: u64) {
        if divisor > 0 {
            self.instructions /= divisor;
            self.branches /= divisor;
            self.branch_misses /= divisor;
            self.cache_misses /= divisor;
            self.pmu_time_enabled_ns /= divisor;
            self.pmu_time_running_ns /= divisor;
            self.duration /= divisor as u32;
            self.iterations /= divisor;
            self.chunks_executed /= divisor;
        }
    }
}

pub struct BenchmarkConfig {
    pub chunk_size: usize,
    pub target_samples: usize,
    pub estimated_ops_per_ms: f64,
}

/// Performance counter controls for fine-grained measurement
#[cfg(target_os = "linux")]
pub struct PerfCounters {
    pub instructions_counter: perf_event::Counter,
    pub cycles_counter: perf_event::Counter,
    pub branch_counter: perf_event::Counter,
    pub branch_misses: perf_event::Counter,
    pub cache_misses: perf_event::Counter,
    pub l1i_misses: perf_event::Counter,
    pub stalled_frontend: perf_event::Counter,
    pub stalled_backend: perf_event::Counter,
    pub start_time: Option<Instant>,
}

#[cfg(target_os = "linux")]
impl Default for PerfCounters {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
impl PerfCounters {
    pub fn try_new() -> io::Result<Self> {
        Ok(PerfCounters {
            instructions_counter: Builder::new(Hardware::INSTRUCTIONS).build()?,
            cycles_counter: Builder::new(Hardware::CPU_CYCLES).build()?,
            branch_counter: Builder::new(Hardware::BRANCH_INSTRUCTIONS).build()?,
            branch_misses: Builder::new(Hardware::BRANCH_MISSES).build()?,
            cache_misses: Builder::new(Hardware::CACHE_MISSES).build()?,
            l1i_misses: Builder::new(Hardware::CACHE_MISSES).build()?, // Use generic as fallback
            stalled_frontend: Builder::new(Hardware::CACHE_MISSES).build()?, // Use generic as fallback
            stalled_backend: Builder::new(Hardware::CACHE_MISSES).build()?, // Use generic as fallback
            start_time: None,
        })
    }

    pub fn new() -> Self {
        Self::try_new().expect("failed to initialize perf counters")
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        let _ = self.instructions_counter.enable();
        let _ = self.cycles_counter.enable();
        let _ = self.branch_counter.enable();
        let _ = self.branch_misses.enable();
        let _ = self.cache_misses.enable();
        let _ = self.l1i_misses.enable();
        let _ = self.stalled_frontend.enable();
        let _ = self.stalled_backend.enable();
    }

    pub fn stop(&mut self) -> (Duration, u64, u64, u64, u64, u64, u64, u64, u64) {
        let _ = self.instructions_counter.disable();
        let _ = self.cycles_counter.disable();
        let _ = self.branch_counter.disable();
        let _ = self.branch_misses.disable();
        let _ = self.cache_misses.disable();
        let _ = self.l1i_misses.disable();
        let _ = self.stalled_frontend.disable();
        let _ = self.stalled_backend.disable();

        let duration = self
            .start_time
            .map_or(Duration::from_secs(0), |start| start.elapsed());
        let instructions = self.instructions_counter.read().unwrap_or(0);
        let cycles = self.cycles_counter.read().unwrap_or(0);
        let branches = self.branch_counter.read().unwrap_or(0);
        let branch_misses = self.branch_misses.read().unwrap_or(0);
        let cache_misses = self.cache_misses.read().unwrap_or(0);
        let l1i_misses = self.l1i_misses.read().unwrap_or(0);
        let stalled_frontend = self.stalled_frontend.read().unwrap_or(0);
        let stalled_backend = self.stalled_backend.read().unwrap_or(0);

        (
            duration,
            instructions,
            cycles,
            branches,
            branch_misses,
            cache_misses,
            l1i_misses,
            stalled_frontend,
            stalled_backend,
        )
    }
}

/// Generic benchmark context that can hold any preparation data
pub trait BenchContext {
    fn prepare(num_chunks: usize) -> Self;

    /// Optional preferred chunk size for this context
    /// If Some(size), skip calibration and use this size
    /// If None, use normal calibration
    fn chunk_size() -> Option<usize> {
        None
    }

    /// Optional: specify how many actual operations each chunk represents
    /// Used for calculating correct throughput metrics
    /// If None, assumes chunk_size == operations
    fn operations_per_chunk() -> Option<u64> {
        None
    }
}

/// Simple context for benchmarks that don't need preparation
pub struct NoContext;
impl BenchContext for NoContext {
    fn prepare(_num_chunks: usize) -> Self {
        NoContext
    }
}

/// Warm-up with custom factory function (Linux version)
#[cfg(target_os = "linux")]
fn warm_up_and_calibrate_with_factory<T: BenchContext>(
    f: &BenchFunction<T>,
    factory: &dyn Fn() -> T,
) -> BenchmarkConfig {
    print!("🔥 Warming up");
    flush_stdout();

    if let Some(preferred_chunk_size) = T::chunk_size() {
        println!(" ✅");
        println!("   Using preferred chunk size: {preferred_chunk_size} ops");

        let warm_up_end = Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
        let mut warm_up_count = 0;
        let mut last_dot_time = Instant::now();
        while Instant::now() < warm_up_end {
            let mut prepared = factory();
            black_box(|| f(&mut prepared, preferred_chunk_size, warm_up_count))();
            warm_up_count += 1;

            if Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100) {
                print!(".");
                flush_stdout();
                last_dot_time = Instant::now();
            }
        }

        return BenchmarkConfig {
            chunk_size: preferred_chunk_size,
            target_samples: MIN_SAMPLES,
            estimated_ops_per_ms: 0.0,
        };
    }

    let mut chunk_size = MIN_CHUNK_SIZE;
    let mut best_chunk_size = chunk_size;
    let mut ops_per_ms = 0.0;

    for i in 0..10 {
        let mut prepared = factory();
        let started = Instant::now();
        black_box(|| f(&mut prepared, chunk_size, 0))();
        let duration = started.elapsed();

        let duration_ms = duration.as_millis() as f64;

        if duration_ms >= 1.0 {
            ops_per_ms = chunk_size as f64 / duration_ms;

            if duration_ms >= TARGET_CHUNK_DURATION_MS as f64 * 0.7
                && duration_ms <= TARGET_CHUNK_DURATION_MS as f64 * 1.5
            {
                best_chunk_size = chunk_size;
                break;
            }

            let target_ms = TARGET_CHUNK_DURATION_MS as f64;
            let scaling_factor = target_ms / duration_ms;
            let new_chunk_size = ((chunk_size as f64) * scaling_factor) as usize;
            chunk_size = new_chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
            best_chunk_size = chunk_size;
        } else {
            chunk_size = (chunk_size * 5).min(MAX_CHUNK_SIZE);
            best_chunk_size = chunk_size;
        }

        if i % 2 == 0 {
            print!(".");
            flush_stdout();
        }
    }

    let warm_up_end = Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
    let mut warm_up_count = 0;
    let mut last_dot_time = Instant::now();
    while Instant::now() < warm_up_end {
        let mut prepared = factory();
        black_box(|| f(&mut prepared, best_chunk_size, warm_up_count))();
        warm_up_count += 1;

        if Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100) {
            print!(".");
            flush_stdout();
            last_dot_time = Instant::now();
        }
    }

    let estimated_chunk_duration_ms = if ops_per_ms > 0.0 {
        best_chunk_size as f64 / ops_per_ms
    } else {
        TARGET_CHUNK_DURATION_MS as f64
    };
    let target_samples = ((MIN_BENCHMARK_DURATION_MS as f64 / estimated_chunk_duration_ms)
        as usize)
        .clamp(MIN_SAMPLES, MAX_SAMPLES);

    println!(" ✅");
    println!("   Optimal chunk size: {best_chunk_size} ops");
    if ops_per_ms > 0.0 {
        println!(
            "   Estimated performance: {:.1} Mops/s",
            ops_per_ms / 1000.0
        );
    } else {
        println!("   Estimated performance: Very fast (>1000 Mops/s)");
    }
    println!("   Target samples: {target_samples}");

    BenchmarkConfig {
        chunk_size: best_chunk_size,
        target_samples,
        estimated_ops_per_ms: ops_per_ms,
    }
}

/// Warm-up with custom factory function (non-Linux version)
#[cfg(not(target_os = "linux"))]
fn warm_up_and_calibrate_with_factory<T: BenchContext>(
    f: &BenchFunction<T>,
    factory: &dyn Fn() -> T,
) -> BenchmarkConfig {
    print!("🔥 Warming up");
    flush_stdout();

    if let Some(preferred_chunk_size) = T::chunk_size() {
        println!(" ✅");
        println!("   Using preferred chunk size: {preferred_chunk_size} ops");

        let warm_up_end = Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
        let mut warm_up_count = 0;
        let mut last_dot_time = Instant::now();
        while Instant::now() < warm_up_end {
            let mut prepared = factory();
            black_box(|| f(&mut prepared, preferred_chunk_size, warm_up_count))();
            warm_up_count += 1;

            if Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100) {
                print!(".");
                flush_stdout();
                last_dot_time = Instant::now();
            }
        }

        return BenchmarkConfig {
            chunk_size: preferred_chunk_size,
            target_samples: MIN_SAMPLES,
            estimated_ops_per_ms: 0.0,
        };
    }

    let mut chunk_size = MIN_CHUNK_SIZE;
    let mut best_chunk_size = chunk_size;
    let mut ops_per_ms = 0.0;

    for i in 0..10 {
        let mut prepared = factory();
        let started = Instant::now();
        black_box(|| f(&mut prepared, chunk_size, 0))();
        let duration = started.elapsed();

        let duration_ms = duration.as_millis() as f64;

        if duration_ms >= 1.0 {
            ops_per_ms = chunk_size as f64 / duration_ms;

            if duration_ms >= TARGET_CHUNK_DURATION_MS as f64 * 0.7
                && duration_ms <= TARGET_CHUNK_DURATION_MS as f64 * 1.5
            {
                best_chunk_size = chunk_size;
                break;
            }

            let target_ms = TARGET_CHUNK_DURATION_MS as f64;
            let scaling_factor = target_ms / duration_ms;
            let new_chunk_size = ((chunk_size as f64) * scaling_factor) as usize;
            chunk_size = new_chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
            best_chunk_size = chunk_size;
        } else {
            chunk_size = (chunk_size * 5).min(MAX_CHUNK_SIZE);
            best_chunk_size = chunk_size;
        }

        if i % 2 == 0 {
            print!(".");
            flush_stdout();
        }
    }

    let warm_up_end = Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
    let mut warm_up_count = 0;
    let mut last_dot_time = Instant::now();
    while Instant::now() < warm_up_end {
        let mut prepared = factory();
        black_box(|| f(&mut prepared, best_chunk_size, warm_up_count))();
        warm_up_count += 1;

        if Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100) {
            print!(".");
            flush_stdout();
            last_dot_time = Instant::now();
        }
    }

    let estimated_chunk_duration_ms = if ops_per_ms > 0.0 {
        best_chunk_size as f64 / ops_per_ms
    } else {
        TARGET_CHUNK_DURATION_MS as f64
    };
    let target_samples = ((MIN_BENCHMARK_DURATION_MS as f64 / estimated_chunk_duration_ms)
        as usize)
        .clamp(MIN_SAMPLES, MAX_SAMPLES);

    println!(" ✅");
    println!("   Optimal chunk size: {best_chunk_size} ops");
    if ops_per_ms > 0.0 {
        println!(
            "   Estimated performance: {:.1} Mops/s",
            ops_per_ms / 1000.0
        );
    } else {
        println!("   Estimated performance: Very fast (>1000 Mops/s)");
    }
    println!("   Target samples: {target_samples}");

    BenchmarkConfig {
        chunk_size: best_chunk_size,
        target_samples,
        estimated_ops_per_ms: ops_per_ms,
    }
}

/// Warm-up phase to determine optimal chunk size and estimate performance
#[cfg(target_os = "linux")]
fn warm_up_and_calibrate<T: BenchContext>(f: &BenchFunction<T>) -> BenchmarkConfig {
    print!("🔥 Warming up");
    flush_stdout();

    // Check if context has a preferred chunk size
    if let Some(preferred_chunk_size) = T::chunk_size() {
        println!(" ✅");
        println!("   Using preferred chunk size: {preferred_chunk_size} ops");

        // Do a quick warm-up with the preferred size
        let warm_up_end = Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
        let mut warm_up_count = 0;
        let mut last_dot_time = Instant::now();
        while Instant::now() < warm_up_end {
            let mut prepared = T::prepare(preferred_chunk_size);
            black_box(|| f(&mut prepared, preferred_chunk_size, warm_up_count))();
            warm_up_count += 1;

            if Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100) {
                print!(".");
                flush_stdout();
                last_dot_time = Instant::now();
            }
        }

        return BenchmarkConfig {
            chunk_size: preferred_chunk_size,
            target_samples: MIN_SAMPLES,
            estimated_ops_per_ms: 0.0, // Will be calculated during actual benchmark
        };
    }

    let mut chunk_size = MIN_CHUNK_SIZE;
    let mut best_chunk_size = chunk_size;
    let mut ops_per_ms = 0.0;

    // Try different chunk sizes to find one that takes target duration
    for i in 0..10 {
        // Max 10 iterations to find good chunk size
        let mut prepared = T::prepare(chunk_size);
        let started = Instant::now();
        black_box(|| f(&mut prepared, chunk_size, 0))();
        let duration = started.elapsed();

        let duration_ms = duration.as_millis() as f64;

        // Only proceed if we got a measurable duration (at least 1ms)
        if duration_ms >= 1.0 {
            ops_per_ms = chunk_size as f64 / duration_ms;

            // Check if we're in the target range
            if duration_ms >= TARGET_CHUNK_DURATION_MS as f64 * 0.7
                && duration_ms <= TARGET_CHUNK_DURATION_MS as f64 * 1.5
            {
                best_chunk_size = chunk_size;
                break;
            }

            // Adjust chunk size based on timing to hit target
            let target_ms = TARGET_CHUNK_DURATION_MS as f64;
            let scaling_factor = target_ms / duration_ms;
            let new_chunk_size = ((chunk_size as f64) * scaling_factor) as usize;
            chunk_size = new_chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
            best_chunk_size = chunk_size;
        } else {
            // Duration too short to measure reliably - increase chunk size dramatically
            chunk_size = (chunk_size * 5).min(MAX_CHUNK_SIZE);
            best_chunk_size = chunk_size;
        }

        if i % 2 == 0 {
            // Only print dot every other iteration
            print!(".");
            flush_stdout();
        }
    }

    // Do additional warm-up iterations with limited dot output
    let warm_up_end = Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
    let mut warm_up_count = 0;
    let mut last_dot_time = Instant::now();
    while Instant::now() < warm_up_end {
        let mut prepared = T::prepare(chunk_size);
        black_box(|| f(&mut prepared, best_chunk_size, warm_up_count))();
        warm_up_count += 1;

        // Print a dot every 100ms instead of every 5 iterations
        if Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100) {
            print!(".");
            flush_stdout();
            last_dot_time = Instant::now();
        }
    }

    // Calculate target sample count based on estimated performance
    let estimated_chunk_duration_ms = if ops_per_ms > 0.0 {
        best_chunk_size as f64 / ops_per_ms
    } else {
        TARGET_CHUNK_DURATION_MS as f64
    };
    let target_samples = ((MIN_BENCHMARK_DURATION_MS as f64 / estimated_chunk_duration_ms)
        as usize)
        .clamp(MIN_SAMPLES, MAX_SAMPLES);

    println!(" ✅");
    println!("   Optimal chunk size: {best_chunk_size} ops");
    if ops_per_ms > 0.0 {
        println!(
            "   Estimated performance: {:.1} Mops/s",
            ops_per_ms / 1000.0
        );
    } else {
        println!("   Estimated performance: Very fast (>1000 Mops/s)");
    }
    println!("   Target samples: {target_samples}");

    BenchmarkConfig {
        chunk_size: best_chunk_size,
        target_samples,
        estimated_ops_per_ms: ops_per_ms,
    }
}

/// Warm-up phase to determine optimal chunk size and estimate performance (non-Linux)
#[cfg(not(target_os = "linux"))]
fn warm_up_and_calibrate<T: BenchContext>(f: &BenchFunction<T>) -> BenchmarkConfig {
    print!("🔥 Warming up");
    flush_stdout();

    // Check if context has a preferred chunk size
    if let Some(preferred_chunk_size) = T::chunk_size() {
        println!(" ✅");
        println!("   Using preferred chunk size: {preferred_chunk_size} ops");

        // Do a quick warm-up with the preferred size
        let warm_up_end = Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
        let mut warm_up_count = 0;
        let mut last_dot_time = Instant::now();
        while Instant::now() < warm_up_end {
            let mut prepared = T::prepare(preferred_chunk_size);
            black_box(|| f(&mut prepared, preferred_chunk_size, warm_up_count))();
            warm_up_count += 1;

            if Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100) {
                print!(".");
                flush_stdout();
                last_dot_time = Instant::now();
            }
        }

        return BenchmarkConfig {
            chunk_size: preferred_chunk_size,
            target_samples: MIN_SAMPLES,
            estimated_ops_per_ms: 0.0,
        };
    }

    let mut chunk_size = MIN_CHUNK_SIZE;
    let mut best_chunk_size = chunk_size;
    let mut ops_per_ms = 0.0;

    // Try different chunk sizes to find one that takes target duration
    for i in 0..10 {
        let mut prepared = T::prepare(chunk_size);
        let started = Instant::now();
        black_box(|| f(&mut prepared, chunk_size, 0))();
        let duration = started.elapsed();

        let duration_ms = duration.as_millis() as f64;

        if duration_ms >= 1.0 {
            ops_per_ms = chunk_size as f64 / duration_ms;

            if duration_ms >= TARGET_CHUNK_DURATION_MS as f64 * 0.7
                && duration_ms <= TARGET_CHUNK_DURATION_MS as f64 * 1.5
            {
                best_chunk_size = chunk_size;
                break;
            }

            let target_ms = TARGET_CHUNK_DURATION_MS as f64;
            let scaling_factor = target_ms / duration_ms;
            let new_chunk_size = ((chunk_size as f64) * scaling_factor) as usize;
            chunk_size = new_chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
            best_chunk_size = chunk_size;
        } else {
            chunk_size = (chunk_size * 5).min(MAX_CHUNK_SIZE);
            best_chunk_size = chunk_size;
        }

        if i % 2 == 0 {
            print!(".");
            flush_stdout();
        }
    }

    // Additional warm-up iterations
    let warm_up_end = Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
    let mut warm_up_count = 0;
    let mut last_dot_time = Instant::now();
    while Instant::now() < warm_up_end {
        let mut prepared = T::prepare(chunk_size);
        black_box(|| f(&mut prepared, best_chunk_size, warm_up_count))();
        warm_up_count += 1;

        if Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100) {
            print!(".");
            flush_stdout();
            last_dot_time = Instant::now();
        }
    }

    let estimated_chunk_duration_ms = if ops_per_ms > 0.0 {
        best_chunk_size as f64 / ops_per_ms
    } else {
        TARGET_CHUNK_DURATION_MS as f64
    };
    let target_samples = ((MIN_BENCHMARK_DURATION_MS as f64 / estimated_chunk_duration_ms)
        as usize)
        .clamp(MIN_SAMPLES, MAX_SAMPLES);

    println!(" ✅");
    println!("   Optimal chunk size: {best_chunk_size} ops");
    if ops_per_ms > 0.0 {
        println!(
            "   Estimated performance: {:.1} Mops/s",
            ops_per_ms / 1000.0
        );
    } else {
        println!("   Estimated performance: Very fast (>1000 Mops/s)");
    }
    println!("   Target samples: {target_samples}");

    BenchmarkConfig {
        chunk_size: best_chunk_size,
        target_samples,
        estimated_ops_per_ms: ops_per_ms,
    }
}

fn execute_timing_only<T: BenchContext>(
    f: &BenchFunction<T>,
    prepared: &mut T,
    chunk_size: usize,
    chunk_num: usize,
    ops: u64,
) -> Results {
    let start_time = Instant::now();
    black_box(|| f(prepared, chunk_size, chunk_num))();
    let duration = start_time.elapsed();

    Results {
        duration,
        iterations: ops,
        chunks_executed: 1,
        ..Results::default()
    }
}

#[cfg(target_os = "linux")]
fn try_build_individual_counter(event: Hardware, name: &str) -> Option<perf_event::Counter> {
    match Builder::new(event).build() {
        Ok(counter) => Some(counter),
        Err(error) => {
            record_perf_issue(format!("perf event '{name}' unavailable: {error}"));
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn read_scaled_counter(counter: &mut Option<perf_event::Counter>, name: &str) -> (u64, u64, u64) {
    let Some(counter) = counter else {
        return (0, 0, 0);
    };

    match counter.read_count_and_time() {
        Ok(cat) => {
            if cat.count > 0 && (cat.time_enabled == 0 || cat.time_running == 0) {
                record_perf_issue(format!(
                    "perf event '{name}' missing timing metadata (enabled/running); using raw count"
                ));
            }
            (
                scale_multiplexed_count(cat.count, cat.time_enabled, cat.time_running),
                cat.time_enabled,
                cat.time_running,
            )
        }
        Err(error) => {
            record_perf_issue(format!("perf event '{name}' read failed: {error}"));
            (0, 0, 0)
        }
    }
}

#[cfg(target_os = "linux")]
fn enable_counter(counter: &mut Option<perf_event::Counter>, name: &str) {
    let Some(mut inner) = counter.take() else {
        return;
    };

    if let Err(error) = inner.enable() {
        record_perf_issue(format!("perf event '{name}' enable failed: {error}"));
        return;
    }

    *counter = Some(inner);
}

#[cfg(target_os = "linux")]
fn disable_counter(counter: &mut Option<perf_event::Counter>, name: &str) {
    let Some(counter) = counter.as_mut() else {
        return;
    };

    if let Err(error) = counter.disable() {
        record_perf_issue(format!("perf event '{name}' disable failed: {error}"));
    }
}

#[cfg(target_os = "linux")]
fn execute_with_individual_counters<T: BenchContext>(
    f: &BenchFunction<T>,
    prepared: &mut T,
    chunk_size: usize,
    chunk_num: usize,
    ops: u64,
) -> Results {
    let mut instructions_counter =
        try_build_individual_counter(Hardware::INSTRUCTIONS, "instructions");
    let mut branches_counter =
        try_build_individual_counter(Hardware::BRANCH_INSTRUCTIONS, "branches");
    let mut branch_misses_counter =
        try_build_individual_counter(Hardware::BRANCH_MISSES, "branch-misses");
    let mut cache_misses_counter =
        try_build_individual_counter(Hardware::CACHE_MISSES, "cache-misses");

    if instructions_counter.is_none()
        && branches_counter.is_none()
        && branch_misses_counter.is_none()
        && cache_misses_counter.is_none()
    {
        return execute_timing_only(f, prepared, chunk_size, chunk_num, ops);
    }

    record_perf_issue("using ungrouped perf counters fallback".to_string());

    enable_counter(&mut instructions_counter, "instructions");
    enable_counter(&mut branches_counter, "branches");
    enable_counter(&mut branch_misses_counter, "branch-misses");
    enable_counter(&mut cache_misses_counter, "cache-misses");

    let start_time = Instant::now();
    black_box(|| f(prepared, chunk_size, chunk_num))();
    let duration = start_time.elapsed();

    disable_counter(&mut instructions_counter, "instructions");
    disable_counter(&mut branches_counter, "branches");
    disable_counter(&mut branch_misses_counter, "branch-misses");
    disable_counter(&mut cache_misses_counter, "cache-misses");

    let (instructions, instructions_enabled, instructions_running) =
        read_scaled_counter(&mut instructions_counter, "instructions");
    let (branches, branches_enabled, branches_running) =
        read_scaled_counter(&mut branches_counter, "branches");
    let (branch_misses, branch_misses_enabled, branch_misses_running) =
        read_scaled_counter(&mut branch_misses_counter, "branch-misses");
    let (cache_misses, cache_misses_enabled, cache_misses_running) =
        read_scaled_counter(&mut cache_misses_counter, "cache-misses");

    let timing_candidates = [
        (instructions_enabled, instructions_running),
        (branches_enabled, branches_running),
        (branch_misses_enabled, branch_misses_running),
        (cache_misses_enabled, cache_misses_running),
    ];

    let (pmu_time_enabled_ns, pmu_time_running_ns) = timing_candidates
        .iter()
        .copied()
        .find(|(_, running)| *running > 0)
        .or_else(|| {
            timing_candidates
                .iter()
                .copied()
                .find(|(enabled, _)| *enabled > 0)
        })
        .unwrap_or((0, 0));

    Results {
        instructions,
        branches,
        branch_misses,
        cache_misses,
        pmu_time_enabled_ns,
        pmu_time_running_ns,
        duration,
        iterations: ops,
        chunks_executed: 1,
    }
}

#[cfg(target_os = "linux")]
fn execute_with_perf_group<T: BenchContext>(
    f: &BenchFunction<T>,
    prepared: &mut T,
    chunk_size: usize,
    chunk_num: usize,
    ops: u64,
) -> Results {
    let Some(mut perf) = build_perf_counter_group() else {
        return execute_with_individual_counters(f, prepared, chunk_size, chunk_num, ops);
    };

    if let Err(error) = perf.group.enable() {
        record_perf_issue(format!("perf group enable failed: {error}"));
        return execute_with_individual_counters(f, prepared, chunk_size, chunk_num, ops);
    }

    let start_time = Instant::now();
    black_box(|| f(prepared, chunk_size, chunk_num))();
    let duration = start_time.elapsed();
    if let Err(error) = perf.group.disable() {
        record_perf_issue(format!("perf group disable failed: {error}"));
    }

    let counts = match perf.group.read() {
        Ok(counts) => counts,
        Err(error) => {
            record_perf_issue(format!("perf group read failed: {error}"));
            return execute_with_individual_counters(f, prepared, chunk_size, chunk_num, ops);
        }
    };

    let enabled_ns = counts
        .time_enabled()
        .map(|duration| duration.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0);
    let running_ns = counts
        .time_running()
        .map(|duration| duration.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0);

    let instructions_raw = perf
        .instructions
        .as_ref()
        .and_then(|counter| counts.get(counter).map(|entry| entry.value()))
        .unwrap_or(0);
    let branches_raw = perf
        .branches
        .as_ref()
        .and_then(|counter| counts.get(counter).map(|entry| entry.value()))
        .unwrap_or(0);
    let branch_misses_raw = perf
        .branch_misses
        .as_ref()
        .and_then(|counter| counts.get(counter).map(|entry| entry.value()))
        .unwrap_or(0);
    let cache_misses_raw = perf
        .cache_misses
        .as_ref()
        .and_then(|counter| counts.get(counter).map(|entry| entry.value()))
        .unwrap_or(0);

    if enabled_ns == 0 || running_ns == 0 {
        record_perf_issue(
            "perf counters reported unusable timing window (enabled/running)".to_string(),
        );
        if instructions_raw == 0
            && branches_raw == 0
            && branch_misses_raw == 0
            && cache_misses_raw == 0
        {
            return execute_with_individual_counters(f, prepared, chunk_size, chunk_num, ops);
        }
    }

    Results {
        instructions: scale_multiplexed_count(instructions_raw, enabled_ns, running_ns),
        branches: scale_multiplexed_count(branches_raw, enabled_ns, running_ns),
        branch_misses: scale_multiplexed_count(branch_misses_raw, enabled_ns, running_ns),
        cache_misses: scale_multiplexed_count(cache_misses_raw, enabled_ns, running_ns),
        pmu_time_enabled_ns: enabled_ns,
        pmu_time_running_ns: running_ns,
        duration,
        iterations: ops,
        chunks_executed: 1,
    }
}

/// Execute a single benchmark sample with custom factory (Linux with perf counters)
#[cfg(target_os = "linux")]
fn execute_sample_with_factory<T: BenchContext>(
    f: &BenchFunction<T>,
    chunk_size: usize,
    chunk_num: usize,
    factory: &dyn Fn() -> T,
) -> Results {
    let mut prepared = factory();
    let ops = T::operations_per_chunk().unwrap_or(chunk_size as u64);
    execute_with_perf_group(f, &mut prepared, chunk_size, chunk_num, ops)
}

/// Execute a single benchmark sample with custom factory (non-Linux)
#[cfg(not(target_os = "linux"))]
fn execute_sample_with_factory<T: BenchContext>(
    f: &BenchFunction<T>,
    chunk_size: usize,
    chunk_num: usize,
    factory: &dyn Fn() -> T,
) -> Results {
    let mut prepared = factory();
    let ops = T::operations_per_chunk().unwrap_or(chunk_size as u64);
    execute_timing_only(f, &mut prepared, chunk_size, chunk_num, ops)
}

/// Execute a single benchmark sample with performance counters (if available)
#[cfg(target_os = "linux")]
fn execute_sample<T: BenchContext>(
    f: &BenchFunction<T>,
    chunk_size: usize,
    chunk_num: usize,
) -> Results {
    let mut prepared = T::prepare(chunk_size);
    let ops = T::operations_per_chunk().unwrap_or(chunk_size as u64);
    execute_with_perf_group(f, &mut prepared, chunk_size, chunk_num, ops)
}

/// Execute a single benchmark sample without performance counters
#[cfg(not(target_os = "linux"))]
fn execute_sample<T: BenchContext>(
    f: &BenchFunction<T>,
    chunk_size: usize,
    chunk_num: usize,
) -> Results {
    let mut prepared = T::prepare(chunk_size);
    let ops = T::operations_per_chunk().unwrap_or(chunk_size as u64);
    execute_timing_only(f, &mut prepared, chunk_size, chunk_num, ops)
}

/// Progress bar with terminal-compatible characters
fn update_progress_bar(current: usize, total: usize, current_throughput: f64) {
    let width = 40;
    let filled = (current * width / total.max(1)).min(width);
    let empty = width - filled;

    let percentage = (current * 100 / total.max(1)).min(100);

    print!("\r⚡ [");

    // Progress bar with ASCII-compatible characters
    for i in 0..filled {
        if i == filled - 1 && current < total {
            print!(">"); // Current position
        } else {
            print!("="); // Completed
        }
    }

    for _ in 0..empty {
        print!(" "); // Empty
    }

    // Display throughput with proper bounds checking
    let throughput_display = if current_throughput.is_finite() && current_throughput > 0.0 {
        if current_throughput > 1000.0 {
            format!("{current_throughput:.0} Mops/s")
        } else {
            format!("{current_throughput:.1} Mops/s")
        }
    } else {
        "Calculating...".to_string()
    };

    print!("] {percentage}% ({current}/{total}) {throughput_display}");

    flush_stdout();
}

/// Benchmark with a custom context factory function
/// Useful for benchmarks where context creation depends on runtime parameters
/// (e.g., VM dispatch tests with different programs)
pub fn op_bench_with_factory<T: BenchContext>(
    name: &str,
    group: &str,
    f: BenchFunction<T>,
    factory: &dyn Fn() -> T,
) {
    op_bench_with_factory_filtered(name, group, f, factory, None);
}

pub fn op_bench_with_factory_filtered<T: BenchContext>(
    name: &str,
    group: &str,
    f: BenchFunction<T>,
    factory: &dyn Fn() -> T,
    filter: Option<&str>,
) {
    // Check if this benchmark should run based on filter
    let should_run = match filter {
        None => true,
        Some(f) => f == "all" || name.contains(f) || group.contains(f) || f == group,
    };

    if !should_run {
        return;
    }

    #[cfg(target_os = "linux")]
    clear_perf_issues();

    let _affinity_guard = BenchAffinityGuard::acquire();

    println!("\n🚀 Benchmarking: {name}");

    // Warm-up and calibration phase (using factory instead of T::prepare)
    let config = warm_up_and_calibrate_with_factory(&f, factory);

    // Main benchmark phase
    println!("⚡ Running {} samples...", config.target_samples);

    let mut all_results: Vec<Results> = Vec::new();
    let mut summed_results = Results::default();
    let mut running_throughput = if config.estimated_ops_per_ms > 0.0 {
        config.estimated_ops_per_ms / 1000.0
    } else {
        0.0
    };

    for sample in 0..config.target_samples {
        let sample_result = execute_sample_with_factory(&f, config.chunk_size, sample, factory);

        let duration_ms = sample_result.duration.as_millis() as f64;
        let sample_throughput_mops =
            safe_ratio_f64(sample_result.iterations as f64, duration_ms) / 1000.0;
        if sample_throughput_mops > 0.0 {
            running_throughput = running_throughput * 0.9 + sample_throughput_mops * 0.1;
        }

        summed_results.add(&sample_result);
        all_results.push(sample_result);

        if sample % 2 == 0 || sample == config.target_samples - 1 {
            update_progress_bar(sample + 1, config.target_samples, running_throughput);
        }
    }

    println!();

    // Calculate statistics (same as op_bench)
    let mut results = summed_results.clone();
    results.divide(config.target_samples as u64);

    let ops_per_sec = safe_ratio_f64(results.iterations as f64, results.duration.as_secs_f64());
    let ns_per_op = safe_ratio_f64(
        results.duration.as_nanos() as f64,
        results.iterations as f64,
    );
    let instructions_per_op =
        safe_ratio_f64(results.instructions as f64, results.iterations as f64);
    let branches_per_op = safe_ratio_f64(results.branches as f64, results.iterations as f64);
    let branch_miss_rate =
        safe_ratio_f64(results.branch_misses as f64, results.branches as f64) * 100.0;
    let branch_misses_per_op =
        safe_ratio_f64(results.branch_misses as f64, results.iterations as f64);
    let cache_miss_rate_per_op =
        safe_ratio_f64(results.cache_misses as f64, results.iterations as f64);
    let cv_percent = coefficient_of_variation_percent(&all_results);

    println!("\n📈 Results for {name}:");

    let has_perf_counters = results.pmu_time_running_ns > 0
        || results.instructions > 0
        || results.branches > 0
        || results.branch_misses > 0
        || results.cache_misses > 0;

    if !has_perf_counters {
        #[cfg(target_os = "linux")]
        println!(
            "   Note: Performance counters not available (insufficient permissions or kernel support)"
        );
        #[cfg(not(target_os = "linux"))]
        println!("   Note: Performance counters not available on this platform");
    }
    #[cfg(target_os = "linux")]
    {
        let issues = current_perf_issues();
        if !issues.is_empty() {
            println!("   PMU issues: {}", issues.join(" | "));
        }
        if let Some(hint) = linux_perf_hint(has_perf_counters, &issues) {
            println!("   PMU hint: {hint}");
        }
    }

    enforce_pmu_quality(name, has_perf_counters, &results);

    let mut table = TableFormatter::new(vec![], vec![23, 23, 23]);

    table.add_row(vec![
        &format!("Ops: {}", results.iterations),
        &format!("Samples: {}", config.target_samples),
        &format!("CV: {cv_percent:.2}%"),
    ]);

    table.add_row(vec![
        &format!("{:.2} Mops/s", ops_per_sec / 1_000_000.0),
        &format!("{ns_per_op:.2} ns/op"),
        &format!("{:.3}s total", summed_results.duration.as_secs_f64()),
    ]);

    if has_perf_counters {
        let active_percent = pmu_active_percent(&results);
        let pmu_avg_running_sec = results.pmu_time_running_ns as f64 / 1_000_000_000.0;
        let pmu_avg_enabled_sec = results.pmu_time_enabled_ns as f64 / 1_000_000_000.0;
        let pmu_total_running_sec = summed_results.pmu_time_running_ns as f64 / 1_000_000_000.0;
        let pmu_total_enabled_sec = summed_results.pmu_time_enabled_ns as f64 / 1_000_000_000.0;
        table.add_row(vec![
            &format!("{instructions_per_op:.1} inst/op"),
            &format!("{branches_per_op:.1} br/op"),
            &format!("{branch_miss_rate:.4}% miss"),
        ]);

        table.add_row(vec![
            &format!("{branch_misses_per_op:.4} br.miss/op"),
            &format!("{cache_miss_rate_per_op:.4} cache.miss/op"),
            &format!("{:.1}M branches", results.branches as f64 / 1_000_000.0),
        ]);

        table.add_row(vec![
            &format!("PMU active: {active_percent:.1}%"),
            &format!("{pmu_avg_running_sec:.3}s avg running"),
            &format!("{pmu_avg_enabled_sec:.3}s avg enabled"),
        ]);

        table.add_row(vec![
            "PMU totals",
            &format!("{pmu_total_running_sec:.3}s total running"),
            &format!("{pmu_total_enabled_sec:.3}s total enabled"),
        ]);
    } else {
        table.add_row(vec![
            &format!("{} chunks", results.chunks_executed),
            "perf counters",
            "unavailable",
        ]);
    }

    table.print();

    let benchmark_result = BenchmarkResult {
        name: name.to_string(),
        group: group.to_string(),
        benchmark_type: "standard".to_string(),
        mops_per_sec: ops_per_sec / 1_000_000.0,
        ns_per_op,
        instructions_per_op,
        branches_per_op,
        branch_miss_rate,
        branch_misses_per_op,
        cache_miss_rate: cache_miss_rate_per_op,
        cv_percent,
        samples: config.target_samples,
        operations: results.iterations,
        total_duration_sec: summed_results.duration.as_secs_f64(),
    };
    add_session_result(benchmark_result);
}

pub fn op_bench<T: BenchContext>(name: &str, group: &str, f: BenchFunction<T>) {
    #[cfg(target_os = "linux")]
    clear_perf_issues();

    let _affinity_guard = BenchAffinityGuard::acquire();

    println!("\n🚀 Benchmarking: {name}");

    // Warm-up and calibration phase
    let config = warm_up_and_calibrate(&f);

    // Main benchmark phase
    println!("⚡ Running {} samples...", config.target_samples);

    let mut all_results: Vec<Results> = Vec::new();
    let mut summed_results = Results::default();
    // Convert initial estimate from ops/ms to Mops/s for display consistency
    let mut running_throughput = if config.estimated_ops_per_ms > 0.0 {
        config.estimated_ops_per_ms / 1000.0
    } else {
        0.0
    };

    for sample in 0..config.target_samples {
        let sample_result = execute_sample(&f, config.chunk_size, sample);

        // Update running throughput estimate using millisecond precision for reliability
        let duration_ms = sample_result.duration.as_millis() as f64;
        let sample_throughput_mops =
            safe_ratio_f64(sample_result.iterations as f64, duration_ms) / 1000.0; // Convert ops/ms to Mops/s
        if sample_throughput_mops > 0.0 {
            running_throughput = running_throughput * 0.9 + sample_throughput_mops * 0.1;
        }

        summed_results.add(&sample_result);
        all_results.push(sample_result);

        // Update progress bar every few samples or on last sample
        if sample % 2 == 0 || sample == config.target_samples - 1 {
            update_progress_bar(sample + 1, config.target_samples, running_throughput);
        }
    }

    println!(); // New line after progress bar

    // Calculate statistics
    let mut results = summed_results.clone();
    results.divide(config.target_samples as u64);

    // Calculate throughput metrics
    let ops_per_sec = safe_ratio_f64(results.iterations as f64, results.duration.as_secs_f64());
    let ns_per_op = safe_ratio_f64(
        results.duration.as_nanos() as f64,
        results.iterations as f64,
    );
    let instructions_per_op =
        safe_ratio_f64(results.instructions as f64, results.iterations as f64);
    let branches_per_op = safe_ratio_f64(results.branches as f64, results.iterations as f64);
    let branch_miss_rate =
        safe_ratio_f64(results.branch_misses as f64, results.branches as f64) * 100.0;
    let branch_misses_per_op =
        safe_ratio_f64(results.branch_misses as f64, results.iterations as f64);
    let cache_miss_rate_per_op =
        safe_ratio_f64(results.cache_misses as f64, results.iterations as f64);
    let cv_percent = coefficient_of_variation_percent(&all_results);

    println!("\n📈 Results for {name}:");

    // Check if performance counters were actually used (non-zero values indicate they worked)
    let has_perf_counters = results.pmu_time_running_ns > 0
        || results.instructions > 0
        || results.branches > 0
        || results.branch_misses > 0
        || results.cache_misses > 0;

    if !has_perf_counters {
        #[cfg(target_os = "linux")]
        println!(
            "   Note: Performance counters not available (insufficient permissions or kernel support)"
        );
        #[cfg(not(target_os = "linux"))]
        println!("   Note: Performance counters not available on this platform");
    }
    #[cfg(target_os = "linux")]
    {
        let issues = current_perf_issues();
        if !issues.is_empty() {
            println!("   PMU issues: {}", issues.join(" | "));
        }
        if let Some(hint) = linux_perf_hint(has_perf_counters, &issues) {
            println!("   PMU hint: {hint}");
        }
    }

    enforce_pmu_quality(name, has_perf_counters, &results);

    // Use the generic TableFormatter for consistent formatting (no headers for metrics grid)
    let mut table = TableFormatter::new(
        vec![], // No headers - this is just a metrics grid
        vec![23, 23, 23],
    );

    table.add_row(vec![
        &format!("Ops: {}", results.iterations),
        &format!("Samples: {}", config.target_samples),
        &format!("CV: {cv_percent:.2}%"),
    ]);

    table.add_row(vec![
        &format!("{:.2} Mops/s", ops_per_sec / 1_000_000.0),
        &format!("{ns_per_op:.2} ns/op"),
        &format!("{:.3}s total", summed_results.duration.as_secs_f64()),
    ]);

    if has_perf_counters {
        let active_percent = pmu_active_percent(&results);
        let pmu_avg_running_sec = results.pmu_time_running_ns as f64 / 1_000_000_000.0;
        let pmu_avg_enabled_sec = results.pmu_time_enabled_ns as f64 / 1_000_000_000.0;
        let pmu_total_running_sec = summed_results.pmu_time_running_ns as f64 / 1_000_000_000.0;
        let pmu_total_enabled_sec = summed_results.pmu_time_enabled_ns as f64 / 1_000_000_000.0;
        table.add_row(vec![
            &format!("{instructions_per_op:.1} inst/op"),
            &format!("{branches_per_op:.1} br/op"),
            &format!("{branch_miss_rate:.4}% miss"),
        ]);

        table.add_row(vec![
            &format!("{branch_misses_per_op:.4} br.miss/op"),
            &format!("{cache_miss_rate_per_op:.4} cache.miss/op"),
            &format!("{:.1}M branches", results.branches as f64 / 1_000_000.0),
        ]);

        table.add_row(vec![
            &format!("PMU active: {active_percent:.1}%"),
            &format!("{pmu_avg_running_sec:.3}s avg running"),
            &format!("{pmu_avg_enabled_sec:.3}s avg enabled"),
        ]);

        table.add_row(vec![
            "PMU totals",
            &format!("{pmu_total_running_sec:.3}s total running"),
            &format!("{pmu_total_enabled_sec:.3}s total enabled"),
        ]);
    } else {
        table.add_row(vec![
            &format!("{} chunks", results.chunks_executed),
            "perf counters",
            "unavailable",
        ]);
    }

    table.print();

    // Collect result for session summary
    let benchmark_result = BenchmarkResult {
        name: name.to_string(),
        group: group.to_string(),
        benchmark_type: "standard".to_string(),
        mops_per_sec: ops_per_sec / 1_000_000.0,
        ns_per_op,
        instructions_per_op,
        branches_per_op,
        branch_miss_rate,
        branch_misses_per_op,
        cache_miss_rate: cache_miss_rate_per_op,
        cv_percent,
        samples: config.target_samples,
        operations: results.iterations,
        total_duration_sec: summed_results.duration.as_secs_f64(),
    };
    add_session_result(benchmark_result);
}

/// Benchmark definition structure
pub struct BenchmarkDef<T: BenchContext> {
    pub name: &'static str,
    pub group: &'static str,
    pub func: BenchFunction<T>,
}

/// Run a specific benchmark definition
pub fn run_benchmark<T: BenchContext>(bench: &BenchmarkDef<T>) {
    op_bench::<T>(bench.name, bench.group, bench.func);
}

/// Run benchmarks from a list based on filter
pub fn run_benchmark_group<T: BenchContext>(
    benchmarks: &[BenchmarkDef<T>],
    group_name: &str,
    filter: Option<&str>,
) {
    let should_run = |name: &str, group: &str| -> bool {
        match filter {
            None => true,
            Some(f) => f == "all" || name.contains(f) || group.contains(f) || f == group,
        }
    };

    let applicable_benchmarks: Vec<_> = benchmarks
        .iter()
        .filter(|b| should_run(b.name, b.group))
        .collect();

    if !applicable_benchmarks.is_empty() {
        eprintln!("\n=== {} ===", group_name.to_uppercase());
        for bench in applicable_benchmarks {
            run_benchmark(bench);
        }
    }
}
