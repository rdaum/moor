// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
use moor_common::{
    model::{CommitResult, ObjAttrs, ObjFlag, ObjectKind, PropFlag},
    util::BitEnum,
};
use moor_db::{Database, TxDB, db_worldstate::db_counters};
use moor_model_checker::bench_common::{
    calculate_percentiles, clear_screen, format_duration, format_throughput, setup_db_path,
    update_spinner,
};
use moor_var::{NOTHING, Obj, Symbol, v_int};
use rand::{Rng, SeedableRng, rngs::SmallRng};
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
        help = "Read/write ratio (0.0 = all writes, 1.0 = all reads)",
        default_value = "0.5"
    )]
    read_ratio: f64,

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
    #[tabled(rename = "Wall Time")]
    wall_time: String,
    #[tabled(rename = "Commit/s")]
    commit_throughput: String,
    #[tabled(rename = "Per-Thrd")]
    per_thread_throughput: String,
    #[tabled(rename = "Conflict%")]
    conflict_pct: String,
    #[tabled(rename = "Commit Thrd")]
    commit_thread_util: String,
    #[tabled(rename = "Avg Check")]
    avg_check: String,
    #[tabled(rename = "Avg Apply")]
    avg_apply: String,
    #[tabled(rename = "Src Put")]
    avg_source_put: String,
    #[tabled(rename = "Idx Ins")]
    avg_index_insert: String,
    #[tabled(rename = "p50")]
    p50: String,
    #[tabled(rename = "p99")]
    p99: String,
    #[tabled(rename = "max")]
    max: String,
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

fn run_worker(
    database: &TxDB,
    setup: &TestSetup,
    ops_count: usize,
    read_ratio: f64,
    seed: u64,
) -> Result<WorkerResult, eyre::Error> {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut reads = 0;
    let mut writes = 0;
    let mut conflicts = 0;
    let mut commit_latencies = Vec::with_capacity(ops_count);

    for _ in 0..ops_count {
        // Pick random object and property
        let obj_idx = rng.random_range(0..setup.objects.len());
        let prop_idx = rng.random_range(0..setup.property_names.len());
        let obj = setup.objects[obj_idx];
        let prop = setup.property_names[prop_idx];

        let is_read = rng.random::<f64>() < read_ratio;

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
}

fn run_workload(
    database: &TxDB,
    setup: &TestSetup,
    args: &Args,
    concurrency: usize,
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
            let ops = args.ops_per_iteration;
            let read_ratio = args.read_ratio;
            let seed = worker_id as u64;
            let conflicts = Arc::clone(&conflict_count);
            let reads = Arc::clone(&read_count);
            let writes = Arc::clone(&write_count);

            std::thread::spawn(move || {
                let setup = TestSetup {
                    objects: setup_objs,
                    property_names: setup_props,
                };
                let result = run_worker(&db, &setup, ops, read_ratio, seed)?;
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
    for _ in 0..3 {
        run_workload(database, setup, args, 1)?;
    }
    info!("Warm-up completed in {:?}", warmup_start.elapsed());

    // Cool down
    info!("Cooling down for 1 second...");
    std::thread::sleep(Duration::from_secs(1));

    let mut concurrency = args.min_concurrency as f32;
    let mut spinner_idx = 0;

    loop {
        if concurrency > args.max_concurrency as f32 {
            break;
        }
        let num_concurrent = concurrency as usize;

        // Capture baseline counters for commit thread utilization
        let counters = db_counters();
        let baseline_check_nanos = counters
            .commit_check_phase
            .cumulative_duration_nanos()
            .sum();
        let baseline_check_count = counters.commit_check_phase.invocations().sum();
        let baseline_apply_nanos = counters
            .commit_apply_phase
            .cumulative_duration_nanos()
            .sum();
        let baseline_apply_count = counters.commit_apply_phase.invocations().sum();
        let baseline_source_put_nanos = counters.apply_source_put.cumulative_duration_nanos().sum();
        let baseline_source_put_count = counters.apply_source_put.invocations().sum();
        let baseline_index_insert_nanos = counters
            .apply_index_insert
            .cumulative_duration_nanos()
            .sum();
        let baseline_index_insert_count = counters.apply_index_insert.invocations().sum();

        // Run multiple iterations and aggregate
        let mut iteration_results = Vec::new();
        for i in 0..args.num_iterations {
            update_spinner(
                &mut spinner_idx,
                &format!(
                    "Concurrency {}: iteration {}/{}...",
                    num_concurrent,
                    i + 1,
                    args.num_iterations
                ),
            );
            let result = run_workload(database, setup, args, num_concurrent)?;
            iteration_results.push(result);
        }

        // Aggregate results
        let total_commits: usize = iteration_results.iter().map(|r| r.commits).sum();
        let total_reads: usize = iteration_results.iter().map(|r| r.reads).sum();
        let total_writes: usize = iteration_results.iter().map(|r| r.writes).sum();
        let total_conflicts: usize = iteration_results.iter().map(|r| r.conflicts).sum();
        let total_wall_time: Duration = iteration_results.iter().map(|r| r.wall_time).sum();
        let avg_wall_time = total_wall_time / args.num_iterations as u32;

        let all_commit_latencies: Vec<Duration> = iteration_results
            .into_iter()
            .flat_map(|r| r.commit_latencies)
            .collect();
        let (p50, _p95, p99, max) = calculate_percentiles(all_commit_latencies.clone());

        let conflict_pct = if total_commits > 0 {
            (total_conflicts as f64 / (total_commits + total_conflicts) as f64) * 100.0
        } else {
            0.0
        };

        // Calculate throughput rates
        let commit_throughput = total_commits as f64 / total_wall_time.as_secs_f64();
        let per_thread_throughput = commit_throughput / num_concurrent as f64;

        // Calculate commit thread utilization (how much of wall time was spent in commit phases)
        let counters = db_counters();
        let check_nanos = counters
            .commit_check_phase
            .cumulative_duration_nanos()
            .sum()
            - baseline_check_nanos;
        let check_count = counters.commit_check_phase.invocations().sum() - baseline_check_count;
        let apply_nanos = counters
            .commit_apply_phase
            .cumulative_duration_nanos()
            .sum()
            - baseline_apply_nanos;
        let apply_count = counters.commit_apply_phase.invocations().sum() - baseline_apply_count;
        let source_put_nanos =
            counters.apply_source_put.cumulative_duration_nanos().sum() - baseline_source_put_nanos;
        let source_put_count =
            counters.apply_source_put.invocations().sum() - baseline_source_put_count;
        let index_insert_nanos = counters
            .apply_index_insert
            .cumulative_duration_nanos()
            .sum()
            - baseline_index_insert_nanos;
        let index_insert_count =
            counters.apply_index_insert.invocations().sum() - baseline_index_insert_count;

        // Utilization based on check+apply (write is background, doesn't block)
        let total_commit_thread_nanos = check_nanos + apply_nanos;
        let commit_thread_util_pct =
            (total_commit_thread_nanos as f64 / total_wall_time.as_nanos() as f64) * 100.0;

        // Average times per phase (only for commits that went through each phase)
        let avg_check_nanos = if check_count > 0 {
            (check_nanos / check_count) as u64
        } else {
            0
        };
        let avg_apply_nanos = if apply_count > 0 {
            (apply_nanos / apply_count) as u64
        } else {
            0
        };
        let avg_source_put_nanos = if source_put_count > 0 {
            (source_put_nanos / source_put_count) as u64
        } else {
            0
        };
        let avg_index_insert_nanos = if index_insert_count > 0 {
            (index_insert_nanos / index_insert_count) as u64
        } else {
            0
        };

        table_rows.push(BenchmarkRow {
            concurrency: num_concurrent,
            commits: total_commits,
            wall_time: format_duration(avg_wall_time),
            commit_throughput: format_throughput(commit_throughput),
            per_thread_throughput: format_throughput(per_thread_throughput),
            conflict_pct: format!("{:.2}%", conflict_pct),
            commit_thread_util: format!("{:.1}%", commit_thread_util_pct),
            avg_check: format_duration(Duration::from_nanos(avg_check_nanos)),
            avg_apply: format_duration(Duration::from_nanos(avg_apply_nanos)),
            avg_source_put: format_duration(Duration::from_nanos(avg_source_put_nanos)),
            avg_index_insert: format_duration(Duration::from_nanos(avg_index_insert_nanos)),
            p50: format_duration(p50),
            p99: format_duration(p99),
            max: format_duration(max),
        });

        results.push(Results {
            concurrency: num_concurrent,
            commits: total_commits,
            reads: total_reads,
            writes: total_writes,
            wall_time: avg_wall_time,
            conflicts: total_conflicts,
            commit_latencies: all_commit_latencies,
        });

        // Redraw table
        clear_screen();
        eprintln!(
            "Property Update Load Test\nObjects: {}, Properties/obj: {}, Read ratio: {:.0}%\n",
            args.num_objects,
            args.num_properties,
            args.read_ratio * 100.0
        );
        eprintln!("{}", Table::new(&table_rows));

        // Scale up concurrency
        let mut next_concurrency = concurrency * 1.25;
        if next_concurrency as usize <= concurrency as usize {
            next_concurrency = concurrency + 1.0;
        }
        concurrency = next_concurrency;
    }

    Ok(results)
}

fn run_swamp_mode(
    database: &TxDB,
    setup: &TestSetup,
    args: &Args,
) -> Result<Vec<Results>, eyre::Error> {
    let duration = Duration::from_secs(args.swamp_duration_seconds);
    let stop_time = Instant::now() + duration;
    let concurrency = args.max_concurrency;

    info!(
        "Starting swamp mode: {} concurrent workers for {} seconds",
        concurrency, args.swamp_duration_seconds
    );

    let total_ops = Arc::new(AtomicU64::new(0));
    let conflict_count = Arc::new(AtomicU64::new(0));
    let read_count = Arc::new(AtomicU64::new(0));
    let write_count = Arc::new(AtomicU64::new(0));

    let start = Instant::now();

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

    let commit_throughput = commits as f64 / wall_time.as_secs_f64();
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
         Conflicts: {} ({:.2}%)",
        concurrency,
        wall_time,
        commits,
        reads,
        writes,
        format_throughput(commit_throughput),
        format_throughput(per_thread_throughput),
        conflicts,
        conflict_pct
    );

    Ok(vec![Results {
        concurrency,
        commits,
        reads,
        writes,
        wall_time,
        conflicts,
        commit_latencies: vec![], // Not tracked in swamp mode
    }])
}

fn main() -> Result<(), eyre::Error> {
    color_eyre::install().expect("Unable to install color_eyre");
    let args: Args = Args::parse();

    moor_common::tracing::init_tracing(args.debug).unwrap_or_else(|e| {
        eprintln!("Unable to configure logging: {e}");
        std::process::exit(1);
    });

    info!("Starting property update load test");

    // Create temporary directory for database if using default path
    let (db_path, _temp_dir) = setup_db_path(&args.db_path, "prop_test_db")?;

    // Create database
    let (database, _) = TxDB::open(Some(&db_path), Default::default());

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
        ];
        writer.write_record(header)?;
        for r in results {
            let commit_throughput = r.commits as f64 / r.wall_time.as_secs_f64();
            let read_throughput = r.reads as f64 / r.wall_time.as_secs_f64();
            let write_throughput = r.writes as f64 / r.wall_time.as_secs_f64();
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
            ];
            writer.write_record(row)?;
        }
        info!("Wrote {} records to {}", num_records, output_file.display());
    }

    Ok(())
}
