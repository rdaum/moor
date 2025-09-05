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

//! Anonymous object creation load test - tests scheduler performance with anonymous object creation
//! using the create() builtin with anonymous flag set to true
//! Note: you should run this in release mode to get decent/comparable results

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use clap::Parser;
use clap_derive::Parser;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use moor_common::model::{CommitResult, ObjAttrs, ObjectRef};
use moor_common::model::{ObjFlag, PropFlag, VerbArgsSpec, VerbFlag};
use moor_common::tasks::{NarrativeEvent, Session, SessionError, SessionFactory, SystemControl};
use moor_common::util::BitEnum;
use moor_compiler::compile;
use moor_db::{Database, TxDB};
use moor_kernel::SchedulerClient;
use moor_kernel::config::{Config, FeaturesConfig, ImportExportConfig, RuntimeConfig};
use moor_kernel::tasks::scheduler::Scheduler;
use moor_kernel::tasks::{NoopTasksDb, TaskResult};
use moor_var::program::ProgramType;
use moor_var::{Error, List, NOTHING, Obj, Symbol, Var, v_int, v_obj};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long, help = "Database path", default_value = "test_db")]
    db_path: PathBuf,

    #[arg(
        long,
        help = "Min number of concurrent fake users to generate load. Load tests will start at `min_concurrent_workload` and increase to `max_concurrent_workload`.",
        default_value = "1"
    )]
    min_concurrent_workload: usize,

    #[arg(
        long,
        help = "Max number of concurrent fake users to generate load.",
        default_value = "32"
    )]
    max_concurrent_workload: usize,

    #[arg(
        long,
        help = "Number of anonymous objects to create per verb invocation",
        default_value = "100"
    )]
    num_objects_per_invocation: usize,

    #[arg(
        long,
        help = "How many times the anonymous object creation verb should be called.",
        default_value = "50"
    )]
    num_verb_invocations: usize,

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

const ANONYMOUS_OBJECT_CREATE_VERB: &str = r#"
let num_objects = args[1];
for i in [1..num_objects + 1]
    create(#1, player, true);
endfor
return num_objects;
"#;

/// Simple session implementation for direct scheduler testing
struct DirectSession {
    player: Obj,
}

impl DirectSession {
    fn new(player: Obj) -> Self {
        Self { player }
    }
}

impl Session for DirectSession {
    fn commit(&self) -> Result<(), SessionError> {
        Ok(())
    }

    fn rollback(&self) -> Result<(), SessionError> {
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(DirectSession::new(self.player)))
    }

    fn request_input(
        &self,
        _player: Obj,
        _input_request_id: uuid::Uuid,
    ) -> Result<(), SessionError> {
        Ok(())
    }

    fn send_event(&self, _player: Obj, _event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        Ok(())
    }

    fn send_system_msg(&self, _player: Obj, _msg: &str) -> Result<(), SessionError> {
        Ok(())
    }

    fn notify_shutdown(&self, _msg: Option<String>) -> Result<(), SessionError> {
        Ok(())
    }

    fn connection_name(&self, _player: Obj) -> Result<String, SessionError> {
        Ok("test-connection".to_string())
    }

    fn disconnect(&self, _player: Obj) -> Result<(), SessionError> {
        Ok(())
    }

    fn connected_players(&self) -> Result<Vec<Obj>, SessionError> {
        Ok(vec![])
    }

    fn connected_seconds(&self, _player: Obj) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn idle_seconds(&self, _player: Obj) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn connections(&self, _player: Option<Obj>) -> Result<Vec<Obj>, SessionError> {
        Ok(vec![])
    }

    fn connection_details(
        &self,
        _player: Option<Obj>,
    ) -> Result<Vec<moor_common::tasks::ConnectionDetails>, SessionError> {
        Ok(vec![])
    }

    fn connection_attributes(&self, _player: Obj) -> Result<HashMap<Symbol, Var>, SessionError> {
        Ok(HashMap::new())
    }
}

/// Simple session factory for direct scheduler testing
struct DirectSessionFactory {}

impl SessionFactory for DirectSessionFactory {
    fn mk_background_session(
        self: Arc<Self>,
        player: &Obj,
    ) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(DirectSession::new(*player)))
    }
}

/// No-op system control for direct scheduler testing
struct NoopSystemControl {}

impl SystemControl for NoopSystemControl {
    fn shutdown(&self, _msg: Option<String>) -> Result<(), Error> {
        Ok(())
    }

    fn listeners(&self) -> Result<Vec<(Obj, String, u16, bool)>, Error> {
        Ok(vec![])
    }

    fn listen(
        &self,
        _handler_object: Obj,
        _host_type: &str,
        _port: u16,
        _print_messages: bool,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn unlisten(&self, _port: u16, _host_type: &str) -> Result<(), Error> {
        Ok(())
    }

    fn switch_player(&self, _connection_obj: Obj, _new_player: Obj) -> Result<(), Error> {
        Ok(())
    }
}

fn setup_test_database(database: &TxDB) -> Result<Obj, eyre::Error> {
    let mut loader = database.loader_client()?;

    // Create a wizard player object
    let player_attrs = ObjAttrs::new(
        NOTHING, // owner (will be set to own itself after creation)
        NOTHING, // parent
        NOTHING, // location
        BitEnum::new_with(ObjFlag::User) | ObjFlag::Wizard, // flags - make it a wizard
        "Wizard", // name
    );

    let player = loader.create_object(Some(Obj::mk_id(1)), &player_attrs)?;
    info!("Created wizard player object: {}", player);

    // Set the player to own itself
    loader.set_object_owner(&player, &player)?;

    // Create system object #0 first (SYSTEM_OBJECT)
    let system_attrs = ObjAttrs::new(
        NOTHING,              // owner (will be set to own itself)
        NOTHING,              // parent
        NOTHING,              // location
        ObjFlag::User.into(), // flags
        "System Object",      // name
    );
    let system_obj = loader.create_object(Some(Obj::mk_id(0)), &system_attrs)?;
    loader.set_object_owner(&system_obj, &system_obj)?;

    // Create server options object with higher tick limits for load testing
    let server_options_attrs = ObjAttrs::new(
        player,               // owner
        NOTHING,              // parent
        NOTHING,              // location
        ObjFlag::User.into(), // flags
        "server_options",     // name
    );
    let server_options_obj = loader.create_object(None, &server_options_attrs)?;

    // Set much higher tick limits - anonymous object creation needs more ticks
    loader.define_property(
        &player,
        &server_options_obj,
        Symbol::mk("fg_ticks"),
        &player,
        PropFlag::Read.into(),
        Some(v_int(10_000_000)), // 10 million ticks
    )?;

    loader.define_property(
        &player,
        &server_options_obj,
        Symbol::mk("bg_ticks"),
        &player,
        PropFlag::Read.into(),
        Some(v_int(10_000_000)), // 10 million ticks
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

    // Compile and add the anonymous object creation verb
    let features_config = FeaturesConfig::default();
    let compile_options = features_config.compile_options();

    // Add and program the anonymous object creation verb
    let create_program = compile(ANONYMOUS_OBJECT_CREATE_VERB, compile_options)?;
    loader.add_verb(
        &player,                                   // obj
        &[Symbol::mk("create_anonymous_objects")], // names
        &player,                                   // owner
        VerbFlag::rx(),                            // flags
        VerbArgsSpec::this_none_this(),            // args
        ProgramType::MooR(create_program),         // program
    )?;

    // Commit all changes
    match loader.commit()? {
        CommitResult::Success => {
            info!("Successfully initialized test database for anonymous object creation");
            Ok(player)
        }
        CommitResult::ConflictRetry => Err(eyre::eyre!("Database conflict during initialization")),
    }
}

async fn workload(
    args: Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
) -> Result<Duration, eyre::Error> {
    let session = Arc::new(DirectSession::new(player));
    let start_time = Instant::now();

    // Submit all tasks concurrently first
    let mut task_handles = Vec::new();
    for _ in 0..args.num_verb_invocations {
        let task_handle = scheduler_client.submit_verb_task(
            &player,
            &ObjectRef::Id(player),
            Symbol::mk("create_anonymous_objects"),
            List::from_iter(vec![v_int(args.num_objects_per_invocation as i64)]),
            "".to_string(),
            &player,
            session.clone(),
        )?;
        task_handles.push(task_handle);
    }

    // Now wait for all results
    for task_handle in task_handles {
        match task_handle.receiver().recv_async().await {
            Ok((_, Ok(TaskResult::Result(result)))) => {
                let Some(result_int) = result.as_integer() else {
                    return Err(eyre::eyre!("Unexpected task result: {:?}", result));
                };
                if result_int != args.num_objects_per_invocation as i64 {
                    return Err(eyre::eyre!(
                        "Anonymous object creation failed: expected {}, got {}",
                        args.num_objects_per_invocation,
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

    Ok(start_time.elapsed())
}

async fn continuous_workload(
    args: Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
    stop_time: Instant,
) -> Result<(Duration, usize), eyre::Error> {
    let session = Arc::new(DirectSession::new(player));
    let start_time = Instant::now();
    let mut task_handles = Vec::new();
    let mut request_count = 0;

    // Submit tasks continuously until time limit
    while Instant::now() < stop_time {
        let task_handle = scheduler_client.submit_verb_task(
            &player,
            &ObjectRef::Id(player),
            Symbol::mk("create_anonymous_objects"),
            List::from_iter(vec![v_int(args.num_objects_per_invocation as i64)]),
            "".to_string(),
            &player,
            session.clone(),
        )?;

        task_handles.push(task_handle);
        request_count += 1;
    }

    // Now wait for all submitted tasks to complete
    for task_handle in task_handles {
        match task_handle.receiver().recv_async().await {
            Ok((_, Ok(TaskResult::Result(_result)))) => {
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

    Ok((start_time.elapsed(), request_count))
}

struct Results {
    /// How many concurrent threads there were.
    concurrency: usize,
    /// How many times the top-level verb was invoked
    total_invocations: usize,
    /// How many total anonymous objects were created
    total_objects_created: usize,
    /// The duration of the whole load test
    total_time: Duration,
    /// The cumulative time actually spent waiting for the scheduler to respond
    cumulative_time: Duration,
    /// The time per object creation (based on cumulative thread time)
    per_object_creation_cumulative: Duration,
    /// The time per object creation (based on wall-clock time)
    per_object_creation_wallclock: Duration,
}

async fn swamp_mode_workload(
    args: &Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
) -> Result<Vec<Results>, eyre::Error> {
    info!("Initializing swamp mode workload session");

    info!(
        "Starting swamp mode - running {} concurrent threads for {} seconds",
        args.max_concurrent_workload, args.swamp_duration_seconds
    );

    let start_time = Instant::now();
    let duration = Duration::from_secs(args.swamp_duration_seconds);
    let stop_time = start_time + duration;

    // Create continuous workload tasks that run for the specified duration
    let mut all_tasks = FuturesUnordered::new();

    for _i in 0..args.max_concurrent_workload {
        let args = args.clone();
        let scheduler_client = scheduler_client.clone();

        all_tasks.push(async move {
            continuous_workload(args, &scheduler_client, player, stop_time).await
        });
    }

    // Wait for all tasks to complete
    let mut times = vec![];
    let mut total_requests = 0;
    while let Some(result) = all_tasks.next().await {
        let (time, requests) = result?;
        times.push(time);
        total_requests += requests;
    }

    let cumulative_time = times.iter().fold(Duration::new(0, 0), |acc, x| acc + *x);
    let total_time = start_time.elapsed();
    let total_objects_created = total_requests * args.num_objects_per_invocation;

    let result = Results {
        concurrency: args.max_concurrent_workload,
        total_invocations: total_requests,
        total_time,
        cumulative_time,
        total_objects_created,
        per_object_creation_cumulative: Duration::from_secs_f64(
            cumulative_time.as_secs_f64() / total_objects_created as f64,
        ),
        per_object_creation_wallclock: Duration::from_secs_f64(
            total_time.as_secs_f64() / total_objects_created as f64,
        ),
    };

    info!(
        "Swamp mode completed: {} concurrent threads, {} total requests, {} objects created, Total Time: {:?}, Cumulative: {:?}, Per Object (cumulative): {:?}, Per Object (wallclock): {:?}",
        result.concurrency,
        result.total_invocations,
        result.total_objects_created,
        result.total_time,
        result.cumulative_time,
        result.per_object_creation_cumulative,
        result.per_object_creation_wallclock
    );

    Ok(vec![result])
}

async fn load_test_workload(
    args: &Args,
    scheduler_client: &SchedulerClient,
    player: Obj,
) -> Result<Vec<Results>, eyre::Error> {
    info!("Initializing anonymous object creation load-test workload session");

    info!("Anonymous object creation load-test workload session initialized, starting load test");

    let mut results = vec![];

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

    let mut concurrency = args.min_concurrent_workload as f32;
    loop {
        if concurrency > args.max_concurrent_workload as f32 {
            break;
        }
        let num_concurrent_workload = concurrency as usize;
        let start_time = Instant::now();

        info!(
            "Starting {num_concurrent_workload} threads workloads, calling anonymous object creation {} times, creating {} objects each...",
            args.num_verb_invocations, args.num_objects_per_invocation
        );

        let mut workload_futures = FuturesUnordered::new();
        for _i in 0..num_concurrent_workload {
            let args = args.clone();
            let scheduler_client = scheduler_client.clone();

            workload_futures.push(async move { workload(args, &scheduler_client, player).await });
        }

        let mut times = vec![];
        while let Some(h) = workload_futures.next().await {
            times.push(h?);
        }

        let cumulative_time = times.iter().fold(Duration::new(0, 0), |acc, x| acc + *x);
        let total_time = start_time.elapsed();
        let total_invocations = args.num_verb_invocations * num_concurrent_workload;
        let total_objects_created = total_invocations * args.num_objects_per_invocation;
        let r = Results {
            concurrency: num_concurrent_workload,
            total_invocations,
            total_time,
            cumulative_time,
            total_objects_created,
            per_object_creation_cumulative: Duration::from_secs_f64(
                cumulative_time.as_secs_f64() / total_objects_created as f64,
            ),
            per_object_creation_wallclock: Duration::from_secs_f64(
                total_time.as_secs_f64() / total_objects_created as f64,
            ),
        };
        info!(
            "@ Concurrency: {} w/ total invocations: {}, ({} total objects created): Total Time: {:?}, Cumulative: {:?}, Per Object (cumulative): {:?}, Per Object (wallclock): {:?}",
            r.concurrency,
            r.total_invocations,
            r.total_objects_created,
            r.total_time,
            r.cumulative_time,
            r.per_object_creation_cumulative,
            r.per_object_creation_wallclock
        );
        results.push(r);

        // Scale up by 25% or 1, whichever is larger, so we don't get stuck on lower values.
        let mut next_concurrency = concurrency * 1.25;
        if next_concurrency as usize <= concurrency as usize {
            next_concurrency = concurrency + 1.0;
        }
        concurrency = next_concurrency;
    }
    Ok(results)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install().expect("Unable to install color_eyre");
    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_max_level(if args.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    info!("Starting anonymous object creation load test");

    // Create temporary directory for database if using default path
    let temp_dir = if args.db_path == PathBuf::from("test_db") {
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
    let player = setup_test_database(&database)?;

    let database = Box::new(database);

    // Create config with higher tick limits for load testing and very long GC interval
    let runtime_config = RuntimeConfig {
        gc_interval: Some(Duration::from_secs(999)), // Very long GC interval during load testing
    };
    let config = Config {
        features: Arc::new(FeaturesConfig::default()),
        runtime: runtime_config,
        ..Default::default()
    };
    let config = Arc::new(config);

    // Create scheduler components
    let system_control = Arc::new(NoopSystemControl {});
    let tasks_db = Box::new(NoopTasksDb {});
    let version = semver::Version::parse("0.9.0-alpha").unwrap();

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
            "total_objects_created".to_string(),
            "total_time_ns".to_string(),
            "cumulative_time_ns".to_string(),
            "per_object_creation_cumulative_ns".to_string(),
            "per_object_creation_wallclock_ns".to_string(),
        ];
        writer.write_record(header)?;
        for r in results {
            let base = vec![
                r.concurrency.to_string(),
                r.total_invocations.to_string(),
                r.total_objects_created.to_string(),
                r.total_time.as_nanos().to_string(),
                r.cumulative_time.as_nanos().to_string(),
                r.per_object_creation_cumulative.as_nanos().to_string(),
                r.per_object_creation_wallclock.as_nanos().to_string(),
            ];
            writer.write_record(base)?
        }
        info!("Wrote {num_records} to {}", output_file.display())
    }

    // Shutdown scheduler
    scheduler_client.submit_shutdown("Load test completed")?;

    Ok(())
}
