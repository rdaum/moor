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

//! Direct scheduler load test - analogous to verb-dispatch-load-test but directly against scheduler
//! with a temporary database, so we can remove RPC dispatching from the load and focus just on
//! scheduler->vm<->db
//! Note: you should run this in release mode to get decent/comparable results

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use clap::Parser;
use clap_derive::Parser;
use futures::{StreamExt, stream::FuturesUnordered};
use moor_common::{
    model::{
        CommitResult, ObjAttrs, ObjFlag, ObjectKind, ObjectRef, PropFlag, VerbArgsSpec, VerbFlag,
    },
    util::BitEnum,
};
use tabled::{Table, Tabled};

use moor_compiler::compile;
use moor_db::{Database, TxDB, db_worldstate::db_counters};
use moor_kernel::{
    SchedulerClient,
    config::{Config, FeaturesConfig},
    tasks::{NoopTasksDb, TaskNotification, sched_counters, scheduler::Scheduler},
};
use moor_model_checker::bench_common::calculate_percentiles;
use moor_model_checker::{DirectSession, DirectSessionFactory, NoopSystemControl};
use moor_var::{List, NOTHING, Obj, Symbol, program::ProgramType, v_int, v_list, v_obj};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::info;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long, help = "Database path", default_value = "test_db")]
    db_path: PathBuf,

    #[arg(long, help = "Min number of concurrent players", default_value = "1")]
    min_concurrency: usize,

    #[arg(long, help = "Max number of concurrent players", default_value = "32")]
    max_concurrency: usize,

    #[arg(long, help = "Number of test objects to create", default_value = "1")]
    num_objects: usize,

    #[arg(
        long,
        help = "Number of verb iterations per invocation (inner loop)",
        default_value = "7000"
    )]
    num_verb_iterations: usize,

    #[arg(
        long,
        help = "Number of times to invoke the top-level workload verb per player",
        default_value = "200"
    )]
    num_invocations: usize,

    #[arg(
        long,
        help = "Maximum ticks per task (higher allows more iterations)",
        default_value = "1000000000"
    )]
    max_ticks: i64,

    #[arg(long, help = "CSV output file for benchmark data")]
    output_file: Option<PathBuf>,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,

    #[arg(
        long,
        help = "Swamp mode: immediately run at maximum concurrency with all requests in parallel to stress test the server",
        default_value = "false"
    )]
    swamp_mode: bool,

    #[arg(
        long,
        help = "Duration in seconds to run swamp mode (continuously sending requests)",
        default_value = "30"
    )]
    swamp_duration_seconds: u64,
}

const LOAD_TEST_INVOKE_VERB: &str = r#"
let num_verb_invocations = args[1];
let objects = player.test_objects;
for i in [1..num_verb_invocations]
    for object in (objects)
        if (object:load_test() != 1)
            raise(E_INVARG, "Load test failed");
        endif
    endfor
endfor
return 1;
"#;

const LOAD_TEST_VERB: &str = r#"
return 1;
"#;

#[derive(Tabled)]
struct BenchmarkRow {
    #[tabled(rename = "Conc")]
    concurrency: usize,
    #[tabled(rename = "Tasks")]
    tasks: usize,
    #[tabled(rename = "Verb Calls")]
    verb_calls: usize,
    #[tabled(rename = "Per-Verb")]
    per_verb_call: String,
    #[tabled(rename = "Amort/Verb")]
    amort_per_verb: String,
    #[tabled(rename = "Wall Time")]
    wall_time: String,
    #[tabled(rename = "Total Thru")]
    total_throughput: String,
    #[tabled(rename = "Per-Thread Thru")]
    per_thread_throughput: String,
    #[tabled(rename = "submit p50")]
    submit_p50: String,
    #[tabled(rename = "submit p95")]
    submit_p95: String,
    #[tabled(rename = "submit p99")]
    submit_p99: String,
    #[tabled(rename = "submit max")]
    submit_max: String,
    #[tabled(rename = "setup avg")]
    setup_avg: String,
    #[tabled(rename = "find avg")]
    find_avg: String,
    #[tabled(rename = "sched_msg avg")]
    sched_msg_avg: String,
    #[tabled(rename = "wakeup avg")]
    wakeup_avg: String,
}

fn setup_test_database(
    database: &TxDB,
    num_objects: usize,
    max_ticks: i64,
) -> Result<Obj, eyre::Error> {
    let mut loader = database.loader_client()?;

    // Create a wizard player object
    let player_attrs = ObjAttrs::new(
        NOTHING, // owner (will be set to own itself after creation)
        NOTHING, // parent
        NOTHING, // location
        BitEnum::new_with(ObjFlag::User) | ObjFlag::Wizard, // flags - make it a wizard
        "Wizard", // name
    );

    let player = loader.create_object(ObjectKind::Objid(Obj::mk_id(1)), &player_attrs)?;
    info!("Created wizard player object: {}", player);

    // Set the player to own itself
    loader.set_object_owner(&player, &player)?;

    // Define test_objects property on the player object
    loader.define_property(
        &player, // definer
        &player, // objid (object to define property on)
        Symbol::mk("test_objects"),
        &player,               // owner
        PropFlag::Read.into(), // flags
        Some(v_list(&[])),     // initial value
    )?;

    // Create test objects
    let mut test_objects = vec![];
    for i in 0..num_objects {
        let obj_attrs = ObjAttrs::new(
            player,                          // owner
            player,                          // parent
            NOTHING,                         // location
            ObjFlag::User.into(),            // flags
            &format!("TestObject{}", i + 1), // name
        );

        let new_obj = loader.create_object(ObjectKind::NextObjid, &obj_attrs)?;
        test_objects.push(v_obj(new_obj));
        info!("Created test object: {}", new_obj);
    }

    // Set the test_objects property
    loader.set_property(
        &player,
        Symbol::mk("test_objects"),
        Some(player),
        None,
        Some(v_list(&test_objects)),
    )?;

    // Create system object #0 first (SYSTEM_OBJECT)
    let system_attrs = ObjAttrs::new(
        NOTHING,              // owner (will be set to own itself)
        NOTHING,              // parent
        NOTHING,              // location
        ObjFlag::User.into(), // flags
        "System Object",      // name
    );
    let system_obj = loader.create_object(ObjectKind::Objid(Obj::mk_id(0)), &system_attrs)?;
    loader.set_object_owner(&system_obj, &system_obj)?;

    // Create server options object with higher tick limits for load testing
    let server_options_attrs = ObjAttrs::new(
        player,               // owner
        NOTHING,              // parent
        NOTHING,              // location
        ObjFlag::User.into(), // flags
        "server_options",     // name
    );
    let server_options_obj = loader.create_object(ObjectKind::NextObjid, &server_options_attrs)?;

    // Set tick limits from CLI arg
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

    // Set the server_options property on the system object to point to our server options object
    loader.define_property(
        &system_obj,
        &system_obj,
        Symbol::mk("server_options"),
        &system_obj,
        PropFlag::Read.into(),
        Some(v_obj(server_options_obj)),
    )?;

    // Compile and add the test verbs
    let features_config = FeaturesConfig::default();
    let compile_options = features_config.compile_options();

    // Add and program the invoke_load_test verb
    let invoke_program = compile(LOAD_TEST_INVOKE_VERB, compile_options.clone())?;
    loader.add_verb(
        &player,                           // obj
        &[Symbol::mk("invoke_load_test")], // names
        &player,                           // owner
        VerbFlag::rx(),                    // flags
        VerbArgsSpec::this_none_this(),    // args
        ProgramType::MooR(invoke_program), // program
    )?;

    // Add and program the load_test verb on each test object
    let load_program = compile(LOAD_TEST_VERB, compile_options)?;
    for obj_var in &test_objects {
        let obj = obj_var.as_object().unwrap();
        loader.add_verb(
            &obj,                                    // obj
            &[Symbol::mk("load_test")],              // names
            &player,                                 // owner
            VerbFlag::rx(),                          // flags
            VerbArgsSpec::this_none_this(),          // args
            ProgramType::MooR(load_program.clone()), // program
        )?;
    }

    // Commit all changes
    match loader.commit()? {
        CommitResult::Success { .. } => {
            info!(
                "Successfully initialized test database with {} objects and programmed verbs",
                num_objects
            );
            Ok(player)
        }
        CommitResult::ConflictRetry => Err(eyre::eyre!("Database conflict during initialization")),
    }
}

async fn workload(
    args: Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
) -> Result<(Duration, Vec<Duration>), eyre::Error> {
    let session = Arc::new(DirectSession::new(player));
    let start_time = Instant::now();
    let mut submit_latencies = Vec::new();

    // Submit tasks sequentially - wait for each to complete before submitting next
    for _ in 0..args.num_invocations {
        let submit_start = Instant::now();
        let task_handle = scheduler_client.submit_verb_task(
            &player,
            &ObjectRef::Id(player),
            Symbol::mk("invoke_load_test"),
            List::from_iter(vec![v_int(args.num_verb_iterations as i64)]),
            "".to_string(),
            &player,
            session.clone(),
        )?;
        submit_latencies.push(submit_start.elapsed());

        // Wait for this task to complete before submitting the next
        let receiver = task_handle.into_receiver();
        let result = loop {
            match receiver.recv_async().await {
                Ok((_task_id, Ok(TaskNotification::Suspended))) => {
                    continue;
                }
                other => break other,
            }
        };

        match result {
            Ok((_, Ok(TaskNotification::Result(result)))) => {
                let Some(result_int) = result.as_integer() else {
                    return Err(eyre::eyre!("Unexpected task result: {:?}", result));
                };
                if result_int != 1 {
                    return Err(eyre::eyre!(
                        "Load test failed: expected 1, got {}",
                        result_int
                    ));
                }
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

    Ok((start_time.elapsed(), submit_latencies))
}

async fn continuous_workload(
    args: Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
    stop_time: Instant,
) -> Result<(Duration, usize, Vec<Duration>), eyre::Error> {
    let session = Arc::new(DirectSession::new(player));
    let start_time = Instant::now();
    let mut submit_latencies = Vec::new();
    let mut request_count = 0;

    // Submit tasks sequentially until time limit
    while Instant::now() < stop_time {
        let submit_start = Instant::now();
        let task_handle = scheduler_client.submit_verb_task(
            &player,
            &ObjectRef::Id(player),
            Symbol::mk("invoke_load_test"),
            List::from_iter(vec![v_int(args.num_verb_iterations as i64)]),
            "".to_string(),
            &player,
            session.clone(),
        )?;
        submit_latencies.push(submit_start.elapsed());
        request_count += 1;

        // Wait for this task to complete before submitting the next
        let receiver = task_handle.into_receiver();
        let result = loop {
            match receiver.recv_async().await {
                Ok((_task_id, Ok(TaskNotification::Suspended))) => continue,
                other => break other,
            }
        };

        match result {
            Ok((_, Ok(TaskNotification::Result(_result)))) => {
                // Task completed successfully
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

    Ok((start_time.elapsed(), request_count, submit_latencies))
}

struct Results {
    /// How many concurrent threads there were.
    concurrency: usize,
    /// How many times the top-level verb was invoked
    total_invocations: usize,
    /// How many total verb calls that led to
    total_verb_calls: usize,
    /// The duration of the whole load test (wall clock)
    wall_time: Duration,
    /// The cumulative time actually spent waiting for the scheduler to respond (sum of all workloads)
    cumulative_time: Duration,
    /// The time per verb dispatch (based on wall time)
    per_verb_call: Duration,
    /// Throughput in verb calls per second
    throughput: f64,
}

async fn swamp_mode_workload(
    args: &Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
) -> Result<Vec<Results>, eyre::Error> {
    info!("Initializing swamp mode workload session");
    // Test objects were already set up in setup_test_database

    info!(
        "Starting swamp mode - running {} concurrent threads for {} seconds",
        args.max_concurrency, args.swamp_duration_seconds
    );

    let start_time = Instant::now();
    let duration = Duration::from_secs(args.swamp_duration_seconds);
    let stop_time = start_time + duration;

    // Create continuous workload tasks that run for the specified duration
    let mut all_tasks = FuturesUnordered::new();

    for _i in 0..args.max_concurrency {
        let args = args.clone();
        let scheduler_client = scheduler_client.clone();

        all_tasks.push(async move {
            continuous_workload(args, &scheduler_client, player, stop_time).await
        });
    }

    // Wait for all tasks to complete
    let mut times = vec![];
    let mut total_requests = 0;
    let mut all_submit_latencies = vec![];
    while let Some(result) = all_tasks.next().await {
        let (time, requests, latencies) = result?;
        times.push(time);
        total_requests += requests;
        all_submit_latencies.extend(latencies);
    }

    let cumulative_time = times.iter().fold(Duration::new(0, 0), |acc, x| acc + *x);
    let total_time = start_time.elapsed();
    let total_verb_calls = total_requests * args.num_verb_iterations + total_requests;

    let result = Results {
        concurrency: args.max_concurrency,
        total_invocations: total_requests,
        total_verb_calls,
        wall_time: total_time,
        cumulative_time,
        per_verb_call: Duration::from_secs_f64(total_time.as_secs_f64() / total_verb_calls as f64),
        throughput: total_verb_calls as f64 / total_time.as_secs_f64(),
    };

    // Get scheduler metrics
    let counters = sched_counters();
    let submit_count = counters.submit_verb_task_latency.invocations().sum();
    let submit_avg_micros = if submit_count > 0 {
        let total_nanos = counters
            .submit_verb_task_latency
            .cumulative_duration_nanos()
            .sum();
        (total_nanos / submit_count) / 1000
    } else {
        0
    };

    // Calculate submit overhead relative to total execution
    let total_submit_nanos = counters
        .submit_verb_task_latency
        .cumulative_duration_nanos()
        .sum();
    let cumulative_time_nanos = result.cumulative_time.as_nanos() as u64;

    let submit_overhead_pct = if cumulative_time_nanos > 0 {
        (total_submit_nanos as f64 / cumulative_time_nanos as f64) * 100.0
    } else {
        0.0
    };

    // Amortize submit overhead across all verb calls (including interior calls)
    let amortized_submit_nanos = if result.total_verb_calls > 0 {
        (total_submit_nanos as u64) / (result.total_verb_calls as u64)
    } else {
        0
    };

    // Calculate histogram percentiles from collected latencies
    let (p50, p95, p99, max) = calculate_percentiles(all_submit_latencies);

    info!(
        "Swamp mode completed: {} concurrent threads, {} total requests, Wall Time: {:?}, Cumulative: {:?}, Per Verb: {:?}, Throughput: {:.2}M/s",
        result.concurrency,
        result.total_invocations,
        result.wall_time,
        result.cumulative_time,
        result.per_verb_call,
        result.throughput / 1_000_000.0
    );
    info!(
        "  submit_verb_task: avg {}µs ({:.1}% of cumulative time, {}ns amortized per dispatch)",
        submit_avg_micros, submit_overhead_pct, amortized_submit_nanos
    );
    info!(
        "  submit_verb_task latency distribution: p50={:?}, p95={:?}, p99={:?}, max={:?}",
        p50, p95, p99, max
    );

    // Report additional per-operation metrics
    let setup_task_count = counters.setup_task.invocations().sum();
    let setup_task_avg_nanos = if setup_task_count > 0 {
        counters.setup_task.cumulative_duration_nanos().sum() / setup_task_count
    } else {
        0
    };

    let find_verb_count = db_counters().find_method_verb_on.invocations().sum();
    let find_verb_avg_nanos = if find_verb_count > 0 {
        db_counters()
            .find_method_verb_on
            .cumulative_duration_nanos()
            .sum()
            / find_verb_count
    } else {
        0
    };

    info!(
        "  setup_task: {} calls, avg {}ns | find_method_verb_on: {} calls, avg {}ns",
        setup_task_count, setup_task_avg_nanos, find_verb_count, find_verb_avg_nanos
    );

    Ok(vec![result])
}

async fn load_test_workload(
    args: &Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
) -> Result<Vec<Results>, eyre::Error> {
    info!("Initializing load-test workload session (creating properties & verbs)");
    // Test objects were already set up in setup_test_database

    info!("Load-test workload session initialized, starting load test");

    let mut results = vec![];
    let mut table_rows = vec![];

    // Do one throw-away workload run to warm up the system.
    info!("Running warm-up workload run...");
    let warmup_start = Instant::now();
    for _ in 0..5 {
        workload(args.clone(), scheduler_client, player).await?;
    }
    info!(
        "Warm-up workload run completed in {:?}",
        warmup_start.elapsed()
    );

    // Cool down for a couple seconds before starting the actual load test.
    info!("Cooling down for 2 seconds before starting the load test...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut concurrency = args.min_concurrency as f32;
    loop {
        if concurrency > args.max_concurrency as f32 {
            break;
        }
        let num_concurrent_workload = concurrency as usize;
        let start_time = Instant::now();

        // Capture counter baselines before this iteration
        let counters = sched_counters();
        let baseline_setup_task_count = counters.setup_task.invocations().sum();
        let baseline_setup_task_nanos = counters.setup_task.cumulative_duration_nanos().sum();
        let baseline_sched_msg_count = counters.handle_scheduler_msg.invocations().sum();
        let baseline_sched_msg_nanos = counters
            .handle_scheduler_msg
            .cumulative_duration_nanos()
            .sum();
        let baseline_wakeup_count = counters.task_wakeup_latency.invocations().sum();
        let baseline_wakeup_nanos = counters
            .task_wakeup_latency
            .cumulative_duration_nanos()
            .sum();
        let baseline_find_verb_count = db_counters().find_method_verb_on.invocations().sum();
        let baseline_find_verb_nanos = db_counters()
            .find_method_verb_on
            .cumulative_duration_nanos()
            .sum();

        let mut workload_futures = FuturesUnordered::new();
        for _i in 0..num_concurrent_workload {
            let args = args.clone();
            let scheduler_client = scheduler_client.clone();

            workload_futures.push(async move { workload(args, &scheduler_client, player).await });
        }

        // Spinner animation
        const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let mut spinner_idx = 0;
        let total_workloads = num_concurrent_workload;

        eprint!("  {} Running {} workloads... ", SPINNER[0], total_workloads);
        std::io::Write::flush(&mut std::io::stderr()).ok();

        let mut times = vec![];
        let mut all_submit_latencies = vec![];
        while let Some(h) = workload_futures.next().await {
            let (duration, latencies) = h?;
            times.push(duration);
            all_submit_latencies.extend(latencies);

            // Update spinner
            spinner_idx = (spinner_idx + 1) % SPINNER.len();
            let completed = times.len();
            eprint!(
                "\r  {} Running {}/{} workloads... ",
                SPINNER[spinner_idx], completed, total_workloads
            );
            std::io::Write::flush(&mut std::io::stderr()).ok();
        }

        eprintln!(
            "\r  ✓ Completed {}/{} workloads in {:?}",
            total_workloads,
            total_workloads,
            start_time.elapsed()
        );

        let cumulative_time = times.iter().fold(Duration::new(0, 0), |acc, x| acc + *x);
        let total_time = start_time.elapsed();
        let total_invocations = args.num_invocations * num_concurrent_workload;
        let total_verb_calls =
            (args.num_invocations * args.num_verb_iterations * num_concurrent_workload)
                + total_invocations;
        let r = Results {
            concurrency: num_concurrent_workload,
            total_invocations,
            total_verb_calls,
            wall_time: total_time,
            cumulative_time,
            per_verb_call: Duration::from_secs_f64(
                total_time.as_secs_f64() / total_verb_calls as f64,
            ),
            throughput: total_verb_calls as f64 / total_time.as_secs_f64(),
        };

        // Get scheduler metrics for this iteration (compute deltas from baseline)
        let counters = sched_counters();

        // Calculate histogram percentiles from collected latencies
        let (p50, p95, p99, max) = calculate_percentiles(all_submit_latencies);

        // Report additional per-operation metrics (using deltas)
        let setup_task_count = counters.setup_task.invocations().sum() - baseline_setup_task_count;
        let setup_task_total_nanos =
            counters.setup_task.cumulative_duration_nanos().sum() - baseline_setup_task_nanos;
        let setup_task_avg_nanos = if setup_task_count > 0 {
            (setup_task_total_nanos / setup_task_count) as u64
        } else {
            0
        };

        let find_verb_count =
            db_counters().find_method_verb_on.invocations().sum() - baseline_find_verb_count;
        let find_verb_total_nanos = db_counters()
            .find_method_verb_on
            .cumulative_duration_nanos()
            .sum()
            - baseline_find_verb_nanos;
        let find_verb_avg_nanos = if find_verb_count > 0 {
            (find_verb_total_nanos / find_verb_count) as u64
        } else {
            0
        };

        let sched_msg_count =
            counters.handle_scheduler_msg.invocations().sum() - baseline_sched_msg_count;
        let sched_msg_total_nanos = counters
            .handle_scheduler_msg
            .cumulative_duration_nanos()
            .sum()
            - baseline_sched_msg_nanos;
        let sched_msg_avg_nanos = if sched_msg_count > 0 {
            (sched_msg_total_nanos / sched_msg_count) as u64
        } else {
            0
        };

        let wakeup_count = counters.task_wakeup_latency.invocations().sum() - baseline_wakeup_count;
        let wakeup_total_nanos = counters
            .task_wakeup_latency
            .cumulative_duration_nanos()
            .sum()
            - baseline_wakeup_nanos;
        let wakeup_avg_nanos = if wakeup_count > 0 {
            (wakeup_total_nanos / wakeup_count) as u64
        } else {
            0
        };

        let per_thread_throughput = r.throughput / r.concurrency as f64;
        // Per-Verb = cumulative CPU time / verb calls (actual processing time)
        let cpu_per_verb =
            Duration::from_secs_f64(r.cumulative_time.as_secs_f64() / r.total_verb_calls as f64);
        table_rows.push(BenchmarkRow {
            concurrency: r.concurrency,
            tasks: r.total_invocations,
            verb_calls: r.total_verb_calls,
            per_verb_call: format!("{:.0?}", cpu_per_verb),
            amort_per_verb: format!("{:.0?}", r.per_verb_call),
            wall_time: format!("{:.2?}", r.wall_time),
            total_throughput: format!("{:.2}M/s", r.throughput / 1_000_000.0),
            per_thread_throughput: format!("{:.2}M/s", per_thread_throughput / 1_000_000.0),
            submit_p50: format!("{:.2?}", p50),
            submit_p95: format!("{:.2?}", p95),
            submit_p99: format!("{:.2?}", p99),
            submit_max: format!("{:.2?}", max),
            setup_avg: format!("{}ns", setup_task_avg_nanos),
            find_avg: format!("{}ns", find_verb_avg_nanos),
            sched_msg_avg: format!("{}ns", sched_msg_avg_nanos),
            wakeup_avg: format!("{}ns", wakeup_avg_nanos),
        });

        // Clear screen and redraw table
        eprint!("\x1B[2J\x1B[1;1H"); // ANSI clear screen and move to top
        eprintln!("{}", Table::new(&table_rows));

        results.push(r);

        // Scale up by 25% or 1, whichever is larger, so we don't get stuck on lower values.
        let mut next_concurrency = concurrency * 1.25;
        if next_concurrency as usize <= concurrency as usize {
            next_concurrency = concurrency + 1.0;
        }
        concurrency = next_concurrency;
    }

    // Final table is already displayed from last iteration
    Ok(results)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install().expect("Unable to install color_eyre");
    let args: Args = Args::parse();

    moor_common::tracing::init_tracing(false).unwrap_or_else(|e| {
        eprintln!("Unable to configure logging: {e}");
        std::process::exit(1);
    });

    info!("Starting direct scheduler load test");

    // Create temporary directory for database if using default path
    let temp_dir = if args.db_path == "test_db" {
        Some(tempfile::tempdir()?)
    } else {
        None
    };

    let db_path = if let Some(ref temp_dir) = temp_dir {
        temp_dir.path().join("test_db")
    } else {
        args.db_path.clone()
    };

    // Create database
    let (database, _) = TxDB::open(Some(&db_path), Default::default());

    // Setup test database and get the player object
    let player = setup_test_database(&database, args.num_objects, args.max_ticks)?;

    let database = Box::new(database);

    // Create config with higher tick limits for load testing
    // Increase tick limits significantly for load testing - each task does 7000 verb calls
    // We need much higher limits than the default 60k/30k
    let config = Config {
        features: Arc::new(FeaturesConfig::default()),
        ..Default::default()
    };
    let config = Arc::new(config);

    // Create scheduler components
    let system_control = Arc::new(NoopSystemControl {});
    let tasks_db = Box::new(NoopTasksDb {});
    let version = semver::Version::parse("1.0.0-beta5").unwrap();

    // Create scheduler
    let scheduler = Scheduler::new(
        version,
        database,
        tasks_db,
        config,
        system_control,
        None, // No workers for this test
        None, // No worker responses
    );

    let scheduler_client = scheduler.client()?;

    // Start scheduler in background thread
    let session_factory = Arc::new(DirectSessionFactory {});
    let _scheduler_handle = std::thread::spawn(move || {
        scheduler.run(session_factory);
    });

    let results = if args.swamp_mode {
        swamp_mode_workload(&args, &scheduler_client, player).await?
    } else {
        load_test_workload(&args, &scheduler_client, player).await?
    };

    if let Some(output_file) = args.output_file {
        let num_records = results.len();
        let mut writer =
            csv::Writer::from_path(&output_file).expect("Could not open benchmark output file");

        let header = vec![
            "concurrency".to_string(),
            "total_invocations".to_string(),
            "total_verb_calls".to_string(),
            "wall_time_ns".to_string(),
            "per_verb_ns".to_string(),
            "throughput_per_sec".to_string(),
        ];
        writer.write_record(header)?;
        for r in results {
            let base = vec![
                r.concurrency.to_string(),
                r.total_invocations.to_string(),
                r.total_verb_calls.to_string(),
                r.wall_time.as_nanos().to_string(),
                r.per_verb_call.as_nanos().to_string(),
                format!("{:.0}", r.throughput),
            ];
            writer.write_record(base)?
        }
        info!("Wrote {num_records} to {}", output_file.display())
    }

    // Shutdown scheduler
    scheduler_client.submit_shutdown("Load test completed")?;

    Ok(())
}
