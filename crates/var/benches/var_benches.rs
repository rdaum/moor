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

#[cfg(target_os = "linux")]
mod bench_code {
    use moor_var::{
        ErrorCode, IndexMode, Var, v_bool, v_error, v_float, v_int, v_list, v_none, v_str,
    };
    #[cfg(target_os = "linux")]
    use perf_event::Builder;
    #[cfg(target_os = "linux")]
    use perf_event::events::Hardware;
    use std::f64::consts::PI;
    use std::fs::{self, File};
    use std::hint::black_box;
    use std::io::{self, Write};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const MIN_CHUNK_SIZE: usize = 100_000; // Large enough for reliable timing
    const MAX_CHUNK_SIZE: usize = 50_000_000; // Maximum reasonable chunk
    const TARGET_CHUNK_DURATION_MS: u64 = 200; // Target 200ms per chunk for accurate timing
    const WARM_UP_DURATION_MS: u64 = 1_000; // 1 second warm-up
    const MIN_BENCHMARK_DURATION_MS: u64 = 5_000; // At least 5 seconds of actual benchmarking
    const MIN_SAMPLES: usize = 20; // More samples for better statistics
    const MAX_SAMPLES: usize = 50; // Reasonable upper bound

    /// Collected benchmark result for session summary and JSON export
    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    pub(crate) struct BenchmarkResult {
        pub name: String,
        pub group: String,
        pub benchmark_type: String, // "standard" or "timed"
        pub mops_per_sec: f64,
        pub ns_per_op: f64,
        pub instructions_per_op: f64,
        pub branches_per_op: f64,
        pub branch_miss_rate: f64,
        pub cache_miss_rate: f64,
        pub cv_percent: f64,
        pub samples: usize,
        pub operations: u64,
        pub total_duration_sec: f64,
    }

    use lazy_static::lazy_static;
    use std::sync::Mutex;

    lazy_static! {
        /// Global storage for all benchmark results in this session
        static ref SESSION_RESULTS: Mutex<Vec<BenchmarkResult>> = Mutex::new(Vec::new());
    }

    /// Add a benchmark result to the session collection
    fn add_session_result(result: BenchmarkResult) {
        if let Ok(mut results) = SESSION_RESULTS.lock() {
            results.push(result);
        }
    }

    /// Get all collected results from this session
    fn get_session_results() -> Vec<BenchmarkResult> {
        SESSION_RESULTS
            .lock()
            .map(|results| results.clone())
            .unwrap_or_default()
    }

    /// Complete benchmark session data
    #[derive(serde::Serialize, serde::Deserialize)]
    struct BenchmarkSession {
        timestamp: String,
        hostname: String,
        git_commit: Option<String>,
        results: Vec<BenchmarkResult>,
    }

    /// Get the target directory for saving benchmark results, following criterion's approach
    fn get_target_directory() -> std::path::PathBuf {
        // Check CARGO_TARGET_DIR environment variable first
        if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
            return std::path::PathBuf::from(target_dir);
        }

        // Try cargo metadata to get target directory
        if let Ok(cargo) = std::env::var("CARGO") {
            if let Ok(output) = std::process::Command::new(cargo)
                .args(&["metadata", "--format-version", "1"])
                .output()
            {
                if let Ok(metadata_str) = String::from_utf8(output.stdout) {
                    // Simple JSON parsing to extract target_directory
                    if let Some(start) = metadata_str.find("\"target_directory\":\"") {
                        let start = start + "\"target_directory\":\"".len();
                        if let Some(end) = metadata_str[start..].find('"') {
                            let target_dir = &metadata_str[start..start + end];
                            return std::path::PathBuf::from(target_dir);
                        }
                    }
                }
            }
        }

        // Fallback to ./target
        std::path::PathBuf::from("target")
    }

    /// Save current session results to JSON file
    fn save_session_results() -> Result<String, Box<dyn std::error::Error>> {
        let results = get_session_results();
        if results.is_empty() {
            return Ok("No results to save".to_string());
        }

        // Get proper target directory
        let target_dir = get_target_directory();
        fs::create_dir_all(&target_dir)?;

        // Generate timestamp for filename
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let filename = target_dir.join(format!("benchmark_results_{}.json", timestamp));

        // Get git commit if available
        let git_commit = std::process::Command::new("git")
            .args(&["rev-parse", "--short", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            });

        // Get hostname
        let hostname = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        let session = BenchmarkSession {
            timestamp: format!("{}", timestamp), // Unix timestamp as string
            hostname,
            git_commit,
            results,
        };

        // Write JSON file
        let file = File::create(&filename)?;
        serde_json::to_writer_pretty(file, &session)?;

        Ok(filename.to_string_lossy().to_string())
    }

    /// Load previous benchmark results for comparison
    fn load_previous_results() -> Option<BenchmarkSession> {
        // Find the most recent benchmark file using proper target directory
        let target_dir = get_target_directory();
        if !target_dir.exists() {
            return None;
        }

        let mut json_files: Vec<_> = fs::read_dir(target_dir)
            .ok()?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("benchmark_results_")
                    && entry.file_name().to_string_lossy().ends_with(".json")
            })
            .collect();

        // Sort by modification time, newest first
        json_files.sort_by_key(|entry| {
            entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        });
        json_files.reverse();

        // Get the most recent file (since current session hasn't been saved yet)
        if json_files.is_empty() {
            return None;
        }

        let previous_file = &json_files[0];
        let file = File::open(previous_file.path()).ok()?;
        serde_json::from_reader(file).ok()
    }

    /// Generate final session summary with regression analysis
    pub(crate) fn generate_session_summary() {
        let current_results = get_session_results();
        if current_results.is_empty() {
            return;
        }

        println!("\n🎯 BENCHMARK SESSION SUMMARY");
        println!("═══════════════════════════════════════════════════════════════════════");

        // Load previous results for comparison
        let previous_session = load_previous_results();

        if let Some(ref prev) = previous_session {
            println!("📊 Comparing with previous run from {}", prev.timestamp);
            if let Some(ref commit) = prev.git_commit {
                println!("   Previous commit: {}", commit);
            }
            println!();
        }

        // Group results by category
        let mut groups: std::collections::HashMap<String, Vec<&BenchmarkResult>> =
            std::collections::HashMap::new();
        for result in &current_results {
            groups.entry(result.group.clone()).or_default().push(result);
        }

        // Display results by group with regression analysis
        for (group_name, group_results) in groups {
            println!(
                "📈 {} ({} benchmarks)",
                group_name.to_uppercase(),
                group_results.len()
            );
            println!("┌─────────────────────────┬─────────────┬─────────────┬─────────────┐");
            println!("│ Benchmark               │   Mops/s    │   ns/op     │   Change    │");
            println!("├─────────────────────────┼─────────────┼─────────────┼─────────────┤");

            for result in group_results {
                let change_info = if let Some(ref prev) = previous_session {
                    // Find matching benchmark from previous run
                    prev.results
                        .iter()
                        .find(|r| r.name == result.name)
                        .map(|prev_result| {
                            let mops_change = ((result.mops_per_sec - prev_result.mops_per_sec)
                                / prev_result.mops_per_sec)
                                * 100.0;
                            if mops_change.abs() < 1.0 {
                                "~0%".to_string()
                            } else if mops_change > 0.0 {
                                format!("+{:.1}% 🚀", mops_change)
                            } else {
                                format!("{:.1}% 📉", mops_change)
                            }
                        })
                        .unwrap_or_else(|| "NEW".to_string())
                } else {
                    "-".to_string()
                };

                // Format each column to exact width to match header exactly
                // Header:  "│ Benchmark               │   Mops/s    │   ns/op     │   Change    │"
                // Let's count: " Benchmark               " = 25 chars total
                //              "   Mops/s    " = 13 chars total
                //              "   ns/op     " = 13 chars total
                //              "   Change    " = 13 chars total
                let col1 = format!(" {:<23} ", truncate_string(&result.name, 23)); // " " + 23 + " " = 25
                let col2 = format!("   {:>7.1}   ", result.mops_per_sec); // "   " + 7 + "   " = 13
                let col3 = format!("   {:>6.2}    ", result.ns_per_op); // "   " + 6 + "    " = 13  
                let col4 = format!("   {:<7}   ", change_info); // "   " + 7 + "   " = 13

                println!("│{}│{}│{}│{}│", col1, col2, col3, col4);
            }
            println!("└─────────────────────────┴─────────────┴─────────────┴─────────────┘");
            println!();
        }

        // Key insights
        println!("🔍 KEY INSIGHTS:");
        let fastest = current_results
            .iter()
            .max_by(|a, b| a.mops_per_sec.partial_cmp(&b.mops_per_sec).unwrap());
        let slowest = current_results
            .iter()
            .min_by(|a, b| a.mops_per_sec.partial_cmp(&b.mops_per_sec).unwrap());

        if let (Some(fast), Some(slow)) = (fastest, slowest) {
            println!(
                "   🏆 Fastest: {} ({:.1} Mops/s)",
                fast.name, fast.mops_per_sec
            );
            println!(
                "   🐌 Slowest: {} ({:.1} Mops/s)",
                slow.name, slow.mops_per_sec
            );
            println!(
                "   📊 Speed difference: {:.1}x",
                fast.mops_per_sec / slow.mops_per_sec
            );
        }

        // Regression analysis summary
        if let Some(ref prev) = previous_session {
            let mut improvements = 0;
            let mut regressions = 0;
            let mut total_change = 0.0;

            for result in &current_results {
                if let Some(prev_result) = prev.results.iter().find(|r| r.name == result.name) {
                    let change = ((result.mops_per_sec - prev_result.mops_per_sec)
                        / prev_result.mops_per_sec)
                        * 100.0;
                    total_change += change;
                    if change > 1.0 {
                        improvements += 1;
                    } else if change < -1.0 {
                        regressions += 1;
                    }
                }
            }

            println!();
            println!("📊 REGRESSION ANALYSIS:");
            println!("   ✅ Improvements: {} benchmarks", improvements);
            println!("   ❌ Regressions: {} benchmarks", regressions);
            println!(
                "   📈 Average change: {:.1}%",
                total_change / current_results.len() as f64
            );
        }

        // Save results
        match save_session_results() {
            Ok(filename) => println!("\n💾 Results saved to: {}", filename),
            Err(e) => println!("\n⚠️  Failed to save results: {}", e),
        }

        println!("═══════════════════════════════════════════════════════════════════════");
    }

    fn truncate_string(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }

    #[derive(Clone, Default)]
    struct Results {
        instructions: u64,
        branches: u64,
        branch_misses: u64,
        cache_misses: u64,
        duration: Duration,
        iterations: u64,
        chunks_executed: u64,
    }

    impl Results {
        fn add(&mut self, other: &Results) {
            self.instructions += other.instructions;
            self.branches += other.branches;
            self.branch_misses += other.branch_misses;
            self.cache_misses += other.cache_misses;
            self.duration += other.duration;
            self.iterations += other.iterations;
            self.chunks_executed += other.chunks_executed;
        }

        fn divide(&mut self, divisor: u64) {
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

    struct BenchmarkConfig {
        chunk_size: usize,
        target_samples: usize,
        estimated_ops_per_ms: f64,
    }

    // Generic benchmark context that can hold any preparation data
    pub(crate) trait BenchContext {
        fn prepare() -> Self;
    }

    // Simple context for benchmarks that don't need preparation
    pub(crate) struct NoContext;
    impl BenchContext for NoContext {
        fn prepare() -> Self {
            NoContext
        }
    }

    // Note: VarContext and PreparedVars were removed as they're not used in the current benchmark suite

    /// Warm-up phase to determine optimal chunk size and estimate performance
    #[cfg(target_os = "linux")]
    fn warm_up_and_calibrate<T: BenchContext, F: Fn(&T, usize, usize)>(
        f: &F,
        prepared: &T,
    ) -> BenchmarkConfig {
        print!("🔥 Warming up");
        io::stdout().flush().unwrap();

        let mut chunk_size = MIN_CHUNK_SIZE;
        let mut best_chunk_size = chunk_size;
        let mut ops_per_ms = 0.0;

        // Try different chunk sizes to find one that takes target duration
        for i in 0..10 {
            // Max 10 iterations to find good chunk size
            let start = minstant::Instant::now();
            black_box(|| f(prepared, chunk_size, 0))();
            let duration = start.elapsed();

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
            black_box(|| f(prepared, best_chunk_size, warm_up_count))();
            warm_up_count += 1;

            // Print a dot every 100ms instead of every 5 iterations
            if minstant::Instant::now().duration_since(last_dot_time) >= Duration::from_millis(100)
            {
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

    /// Performance counter controls for fine-grained measurement
    #[cfg(target_os = "linux")]
    pub(crate) struct PerfCounters {
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

    /// Enhanced benchmark context trait that supports explicit timing control
    pub(crate) trait TimedBenchContext {
        fn prepare() -> Self;
        fn run_timed(&self, chunk_size: usize, chunk_num: usize, counters: &mut PerfCounters);
    }

    /// Execute a single benchmark sample with performance counters
    #[cfg(target_os = "linux")]
    fn execute_sample<T: BenchContext, F: Fn(&T, usize, usize)>(
        f: &F,
        prepared: &T,
        chunk_size: usize,
        chunk_num: usize,
    ) -> Results {
        let start_time = minstant::Instant::now();
        let mut instructions_counter = Builder::new(Hardware::INSTRUCTIONS).build().unwrap();
        let mut branch_counter = Builder::new(Hardware::BRANCH_INSTRUCTIONS).build().unwrap();
        let mut branch_misses = Builder::new(Hardware::BRANCH_MISSES).build().unwrap();
        let mut cache_misses = Builder::new(Hardware::CACHE_MISSES).build().unwrap();

        instructions_counter.enable().unwrap();
        branch_counter.enable().unwrap();
        branch_misses.enable().unwrap();
        cache_misses.enable().unwrap();

        black_box(|| f(prepared, chunk_size, chunk_num))();

        instructions_counter.disable().unwrap();
        branch_counter.disable().unwrap();
        branch_misses.disable().unwrap();
        cache_misses.disable().unwrap();

        Results {
            instructions: instructions_counter.read().unwrap(),
            branches: branch_counter.read().unwrap(),
            branch_misses: branch_misses.read().unwrap(),
            cache_misses: cache_misses.read().unwrap(),
            duration: start_time.elapsed(),
            iterations: chunk_size as u64,
            chunks_executed: 1,
        }
    }

    /// Execute a single timed benchmark sample with explicit counter control
    #[cfg(target_os = "linux")]
    fn execute_timed_sample<T: TimedBenchContext>(
        prepared: &T,
        chunk_size: usize,
        chunk_num: usize,
    ) -> Results {
        let mut counters = PerfCounters::new();

        // Run the benchmark - it will control when timing starts/stops
        prepared.run_timed(chunk_size, chunk_num, &mut counters);

        // This should never be called if run_timed doesn't call stop
        let (duration, instructions, branches, branch_misses, cache_misses) =
            if counters.start_time.is_some() {
                counters.stop()
            } else {
                (Duration::from_nanos(0), 0, 0, 0, 0)
            };

        Results {
            instructions,
            branches,
            branch_misses,
            cache_misses,
            duration,
            iterations: chunk_size as u64,
            chunks_executed: 1,
        }
    }

    /// Pretty progress bar with terminal-compatible characters
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

    /// Enhanced benchmark runner with warm-up and beautiful progress visualization
    #[cfg(target_os = "linux")]
    pub(crate) fn op_bench<T: BenchContext, F: Fn(&T, usize, usize)>(
        name: &str,
        group: &str,
        f: F,
    ) {
        println!("\n🚀 Benchmarking: {}", name);

        let prepared = T::prepare();

        // Warm-up and calibration phase
        let config = warm_up_and_calibrate(&f, &prepared);

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
            let sample_result = execute_sample(&f, &prepared, config.chunk_size, sample);

            // Update running throughput estimate using millisecond precision for reliability
            let duration_ms = sample_result.duration.as_millis() as f64;
            if duration_ms > 0.0 {
                let sample_throughput_mops =
                    (sample_result.iterations as f64 / duration_ms) / 1000.0; // Convert ops/ms to Mops/s
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

        let mean_throughput =
            sample_throughputs.iter().sum::<f64>() / sample_throughputs.len() as f64;
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

        // Helper function to ensure exact column width
        let pad_cell = |text: &str, width: usize| -> String {
            if text.len() >= width {
                text[..width].to_string()
            } else {
                let padding = width - text.len();
                let left_pad = padding / 2;
                let right_pad = padding - left_pad;
                format!("{}{}{}", " ".repeat(left_pad), text, " ".repeat(right_pad))
            }
        };

        let col_width = 23; // Each column exactly 23 chars

        // Generate table rows with exact width control
        let row1_col1 = pad_cell(&format!("Ops: {}", results.iterations), col_width);
        let row1_col2 = pad_cell(&format!("Samples: {}", config.target_samples), col_width);
        let row1_col3 = pad_cell(&format!("CV: {:.2}%", cv_percent), col_width);

        let row2_col1 = pad_cell(
            &format!("{:.2} Mops/s", ops_per_sec / 1_000_000.0),
            col_width,
        );
        let row2_col2 = pad_cell(&format!("{:.2} ns/op", ns_per_op), col_width);
        let row2_col3 = pad_cell(
            &format!("{:.3}s total", summed_results.duration.as_secs_f64()),
            col_width,
        );

        let row3_col1 = pad_cell(&format!("{:.1} inst/op", instructions_per_op), col_width);
        let row3_col2 = pad_cell(&format!("{:.1} br/op", branches_per_op), col_width);
        let row3_col3 = pad_cell(&format!("{:.4}% miss", branch_miss_rate), col_width);

        let row4_col1 = pad_cell(&format!("{:.4} miss/op", cache_miss_rate_per_op), col_width);
        let row4_col2 = pad_cell(
            &format!("{:.1}M branches", results.branches as f64 / 1_000_000.0),
            col_width,
        );
        let row4_col3 = pad_cell(&format!("{} chunks", results.chunks_executed), col_width);

        // Calculate exact box width: 3 columns + 4 borders = 3*23 + 4 = 73
        let box_width = col_width * 3 + 4;
        let box_line = format!("┌{}┐", "─".repeat(box_width - 2));
        let sep_line = format!("├{}┤", "─".repeat(box_width - 2));
        let end_line = format!("└{}┘", "─".repeat(box_width - 2));

        println!("\n📈 Results for {}:", name);
        println!("{}", box_line);
        println!("│{}│{}│{}│", row1_col1, row1_col2, row1_col3);
        println!("{}", sep_line);
        println!("│{}│{}│{}│", row2_col1, row2_col2, row2_col3);
        println!("{}", sep_line);
        println!("│{}│{}│{}│", row3_col1, row3_col2, row3_col3);
        println!("│{}│{}│{}│", row4_col1, row4_col2, row4_col3);
        println!("{}", end_line);

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
    pub(crate) struct BenchmarkDef<T: BenchContext> {
        pub name: &'static str,
        pub group: &'static str,
        pub func: fn(&T, usize, usize),
    }

    /// Enhanced benchmark runner for timed benchmarks
    #[cfg(target_os = "linux")]
    pub(crate) fn op_bench_timed<T: TimedBenchContext>(name: &str, group: &str) {
        println!("\n🚀 Benchmarking: {}", name);

        let prepared = T::prepare();

        // Simple warm-up for timed benchmarks - we can't easily auto-calibrate
        print!("🔥 Warming up");
        io::stdout().flush().unwrap();

        let chunk_size = MIN_CHUNK_SIZE; // Use minimum for timed benchmarks

        // Warm-up iterations
        let mut counters = PerfCounters::new();
        for i in 0..10 {
            prepared.run_timed(chunk_size, i, &mut counters);
            if i % 2 == 0 {
                print!(".");
                io::stdout().flush().unwrap();
            }
        }

        println!(" ✅");
        println!("   Chunk size: {} ops", chunk_size);

        let target_samples = 30; // Fixed samples for timed benchmarks

        // Main benchmark phase
        println!("⚡ Running {} samples...", target_samples);

        let mut all_results: Vec<Results> = Vec::new();
        let mut summed_results = Results::default();
        let mut running_throughput = 0.0;

        for sample in 0..target_samples {
            let sample_result = execute_timed_sample(&prepared, chunk_size, sample);

            // Update running throughput estimate using millisecond precision for reliability
            let duration_ms = sample_result.duration.as_millis() as f64;
            if duration_ms > 0.0 {
                let sample_throughput_mops =
                    (sample_result.iterations as f64 / duration_ms) / 1000.0; // Convert ops/ms to Mops/s
                running_throughput = running_throughput * 0.9 + sample_throughput_mops * 0.1;
            }

            summed_results.add(&sample_result);
            all_results.push(sample_result);

            // Update progress bar every few samples or on last sample
            if sample % 2 == 0 || sample == target_samples - 1 {
                update_progress_bar(sample + 1, target_samples, running_throughput);
            }
        }

        println!(); // New line after progress bar

        // Calculate statistics (same as regular benchmarks)
        let mut results = summed_results.clone();
        results.divide(target_samples as u64);

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

        let mean_throughput =
            sample_throughputs.iter().sum::<f64>() / sample_throughputs.len() as f64;
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

        // Helper function to ensure exact column width
        let pad_cell = |text: &str, width: usize| -> String {
            if text.len() >= width {
                text[..width].to_string()
            } else {
                let padding = width - text.len();
                let left_pad = padding / 2;
                let right_pad = padding - left_pad;
                format!("{}{}{}", " ".repeat(left_pad), text, " ".repeat(right_pad))
            }
        };

        let col_width = 23; // Each column exactly 23 chars

        // Generate table rows with exact width control
        let row1_col1 = pad_cell(&format!("Ops: {}", results.iterations), col_width);
        let row1_col2 = pad_cell(&format!("Samples: {}", target_samples), col_width);
        let row1_col3 = pad_cell(&format!("CV: {:.2}%", cv_percent), col_width);

        let row2_col1 = pad_cell(
            &format!("{:.2} Mops/s", ops_per_sec / 1_000_000.0),
            col_width,
        );
        let row2_col2 = pad_cell(&format!("{:.2} ns/op", ns_per_op), col_width);
        let row2_col3 = pad_cell(
            &format!("{:.3}s total", summed_results.duration.as_secs_f64()),
            col_width,
        );

        let row3_col1 = pad_cell(&format!("{:.1} inst/op", instructions_per_op), col_width);
        let row3_col2 = pad_cell(&format!("{:.1} br/op", branches_per_op), col_width);
        let row3_col3 = pad_cell(&format!("{:.4}% miss", branch_miss_rate), col_width);

        let row4_col1 = pad_cell(&format!("{:.4} miss/op", cache_miss_rate_per_op), col_width);
        let row4_col2 = pad_cell(
            &format!("{:.1}M branches", results.branches as f64 / 1_000_000.0),
            col_width,
        );
        let row4_col3 = pad_cell(&format!("{} chunks", results.chunks_executed), col_width);

        // Calculate exact box width: 3 columns + 4 borders = 3*23 + 4 = 73
        let box_width = col_width * 3 + 4;
        let box_line = format!("┌{}┐", "─".repeat(box_width - 2));
        let sep_line = format!("├{}┤", "─".repeat(box_width - 2));
        let end_line = format!("└{}┘", "─".repeat(box_width - 2));

        println!("\n📈 Results for {}:", name);
        println!("{}", box_line);
        println!("│{}│{}│{}│", row1_col1, row1_col2, row1_col3);
        println!("{}", sep_line);
        println!("│{}│{}│{}│", row2_col1, row2_col2, row2_col3);
        println!("{}", sep_line);
        println!("│{}│{}│{}│", row3_col1, row3_col2, row3_col3);
        println!("│{}│{}│{}│", row4_col1, row4_col2, row4_col3);
        println!("{}", end_line);

        // Collect result for session summary
        let benchmark_result = BenchmarkResult {
            name: name.to_string(),
            group: group.to_string(),
            benchmark_type: "timed".to_string(),
            mops_per_sec: ops_per_sec / 1_000_000.0,
            ns_per_op,
            instructions_per_op,
            branches_per_op,
            branch_miss_rate,
            cache_miss_rate: cache_miss_rate_per_op,
            cv_percent,
            samples: target_samples,
            operations: results.iterations,
            total_duration_sec: summed_results.duration.as_secs_f64(),
        };
        add_session_result(benchmark_result);
    }

    /// Run a specific benchmark definition
    #[cfg(target_os = "linux")]
    pub(crate) fn run_benchmark<T: BenchContext>(bench: &BenchmarkDef<T>) {
        op_bench::<T, _>(bench.name, bench.group, bench.func);
    }

    /// Run benchmarks from a list based on filter
    #[cfg(target_os = "linux")]
    pub(crate) fn run_benchmark_group<T: BenchContext>(
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

    // Context for integer benchmarks
    pub(crate) struct IntContext(Var);
    impl BenchContext for IntContext {
        fn prepare() -> Self {
            IntContext(v_int(0))
        }
    }

    // Context for small list benchmarks
    pub(crate) struct SmallListContext(Var);
    impl BenchContext for SmallListContext {
        fn prepare() -> Self {
            SmallListContext(v_list(&(0..8).map(v_int).collect::<Vec<_>>()))
        }
    }

    // Context for large list benchmarks
    pub(crate) struct LargeListContext(Var);
    impl BenchContext for LargeListContext {
        fn prepare() -> Self {
            LargeListContext(v_list(&(0..100_000).map(v_int).collect::<Vec<_>>()))
        }
    }

    pub(crate) fn int_add(ctx: &IntContext, chunk_size: usize, _chunk_num: usize) {
        let mut v = ctx.0.clone();
        for _ in 0..chunk_size {
            v = v.add(&v_int(1)).unwrap();
        }
    }

    pub(crate) fn int_eq(ctx: &IntContext, chunk_size: usize, _chunk_num: usize) {
        let v = ctx.0.clone();
        for _ in 0..chunk_size {
            let _ = v.eq(&v);
        }
    }

    pub(crate) fn int_cmp(ctx: &IntContext, chunk_size: usize, _chunk_num: usize) {
        let v = ctx.0.clone();
        for _ in 0..chunk_size {
            let _ = v.cmp(&v);
        }
    }

    pub(crate) fn list_push(ctx: &SmallListContext, chunk_size: usize, _chunk_num: usize) {
        let mut v = ctx.0.clone();
        for _ in 0..chunk_size {
            v = v.push(&v_int(1)).unwrap();
        }
    }

    pub(crate) fn list_index_pos(ctx: &LargeListContext, chunk_size: usize, _chunk_num: usize) {
        let v = ctx.0.clone();
        let list_len = 100_000; // LargeListContext has 100k items
        for c in 0..chunk_size {
            let index = c % list_len; // Cycle through available indices
            let _ = v.index(&v_int(index as i64), IndexMode::ZeroBased).unwrap();
        }
    }

    pub(crate) fn list_index_assign(ctx: &LargeListContext, chunk_size: usize, _chunk_num: usize) {
        let mut v = ctx.0.clone();
        let list_len = 100_000; // LargeListContext has 100k items
        for c in 0..chunk_size {
            let index = c % list_len; // Cycle through available indices
            v = v
                .index_set(
                    &v_int(index as i64),
                    &v_int(index as i64),
                    IndexMode::ZeroBased,
                )
                .unwrap();
        }
    }

    // === SCOPE SIMULATION BENCHMARKS ===
    // These simulate the creation and destruction of scopes (Vec<Var>) with different patterns

    pub(crate) fn scope_create_drop_ints(_ctx: &NoContext, chunk_size: usize, _chunk_num: usize) {
        for _ in 0..chunk_size {
            let scope: Vec<Var> = vec![v_int(1), v_int(2), v_int(3), v_int(4), v_int(5)];
            black_box(&scope); // Prevent optimization
            // scope drops here
        }
    }

    pub(crate) fn scope_create_drop_strings(
        _ctx: &NoContext,
        chunk_size: usize,
        _chunk_num: usize,
    ) {
        for _ in 0..chunk_size {
            let scope: Vec<Var> = vec![
                v_str("hello"),
                v_str("world"),
                v_str("test"),
                v_str("variable"),
                v_str("scope"),
            ];
            black_box(&scope);
            // scope drops here
        }
    }

    pub(crate) fn scope_create_drop_lists(_ctx: &NoContext, chunk_size: usize, _chunk_num: usize) {
        for _ in 0..chunk_size {
            let scope: Vec<Var> = vec![
                v_list(&[v_int(1), v_int(2)]),
                v_list(&[v_str("a"), v_str("b")]),
                v_list(&[v_int(3), v_int(4), v_int(5)]),
                v_list(&[]),
                v_list(&[v_int(6)]),
            ];
            black_box(&scope);
            // scope drops here
        }
    }

    pub(crate) fn scope_create_drop_mixed(_ctx: &NoContext, chunk_size: usize, _chunk_num: usize) {
        for _ in 0..chunk_size {
            let scope: Vec<Var> = vec![
                v_int(42),
                v_str("mixed"),
                v_list(&[v_int(1), v_str("nested")]),
                v_float(PI),
                v_bool(true),
                v_none(),
                v_error(ErrorCode::E_INVARG.into()),
            ];
            black_box(&scope);
            // scope drops here
        }
    }

    // === VAR CONSTRUCTION BENCHMARKS ===
    // These measure the cost of creating different types of Vars

    pub(crate) fn var_construct_ints(_ctx: &NoContext, chunk_size: usize, _chunk_num: usize) {
        for i in 0..chunk_size {
            let var = v_int(i as i64);
            black_box(var);
        }
    }

    pub(crate) fn var_construct_strings(_ctx: &NoContext, chunk_size: usize, chunk_num: usize) {
        for i in 0..chunk_size {
            let s = format!("string_{}_{})", chunk_num, i);
            let var = v_str(&s);
            black_box(var);
        }
    }

    pub(crate) fn var_construct_small_lists(
        _ctx: &NoContext,
        chunk_size: usize,
        _chunk_num: usize,
    ) {
        for i in 0..chunk_size {
            let var = v_list(&[v_int(i as i64), v_int((i + 1) as i64)]);
            black_box(var);
        }
    }

    pub(crate) fn var_construct_nested_lists(
        _ctx: &NoContext,
        chunk_size: usize,
        _chunk_num: usize,
    ) {
        for i in 0..chunk_size {
            let inner = v_list(&[v_int(i as i64), v_str("nested")]);
            let var = v_list(&[inner, v_int((i + 1) as i64)]);
            black_box(var);
        }
    }

    // === VAR DESTRUCTION BENCHMARKS ===
    // These measure ONLY the destruction cost using timed approach

    // Context that pre-builds a large pool of Vars to drop from
    pub(crate) struct DropContext {
        int_vars: Vec<Var>,
        string_vars: Vec<Var>,
        list_vars: Vec<Var>,
        mixed_vars: Vec<Var>,
    }

    impl TimedBenchContext for DropContext {
        fn prepare() -> Self {
            let pool_size = 100_000; // Reasonable size for fast preparation
            DropContext {
                int_vars: (0..pool_size).map(|i| v_int(i as i64)).collect(),
                string_vars: (0..pool_size)
                    .map(|i| v_str(&format!("string_{}", i)))
                    .collect(),
                list_vars: (0..pool_size)
                    .map(|i| v_list(&[v_int(i as i64), v_str("item")]))
                    .collect(),
                mixed_vars: (0..pool_size)
                    .map(|i| match i % 5 {
                        0 => v_int(i as i64),
                        1 => v_str(&format!("str_{}", i)),
                        2 => v_list(&[v_int(i as i64)]),
                        3 => v_float(i as f64),
                        _ => v_bool(i % 2 == 0),
                    })
                    .collect(),
            }
        }

        fn run_timed(&self, _chunk_size: usize, _chunk_num: usize, _counters: &mut PerfCounters) {
            // This method will be overridden by specific drop benchmark implementations
            panic!("DropContext should not be called directly");
        }
    }

    pub(crate) struct DropIntsContext(DropContext);
    impl TimedBenchContext for DropIntsContext {
        fn prepare() -> Self {
            DropIntsContext(DropContext::prepare())
        }

        fn run_timed(&self, chunk_size: usize, _chunk_num: usize, counters: &mut PerfCounters) {
            // PRE-TIMING: Create the data to drop (clone cost not measured)
            let mut data_to_drop = Vec::with_capacity(chunk_size);
            for i in 0..chunk_size {
                data_to_drop.push(self.0.int_vars[i % self.0.int_vars.len()].clone());
            }

            // START TIMING: Measure only the drop cost
            counters.start();

            // TIMED PORTION: The actual drop
            drop(data_to_drop);

            // STOP TIMING
            let _results = counters.stop();
        }
    }

    pub(crate) struct DropStringsContext(DropContext);
    impl TimedBenchContext for DropStringsContext {
        fn prepare() -> Self {
            DropStringsContext(DropContext::prepare())
        }

        fn run_timed(&self, chunk_size: usize, _chunk_num: usize, counters: &mut PerfCounters) {
            // PRE-TIMING: Create the data to drop (clone cost not measured)
            let mut data_to_drop = Vec::with_capacity(chunk_size);
            for i in 0..chunk_size {
                data_to_drop.push(self.0.string_vars[i % self.0.string_vars.len()].clone());
            }

            // START TIMING: Measure only the drop cost
            counters.start();

            // TIMED PORTION: The actual drop
            drop(data_to_drop);

            // STOP TIMING
            let _results = counters.stop();
        }
    }

    pub(crate) struct DropListsContext(DropContext);
    impl TimedBenchContext for DropListsContext {
        fn prepare() -> Self {
            DropListsContext(DropContext::prepare())
        }

        fn run_timed(&self, chunk_size: usize, _chunk_num: usize, counters: &mut PerfCounters) {
            // PRE-TIMING: Create the data to drop (clone cost not measured)
            let mut data_to_drop = Vec::with_capacity(chunk_size);
            for i in 0..chunk_size {
                data_to_drop.push(self.0.list_vars[i % self.0.list_vars.len()].clone());
            }

            // START TIMING: Measure only the drop cost
            counters.start();

            // TIMED PORTION: The actual drop
            drop(data_to_drop);

            // STOP TIMING
            let _results = counters.stop();
        }
    }

    pub(crate) struct DropMixedContext(DropContext);
    impl TimedBenchContext for DropMixedContext {
        fn prepare() -> Self {
            DropMixedContext(DropContext::prepare())
        }

        fn run_timed(&self, chunk_size: usize, _chunk_num: usize, counters: &mut PerfCounters) {
            // PRE-TIMING: Create the data to drop (clone cost not measured)
            let mut data_to_drop = Vec::with_capacity(chunk_size);
            for i in 0..chunk_size {
                data_to_drop.push(self.0.mixed_vars[i % self.0.mixed_vars.len()].clone());
            }

            // START TIMING: Measure only the drop cost
            counters.start();

            // TIMED PORTION: The actual drop
            drop(data_to_drop);

            // STOP TIMING
            let _results = counters.stop();
        }
    }

    // === CLONE BENCHMARKS ===
    // These measure cloning costs which are relevant for scope operations

    // Context for string clone benchmarks
    pub(crate) struct StringCloneContext(Var);
    impl BenchContext for StringCloneContext {
        fn prepare() -> Self {
            StringCloneContext(v_str("test_string_for_cloning"))
        }
    }

    // Context for list clone benchmarks
    pub(crate) struct ListCloneContext(Var);
    impl BenchContext for ListCloneContext {
        fn prepare() -> Self {
            ListCloneContext(v_list(&[v_int(1), v_str("test"), v_int(2), v_str("clone")]))
        }
    }

    pub(crate) fn var_clone_strings(
        ctx: &StringCloneContext,
        chunk_size: usize,
        _chunk_num: usize,
    ) {
        for _ in 0..chunk_size {
            let cloned = ctx.0.clone();
            black_box(cloned);
        }
    }

    pub(crate) fn var_clone_lists(ctx: &ListCloneContext, chunk_size: usize, _chunk_num: usize) {
        for _ in 0..chunk_size {
            let cloned = ctx.0.clone();
            black_box(cloned);
        }
    }
}

// Linux only...
#[cfg(target_os = "linux")]
pub fn main() {
    use crate::bench_code::{
        BenchmarkDef,
        // Contexts
        DropIntsContext,
        DropListsContext,
        DropMixedContext,
        DropStringsContext,
        IntContext,
        LargeListContext,
        ListCloneContext,
        NoContext,
        SmallListContext,
        StringCloneContext,
        generate_session_summary,
        // Functions
        int_add,
        int_cmp,
        int_eq,
        list_index_assign,
        list_index_pos,
        list_push,
        op_bench_timed,
        run_benchmark_group,
        scope_create_drop_ints,
        scope_create_drop_lists,
        scope_create_drop_mixed,
        scope_create_drop_strings,
        var_clone_lists,
        var_clone_strings,
        var_construct_ints,
        var_construct_nested_lists,
        var_construct_small_lists,
        var_construct_strings,
    };
    #[cfg(target_os = "linux")]
    use perf_event::Builder;
    #[cfg(target_os = "linux")]
    use perf_event::events::Hardware;
    use std::env;

    // Check if we can do perf events, and if not just exit early (without panic) so that test runners etc
    // don't fail.
    if Builder::new(Hardware::INSTRUCTIONS).build().is_err() {
        eprintln!("Perf events are not supported on this system. Skipping benchmarks.");
        return;
    }

    let args: Vec<String> = env::args().collect();
    // Look for filter arguments after "--"
    let filter = if let Some(separator_pos) = args.iter().position(|arg| arg == "--") {
        // Filter is the first argument after "--"
        args.get(separator_pos + 1).map(|s| s.as_str())
    } else {
        // Fallback: look for any non-flag argument that's not our binary name
        args.iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--") && !args[0].contains(arg.as_str()))
            .map(|s| s.as_str())
    };

    if let Some(f) = filter {
        eprintln!("Running benchmarks matching filter: '{}'", f);
        eprintln!(
            "Available filters: all, original, scope, construct, drop, clone, or any benchmark name substring"
        );
        eprintln!();
    }

    // Define all benchmark groups declaratively
    let original_int_benchmarks = [
        BenchmarkDef {
            name: "int_add",
            group: "original",
            func: int_add,
        },
        BenchmarkDef {
            name: "int_eq",
            group: "original",
            func: int_eq,
        },
        BenchmarkDef {
            name: "int_cmp",
            group: "original",
            func: int_cmp,
        },
    ];

    let original_small_list_benchmarks = [BenchmarkDef {
        name: "list_push",
        group: "original",
        func: list_push,
    }];

    let original_large_list_benchmarks = [
        BenchmarkDef {
            name: "list_index_pos",
            group: "original",
            func: list_index_pos,
        },
        BenchmarkDef {
            name: "list_index_assign",
            group: "original",
            func: list_index_assign,
        },
    ];

    let scope_benchmarks = [
        BenchmarkDef {
            name: "scope_create_drop_ints",
            group: "scope",
            func: scope_create_drop_ints,
        },
        BenchmarkDef {
            name: "scope_create_drop_strings",
            group: "scope",
            func: scope_create_drop_strings,
        },
        BenchmarkDef {
            name: "scope_create_drop_lists",
            group: "scope",
            func: scope_create_drop_lists,
        },
        BenchmarkDef {
            name: "scope_create_drop_mixed",
            group: "scope",
            func: scope_create_drop_mixed,
        },
    ];

    let construct_benchmarks = [
        BenchmarkDef {
            name: "var_construct_ints",
            group: "construct",
            func: var_construct_ints,
        },
        BenchmarkDef {
            name: "var_construct_strings",
            group: "construct",
            func: var_construct_strings,
        },
        BenchmarkDef {
            name: "var_construct_small_lists",
            group: "construct",
            func: var_construct_small_lists,
        },
        BenchmarkDef {
            name: "var_construct_nested_lists",
            group: "construct",
            func: var_construct_nested_lists,
        },
    ];

    // Timed drop benchmarks are handled separately since they use TimedBenchContext
    let should_run_drop = |filter: Option<&str>| -> bool {
        match filter {
            None => true,
            Some(f) => f == "all" || f.contains("drop") || f == "drop",
        }
    };

    let clone_string_benchmarks = [BenchmarkDef {
        name: "var_clone_strings",
        group: "clone",
        func: var_clone_strings,
    }];

    let clone_list_benchmarks = [BenchmarkDef {
        name: "var_clone_lists",
        group: "clone",
        func: var_clone_lists,
    }];

    // Run benchmark groups
    run_benchmark_group::<IntContext>(&original_int_benchmarks, "Original Int Benchmarks", filter);
    run_benchmark_group::<SmallListContext>(
        &original_small_list_benchmarks,
        "Original Small List Benchmarks",
        filter,
    );
    run_benchmark_group::<LargeListContext>(
        &original_large_list_benchmarks,
        "Original Large List Benchmarks",
        filter,
    );
    run_benchmark_group::<NoContext>(&scope_benchmarks, "Scope Simulation Benchmarks", filter);
    run_benchmark_group::<NoContext>(&construct_benchmarks, "Var Construction Benchmarks", filter);
    // Run timed drop benchmarks
    if should_run_drop(filter) {
        eprintln!("\n=== VAR DESTRUCTION (PURE DROP) BENCHMARKS ===");
        op_bench_timed::<DropIntsContext>("var_drop_ints", "drop");
        op_bench_timed::<DropStringsContext>("var_drop_strings", "drop");
        op_bench_timed::<DropListsContext>("var_drop_lists", "drop");
        op_bench_timed::<DropMixedContext>("var_drop_mixed", "drop");
    }
    run_benchmark_group::<StringCloneContext>(
        &clone_string_benchmarks,
        "Var Clone (String) Benchmarks",
        filter,
    );
    run_benchmark_group::<ListCloneContext>(
        &clone_list_benchmarks,
        "Var Clone (List) Benchmarks",
        filter,
    );

    if filter.is_some() {
        eprintln!("\nBenchmark filtering complete.");
    }

    // Generate session summary with regression analysis
    generate_session_summary();
}

// Non-linux platforms will not run the benchmarks
#[cfg(not(target_os = "linux"))]
pub fn main() {
    eprintln!("Var micro-benchmarks are only supported on Linux due to perf_event usage.");
}
