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

//! Benchmark for measuring suspend/resume cycle latency
//! Measures how long it takes for a task to commit, suspend, and resume with a new transaction

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use clap::Parser;
use clap_derive::Parser;
use futures::{StreamExt, stream::FuturesUnordered};
use moor_common::{
    model::{CommitResult, ObjAttrs, ObjFlag, ObjectKind, ObjectRef, VerbArgsSpec, VerbFlag},
    util::BitEnum,
};
use moor_compiler::compile;
use moor_db::{Database, TxDB};
use moor_kernel::{
    config::{Config, FeaturesConfig},
    tasks::{NoopTasksDb, TaskNotification, scheduler::Scheduler},
};
use moor_model_checker::{DirectSession, DirectSessionFactory, NoopSystemControl};
use moor_var::{List, NOTHING, Obj, Symbol, program::ProgramType, v_int};
use std::{sync::Arc, time::Instant};
use tracing::info;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long, help = "Number of concurrent tasks to run", default_value = "1")]
    concurrency: usize,

    #[arg(
        long,
        help = "Number of suspend/resume cycles per task",
        default_value = "100000"
    )]
    cycles: usize,
}

// This verb does N suspend(0) cycles
// Each suspend(0) triggers: commit -> WakeCondition::Immediate -> scheduler wakes task -> new transaction
const SUSPEND_BENCH_VERB: &str = r#"
num_cycles = args[1];
for i in [1..num_cycles]
    suspend(0);
endfor
return 1;
"#;

fn setup_bench_database(database: &TxDB) -> Result<Obj, eyre::Error> {
    let mut loader = database.loader_client()?;

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

    // Create a player object
    let player_attrs = ObjAttrs::new(
        NOTHING,
        NOTHING,
        NOTHING,
        BitEnum::new_with(ObjFlag::User) | ObjFlag::Wizard,
        "BenchPlayer",
    );

    let player = loader.create_object(ObjectKind::Objid(Obj::mk_id(1)), &player_attrs)?;
    loader.set_object_owner(&player, &player)?;

    // Compile and add the suspend benchmark verb
    let features_config = FeaturesConfig::default();
    let compile_options = features_config.compile_options();
    let program = compile(SUSPEND_BENCH_VERB, compile_options)?;

    loader.add_verb(
        &player,
        &[Symbol::mk("suspend_bench")],
        &player,
        VerbFlag::rx(),
        VerbArgsSpec::this_none_this(),
        ProgramType::MooR(program),
    )?;

    // Commit all changes
    match loader.commit()? {
        CommitResult::Success { .. } => {
            info!("Successfully initialized benchmark database");
            Ok(player)
        }
        CommitResult::ConflictRetry { .. } => {
            Err(eyre::eyre!("Database conflict during initialization"))
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install()?;

    let args: Args = Args::parse();
    moor_common::tracing::init_tracing(false).unwrap_or_else(|e| {
        eprintln!("Unable to configure logging: {e}");
        std::process::exit(1);
    });

    info!("Starting suspend/resume benchmark");
    info!(
        "Configuration: {} concurrent tasks, {} cycles each",
        args.concurrency, args.cycles
    );

    // Create temporary database
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("bench_db");

    let (database, _) = TxDB::open(Some(&db_path), Default::default());
    let player = setup_bench_database(&database)?;
    let database = Box::new(database);

    // Create scheduler components
    let config = Arc::new(Config {
        features: Arc::new(FeaturesConfig::default()),
        ..Default::default()
    });
    let system_control = Arc::new(NoopSystemControl {});
    let tasks_db = Box::new(NoopTasksDb {});
    let version = semver::Version::parse("1.0.0-beta5").unwrap();

    // Create and start scheduler
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

    // Wait a bit for scheduler to be ready
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    info!("Running benchmark...");

    let start = Instant::now();

    // Submit all tasks concurrently
    let mut task_futures = FuturesUnordered::new();
    for _ in 0..args.concurrency {
        let session = Arc::new(DirectSession::new(player));
        let task_handle = scheduler_client.submit_verb_task(
            &player,
            &ObjectRef::Id(player),
            Symbol::mk("suspend_bench"),
            List::from_iter(vec![v_int(args.cycles as i64)]),
            "".to_string(),
            &player,
            session,
        )?;

        let receiver = task_handle.into_receiver();
        task_futures.push(async move {
            loop {
                match receiver.recv_async().await {
                    Ok((_task_id, Ok(TaskNotification::Suspended))) => {
                        // Ignore intermediate suspension notifications
                        continue;
                    }
                    other => break other,
                }
            }
        });
    }

    // Wait for all tasks to complete
    let mut completed = 0;
    while let Some(result) = task_futures.next().await {
        match result {
            Ok((_, Ok(TaskNotification::Result(result)))) => {
                let result_int = result
                    .as_integer()
                    .ok_or_else(|| eyre::eyre!("Expected integer result"))?;
                if result_int != 1 {
                    return Err(eyre::eyre!(
                        "Benchmark failed: expected 1, got {}",
                        result_int
                    ));
                }
                completed += 1;
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

    let elapsed = start.elapsed();
    let total_cycles = args.concurrency * args.cycles;
    let elapsed_secs = elapsed.as_secs_f64();
    let latency_micros = (elapsed.as_micros() as f64 / args.cycles as f64) as u128;
    let throughput_per_sec = (total_cycles as f64) / elapsed_secs;

    info!("=== BENCHMARK RESULTS ===");
    info!(
        "Completed {} tasks ({} total cycles)",
        completed, total_cycles
    );
    info!("Total wall-clock time: {:.3} seconds", elapsed_secs);
    info!("Latency per cycle: {} μs", latency_micros);
    info!("Throughput: {:.0} suspends/sec", throughput_per_sec);
    info!("========================");

    // Shutdown scheduler
    scheduler_client.submit_shutdown("Benchmark completed")?;

    Ok(())
}
