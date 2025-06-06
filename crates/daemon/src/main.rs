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

use std::path::PathBuf;
use std::sync::Arc;

use crate::args::Args;
use crate::connections::ConnectionRegistryFactory;
use crate::rpc_server::RpcServer;
use crate::workers_server::WorkersServer;
use clap::Parser;
use eyre::{Report, bail};
use mimalloc::MiMalloc;
use moor_common::build;
use moor_db::{Database, TxDB};
use moor_kernel::config::ImportExportFormat;
use moor_kernel::tasks::scheduler::Scheduler;
use moor_kernel::tasks::{NoopTasksDb, TasksDb};
use moor_objdef::ObjectDefinitionLoader;
use moor_textdump::textdump_load;
use rpc_common::load_keypair;
use tracing::{debug, error, info, warn};
use tracing_subscriber::fmt::format::FmtSpan;

mod args;
mod connections;
mod rpc_hosts;
mod rpc_server;
mod rpc_session;
mod sys_ctrl;
mod tasks_fjall;
mod workers_server;

// main.rs
use moor_common::model::CommitResult;
use moor_common::model::loader::LoaderInterface;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn perform_import(
    config: &moor_kernel::config::Config,
    import_path: &PathBuf,
    mut loader_interface: Box<dyn LoaderInterface>,
    version: semver::Version,
) -> Result<(), Report> {
    let start = std::time::Instant::now();
    // We have two ways of loading textdump.
    // legacy "textdump" format from LambdaMOO,
    // or our own exploded objdef format.
    match &config.import_export_config.import_format {
        ImportExportFormat::Objdef => {
            let mut od = ObjectDefinitionLoader::new(loader_interface.as_mut());
            od.read_dirdump(
                config.features_config.compile_options(),
                import_path.as_ref(),
            )
            .unwrap();
        }
        ImportExportFormat::Textdump => {
            textdump_load(
                loader_interface.as_mut(),
                import_path.clone(),
                version.clone(),
                config.features_config.compile_options(),
            )
            .unwrap();
        }
    }

    let result = loader_interface.commit()?;

    if result == CommitResult::Success {
        info!("Import complete in {:?}", start.elapsed());
    } else {
        error!("Import failed due to commit failure: {:?}", result);
        bail!("Import failed");
    }
    Ok(())
}

/// Host for the moor runtime.
///   * Brings up the database
///   * Instantiates a scheduler
///   * Exposes RPC interface for session/connection management.
fn main() -> Result<(), Report> {
    color_eyre::install()?;

    let version = semver::Version::parse(build::PKG_VERSION).expect("Invalid moor version");

    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_file(true)
        .with_target(false)
        .with_line_number(true)
        .with_thread_names(true)
        .with_span_events(FmtSpan::NONE)
        .with_max_level(if args.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(main_subscriber).unwrap_or_else(|e| {
        eprintln!("Unable to set configure logging: {}", e);
        std::process::exit(1);
    });

    // Check the public/private keypair file to see if it exists. If it does, parse it and establish
    // the keypair from it...
    let (private_key, public_key) = if args.public_key.exists() && args.private_key.exists() {
        match load_keypair(&args.public_key, &args.private_key) {
            Ok(keypair) => keypair,
            Err(e) => {
                error!(
                    "Unable to load keypair from public and private key files: {}",
                    e
                );
                std::process::exit(1);
            }
        }
    } else {
        error!(
            "Public ({:?}) and/or private ({:?}) key files must exist",
            args.public_key, args.private_key
        );
        std::process::exit(1);
    };

    let config = match args.config_file.as_ref() {
        Some(path) => {
            let file = match std::fs::File::open(path) {
                Ok(file) => file,
                Err(e) => {
                    error!("Unable to open config file {}: {}", path.display(), e);
                    std::process::exit(1);
                }
            };

            match serde_json::from_reader(file) {
                Ok(config) => config,
                Err(e) => {
                    error!("Unable to parse config file {}: {}", path.display(), e);
                    std::process::exit(1);
                }
            }
        }
        None => Default::default(),
    };
    let config = Arc::new(args.merge_config(config));

    if let Some(write_config) = args.write_merged_config.as_ref() {
        let merged_config_json =
            serde_json::to_string_pretty(config.as_ref()).expect("Unable to serialize config");
        debug!("Merged config: {}", merged_config_json);
        std::fs::write(write_config, merged_config_json).expect("Unable to write merged config");
    }

    let (phys_cores, logical_cores) = (
        gdt_cpus::num_physical_cores()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        gdt_cpus::num_logical_cores()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
    );

    info!(
        "moor {} daemon starting. {phys_cores} physical cores; {logical_cores} logical cores. Using database at {:?}",
        version, args.db_args.db
    );
    let (database, freshly_made) = TxDB::open(
        Some(&args.db_args.db),
        config.database_config.clone().unwrap_or_default(),
    );
    let database = Box::new(database);
    info!(path = ?args.db_args.db, "Opened database");

    if let Some(import_path) = config.import_export_config.input_path.as_ref() {
        // If the database already existed, do not try to import the textdump...
        if !freshly_made {
            info!("Database already exists, skipping textdump import");
        } else {
            info!("Loading textdump from {:?}", import_path);
            let loader_interface = database
                .loader_client()
                .expect("Unable to get loader interface from database");

            if perform_import(
                config.as_ref(),
                import_path,
                loader_interface,
                version.clone(),
            )
            .is_err()
            {
                // If import failed, we need to get the old database file out of the way. We don't want to
                // leave a dirty empty database file around, or it will be picked up next time.
                // We could just delete the databse, but we might have regrets about that,
                // So let's move the whole directory to XXX.timestamp ...
                info!("Import failed. Deleting database at {:?}", args.db_args.db);
                let destination = format!(
                    "{}.{}",
                    args.db_args.db.to_str().unwrap(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                );
                if let Err(e) = std::fs::rename(&args.db_args.db, &destination) {
                    error!("Failed to rename database: {:?}", e);
                } else {
                    info!("Moved (likely empty) database to {:?}", destination);
                }
                error!("Exiting due to import failure...");
                std::process::exit(1);
            }
        }
    }

    let tasks_db: Box<dyn TasksDb> = if config.features_config.persistent_tasks {
        Box::new(tasks_fjall::FjallTasksDB::open(&args.tasks_db).0)
    } else {
        Box::new(NoopTasksDb {})
    };

    // We have to create the RpcServer before starting the scheduler because we need to pass it in
    // as a parameter to the scheduler for background session construction.
    let zmq_ctx = zmq::Context::new();
    zmq_ctx
        .set_io_threads(args.num_io_threads)
        .expect("Failed to set number of IO threads");

    // Create the connections registry based on args/config
    let connections = match &args.connections_file {
        Some(path) => {
            info!("Using connections database at {:?}", path);
            ConnectionRegistryFactory::with_fjall_persistence(Some(path))
                .expect("Failed to create connections database")
        }
        None => {
            info!("Using in-memory connections registry");
            ConnectionRegistryFactory::in_memory_only()
                .expect("Failed to create in-memory connections registry")
        }
    };

    let rpc_server = Arc::new(RpcServer::new(
        public_key.clone(),
        private_key.clone(),
        connections,
        zmq_ctx.clone(),
        args.events_listen.as_str(),
        config.clone(),
    ));
    let kill_switch = rpc_server.kill_switch.clone();

    let (worker_scheduler_send, worker_scheduler_recv) = flume::unbounded();

    // Workers RPC server
    let mut workers_server = WorkersServer::new(
        kill_switch.clone(),
        public_key,
        private_key,
        zmq_ctx.clone(),
        worker_scheduler_send,
    );
    let workers_sender = workers_server
        .start(&args.workers_request_listen)
        .expect("Failed to start workers server");

    std::thread::spawn(move || {
        workers_server
            .listen(&args.workers_response_listen)
            .expect("Failed to listen for workers");
    });

    // The pieces from core we're going to use:
    //   Our DB.
    //   Our scheduler.
    let scheduler = Scheduler::new(
        version,
        database,
        tasks_db,
        config.clone(),
        rpc_server.clone(),
        Some(workers_sender),
        Some(worker_scheduler_recv),
    );
    let scheduler_client = scheduler.client().expect("Failed to get scheduler client");

    // The scheduler thread:
    let scheduler_rpc_server = rpc_server.clone();
    let scheduler_loop_jh = std::thread::Builder::new()
        .name("moor-scheduler".to_string())
        .spawn(move || scheduler.run(scheduler_rpc_server))?;

    // Background DB checkpoint thread.
    if let (Some(checkpoint_interval), Some(output_path)) = (
        config.import_export_config.checkpoint_interval,
        config.import_export_config.output_path.clone(),
    ) {
        let checkpoint_kill_switch = kill_switch.clone();
        let checkpoint_scheduler_client = scheduler_client.clone();
        info!(
            "Checkpointing enabled to {}. Interval: {:?}",
            output_path.as_path().display(),
            checkpoint_interval
        );
        std::thread::Builder::new()
            .name("moor-checkpoint".to_string())
            .spawn(move || {
                loop {
                    if checkpoint_kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                        info!("Checkpointing thread exiting.");
                        break;
                    }
                    checkpoint_scheduler_client
                        .request_checkpoint()
                        .expect("Failed to submit checkpoint");
                    std::thread::sleep(checkpoint_interval);
                }
            })?;
    } else {
        info!("Checkpointing disabled.");
    }

    let rpc_loop_scheduler_client = scheduler_client.clone();
    let rpc_listen = args.rpc_listen.clone();
    let rpc_loop_thread = std::thread::Builder::new()
        .name("moor-rpc".to_string())
        .spawn(move || {
            rpc_server
                .request_loop(rpc_listen, rpc_loop_scheduler_client)
                .expect("RPC thread failed");
        })?;

    signal_hook::flag::register(signal_hook::consts::SIGTERM, kill_switch.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, kill_switch.clone())?;
    info!(
        rpc_endpoint = args.rpc_listen,
        events_endpoint = args.events_listen,
        "Daemon started. Listening for RPC events."
    );
    rpc_loop_thread.join().expect("RPC thread panicked");
    warn!("RPC thread exited. Departing...");

    scheduler_client
        .submit_shutdown("System shutting down")
        .expect("Scheduler thread failed to stop");
    scheduler_loop_jh.join().expect("Scheduler thread panicked");
    info!("Done.");

    Ok(())
}
