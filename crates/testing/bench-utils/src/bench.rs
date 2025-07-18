// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::{BenchmarkResult, TableFormatter, add_session_result};
use minstant;
use std::hint::black_box;
use std::io::{self, Write};
use std::time::Duration;

#[cfg(target_os = "linux")]
use perf_event::{Builder, events::Hardware};

const MIN_CHUNK_SIZE: usize = 100_000; // Large enough for reliable timing
const MAX_CHUNK_SIZE: usize = 50_000_000; // Maximum reasonable chunk
const TARGET_CHUNK_DURATION_MS: u64 = 200; // Target 200ms per chunk for accurate timing
const WARM_UP_DURATION_MS: u64 = 1_000; // 1 second warm-up
const MIN_BENCHMARK_DURATION_MS: u64 = 5_000; // At least 5 seconds of actual benchmarking
const MIN_SAMPLES: usize = 20; // More samples for better statistics
const MAX_SAMPLES: usize = 50; // Reasonable upper bound

type BenchFunction<T> = fn(&mut T, usize, usize);

#[derive(Clone, Default)]
pub struct Results {
    pub instructions: u64,
    pub branches: u64,
    pub branch_misses: u64,
    pub cache_misses: u64,
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
    pub branch_counter: perf_event::Counter,
    pub branch_misses: perf_event::Counter,
    pub cache_misses: perf_event::Counter,
    pub start_time: Option<minstant::Instant>,
}

#[cfg(target_os = "linux")]
impl PerfCounters {
    pub fn new() -> Self {
        PerfCounters {
            instructions_counter: Builder::new(Hardware::INSTRUCTIONS).build().unwrap(),
            branch_counter: Builder::new(Hardware::BRANCH_INSTRUCTIONS).build().unwrap(),
            branch_misses: Builder::new(Hardware::BRANCH_MISSES).build().unwrap(),
            cache_misses: Builder::new(Hardware::CACHE_MISSES).build().unwrap(),
            start_time: None,
        }
    }

    pub fn start(&mut self) {
        self.start_time = Some(minstant::Instant::now());
        self.instructions_counter.enable().unwrap();
        self.branch_counter.enable().unwrap();
        self.branch_misses.enable().unwrap();
        self.cache_misses.enable().unwrap();
    }

    pub fn stop(&mut self) -> (Duration, u64, u64, u64, u64) {
        self.instructions_counter.disable().unwrap();
        self.branch_counter.disable().unwrap();
        self.branch_misses.disable().unwrap();
        self.cache_misses.disable().unwrap();

        let duration = self.start_time.unwrap().elapsed();
        let instructions = self.instructions_counter.read().unwrap();
        let branches = self.branch_counter.read().unwrap();
        let branch_misses = self.branch_misses.read().unwrap();
        let cache_misses = self.cache_misses.read().unwrap();

        (
            duration,
            instructions,
            branches,
            branch_misses,
            cache_misses,
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
}

/// Simple context for benchmarks that don't need preparation
pub struct NoContext;
impl BenchContext for NoContext {
    fn prepare(_num_chunks: usize) -> Self {
        NoContext
    }
}

/// Warm-up phase to determine optimal chunk size and estimate performance
#[cfg(target_os = "linux")]
fn warm_up_and_calibrate<T: BenchContext>(f: &BenchFunction<T>) -> BenchmarkConfig {
    print!("🔥 Warming up");
    io::stdout().flush().unwrap();

    // Check if context has a preferred chunk size
    if let Some(preferred_chunk_size) = T::chunk_size() {
        println!(" ✅");
        println!(
            "   Using preferred chunk size: {} ops",
            preferred_chunk_size
        );

        // Do a quick warm-up with the preferred size
        let warm_up_end = minstant::Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
        let mut warm_up_count = 0;
        let mut last_dot_time = minstant::Instant::now();
        while minstant::Instant::now() < warm_up_end {
            let mut prepared = T::prepare(preferred_chunk_size);
            black_box(|| f(&mut prepared, preferred_chunk_size, warm_up_count))();
            warm_up_count += 1;

            if minstant::Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100)
            {
                print!(".");
                io::stdout().flush().unwrap();
                last_dot_time = minstant::Instant::now();
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
        let started = minstant::Instant::now();
        let _context_return = black_box(|| f(&mut prepared, chunk_size, 0))();
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
            io::stdout().flush().unwrap();
        }
    }

    // Do additional warm-up iterations with limited dot output
    let warm_up_end = minstant::Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
    let mut warm_up_count = 0;
    let mut last_dot_time = minstant::Instant::now();
    while minstant::Instant::now() < warm_up_end {
        let mut prepared = T::prepare(chunk_size);
        black_box(|| f(&mut prepared, best_chunk_size, warm_up_count))();
        warm_up_count += 1;

        // Print a dot every 100ms instead of every 5 iterations
        if minstant::Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100) {
            print!(".");
            io::stdout().flush().unwrap();
            last_dot_time = minstant::Instant::now();
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
    println!("   Optimal chunk size: {} ops", best_chunk_size);
    if ops_per_ms > 0.0 {
        println!(
            "   Estimated performance: {:.1} Mops/s",
            ops_per_ms / 1000.0
        );
    } else {
        println!("   Estimated performance: Very fast (>1000 Mops/s)");
    }
    println!("   Target samples: {}", target_samples);

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
    io::stdout().flush().unwrap();

    // Check if context has a preferred chunk size
    if let Some(preferred_chunk_size) = T::chunk_size() {
        println!(" ✅");
        println!(
            "   Using preferred chunk size: {} ops",
            preferred_chunk_size
        );

        // Do a quick warm-up with the preferred size
        let warm_up_end = minstant::Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
        let mut warm_up_count = 0;
        let mut last_dot_time = minstant::Instant::now();
        while minstant::Instant::now() < warm_up_end {
            let mut prepared = T::prepare(preferred_chunk_size);
            black_box(|| f(&mut prepared, preferred_chunk_size, warm_up_count))();
            warm_up_count += 1;

            if minstant::Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100)
            {
                print!(".");
                io::stdout().flush().unwrap();
                last_dot_time = minstant::Instant::now();
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
        let started = minstant::Instant::now();
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
            io::stdout().flush().unwrap();
        }
    }

    // Additional warm-up iterations
    let warm_up_end = minstant::Instant::now() + Duration::from_millis(WARM_UP_DURATION_MS);
    let mut warm_up_count = 0;
    let mut last_dot_time = minstant::Instant::now();
    while minstant::Instant::now() < warm_up_end {
        let mut prepared = T::prepare(chunk_size);
        black_box(|| f(&mut prepared, best_chunk_size, warm_up_count))();
        warm_up_count += 1;

        if minstant::Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100) {
            print!(".");
            io::stdout().flush().unwrap();
            last_dot_time = minstant::Instant::now();
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
    println!("   Optimal chunk size: {} ops", best_chunk_size);
    if ops_per_ms > 0.0 {
        println!(
            "   Estimated performance: {:.1} Mops/s",
            ops_per_ms / 1000.0
        );
    } else {
        println!("   Estimated performance: Very fast (>1000 Mops/s)");
    }
    println!("   Target samples: {}", target_samples);

    BenchmarkConfig {
        chunk_size: best_chunk_size,
        target_samples,
        estimated_ops_per_ms: ops_per_ms,
    }
}

/// Execute a single benchmark sample with performance counters (if available)
#[cfg(target_os = "linux")]
fn execute_sample<T: BenchContext>(
    f: &BenchFunction<T>,
    chunk_size: usize,
    chunk_num: usize,
) -> Results {
    let mut prepared = T::prepare(chunk_size);

    // Try to create performance counters, fall back to timing-only if they fail
    let counters = (|| -> Result<_, Box<dyn std::error::Error>> {
        let instructions_counter = Builder::new(Hardware::INSTRUCTIONS).build()?;
        let branch_counter = Builder::new(Hardware::BRANCH_INSTRUCTIONS).build()?;
        let branch_misses = Builder::new(Hardware::BRANCH_MISSES).build()?;
        let cache_misses = Builder::new(Hardware::CACHE_MISSES).build()?;
        Ok((
            instructions_counter,
            branch_counter,
            branch_misses,
            cache_misses,
        ))
    })();

    match counters {
        Ok((mut instructions_counter, mut branch_counter, mut branch_misses, mut cache_misses)) => {
            // Performance counters available - use them
            instructions_counter.enable().unwrap();
            branch_counter.enable().unwrap();
            branch_misses.enable().unwrap();
            cache_misses.enable().unwrap();

            let start_time = minstant::Instant::now();
            black_box(|| f(&mut prepared, chunk_size, chunk_num))();
            let duration = start_time.elapsed();

            instructions_counter.disable().unwrap();
            branch_counter.disable().unwrap();
            branch_misses.disable().unwrap();
            cache_misses.disable().unwrap();

            Results {
                instructions: instructions_counter.read().unwrap(),
                branches: branch_counter.read().unwrap(),
                branch_misses: branch_misses.read().unwrap(),
                cache_misses: cache_misses.read().unwrap(),
                duration,
                iterations: chunk_size as u64,
                chunks_executed: 1,
            }
        }
        Err(_) => {
            // Performance counters not available - fall back to timing only
            let start_time = minstant::Instant::now();
            black_box(|| f(&mut prepared, chunk_size, chunk_num))();
            let duration = start_time.elapsed();

            Results {
                instructions: 0,
                branches: 0,
                branch_misses: 0,
                cache_misses: 0,
                duration,
                iterations: chunk_size as u64,
                chunks_executed: 1,
            }
        }
    }
}

/// Execute a single benchmark sample without performance counters
#[cfg(not(target_os = "linux"))]
fn execute_sample<T: BenchContext>(
    f: &BenchFunction<T>,
    chunk_size: usize,
    chunk_num: usize,
) -> Results {
    let mut prepared = T::prepare(chunk_size);

    let start_time = minstant::Instant::now();
    black_box(|| f(&mut prepared, chunk_size, chunk_num))();
    let duration = start_time.elapsed();

    Results {
        instructions: 0,
        branches: 0,
        branch_misses: 0,
        cache_misses: 0,
        duration,
        iterations: chunk_size as u64,
        chunks_executed: 1,
    }
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
            format!("{:.0} Mops/s", current_throughput)
        } else {
            format!("{:.1} Mops/s", current_throughput)
        }
    } else {
        "Calculating...".to_string()
    };

    print!(
        "] {}% ({}/{}) {}",
        percentage, current, total, throughput_display
    );

    io::stdout().flush().unwrap();
}

pub fn op_bench<T: BenchContext>(name: &str, group: &str, f: BenchFunction<T>) {
    println!("\n🚀 Benchmarking: {}", name);

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
        if duration_ms > 0.0 {
            let sample_throughput_mops = (sample_result.iterations as f64 / duration_ms) / 1000.0; // Convert ops/ms to Mops/s
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
    let ops_per_sec = results.iterations as f64 / results.duration.as_secs_f64();
    let ns_per_op = results.duration.as_nanos() as f64 / results.iterations as f64;
    let instructions_per_op = results.instructions as f64 / results.iterations as f64;
    let branches_per_op = results.branches as f64 / results.iterations as f64;
    let branch_miss_rate = if results.branches > 0 {
        (results.branch_misses as f64 / results.branches as f64) * 100.0
    } else {
        0.0
    };
    let cache_miss_rate_per_op = results.cache_misses as f64 / results.iterations as f64;

    // Calculate variance for throughput using consistent units
    let sample_throughputs: Vec<f64> = all_results
        .iter()
        .map(|r| r.iterations as f64 / r.duration.as_secs_f64())
        .collect();

    let mean_throughput = sample_throughputs.iter().sum::<f64>() / sample_throughputs.len() as f64;
    let variance: f64 = sample_throughputs
        .iter()
        .map(|&throughput| (throughput - mean_throughput).powi(2))
        .sum::<f64>()
        / sample_throughputs.len() as f64;
    let std_dev = variance.sqrt();
    let cv_percent = if mean_throughput > 0.0 {
        (std_dev / mean_throughput) * 100.0
    } else {
        0.0
    };

    println!("\n📈 Results for {}:", name);

    // Check if performance counters were actually used (non-zero values indicate they worked)
    let has_perf_counters = results.instructions > 0 || results.branches > 0;

    if !has_perf_counters {
        #[cfg(target_os = "linux")]
        println!(
            "   Note: Performance counters not available (insufficient permissions or kernel support)"
        );
        #[cfg(not(target_os = "linux"))]
        println!("   Note: Performance counters not available on this platform");
    }

    // Use the generic TableFormatter for consistent formatting (no headers for metrics grid)
    let mut table = TableFormatter::new(
        vec![], // No headers - this is just a metrics grid
        vec![23, 23, 23],
    );

    table.add_row(vec![
        &format!("Ops: {}", results.iterations),
        &format!("Samples: {}", config.target_samples),
        &format!("CV: {:.2}%", cv_percent),
    ]);

    table.add_row(vec![
        &format!("{:.2} Mops/s", ops_per_sec / 1_000_000.0),
        &format!("{:.2} ns/op", ns_per_op),
        &format!("{:.3}s total", summed_results.duration.as_secs_f64()),
    ]);

    if has_perf_counters {
        table.add_row(vec![
            &format!("{:.1} inst/op", instructions_per_op),
            &format!("{:.1} br/op", branches_per_op),
            &format!("{:.4}% miss", branch_miss_rate),
        ]);

        table.add_row(vec![
            &format!("{:.4} miss/op", cache_miss_rate_per_op),
            &format!("{:.1}M branches", results.branches as f64 / 1_000_000.0),
            &format!("{} chunks", results.chunks_executed),
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
