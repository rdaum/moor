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

//! Property update load test - measures DB transaction performance under write-heavy workloads.
//! Creates N objects with M properties each, then performs scattered random property updates
//! and reads across concurrent workers. Focuses on exercising the DB TX model rather than
//! the scheduler or VM.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use clap::Parser;
use clap_derive::Parser;
#[cfg(target_os = "linux")]
use micromeasure::{LinuxPerfBackend, MeasurementBackend, MetricValue};
use moor_common::model::{WorldStateCountOp, WorldStateTimerOp};
use moor_common::{
    model::{CommitResult, ObjAttrs, ObjFlag, ObjectKind, PropFlag},
    util::{BitEnum, scale_hot_sample_sum_nanos, scale_rare_sample_sum_nanos},
};
use moor_db::{Database, StorageMaintenanceStats, TxDB, db_counters};
use moor_model_checker::bench_common::{
    calculate_percentiles, clear_screen, format_duration, format_throughput, setup_db_path,
    update_spinner,
};
use moor_var::{NOTHING, Obj, Symbol, v_int};
use rand::{RngExt, SeedableRng, rngs::SmallRng, rngs::StdRng, seq::SliceRandom};
use tabled::{Table, Tabled};
use tracing::info;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long, help = "Database path", default_value = "prop_test_db")]
    db_path: PathBuf,

    #[arg(long, help = "Min number of concurrent workers", default_value = "1")]
    min_concurrency: usize,

    #[arg(long, help = "Max number of concurrent workers", default_value = "32")]
    max_concurrency: usize,

    #[arg(long, help = "Number of test objects to create", default_value = "100")]
    num_objects: usize,

    #[arg(long, help = "Number of properties per object", default_value = "10")]
    num_properties: usize,

    #[arg(
        long,
        help = "Number of operations per worker per iteration",
        default_value = "1000"
    )]
    ops_per_iteration: usize,

    #[arg(
        long,
        help = "Number of iterations per concurrency level",
        default_value = "10"
    )]
    num_iterations: usize,

    #[arg(
        long,
        help = "Run every concurrency level for this many seconds instead of using fixed iterations"
    )]
    measurement_duration_seconds: Option<u64>,

    #[arg(
        long,
        help = "Measure idle package power for this many milliseconds before each concurrency level",
        default_value = "1000"
    )]
    idle_sample_millis: u64,

    #[arg(
        long,
        help = "Wait up to this many milliseconds for storage maintenance to settle before each level",
        default_value = "30000"
    )]
    maintenance_settle_millis: u64,

    #[arg(
        long,
        help = "Run concurrency levels in deterministic randomized order"
    )]
    randomize_concurrency: bool,

    #[arg(
        long,
        help = "Seed used with --randomize-concurrency",
        default_value = "12648430"
    )]
    concurrency_seed: u64,

    #[arg(
        long,
        help = "Read/write ratio (0.0 = all writes, 1.0 = all reads)",
        default_value = "0.9"
    )]
    read_ratio: f64,

    #[arg(
        long,
        help = "Stop each concurrency level after this many successful writes",
        default_value = "50000"
    )]
    max_writes_per_level: usize,

    #[arg(long, help = "CSV output file for benchmark data")]
    output_file: Option<PathBuf>,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,

    #[arg(
        long,
        help = "Swamp mode: immediately run at maximum concurrency",
        default_value = "false"
    )]
    swamp_mode: bool,

    #[arg(
        long,
        help = "Duration in seconds for swamp mode",
        default_value = "30"
    )]
    swamp_duration_seconds: u64,
}

#[derive(Tabled)]
struct BenchmarkRow {
    #[tabled(rename = "Conc")]
    concurrency: usize,
    #[tabled(rename = "Commits")]
    commits: usize,
    #[tabled(rename = "Read/s")]
    read_throughput: String,
    #[tabled(rename = "Write/s")]
    write_throughput: String,
    #[tabled(rename = "Wall Time")]
    wall_time: String,
    #[tabled(rename = "Commit/s")]
    commit_throughput: String,
    #[tabled(rename = "Per-Thrd")]
    per_thread_throughput: String,
    #[tabled(rename = "Conflict%")]
    conflict_pct: String,
    #[tabled(rename = "Retry/Commit")]
    retries_per_commit: String,
    #[tabled(rename = "Commit Phase")]
    commit_phase_share: String,
    #[tabled(rename = "Avg Check")]
    avg_check: String,
    #[tabled(rename = "Avg Apply")]
    avg_apply: String,
    #[tabled(rename = "Idx Ins")]
    avg_index_insert: String,
    #[tabled(rename = "BW Pressure")]
    batch_writer_backpressure: u64,
    #[tabled(rename = "BW Block")]
    avg_batch_writer_block: String,
    #[tabled(rename = "Idle Maint")]
    idle_maintenance: String,
    #[tabled(rename = "Settle")]
    maintenance_settle: String,
    #[tabled(rename = "WB End")]
    write_buffer_end: String,
    #[tabled(rename = "Flush End")]
    outstanding_flushes_end: String,
    #[tabled(rename = "Comp End/Δ")]
    compactions_completed: String,
    #[tabled(rename = "p50")]
    p50: String,
    #[tabled(rename = "p95")]
    p95: String,
    #[tabled(rename = "p99")]
    p99: String,
    #[tabled(rename = "max")]
    max: String,
    #[tabled(rename = "Pkg Power")]
    package_power: String,
    #[tabled(rename = "Idle Power")]
    idle_power: String,
    #[tabled(rename = "Dyn Power")]
    dynamic_power: String,
    #[tabled(rename = "Energy/Commit")]
    energy_per_commit: String,
    #[tabled(rename = "Dyn Energy/Commit")]
    dynamic_energy_per_commit: String,
}

/// Test objects and their properties
struct TestSetup {
    objects: Vec<Obj>,
    property_names: Vec<Symbol>,
}

fn setup_test_database(
    database: &TxDB,
    num_objects: usize,
    num_properties: usize,
) -> Result<TestSetup, eyre::Error> {
    let mut loader = database.loader_client()?;

    // Create a wizard player object to own everything
    let player_attrs = ObjAttrs::new(
        NOTHING,
        NOTHING,
        NOTHING,
        BitEnum::new_with(ObjFlag::User) | ObjFlag::Wizard,
        "Wizard",
    );

    let player = loader.create_object(ObjectKind::Objid(Obj::mk_id(1)), &player_attrs)?;
    loader.set_object_owner(&player, &player)?;
    info!("Created wizard player object: {}", player);

    // Create system object #0
    let system_attrs = ObjAttrs::new(
        NOTHING,
        NOTHING,
        NOTHING,
        ObjFlag::User.into(),
        "System Object",
    );
    let system_obj = loader.create_object(ObjectKind::Objid(Obj::mk_id(0)), &system_attrs)?;
    loader.set_object_owner(&system_obj, &system_obj)?;

    // Generate property names
    let property_names: Vec<Symbol> = (0..num_properties)
        .map(|i| Symbol::mk(&format!("prop_{}", i)))
        .collect();

    // Create test objects with properties
    let mut objects = Vec::with_capacity(num_objects);
    for i in 0..num_objects {
        let obj_attrs = ObjAttrs::new(
            player,
            NOTHING,
            NOTHING,
            ObjFlag::User.into(),
            &format!("TestObject{}", i),
        );

        let new_obj = loader.create_object(ObjectKind::NextObjid, &obj_attrs)?;

        // Define properties on each object with initial value
        for prop_name in &property_names {
            loader.define_property(
                &new_obj,
                &new_obj,
                *prop_name,
                &player,
                BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                Some(v_int(0)),
            )?;
        }

        objects.push(new_obj);

        if (i + 1) % 100 == 0 {
            info!("Created {} test objects...", i + 1);
        }
    }

    match loader.commit()? {
        CommitResult::Success { .. } => {
            info!(
                "Initialized test database: {} objects, {} properties each",
                num_objects, num_properties
            );
            Ok(TestSetup {
                objects,
                property_names,
            })
        }
        CommitResult::ConflictRetry { .. } => {
            Err(eyre::eyre!("Database conflict during initialization"))
        }
    }
}

struct WorkerResult {
    reads: usize,
    writes: usize,
    conflicts: usize,
    commit_latencies: Vec<Duration>,
}

#[derive(Clone, Copy)]
enum WorkLimit {
    Operations(usize),
    Deadline(Instant),
}

impl WorkLimit {
    fn should_continue(self, completed: usize) -> bool {
        match self {
            Self::Operations(operations) => completed < operations,
            Self::Deadline(deadline) => Instant::now() < deadline,
        }
    }
}

fn run_worker(
    database: &TxDB,
    setup: &TestSetup,
    limit: WorkLimit,
    read_ratio: f64,
    seed: u64,
    writes_remaining: &AtomicU64,
) -> Result<WorkerResult, eyre::Error> {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut reads = 0;
    let mut writes = 0;
    let mut conflicts = 0;
    let mut commit_latencies = Vec::new();

    while limit.should_continue(reads + writes) {
        if writes_remaining.load(Ordering::Relaxed) == 0 {
            break;
        }

        // Pick random object and property
        let obj_idx = rng.random_range(0..setup.objects.len());
        let prop_idx = rng.random_range(0..setup.property_names.len());
        let obj = setup.objects[obj_idx];
        let prop = setup.property_names[prop_idx];

        let is_read = rng.random::<f64>() < read_ratio;

        if !is_read
            && writes_remaining
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_err()
        {
            break;
        }

        // Execute operation in a transaction
        loop {
            let mut tx = database.loader_client()?;

            if is_read {
                let _ = tx.get_existing_property_value(&obj, prop);
                reads += 1;
            } else {
                let new_value = v_int(rng.random_range(0i64..1_000_000));
                tx.set_property(&obj, prop, None, None, Some(new_value))?;
                writes += 1;
            }

            let commit_start = Instant::now();
            match tx.commit()? {
                CommitResult::Success { .. } => {
                    commit_latencies.push(commit_start.elapsed());
                    break;
                }
                CommitResult::ConflictRetry { .. } => {
                    conflicts += 1;
                    // Decrement the counter since we'll retry
                    if is_read {
                        reads -= 1;
                    } else {
                        writes -= 1;
                    }
                    continue;
                }
            }
        }
    }

    Ok(WorkerResult {
        reads,
        writes,
        conflicts,
        commit_latencies,
    })
}

struct Results {
    concurrency: usize,
    commits: usize,
    reads: usize,
    writes: usize,
    wall_time: Duration,
    conflicts: usize,
    commit_latencies: Vec<Duration>,
    package_energy_joules: Option<f64>,
    package_power_watts: Option<f64>,
    package_uj_per_commit: Option<f64>,
    idle_package_power_watts: Option<f64>,
    dynamic_package_power_watts: Option<f64>,
    dynamic_package_uj_per_commit: Option<f64>,
    batch_writer_backpressure: u64,
    batch_writer_block_time: Duration,
    storage_before_idle: Option<StorageMaintenanceStats>,
    storage_before_load: Option<StorageMaintenanceStats>,
    storage_after_load: Option<StorageMaintenanceStats>,
    maintenance_settle_time: Duration,
    maintenance_settled: bool,
}

#[derive(Default)]
struct RaplMetrics {
    package_energy_joules: Option<f64>,
    package_power_watts: Option<f64>,
    package_uj_per_commit: Option<f64>,
}

#[cfg(target_os = "linux")]
struct RaplMeasurement {
    backend: LinuxPerfBackend,
}

#[cfg(target_os = "linux")]
impl RaplMeasurement {
    fn begin() -> Self {
        let mut backend = LinuxPerfBackend::new().with_rapl_energy();
        backend.begin();
        Self { backend }
    }

    fn finish(mut self, elapsed: Duration, commits: usize) -> RaplMetrics {
        self.backend.end();
        let mut results = Default::default();
        let mut metrics = Vec::new();
        self.backend
            .collect(elapsed, commits as u64, 0, &mut results, &mut metrics);
        let metric = |name| {
            metrics
                .iter()
                .find(|metric: &&MetricValue| metric.name == name)
                .map(|metric| metric.value)
        };
        RaplMetrics {
            package_energy_joules: metric("rapl_package_joules"),
            package_power_watts: metric("rapl_package_watts"),
            package_uj_per_commit: metric("rapl_package_uj_per_op"),
        }
    }
}

#[cfg(not(target_os = "linux"))]
struct RaplMeasurement;

#[cfg(not(target_os = "linux"))]
impl RaplMeasurement {
    fn begin() -> Self {
        Self
    }

    fn finish(self, _elapsed: Duration, _commits: usize) -> RaplMetrics {
        RaplMetrics::default()
    }
}

fn measure_idle_package_power(duration: Duration) -> Option<f64> {
    if duration.is_zero() {
        return None;
    }
    let rapl = RaplMeasurement::begin();
    let start = Instant::now();
    std::thread::sleep(duration);
    rapl.finish(start.elapsed(), 0).package_power_watts
}

fn dynamic_rapl_metrics(
    package_power_watts: Option<f64>,
    idle_package_power_watts: Option<f64>,
    throughput: f64,
) -> (Option<f64>, Option<f64>) {
    let dynamic_power = package_power_watts
        .zip(idle_package_power_watts)
        .map(|(package, idle)| (package - idle).max(0.0));
    let dynamic_uj_per_commit = dynamic_power
        .filter(|_| throughput > 0.0)
        .map(|watts| watts * 1_000_000.0 / throughput);
    (dynamic_power, dynamic_uj_per_commit)
}

fn format_optional(value: Option<f64>, unit: &str) -> String {
    value.map_or_else(|| "-".to_string(), |value| format!("{value:.2}{unit}"))
}

fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    format!("{:.1}MiB", bytes as f64 / MIB)
}

fn maintenance_during_idle(
    before: Option<StorageMaintenanceStats>,
    after: Option<StorageMaintenanceStats>,
) -> bool {
    before.zip(after).is_some_and(|(before, after)| {
        before.is_active()
            || after.is_active()
            || after.write_buffer_bytes != before.write_buffer_bytes
            || after.outstanding_flushes != before.outstanding_flushes
            || after.compactions_completed != before.compactions_completed
            || after.compaction_time != before.compaction_time
            || after.journal_count != before.journal_count
            || after.journal_bytes != before.journal_bytes
            || after.disk_bytes != before.disk_bytes
    })
}

fn wait_for_storage_quiescence(database: &TxDB, timeout: Duration) -> (Duration, bool) {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const STABLE_INTERVAL: Duration = Duration::from_millis(500);

    let start = Instant::now();
    let Some(mut previous) = database.storage_maintenance_stats() else {
        return (Duration::ZERO, true);
    };
    let mut stable_since = (!previous.is_active()).then(Instant::now);

    loop {
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            return (elapsed, false);
        }

        std::thread::sleep(POLL_INTERVAL.min(timeout - elapsed));
        let Some(current) = database.storage_maintenance_stats() else {
            return (start.elapsed(), true);
        };

        if current.is_active() || current != previous {
            stable_since = None;
        } else if stable_since.is_none() {
            stable_since = Some(Instant::now());
        }

        if stable_since.is_some_and(|stable_since| stable_since.elapsed() >= STABLE_INTERVAL) {
            return (start.elapsed(), true);
        }
        previous = current;
    }
}

fn avg_u64(total: u64, count: u64) -> u64 {
    total.checked_div(count).unwrap_or(0)
}

fn concurrency_levels(args: &Args) -> Vec<usize> {
    let mut levels = Vec::new();
    let mut concurrency = args.min_concurrency as f32;
    while concurrency <= args.max_concurrency as f32 {
        levels.push(concurrency as usize);
        let mut next = concurrency * 1.25;
        if next as usize <= concurrency as usize {
            next = concurrency + 1.0;
        }
        concurrency = next;
    }
    if args.randomize_concurrency {
        levels.shuffle(&mut StdRng::seed_from_u64(args.concurrency_seed));
    }
    levels
}

fn run_workload(
    database: &TxDB,
    setup: &TestSetup,
    args: &Args,
    concurrency: usize,
    limit: WorkLimit,
    seed_offset: u64,
    writes_remaining: Arc<AtomicU64>,
) -> Result<Results, eyre::Error> {
    let conflict_count = Arc::new(AtomicU64::new(0));
    let read_count = Arc::new(AtomicU64::new(0));
    let write_count = Arc::new(AtomicU64::new(0));

    let start = Instant::now();

    // Spawn worker threads
    let handles: Vec<_> = (0..concurrency)
        .map(|worker_id| {
            let db = database.clone();
            let setup_objs = setup.objects.clone();
            let setup_props = setup.property_names.clone();
            let read_ratio = args.read_ratio;
            let seed = seed_offset.wrapping_add(worker_id as u64);
            let conflicts = Arc::clone(&conflict_count);
            let reads = Arc::clone(&read_count);
            let writes = Arc::clone(&write_count);
            let writes_remaining = Arc::clone(&writes_remaining);

            std::thread::spawn(move || {
                let setup = TestSetup {
                    objects: setup_objs,
                    property_names: setup_props,
                };
                let result = run_worker(&db, &setup, limit, read_ratio, seed, &writes_remaining)?;
                conflicts.fetch_add(result.conflicts as u64, Ordering::Relaxed);
                reads.fetch_add(result.reads as u64, Ordering::Relaxed);
                writes.fetch_add(result.writes as u64, Ordering::Relaxed);
                Ok::<_, eyre::Error>(result.commit_latencies)
            })
        })
        .collect();

    // Collect results
    let mut all_commit_latencies = Vec::new();
    for handle in handles {
        let latencies = handle
            .join()
            .map_err(|_| eyre::eyre!("Worker thread panicked"))??;
        all_commit_latencies.extend(latencies);
    }

    let wall_time = start.elapsed();
    let conflicts = conflict_count.load(Ordering::Relaxed) as usize;
    let reads = read_count.load(Ordering::Relaxed) as usize;
    let writes = write_count.load(Ordering::Relaxed) as usize;
    let commits = all_commit_latencies.len();

    Ok(Results {
        concurrency,
        commits,
        reads,
        writes,
        wall_time,
        conflicts,
        commit_latencies: all_commit_latencies,
        package_energy_joules: None,
        package_power_watts: None,
        package_uj_per_commit: None,
        idle_package_power_watts: None,
        dynamic_package_power_watts: None,
        dynamic_package_uj_per_commit: None,
        batch_writer_backpressure: 0,
        batch_writer_block_time: Duration::ZERO,
        storage_before_idle: None,
        storage_before_load: None,
        storage_after_load: None,
        maintenance_settle_time: Duration::ZERO,
        maintenance_settled: true,
    })
}

fn run_benchmark(
    database: &TxDB,
    setup: &TestSetup,
    args: &Args,
) -> Result<Vec<Results>, eyre::Error> {
    let mut results = vec![];
    let mut table_rows = vec![];

    // Warm-up run
    info!("Running warm-up...");
    let warmup_start = Instant::now();
    for iteration in 0..3 {
        run_workload(
            database,
            setup,
            args,
            1,
            WorkLimit::Operations(args.ops_per_iteration),
            iteration,
            Arc::new(AtomicU64::new(args.max_writes_per_level as u64)),
        )?;
    }
    info!("Warm-up completed in {:?}", warmup_start.elapsed());

    // Cool down
    info!("Cooling down for 1 second...");
    std::thread::sleep(Duration::from_secs(1));

    let mut spinner_idx = 0;

    for num_concurrent in concurrency_levels(args) {
        let (maintenance_settle_time, maintenance_settled) = wait_for_storage_quiescence(
            database,
            Duration::from_millis(args.maintenance_settle_millis),
        );
        let storage_before_idle = database.storage_maintenance_stats();
        let idle_package_power_watts =
            measure_idle_package_power(Duration::from_millis(args.idle_sample_millis));
        let storage_before_load = database.storage_maintenance_stats();
        let idle_maintenance = maintenance_during_idle(storage_before_idle, storage_before_load);
        let rapl = RaplMeasurement::begin();
        let measurement_start = Instant::now();

        // Capture baseline counters for commit thread utilization
        let counters = db_counters();
        let baseline_check_nanos = scale_hot_sample_sum_nanos(
            counters
                .timers_hot
                .sample_sum_nanos(WorldStateTimerOp::CommitCheckPhase),
        );
        let baseline_check_count = counters
            .timers_hot
            .calls(WorldStateTimerOp::CommitCheckPhase);
        let baseline_apply_nanos = scale_hot_sample_sum_nanos(
            counters
                .timers_hot
                .sample_sum_nanos(WorldStateTimerOp::CommitApplyPhase),
        );
        let baseline_apply_count = counters
            .timers_hot
            .calls(WorldStateTimerOp::CommitApplyPhase);
        let baseline_index_insert_nanos = scale_rare_sample_sum_nanos(
            counters
                .timers_rare
                .sample_sum_nanos(WorldStateTimerOp::ApplyIndexInsert),
        );
        let baseline_index_insert_count = counters
            .timers_rare
            .calls(WorldStateTimerOp::ApplyIndexInsert);
        let baseline_batch_writer_backpressure = counters
            .counters
            .get(WorldStateCountOp::BatchWriterBackpressure);
        let baseline_batch_writer_block_nanos = scale_rare_sample_sum_nanos(
            counters
                .timers_rare
                .sample_sum_nanos(WorldStateTimerOp::BatchWriterBackpressureBlock),
        );
        let baseline_batch_writer_block_count = counters
            .timers_rare
            .calls(WorldStateTimerOp::BatchWriterBackpressureBlock);

        // Run multiple iterations and aggregate
        let iterations = if args.measurement_duration_seconds.is_some() {
            1
        } else {
            args.num_iterations
        };
        let deadline = args
            .measurement_duration_seconds
            .map(|seconds| Instant::now() + Duration::from_secs(seconds));
        let mut iteration_results = Vec::with_capacity(iterations);
        let writes_remaining = Arc::new(AtomicU64::new(args.max_writes_per_level as u64));
        for i in 0..iterations {
            update_spinner(
                &mut spinner_idx,
                &format!(
                    "Concurrency {}: iteration {}/{}...",
                    num_concurrent,
                    i + 1,
                    iterations
                ),
            );
            let limit = deadline
                .map_or(WorkLimit::Operations(args.ops_per_iteration), |deadline| {
                    WorkLimit::Deadline(deadline)
                });
            let seed_offset = args
                .concurrency_seed
                .wrapping_add((i as u64) << 32)
                .wrapping_add(num_concurrent as u64);
            let result = run_workload(
                database,
                setup,
                args,
                num_concurrent,
                limit,
                seed_offset,
                Arc::clone(&writes_remaining),
            )?;
            iteration_results.push(result);
            if writes_remaining.load(Ordering::Relaxed) == 0 {
                break;
            }
        }

        // Aggregate results
        let total_commits: usize = iteration_results.iter().map(|r| r.commits).sum();
        let total_reads: usize = iteration_results.iter().map(|r| r.reads).sum();
        let total_writes: usize = iteration_results.iter().map(|r| r.writes).sum();
        let total_conflicts: usize = iteration_results.iter().map(|r| r.conflicts).sum();
        let total_wall_time: Duration = iteration_results.iter().map(|r| r.wall_time).sum();
        let measurement_time = measurement_start.elapsed();
        let rapl = rapl.finish(measurement_time, total_commits);
        let storage_after_load = database.storage_maintenance_stats();

        let all_commit_latencies: Vec<Duration> = iteration_results
            .into_iter()
            .flat_map(|r| r.commit_latencies)
            .collect();
        let (p50, p95, p99, max) = calculate_percentiles(all_commit_latencies.clone());

        let conflict_pct = if total_commits > 0 {
            (total_conflicts as f64 / (total_commits + total_conflicts) as f64) * 100.0
        } else {
            0.0
        };

        // Calculate throughput rates
        let commit_throughput = total_commits as f64 / total_wall_time.as_secs_f64();
        let read_throughput = total_reads as f64 / total_wall_time.as_secs_f64();
        let write_throughput = total_writes as f64 / total_wall_time.as_secs_f64();
        let per_thread_throughput = commit_throughput / num_concurrent as f64;
        let retries_per_commit = total_conflicts as f64 / total_commits.max(1) as f64;
        let valid_idle_power = (maintenance_settled && !idle_maintenance)
            .then_some(idle_package_power_watts)
            .flatten();
        let (dynamic_package_power_watts, dynamic_package_uj_per_commit) = dynamic_rapl_metrics(
            rapl.package_power_watts,
            valid_idle_power,
            commit_throughput,
        );

        // Calculate commit thread utilization (how much of wall time was spent in commit phases)
        let counters = db_counters();
        let check_nanos = scale_hot_sample_sum_nanos(
            counters
                .timers_hot
                .sample_sum_nanos(WorldStateTimerOp::CommitCheckPhase),
        ) - baseline_check_nanos;
        let check_count = counters
            .timers_hot
            .calls(WorldStateTimerOp::CommitCheckPhase)
            - baseline_check_count;
        let apply_nanos = scale_hot_sample_sum_nanos(
            counters
                .timers_hot
                .sample_sum_nanos(WorldStateTimerOp::CommitApplyPhase),
        ) - baseline_apply_nanos;
        let apply_count = counters
            .timers_hot
            .calls(WorldStateTimerOp::CommitApplyPhase)
            - baseline_apply_count;
        let index_insert_nanos = scale_rare_sample_sum_nanos(
            counters
                .timers_rare
                .sample_sum_nanos(WorldStateTimerOp::ApplyIndexInsert),
        ) - baseline_index_insert_nanos;
        let index_insert_count = counters
            .timers_rare
            .calls(WorldStateTimerOp::ApplyIndexInsert)
            - baseline_index_insert_count;
        let batch_writer_backpressure = counters
            .counters
            .get(WorldStateCountOp::BatchWriterBackpressure)
            .saturating_sub(baseline_batch_writer_backpressure)
            as u64;
        let batch_writer_block_nanos = scale_rare_sample_sum_nanos(
            counters
                .timers_rare
                .sample_sum_nanos(WorldStateTimerOp::BatchWriterBackpressureBlock),
        ) - baseline_batch_writer_block_nanos;
        let batch_writer_block_count = counters
            .timers_rare
            .calls(WorldStateTimerOp::BatchWriterBackpressureBlock)
            - baseline_batch_writer_block_count;

        // Utilization based on check+apply (write is background, doesn't block)
        let total_commit_thread_nanos = check_nanos + apply_nanos;
        let commit_thread_util_pct =
            (total_commit_thread_nanos as f64 / total_wall_time.as_nanos() as f64) * 100.0;

        // Average times per phase (only for commits that went through each phase)
        let avg_check_nanos = avg_u64(check_nanos, check_count);
        let avg_apply_nanos = avg_u64(apply_nanos, apply_count);
        let avg_index_insert_nanos = avg_u64(index_insert_nanos, index_insert_count);
        let avg_batch_writer_block_nanos =
            avg_u64(batch_writer_block_nanos, batch_writer_block_count);
        let write_buffer_end = storage_after_load
            .map(|stats| format_bytes(stats.write_buffer_bytes))
            .unwrap_or_else(|| "-".to_string());
        let outstanding_flushes_end = storage_after_load
            .map(|stats| stats.outstanding_flushes.to_string())
            .unwrap_or_else(|| "-".to_string());
        let compactions_completed = storage_before_load
            .zip(storage_after_load)
            .map(|(before, after)| {
                let completed = after
                    .compactions_completed
                    .saturating_sub(before.compactions_completed);
                format!("{}/{completed}", after.active_compactions)
            })
            .unwrap_or_else(|| "-".to_string());

        table_rows.push(BenchmarkRow {
            concurrency: num_concurrent,
            commits: total_commits,
            read_throughput: format_throughput(read_throughput),
            write_throughput: format_throughput(write_throughput),
            wall_time: format_duration(total_wall_time),
            commit_throughput: format_throughput(commit_throughput),
            per_thread_throughput: format_throughput(per_thread_throughput),
            conflict_pct: format!("{:.2}%", conflict_pct),
            retries_per_commit: format!("{retries_per_commit:.3}"),
            commit_phase_share: format!("{:.1}%", commit_thread_util_pct),
            avg_check: format_duration(Duration::from_nanos(avg_check_nanos)),
            avg_apply: format_duration(Duration::from_nanos(avg_apply_nanos)),
            avg_index_insert: format_duration(Duration::from_nanos(avg_index_insert_nanos)),
            batch_writer_backpressure,
            avg_batch_writer_block: format_duration(Duration::from_nanos(
                avg_batch_writer_block_nanos,
            )),
            idle_maintenance: if idle_maintenance { "yes" } else { "no" }.to_string(),
            maintenance_settle: format!(
                "{}{}",
                format_duration(maintenance_settle_time),
                if maintenance_settled { "" } else { "!" }
            ),
            write_buffer_end,
            outstanding_flushes_end,
            compactions_completed,
            p50: format_duration(p50),
            p95: format_duration(p95),
            p99: format_duration(p99),
            max: format_duration(max),
            package_power: format_optional(rapl.package_power_watts, "W"),
            idle_power: format_optional(idle_package_power_watts, "W"),
            dynamic_power: format_optional(dynamic_package_power_watts, "W"),
            energy_per_commit: format_optional(rapl.package_uj_per_commit, "µJ"),
            dynamic_energy_per_commit: format_optional(dynamic_package_uj_per_commit, "µJ"),
        });

        results.push(Results {
            concurrency: num_concurrent,
            commits: total_commits,
            reads: total_reads,
            writes: total_writes,
            wall_time: total_wall_time,
            conflicts: total_conflicts,
            commit_latencies: all_commit_latencies,
            package_energy_joules: rapl.package_energy_joules,
            package_power_watts: rapl.package_power_watts,
            package_uj_per_commit: rapl.package_uj_per_commit,
            idle_package_power_watts,
            dynamic_package_power_watts,
            dynamic_package_uj_per_commit,
            batch_writer_backpressure,
            batch_writer_block_time: Duration::from_nanos(batch_writer_block_nanos),
            storage_before_idle,
            storage_before_load,
            storage_after_load,
            maintenance_settle_time,
            maintenance_settled,
        });

        // Redraw table
        clear_screen();
        eprintln!(
            "Property Update Load Test\nObjects: {}, Properties/obj: {}, Read ratio: {:.0}%, Write cap/level: {}\n",
            args.num_objects,
            args.num_properties,
            args.read_ratio * 100.0,
            args.max_writes_per_level,
        );
        eprintln!("{}", Table::new(&table_rows));
    }

    Ok(results)
}

fn run_swamp_mode(
    database: &TxDB,
    setup: &TestSetup,
    args: &Args,
) -> Result<Vec<Results>, eyre::Error> {
    let duration = Duration::from_secs(args.swamp_duration_seconds);
    let concurrency = args.max_concurrency;

    info!(
        "Starting swamp mode: {} concurrent workers for {} seconds",
        concurrency, args.swamp_duration_seconds
    );

    let total_ops = Arc::new(AtomicU64::new(0));
    let conflict_count = Arc::new(AtomicU64::new(0));
    let read_count = Arc::new(AtomicU64::new(0));
    let write_count = Arc::new(AtomicU64::new(0));

    let (maintenance_settle_time, maintenance_settled) = wait_for_storage_quiescence(
        database,
        Duration::from_millis(args.maintenance_settle_millis),
    );
    let storage_before_idle = database.storage_maintenance_stats();
    let idle_package_power_watts =
        measure_idle_package_power(Duration::from_millis(args.idle_sample_millis));
    let storage_before_load = database.storage_maintenance_stats();
    let idle_maintenance = maintenance_during_idle(storage_before_idle, storage_before_load);
    let counters = db_counters();
    let baseline_batch_writer_backpressure = counters
        .counters
        .get(WorldStateCountOp::BatchWriterBackpressure);
    let baseline_batch_writer_block_nanos = scale_rare_sample_sum_nanos(
        counters
            .timers_rare
            .sample_sum_nanos(WorldStateTimerOp::BatchWriterBackpressureBlock),
    );
    let rapl = RaplMeasurement::begin();
    let start = Instant::now();
    let stop_time = start + duration;

    // Spawn worker threads that run until stop_time
    let handles: Vec<_> = (0..concurrency)
        .map(|worker_id| {
            let db = database.clone();
            let setup_objs = setup.objects.clone();
            let setup_props = setup.property_names.clone();
            let read_ratio = args.read_ratio;
            let ops = Arc::clone(&total_ops);
            let conflicts = Arc::clone(&conflict_count);
            let reads = Arc::clone(&read_count);
            let writes = Arc::clone(&write_count);

            std::thread::spawn(move || {
                let mut rng = SmallRng::seed_from_u64(worker_id as u64);
                let setup = TestSetup {
                    objects: setup_objs,
                    property_names: setup_props,
                };

                while Instant::now() < stop_time {
                    let obj_idx = rng.random_range(0..setup.objects.len());
                    let prop_idx = rng.random_range(0..setup.property_names.len());
                    let obj = setup.objects[obj_idx];
                    let prop = setup.property_names[prop_idx];
                    let is_read = rng.random::<f64>() < read_ratio;

                    loop {
                        let mut tx = db.loader_client().unwrap();

                        if is_read {
                            let _ = tx.get_existing_property_value(&obj, prop);
                        } else {
                            let new_value = v_int(rng.random_range(0i64..1_000_000));
                            tx.set_property(&obj, prop, None, None, Some(new_value))
                                .unwrap();
                        }

                        match tx.commit().unwrap() {
                            CommitResult::Success { .. } => {
                                ops.fetch_add(1, Ordering::Relaxed);
                                if is_read {
                                    reads.fetch_add(1, Ordering::Relaxed);
                                } else {
                                    writes.fetch_add(1, Ordering::Relaxed);
                                }
                                break;
                            }
                            CommitResult::ConflictRetry { .. } => {
                                conflicts.fetch_add(1, Ordering::Relaxed);
                                continue;
                            }
                        }
                    }
                }
            })
        })
        .collect();

    // Wait for all workers
    for handle in handles {
        handle
            .join()
            .map_err(|_| eyre::eyre!("Worker thread panicked"))?;
    }

    let wall_time = start.elapsed();
    let commits = total_ops.load(Ordering::Relaxed) as usize;
    let conflicts = conflict_count.load(Ordering::Relaxed) as usize;
    let reads = read_count.load(Ordering::Relaxed) as usize;
    let writes = write_count.load(Ordering::Relaxed) as usize;
    let storage_after_load = database.storage_maintenance_stats();

    let commit_throughput = commits as f64 / wall_time.as_secs_f64();
    let rapl = rapl.finish(wall_time, commits);
    let valid_idle_power = (maintenance_settled && !idle_maintenance)
        .then_some(idle_package_power_watts)
        .flatten();
    let (dynamic_package_power_watts, dynamic_package_uj_per_commit) = dynamic_rapl_metrics(
        rapl.package_power_watts,
        valid_idle_power,
        commit_throughput,
    );
    let counters = db_counters();
    let batch_writer_backpressure = counters
        .counters
        .get(WorldStateCountOp::BatchWriterBackpressure)
        .saturating_sub(baseline_batch_writer_backpressure)
        as u64;
    let batch_writer_block_nanos = scale_rare_sample_sum_nanos(
        counters
            .timers_rare
            .sample_sum_nanos(WorldStateTimerOp::BatchWriterBackpressureBlock),
    ) - baseline_batch_writer_block_nanos;
    let conflict_pct = if commits > 0 {
        (conflicts as f64 / (commits + conflicts) as f64) * 100.0
    } else {
        0.0
    };

    clear_screen();
    eprintln!(
        "Property Update Load Test - Swamp Mode\nObjects: {}, Properties/obj: {}, Read ratio: {:.0}%\n",
        args.num_objects,
        args.num_properties,
        args.read_ratio * 100.0
    );
    let per_thread_throughput = commit_throughput / concurrency as f64;

    eprintln!(
        "Concurrency: {}, Duration: {:?}\n\
         Commits: {}, Reads: {}, Writes: {}\n\
         Total Commit/s: {}, Per-Thread: {}\n\
         Conflicts: {} ({:.2}%)\n\
         Package Power: {}, Idle Power: {}, Dynamic Power: {}\n\
         Energy/Commit: {}, Dynamic Energy/Commit: {}",
        concurrency,
        wall_time,
        commits,
        reads,
        writes,
        format_throughput(commit_throughput),
        format_throughput(per_thread_throughput),
        conflicts,
        conflict_pct,
        format_optional(rapl.package_power_watts, "W"),
        format_optional(idle_package_power_watts, "W"),
        format_optional(dynamic_package_power_watts, "W"),
        format_optional(rapl.package_uj_per_commit, "µJ"),
        format_optional(dynamic_package_uj_per_commit, "µJ"),
    );

    Ok(vec![Results {
        concurrency,
        commits,
        reads,
        writes,
        wall_time,
        conflicts,
        commit_latencies: vec![], // Not tracked in swamp mode
        package_energy_joules: rapl.package_energy_joules,
        package_power_watts: rapl.package_power_watts,
        package_uj_per_commit: rapl.package_uj_per_commit,
        idle_package_power_watts,
        dynamic_package_power_watts,
        dynamic_package_uj_per_commit,
        batch_writer_backpressure,
        batch_writer_block_time: Duration::from_nanos(batch_writer_block_nanos),
        storage_before_idle,
        storage_before_load,
        storage_after_load,
        maintenance_settle_time,
        maintenance_settled,
    }])
}

fn main() -> Result<(), eyre::Error> {
    color_eyre::install().expect("Unable to install color_eyre");
    let args: Args = Args::parse();
    if !(0.0..=1.0).contains(&args.read_ratio) {
        return Err(eyre::eyre!("--read-ratio must be between 0 and 1"));
    }
    if args.num_objects == 0 || args.num_properties == 0 {
        return Err(eyre::eyre!(
            "--num-objects and --num-properties must be greater than zero"
        ));
    }
    if args.min_concurrency == 0 || args.min_concurrency > args.max_concurrency {
        return Err(eyre::eyre!(
            "concurrency must be non-zero and min must not exceed max"
        ));
    }
    if args.measurement_duration_seconds == Some(0) {
        return Err(eyre::eyre!(
            "--measurement-duration-seconds must be greater than zero"
        ));
    }
    if args.max_writes_per_level == 0 {
        return Err(eyre::eyre!(
            "--max-writes-per-level must be greater than zero"
        ));
    }
    if args.measurement_duration_seconds.is_none()
        && (args.ops_per_iteration == 0 || args.num_iterations == 0)
    {
        return Err(eyre::eyre!(
            "--ops-per-iteration and --num-iterations must be greater than zero"
        ));
    }

    moor_common::tracing::init_tracing(args.debug).unwrap_or_else(|e| {
        eprintln!("Unable to configure logging: {e}");
        std::process::exit(1);
    });

    info!("Starting property update load test");

    // Create temporary directory for database if using default path
    let (db_path, _temp_dir) = setup_db_path(&args.db_path, "prop_test_db")?;

    // Create database
    let (database, _) = TxDB::try_open_fjall(&db_path, Default::default()).unwrap();

    // Setup test database
    info!(
        "Creating {} objects with {} properties each...",
        args.num_objects, args.num_properties
    );
    let setup = setup_test_database(&database, args.num_objects, args.num_properties)?;

    // Run benchmark
    let results = if args.swamp_mode {
        run_swamp_mode(&database, &setup, &args)?
    } else {
        run_benchmark(&database, &setup, &args)?
    };

    // Write CSV if requested
    if let Some(output_file) = args.output_file {
        let num_records = results.len();
        let mut writer =
            csv::Writer::from_path(&output_file).expect("Could not open benchmark output file");

        let header = vec![
            "concurrency".to_string(),
            "commits".to_string(),
            "reads".to_string(),
            "writes".to_string(),
            "wall_time_ns".to_string(),
            "conflicts".to_string(),
            "commit_throughput".to_string(),
            "read_throughput".to_string(),
            "write_throughput".to_string(),
            "conflict_percent".to_string(),
            "retries_per_commit".to_string(),
            "commit_latency_p50_ns".to_string(),
            "commit_latency_p95_ns".to_string(),
            "commit_latency_p99_ns".to_string(),
            "commit_latency_max_ns".to_string(),
            "package_energy_joules".to_string(),
            "package_power_watts".to_string(),
            "package_uj_per_commit".to_string(),
            "idle_package_power_watts".to_string(),
            "dynamic_package_power_watts".to_string(),
            "dynamic_package_uj_per_commit".to_string(),
            "batch_writer_backpressure_events".to_string(),
            "batch_writer_block_time_ns".to_string(),
            "storage_before_idle_write_buffer_bytes".to_string(),
            "storage_before_idle_outstanding_flushes".to_string(),
            "storage_before_idle_active_compactions".to_string(),
            "storage_before_idle_compactions_completed".to_string(),
            "storage_before_idle_compaction_time_ns".to_string(),
            "storage_before_idle_journal_count".to_string(),
            "storage_before_idle_journal_bytes".to_string(),
            "storage_before_idle_disk_bytes".to_string(),
            "storage_before_load_write_buffer_bytes".to_string(),
            "storage_before_load_outstanding_flushes".to_string(),
            "storage_before_load_active_compactions".to_string(),
            "storage_before_load_compactions_completed".to_string(),
            "storage_before_load_compaction_time_ns".to_string(),
            "storage_before_load_journal_count".to_string(),
            "storage_before_load_journal_bytes".to_string(),
            "storage_before_load_disk_bytes".to_string(),
            "storage_after_load_write_buffer_bytes".to_string(),
            "storage_after_load_outstanding_flushes".to_string(),
            "storage_after_load_active_compactions".to_string(),
            "storage_after_load_compactions_completed".to_string(),
            "storage_after_load_compaction_time_ns".to_string(),
            "storage_after_load_journal_count".to_string(),
            "storage_after_load_journal_bytes".to_string(),
            "storage_after_load_disk_bytes".to_string(),
            "idle_maintenance_active".to_string(),
            "maintenance_settle_time_ns".to_string(),
            "maintenance_settled".to_string(),
        ];
        writer.write_record(header)?;
        for r in results {
            let commit_throughput = r.commits as f64 / r.wall_time.as_secs_f64();
            let read_throughput = r.reads as f64 / r.wall_time.as_secs_f64();
            let write_throughput = r.writes as f64 / r.wall_time.as_secs_f64();
            let conflict_pct = r.conflicts as f64 / (r.commits + r.conflicts).max(1) as f64 * 100.0;
            let retries_per_commit = r.conflicts as f64 / r.commits.max(1) as f64;
            let (p50, p95, p99, max) = calculate_percentiles(r.commit_latencies);
            let stats_fields = |stats: Option<StorageMaintenanceStats>| {
                stats.map_or_else(
                    || vec![String::new(); 8],
                    |stats| {
                        vec![
                            stats.write_buffer_bytes.to_string(),
                            stats.outstanding_flushes.to_string(),
                            stats.active_compactions.to_string(),
                            stats.compactions_completed.to_string(),
                            stats.compaction_time.as_nanos().to_string(),
                            stats.journal_count.to_string(),
                            stats.journal_bytes.to_string(),
                            stats.disk_bytes.to_string(),
                        ]
                    },
                )
            };
            let idle_maintenance =
                maintenance_during_idle(r.storage_before_idle, r.storage_before_load);
            let row = vec![
                r.concurrency.to_string(),
                r.commits.to_string(),
                r.reads.to_string(),
                r.writes.to_string(),
                r.wall_time.as_nanos().to_string(),
                r.conflicts.to_string(),
                format!("{:.0}", commit_throughput),
                format!("{:.0}", read_throughput),
                format!("{:.0}", write_throughput),
                conflict_pct.to_string(),
                retries_per_commit.to_string(),
                p50.as_nanos().to_string(),
                p95.as_nanos().to_string(),
                p99.as_nanos().to_string(),
                max.as_nanos().to_string(),
                r.package_energy_joules
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                r.package_power_watts
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                r.package_uj_per_commit
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                r.idle_package_power_watts
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                r.dynamic_package_power_watts
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                r.dynamic_package_uj_per_commit
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                r.batch_writer_backpressure.to_string(),
                r.batch_writer_block_time.as_nanos().to_string(),
            ]
            .into_iter()
            .chain(stats_fields(r.storage_before_idle))
            .chain(stats_fields(r.storage_before_load))
            .chain(stats_fields(r.storage_after_load))
            .chain([idle_maintenance.to_string()])
            .chain([
                r.maintenance_settle_time.as_nanos().to_string(),
                r.maintenance_settled.to_string(),
            ])
            .collect::<Vec<_>>();
            writer.write_record(row)?;
        }
        info!("Wrote {} records to {}", num_records, output_file.display());
    }

    Ok(())
}
