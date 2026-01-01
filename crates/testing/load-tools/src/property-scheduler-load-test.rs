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

//! Property scheduler load test - measures property read/write performance through the full
//! scheduler/VM/worldstate/DB stack, as driven by MOO code.
//! Complements property-update-load-test which measures direct DB access.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use clap::Parser;
use clap_derive::Parser;
use futures::{StreamExt, stream::FuturesUnordered};
use moor_common::{
    model::{
        CommitResult, ObjAttrs, ObjFlag, ObjectKind, ObjectRef, PropFlag, VerbArgsSpec, VerbFlag,
    },
    util::BitEnum,
};
use moor_compiler::compile;
use moor_db::{Database, TxDB};
use moor_kernel::{
    SchedulerClient,
    config::{Config, FeaturesConfig},
    tasks::{NoopTasksDb, TaskNotification, scheduler::Scheduler},
};
use moor_model_checker::bench_common::{calculate_percentiles, clear_screen, setup_db_path};
use moor_model_checker::{DirectSession, DirectSessionFactory, NoopSystemControl};
use moor_var::{List, NOTHING, Obj, Symbol, program::ProgramType, v_int, v_obj};
use tabled::{Table, Tabled};
use tracing::info;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long, help = "Database path", default_value = "prop_sched_test_db")]
    db_path: PathBuf,

    #[arg(long, help = "Min number of concurrent players", default_value = "1")]
    min_concurrency: usize,

    #[arg(long, help = "Max number of concurrent players", default_value = "32")]
    max_concurrency: usize,

    #[arg(
        long,
        help = "Number of properties per test object",
        default_value = "10"
    )]
    num_properties: usize,

    #[arg(
        long,
        help = "Number of property operations per verb invocation (inner loop)",
        default_value = "1000"
    )]
    num_prop_iterations: usize,

    #[arg(
        long,
        help = "Number of times to invoke the property test verb per player",
        default_value = "100"
    )]
    num_invocations: usize,

    #[arg(
        long,
        help = "Read/write ratio (0.0 = all writes, 1.0 = all reads)",
        default_value = "0.5"
    )]
    read_ratio: f64,

    #[arg(long, help = "Maximum ticks per task", default_value = "1000000000")]
    max_ticks: i64,

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
    #[tabled(rename = "Tasks")]
    tasks: usize,
    #[tabled(rename = "Prop Ops")]
    prop_ops: String,
    #[tabled(rename = "Wall Time")]
    wall_time: String,
    #[tabled(rename = "Prop/s")]
    prop_throughput: String,
    #[tabled(rename = "Per-Thread")]
    per_thread_throughput: String,
    #[tabled(rename = "p50")]
    p50: String,
    #[tabled(rename = "p95")]
    p95: String,
    #[tabled(rename = "p99")]
    p99: String,
    #[tabled(rename = "max")]
    max: String,
}

/// Generates MOO code for a property write test verb.
/// Writes to properties in a loop.
fn generate_write_verb(num_properties: usize, num_iterations: usize) -> String {
    let mut code = String::new();
    code.push_str("for i in [1..");
    code.push_str(&num_iterations.to_string());
    code.push_str("]\n");

    // Write to each property
    for p in 0..num_properties {
        code.push_str(&format!("    this.prop_{} = i;\n", p));
    }

    code.push_str("endfor\n");
    code.push_str("return 1;\n");
    code
}

/// Generates MOO code for a property read test verb.
/// Reads properties in a loop and accumulates them.
fn generate_read_verb(num_properties: usize, num_iterations: usize) -> String {
    let mut code = String::new();
    code.push_str("x = 0;\n");
    code.push_str("for i in [1..");
    code.push_str(&num_iterations.to_string());
    code.push_str("]\n");

    // Read each property
    for p in 0..num_properties {
        code.push_str(&format!("    x = x + this.prop_{};\n", p));
    }

    code.push_str("endfor\n");
    code.push_str("return x;\n");
    code
}

/// Generates MOO code for a mixed read/write test verb.
/// Alternates between reads and writes based on ratio.
fn generate_mixed_verb(num_properties: usize, num_iterations: usize, read_ratio: f64) -> String {
    // For simplicity, we'll generate code that does both reads and writes each iteration
    // The ratio determines how many of each
    let read_props = (num_properties as f64 * read_ratio).round() as usize;
    let write_props = num_properties - read_props;

    let mut code = String::new();
    code.push_str("x = 0;\n");
    code.push_str("for i in [1..");
    code.push_str(&num_iterations.to_string());
    code.push_str("]\n");

    // Write to first N properties
    for p in 0..write_props {
        code.push_str(&format!("    this.prop_{} = i;\n", p));
    }

    // Read from remaining properties
    for p in write_props..num_properties {
        code.push_str(&format!("    x = x + this.prop_{};\n", p));
    }

    code.push_str("endfor\n");
    code.push_str("return x;\n");
    code
}

fn setup_database(
    database: &TxDB,
    num_properties: usize,
    num_iterations: usize,
    read_ratio: f64,
    max_ticks: i64,
) -> Result<Obj, eyre::Error> {
    let mut loader = database.loader_client()?;

    // Create a wizard player object
    let player_attrs = ObjAttrs::new(
        NOTHING,
        NOTHING,
        NOTHING,
        BitEnum::new_with(ObjFlag::User) | ObjFlag::Wizard,
        "Wizard",
    );

    let player = loader.create_object(ObjectKind::Objid(Obj::mk_id(1)), &player_attrs)?;
    loader.set_object_owner(&player, &player)?;

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

    // Create server options with high tick limits
    let server_options_attrs = ObjAttrs::new(
        player,
        NOTHING,
        NOTHING,
        ObjFlag::User.into(),
        "server_options",
    );
    let server_options_obj = loader.create_object(ObjectKind::NextObjid, &server_options_attrs)?;

    loader.define_property(
        &player,
        &server_options_obj,
        Symbol::mk("fg_ticks"),
        &player,
        PropFlag::Read.into(),
        Some(v_int(max_ticks)),
    )?;

    loader.define_property(
        &player,
        &server_options_obj,
        Symbol::mk("bg_ticks"),
        &player,
        PropFlag::Read.into(),
        Some(v_int(max_ticks)),
    )?;

    loader.define_property(
        &system_obj,
        &system_obj,
        Symbol::mk("server_options"),
        &system_obj,
        PropFlag::Read.into(),
        Some(v_obj(server_options_obj)),
    )?;

    // Define properties on the player object for testing
    for p in 0..num_properties {
        loader.define_property(
            &player,
            &player,
            Symbol::mk(&format!("prop_{}", p)),
            &player,
            BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
            Some(v_int(0)),
        )?;
    }

    // Compile and add the test verb
    let features_config = FeaturesConfig::default();
    let compile_options = features_config.compile_options();

    let verb_code = if read_ratio >= 1.0 {
        generate_read_verb(num_properties, num_iterations)
    } else if read_ratio <= 0.0 {
        generate_write_verb(num_properties, num_iterations)
    } else {
        generate_mixed_verb(num_properties, num_iterations, read_ratio)
    };

    info!("Generated verb code:\n{}", verb_code);

    let program = compile(&verb_code, compile_options)?;

    loader.add_verb(
        &player,
        &[Symbol::mk("prop_test")],
        &player,
        VerbFlag::rx(),
        VerbArgsSpec::this_none_this(),
        ProgramType::MooR(program),
    )?;

    match loader.commit()? {
        CommitResult::Success { .. } => {
            info!(
                "Initialized property scheduler test database: {} properties, {} iterations, {:.0}% reads",
                num_properties,
                num_iterations,
                read_ratio * 100.0
            );
            Ok(player)
        }
        CommitResult::ConflictRetry => Err(eyre::eyre!("Database conflict during initialization")),
    }
}

async fn workload(
    args: &Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
) -> Result<(Duration, Vec<Duration>), eyre::Error> {
    let session = Arc::new(DirectSession::new(player));
    let start_time = Instant::now();
    let mut task_latencies = Vec::new();

    for _ in 0..args.num_invocations {
        let task_start = Instant::now();
        let task_handle = scheduler_client.submit_verb_task(
            &player,
            &ObjectRef::Id(player),
            Symbol::mk("prop_test"),
            List::mk_list(&[]),
            "".to_string(),
            &player,
            session.clone(),
        )?;

        let receiver = task_handle.into_receiver();
        let result = loop {
            match receiver.recv_async().await {
                Ok((_task_id, Ok(TaskNotification::Suspended))) => continue,
                other => break other,
            }
        };

        match result {
            Ok((_, Ok(TaskNotification::Result(_)))) => {
                task_latencies.push(task_start.elapsed());
            }
            Ok((_, Err(e))) => {
                return Err(eyre::eyre!("Task failed: {:?}", e));
            }
            Err(e) => {
                return Err(eyre::eyre!("Failed to receive task result: {:?}", e));
            }
            _ => {
                return Err(eyre::eyre!("Unexpected task result type"));
            }
        }
    }

    Ok((start_time.elapsed(), task_latencies))
}

async fn continuous_workload(
    _args: &Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
    stop_time: Instant,
) -> Result<(Duration, usize, Vec<Duration>), eyre::Error> {
    let session = Arc::new(DirectSession::new(player));
    let start_time = Instant::now();
    let mut task_latencies = Vec::new();
    let mut request_count = 0;

    while Instant::now() < stop_time {
        let task_start = Instant::now();
        let task_handle = scheduler_client.submit_verb_task(
            &player,
            &ObjectRef::Id(player),
            Symbol::mk("prop_test"),
            List::mk_list(&[]),
            "".to_string(),
            &player,
            session.clone(),
        )?;
        request_count += 1;

        let receiver = task_handle.into_receiver();
        let result = loop {
            match receiver.recv_async().await {
                Ok((_task_id, Ok(TaskNotification::Suspended))) => continue,
                other => break other,
            }
        };

        match result {
            Ok((_, Ok(TaskNotification::Result(_)))) => {
                task_latencies.push(task_start.elapsed());
            }
            Ok((_, Err(e))) => {
                return Err(eyre::eyre!("Task failed: {:?}", e));
            }
            Err(e) => {
                return Err(eyre::eyre!("Failed to receive task result: {:?}", e));
            }
            _ => {
                return Err(eyre::eyre!("Unexpected task result type"));
            }
        }
    }

    Ok((start_time.elapsed(), request_count, task_latencies))
}

struct Results {
    concurrency: usize,
    total_invocations: usize,
    total_prop_ops: usize,
    wall_time: Duration,
    throughput: f64,
}

async fn run_benchmark(
    args: &Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
) -> Result<Vec<Results>, eyre::Error> {
    let mut results = vec![];
    let mut table_rows = vec![];

    // Calculate property ops per invocation
    let prop_ops_per_invocation = args.num_prop_iterations * args.num_properties;

    // Warm-up
    info!("Running warm-up...");
    let warmup_start = Instant::now();
    for _ in 0..5 {
        workload(args, scheduler_client, player).await?;
    }
    info!("Warm-up completed in {:?}", warmup_start.elapsed());

    // Cool down
    info!("Cooling down for 2 seconds...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut concurrency = args.min_concurrency as f32;

    loop {
        if concurrency > args.max_concurrency as f32 {
            break;
        }
        let num_concurrent = concurrency as usize;
        let start_time = Instant::now();

        let mut workload_futures = FuturesUnordered::new();
        for _ in 0..num_concurrent {
            let args = args.clone();
            let scheduler_client = scheduler_client.clone();
            workload_futures.push(async move { workload(&args, &scheduler_client, player).await });
        }

        // Spinner
        const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let mut spinner_idx = 0;

        eprint!("  {} Running {} workloads... ", SPINNER[0], num_concurrent);
        std::io::Write::flush(&mut std::io::stderr()).ok();

        let mut all_latencies = vec![];
        let mut completed = 0;
        while let Some(result) = workload_futures.next().await {
            let (_, latencies) = result?;
            all_latencies.extend(latencies);
            completed += 1;
            spinner_idx = (spinner_idx + 1) % SPINNER.len();
            eprint!(
                "\r  {} Running {}/{} workloads... ",
                SPINNER[spinner_idx], completed, num_concurrent
            );
            std::io::Write::flush(&mut std::io::stderr()).ok();
        }

        eprintln!(
            "\r  ✓ Completed {}/{} workloads in {:?}",
            num_concurrent,
            num_concurrent,
            start_time.elapsed()
        );

        let wall_time = start_time.elapsed();
        let total_invocations = args.num_invocations * num_concurrent;
        let total_prop_ops = total_invocations * prop_ops_per_invocation;
        let throughput = total_prop_ops as f64 / wall_time.as_secs_f64();
        let per_thread_throughput = throughput / num_concurrent as f64;

        let (p50, p95, p99, max) = calculate_percentiles(all_latencies);

        table_rows.push(BenchmarkRow {
            concurrency: num_concurrent,
            tasks: total_invocations,
            prop_ops: format!("{}", total_prop_ops),
            wall_time: format!("{:.2?}", wall_time),
            prop_throughput: format!("{:.2}M/s", throughput / 1_000_000.0),
            per_thread_throughput: format!("{:.2}M/s", per_thread_throughput / 1_000_000.0),
            p50: format!("{:.2?}", p50),
            p95: format!("{:.2?}", p95),
            p99: format!("{:.2?}", p99),
            max: format!("{:.2?}", max),
        });

        results.push(Results {
            concurrency: num_concurrent,
            total_invocations,
            total_prop_ops,
            wall_time,
            throughput,
        });

        // Redraw table
        clear_screen();
        eprintln!(
            "Property Scheduler Load Test\nProperties: {}, Iterations: {}, Read ratio: {:.0}%\n",
            args.num_properties,
            args.num_prop_iterations,
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

async fn run_swamp_mode(
    args: &Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
) -> Result<Vec<Results>, eyre::Error> {
    let duration = Duration::from_secs(args.swamp_duration_seconds);
    let stop_time = Instant::now() + duration;
    let concurrency = args.max_concurrency;
    let prop_ops_per_invocation = args.num_prop_iterations * args.num_properties;

    info!(
        "Starting swamp mode: {} concurrent workers for {} seconds",
        concurrency, args.swamp_duration_seconds
    );

    let start_time = Instant::now();

    let mut all_tasks = FuturesUnordered::new();
    for _ in 0..concurrency {
        let args = args.clone();
        let scheduler_client = scheduler_client.clone();
        all_tasks.push(async move {
            continuous_workload(&args, &scheduler_client, player, stop_time).await
        });
    }

    let mut total_requests = 0;
    let mut all_latencies = vec![];
    while let Some(result) = all_tasks.next().await {
        let (_, requests, latencies) = result?;
        total_requests += requests;
        all_latencies.extend(latencies);
    }

    let wall_time = start_time.elapsed();
    let total_prop_ops = total_requests * prop_ops_per_invocation;
    let throughput = total_prop_ops as f64 / wall_time.as_secs_f64();

    let (p50, p95, p99, max) = calculate_percentiles(all_latencies);

    clear_screen();
    eprintln!(
        "Property Scheduler Load Test - Swamp Mode\nProperties: {}, Iterations: {}, Read ratio: {:.0}%\n",
        args.num_properties,
        args.num_prop_iterations,
        args.read_ratio * 100.0
    );
    eprintln!(
        "Concurrency: {}, Duration: {:?}\n\
         Tasks: {}, Prop Ops: {}\n\
         Throughput: {:.2}M prop ops/s\n\
         Task latency: p50={:?}, p95={:?}, p99={:?}, max={:?}",
        concurrency,
        wall_time,
        total_requests,
        total_prop_ops,
        throughput / 1_000_000.0,
        p50,
        p95,
        p99,
        max
    );

    Ok(vec![Results {
        concurrency,
        total_invocations: total_requests,
        total_prop_ops,
        wall_time,
        throughput,
    }])
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install().expect("Unable to install color_eyre");
    let args: Args = Args::parse();

    moor_common::tracing::init_tracing(args.debug).unwrap_or_else(|e| {
        eprintln!("Unable to configure logging: {e}");
        std::process::exit(1);
    });

    info!("Starting property scheduler load test");

    let (db_path, _temp_dir) = setup_db_path(&args.db_path, "prop_sched_test_db")?;

    let (database, _) = TxDB::open(Some(&db_path), Default::default());
    let player = setup_database(
        &database,
        args.num_properties,
        args.num_prop_iterations,
        args.read_ratio,
        args.max_ticks,
    )?;

    let database = Box::new(database);

    let config = Config {
        features: Arc::new(FeaturesConfig::default()),
        ..Default::default()
    };
    let config = Arc::new(config);

    let system_control = Arc::new(NoopSystemControl {});
    let tasks_db = Box::new(NoopTasksDb {});
    let version = semver::Version::parse("1.0.0-beta5").unwrap();

    let scheduler = Scheduler::new(
        version,
        database,
        tasks_db,
        config,
        system_control,
        None,
        None,
    );

    let scheduler_client = scheduler.client()?;

    let session_factory = Arc::new(DirectSessionFactory {});
    let _scheduler_handle = std::thread::spawn(move || {
        scheduler.run(session_factory);
    });

    let results = if args.swamp_mode {
        run_swamp_mode(&args, &scheduler_client, player).await?
    } else {
        run_benchmark(&args, &scheduler_client, player).await?
    };

    if let Some(output_file) = args.output_file {
        let num_records = results.len();
        let mut writer =
            csv::Writer::from_path(&output_file).expect("Could not open benchmark output file");

        let header = vec![
            "concurrency".to_string(),
            "total_invocations".to_string(),
            "total_prop_ops".to_string(),
            "wall_time_ns".to_string(),
            "throughput_per_sec".to_string(),
        ];
        writer.write_record(header)?;
        for r in results {
            let row = vec![
                r.concurrency.to_string(),
                r.total_invocations.to_string(),
                r.total_prop_ops.to_string(),
                r.wall_time.as_nanos().to_string(),
                format!("{:.0}", r.throughput),
            ];
            writer.write_record(row)?;
        }
        info!("Wrote {} records to {}", num_records, output_file.display());
    }

    scheduler_client.submit_shutdown("Load test completed")?;

    Ok(())
}
