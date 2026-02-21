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

//! Opcode throughput load test - measures raw interpreter loop speed without verb dispatch overhead.
//! Creates a simple arithmetic loop and measures opcode execution throughput.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use clap::Parser;
use clap_derive::Parser;
use moor_common::model::ObjectRef;
use moor_common::{
    model::{CommitResult, ObjAttrs, ObjFlag, ObjectKind, PropFlag, VerbArgsSpec, VerbFlag},
    util::BitEnum,
};
use moor_compiler::compile;
use moor_db::{Database, TxDB};
use moor_kernel::{
    SchedulerClient,
    config::{Config, FeaturesConfig},
    tasks::{NoopTasksDb, TaskNotification, scheduler::Scheduler},
};
use moor_model_checker::bench_common::{clear_screen, setup_db_path};
use moor_model_checker::{DirectSession, DirectSessionFactory, NoopSystemControl};
use moor_var::{
    List, NOTHING, Obj, Symbol, Variant, program::ProgramType, v_empty_str, v_int, v_obj,
};
use tabled::{Table, Tabled};
use tracing::info;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long, help = "Database path", default_value = "opcode_test_db")]
    db_path: PathBuf,

    #[arg(
        long,
        help = "Number of times to invoke the opcode test verb",
        default_value = "200"
    )]
    num_invocations: usize,

    #[arg(
        long,
        help = "Number of loop iterations in the test verb (for i in [1..N])",
        default_value = "100000"
    )]
    loop_iterations: usize,

    #[arg(
        long,
        help = "Maximum ticks per task (higher allows more iterations)",
        default_value = "1000000000"
    )]
    max_ticks: i64,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,
}

#[derive(Tabled)]
struct BenchmarkRow {
    #[tabled(rename = "Invocations")]
    invocations: usize,
    #[tabled(rename = "Loop Iters")]
    loop_iterations: String,
    #[tabled(rename = "Opcodes")]
    opcodes: String,
    #[tabled(rename = "Wall Time")]
    wall_time: String,
    #[tabled(rename = "Per-Opcode")]
    per_opcode: String,
    #[tabled(rename = "Op Throughput")]
    op_throughput: String,
    #[tabled(rename = "Per-Iter")]
    per_iteration: String,
    #[tabled(rename = "Iter Throughput")]
    iter_throughput: String,
}

/// Generates MOO code for an opcode test verb with a tight arithmetic loop.
fn generate_opcode_test_verb(loop_iterations: usize) -> String {
    format!(
        r#"
x = 0;
for i in [1..{}]
    x = x + i;
endfor
return x;
"#,
        loop_iterations
    )
}

/// Returns (player, opcodes_per_iteration) where opcodes_per_iteration is an estimate of
/// opcodes executed per loop iteration (loop body + control flow).
fn setup_database(
    database: &TxDB,
    loop_iterations: usize,
    max_ticks: i64,
) -> Result<(Obj, usize), eyre::Error> {
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

    // Compile the opcode test verb
    let features_config = FeaturesConfig::default();
    let compile_options = features_config.compile_options();

    let opcode_verb_code = generate_opcode_test_verb(loop_iterations);
    let opcode_program = compile(&opcode_verb_code, compile_options)?;

    // Read actual opcode count from compiled program
    let opcodes_per_invocation = opcode_program.0.main_vector.len();

    loader.add_verb(
        &player,
        &[Symbol::mk("opcode_test")],
        &player,
        VerbFlag::rx(),
        VerbArgsSpec::this_none_this(),
        ProgramType::MooR(opcode_program),
    )?;

    match loader.commit()? {
        CommitResult::Success { .. } => {
            info!(
                "Initialized opcode test database: {} loop iterations -> {} opcodes per invocation",
                loop_iterations, opcodes_per_invocation
            );
            Ok((player, opcodes_per_invocation))
        }
        CommitResult::ConflictRetry { .. } => {
            Err(eyre::eyre!("Database conflict during initialization"))
        }
    }
}

async fn run_workload(
    args: &Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
    opcodes_per_invocation: usize,
) -> Result<(), eyre::Error> {
    let session = Arc::new(DirectSession::new(player));

    info!(
        "Starting opcode throughput test: {} invocations, {} loop iterations, {} opcodes per invocation",
        args.num_invocations, args.loop_iterations, opcodes_per_invocation
    );

    // Calculate expected return value: sum(1..N) = N*(N+1)/2
    let n = args.loop_iterations as i64;
    let expected_sum = n * (n + 1) / 2;
    info!(
        "Expected return value for {} iterations: {}",
        n, expected_sum
    );

    // Warm-up run with validation
    info!("Running warm-up...");
    let mut first_value: Option<i64> = None;
    for i in 0..5 {
        let task_handle = scheduler_client.submit_verb_task(
            &player,
            &ObjectRef::Id(player),
            Symbol::mk("opcode_test"),
            List::mk_list(&[]),
            v_empty_str(),
            &player,
            session.clone(),
        )?;

        let receiver = task_handle.into_receiver();
        loop {
            match receiver.recv_async().await {
                Ok((_, Ok(TaskNotification::Result(result)))) => {
                    if let Variant::Int(actual) = result.variant() {
                        if let Some(val) = first_value {
                            if actual != val {
                                return Err(eyre::eyre!(
                                    "Warm-up {}: Inconsistent return value! First was {}, got {}",
                                    i,
                                    val,
                                    actual
                                ));
                            }
                        } else {
                            first_value = Some(actual);
                            info!(
                                "First return value: {} (expected: {})",
                                actual, expected_sum
                            );
                            if actual != expected_sum {
                                return Err(eyre::eyre!(
                                    "Wrong return value! Expected {}, got {}. Task may have exited early or hit tick limit.",
                                    expected_sum,
                                    actual
                                ));
                            }
                        }
                    } else {
                        return Err(eyre::eyre!(
                            "Warm-up {}: Expected integer return value, got {:?}",
                            i,
                            result
                        ));
                    }
                    break;
                }
                Ok((_, Ok(TaskNotification::Suspended))) => continue,
                Ok((_, Err(e))) => return Err(eyre::eyre!("Warm-up task failed: {:?}", e)),
                Err(e) => return Err(eyre::eyre!("Failed to receive warm-up result: {:?}", e)),
            }
        }
    }
    info!("Warm-up completed successfully with consistent return values");

    eprintln!(
        "\nmooR Opcode Throughput Test\nMeasures raw interpreter loop speed without verb dispatch overhead.\n"
    );

    let mut table_rows = vec![];

    let start_time = Instant::now();

    // Submit all tasks
    let mut task_handles = Vec::new();
    for _ in 0..args.num_invocations {
        let task_handle = scheduler_client.submit_verb_task(
            &player,
            &ObjectRef::Id(player),
            Symbol::mk("opcode_test"),
            List::mk_list(&[]),
            v_empty_str(),
            &player,
            session.clone(),
        )?;
        task_handles.push(task_handle);
    }

    // Spinner animation
    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let mut spinner_idx = 0;
    let total = args.num_invocations;
    let mut completed = 0;

    eprint!(
        "  {} Running opcode workload... 0/{} rounds",
        SPINNER[0], total
    );
    std::io::Write::flush(&mut std::io::stderr()).ok();

    // Wait for all tasks and validate results
    let mut value_errors = 0;
    let expected = first_value.unwrap();
    for task_handle in task_handles {
        let receiver = task_handle.into_receiver();
        loop {
            match receiver.recv_async().await {
                Ok((_, Ok(TaskNotification::Result(result)))) => {
                    if let Variant::Int(actual) = result.variant() {
                        if actual != expected {
                            value_errors += 1;
                            if value_errors <= 3 {
                                eprintln!(
                                    "\nERROR: Inconsistent return value! Expected {}, got {}",
                                    expected, actual
                                );
                            }
                        }
                    } else {
                        value_errors += 1;
                        if value_errors <= 3 {
                            eprintln!("\nERROR: Expected integer return, got {:?}", result);
                        }
                    }
                    break;
                }
                Ok((_, Ok(TaskNotification::Suspended))) => continue,
                Ok((_, Err(e))) => return Err(eyre::eyre!("Task failed: {:?}", e)),
                Err(e) => return Err(eyre::eyre!("Failed to receive result: {:?}", e)),
            }
        }
        completed += 1;
        spinner_idx = (spinner_idx + 1) % SPINNER.len();
        eprint!(
            "\r  {} Running opcode workload... {}/{} rounds",
            SPINNER[spinner_idx], completed, total
        );
        std::io::Write::flush(&mut std::io::stderr()).ok();
    }

    if value_errors > 0 {
        return Err(eyre::eyre!(
            "{} tasks returned inconsistent values! Expected {} for {} iterations",
            value_errors,
            expected,
            args.loop_iterations
        ));
    }

    let wall_time = start_time.elapsed();

    // Opcode-based metrics
    let total_opcodes = args.num_invocations * opcodes_per_invocation;
    let per_opcode = Duration::from_secs_f64(wall_time.as_secs_f64() / total_opcodes as f64);
    let op_throughput = total_opcodes as f64 / wall_time.as_secs_f64();

    // Loop iteration metrics (work-equivalent across compilers)
    let total_iterations = args.num_invocations * args.loop_iterations;
    let per_iteration = Duration::from_secs_f64(wall_time.as_secs_f64() / total_iterations as f64);
    let iter_throughput = total_iterations as f64 / wall_time.as_secs_f64();

    eprintln!(
        "\r  ✓ Completed: {:?} for {} opcodes ({:.1}ns/op, {:.1}M op/s) | {} iterations ({:.1}ns/iter, {:.1}M iter/s)",
        wall_time,
        total_opcodes,
        per_opcode.as_nanos(),
        op_throughput / 1_000_000.0,
        total_iterations,
        per_iteration.as_nanos(),
        iter_throughput / 1_000_000.0
    );

    table_rows.push(BenchmarkRow {
        invocations: args.num_invocations,
        loop_iterations: format!("{}", total_iterations),
        opcodes: format!("{}", total_opcodes),
        wall_time: format!("{:.2?}", wall_time),
        per_opcode: format!("{:.1}ns", per_opcode.as_nanos()),
        op_throughput: format!("{:.1}M/s", op_throughput / 1_000_000.0),
        per_iteration: format!("{:.1}ns", per_iteration.as_nanos()),
        iter_throughput: format!("{:.1}M/s", iter_throughput / 1_000_000.0),
    });

    clear_screen();
    eprintln!("{}", Table::new(&table_rows));

    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install().expect("Unable to install color_eyre");
    let args: Args = Args::parse();

    moor_common::tracing::init_tracing(args.debug).unwrap_or_else(|e| {
        eprintln!("Unable to configure logging: {e}");
        std::process::exit(1);
    });

    info!("Starting opcode load test");

    let (db_path, _temp_dir) = setup_db_path(&args.db_path, "opcode_test_db")?;

    let (database, _) = TxDB::open(Some(&db_path), Default::default());
    let (player, opcodes_per_invocation) =
        setup_database(&database, args.loop_iterations, args.max_ticks)?;

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

    run_workload(&args, &scheduler_client, player, opcodes_per_invocation).await?;

    scheduler_client.submit_shutdown("Load test completed")?;

    Ok(())
}
