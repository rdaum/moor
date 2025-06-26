#![recursion_limit = "256"]
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

use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::args::Args;
use eyre::{bail, eyre};
use fs2::FileExt;

use crate::connections::ConnectionRegistryFactory;
use crate::rpc::{RpcServer, Transport, transport::RpcTransport};
use crate::workers::WorkersServer;
use clap::Parser;
use eyre::Report;
use mimalloc::MiMalloc;
use moor_common::build;
use moor_db::{Database, TxDB};
use moor_kernel::config::{Config, ImportExportFormat};
use moor_kernel::tasks::scheduler::Scheduler;
use moor_kernel::tasks::{NoopTasksDb, TasksDb};
use moor_objdef::ObjectDefinitionLoader;
use moor_textdump::textdump_load;
use rpc_common::load_keypair;
use tracing::{error, info, warn};
use tracing_subscriber::fmt::format::FmtSpan;

mod args;
mod connections;
mod event_log;
mod feature_args;
mod rpc;
mod system_control;
mod tasks;
mod workers;

// main.rs
use crate::tasks::tasks_db_fjall::FjallTasksDB;
use moor_common::model::CommitResult;
use moor_common::model::loader::LoaderInterface;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Acquire an exclusive lock on the data directory to prevent multiple daemon instances
/// from operating on the same data.
fn acquire_data_directory_lock(data_dir: &PathBuf) -> Result<File, Report> {
    // Create the data directory if it doesn't exist
    std::fs::create_dir_all(data_dir)?;

    // Create a lock file in the data directory
    let lock_file_path = data_dir.join(".moor-daemon.lock");

    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&lock_file_path)?;

    // Try to acquire exclusive lock
    match lock_file.try_lock_exclusive() {
        Ok(()) => {
            info!("Acquired exclusive lock on data directory: {:?}", data_dir);
            Ok(lock_file)
        }
        Err(e) => {
            error!(
                "Failed to acquire lock on data directory {:?}. Another moor-daemon instance may already be running in this directory.",
                data_dir
            );
            bail!("Directory lock acquisition failed: {}", e);
        }
    }
}

fn perform_import(
    config: &Config,
    import_path: &PathBuf,
    mut loader_interface: Box<dyn LoaderInterface>,
    version: semver::Version,
) -> Result<(), Report> {
    let start = std::time::Instant::now();
    // We have two ways of loading textdump.
    // legacy "textdump" format from LambdaMOO,
    // or our own exploded objdef format.
    match &config.import_export.import_format {
        ImportExportFormat::Objdef => {
            let mut od = ObjectDefinitionLoader::new(loader_interface.as_mut());
            od.read_dirdump(config.features.compile_options(), import_path.as_ref())?;
        }
        ImportExportFormat::Textdump => {
            textdump_load(
                loader_interface.as_mut(),
                import_path.clone(),
                version.clone(),
                config.features.compile_options(),
            )?;
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

/// Create the RPC transport layer for production use
fn create_rpc_transport(
    zmq_context: zmq::Context,
    kill_switch: Arc<AtomicBool>,
    events_listen: &str,
) -> Result<Arc<dyn Transport>, Report> {
    let transport = Arc::new(
        RpcTransport::new(zmq_context, kill_switch, events_listen)
            .map_err(|e| eyre!("Failed to create RPC transport: {}", e))?,
    ) as Arc<dyn Transport>;
    Ok(transport)
}

/// Host for the moor runtime.
///   * Brings up the database
///   * Instantiates a scheduler
///   * Exposes RPC interface for session/connection management.
fn main() -> Result<(), Report> {
    color_eyre::install()?;

    let args = Args::parse();

    let version = semver::Version::parse(build::PKG_VERSION)
        .map_err(|e| eyre!("Invalid moor version '{}': {}", build::PKG_VERSION, e))?;

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
    tracing::subscriber::set_global_default(main_subscriber)
        .map_err(|e| eyre!("Unable to configure logging: {}", e))?;

    // Check the public/private keypair file to see if it exists. If it does, parse it and establish
    // the keypair from it...
    let (private_key, public_key) = if args.public_key.exists() && args.private_key.exists() {
        load_keypair(&args.public_key, &args.private_key).map_err(|e| {
            eyre!(
                "Unable to load keypair from public and private key files: {}",
                e
            )
        })?
    } else {
        bail!(
            "Public ({:?}) and/or private ({:?}) key files must exist",
            args.public_key,
            args.private_key
        );
    };

    // Acquire exclusive lock on the data directory to prevent multiple daemon instances
    let _data_dir_lock = acquire_data_directory_lock(&args.data_dir)?;

    let (phys_cores, logical_cores) = (
        gdt_cpus::num_physical_cores()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        gdt_cpus::num_logical_cores()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
    );

    let config = args.load_config()?;

    let resolved_db_path = args.resolved_db_path();
    info!(
        "moor {} daemon starting. {phys_cores} physical cores; {logical_cores} logical cores. Using database at {:?}",
        version, resolved_db_path
    );
    let (database, freshly_made) = TxDB::open(
        Some(&resolved_db_path),
        config.database.clone().unwrap_or_default(),
    );
    let database = Box::new(database);
    info!(path = ?resolved_db_path, "Opened database");

    if let Some(import_path) = config.import_export.input_path.as_ref() {
        // If the database already existed, do not try to import the textdump...
        if !freshly_made {
            info!("Database already exists, skipping textdump import");
        } else {
            info!("Loading textdump from {:?}", import_path);
            let loader_interface = database
                .loader_client()
                .map_err(|e| eyre!("Unable to get loader interface from database: {}", e))?;

            if let Err(import_error) = perform_import(
                config.as_ref(),
                import_path,
                loader_interface,
                version.clone(),
            ) {
                error!("Import failed: {}", import_error);

                // Just delete the created database file if the import fails.
                if let Err(e) = std::fs::remove_file(&resolved_db_path) {
                    panic!(
                        "Failed to remove database {resolved_db_path:?} after import failure: {e}"
                    );
                } else {
                    info!(
                        "Removed bad database {:?} after import failure",
                        resolved_db_path
                    );
                }

                exit(1);
            }
        }
    }

    let resolved_tasks_db_path = args.resolved_tasks_db_path();
    let tasks_db: Box<dyn TasksDb> = if config.features.persistent_tasks {
        Box::new(FjallTasksDB::open(&resolved_tasks_db_path).0)
    } else {
        Box::new(NoopTasksDb {})
    };

    // We have to create the RpcServer before starting the scheduler because we need to pass it in
    // as a parameter to the scheduler for background session construction.
    let zmq_ctx = zmq::Context::new();
    zmq_ctx.set_io_threads(args.num_io_threads).map_err(|e| {
        eyre!(
            "Failed to set number of IO threads to {}: {}",
            args.num_io_threads,
            e
        )
    })?;

    // Create the connections registry based on args/config
    let resolved_connections_db_path = args.resolved_connections_db_path();
    let connections = match resolved_connections_db_path {
        Some(path) => {
            info!("Using connections database at {:?}", path);
            ConnectionRegistryFactory::with_fjall_persistence(Some(&path))
                .map_err(|e| eyre!("Failed to create connections database at {:?}: {}", path, e))?
        }
        None => {
            info!("Using in-memory connections registry");
            ConnectionRegistryFactory::in_memory_only()
                .map_err(|e| eyre!("Failed to create in-memory connections registry: {}", e))?
        }
    };

    // Create the kill switch that will be shared across components
    let kill_switch = Arc::new(AtomicBool::new(false));

    let resolved_events_db_path = args.resolved_events_db_path();

    // Create the RPC transport
    let rpc_transport = create_rpc_transport(
        zmq_ctx.clone(),
        kill_switch.clone(),
        args.events_listen.as_str(),
    )?;

    let (rpc_server, task_monitor, system_control) = RpcServer::new(
        kill_switch.clone(),
        public_key.clone(),
        private_key.clone(),
        connections,
        rpc_transport,
        config.clone(),
        &resolved_events_db_path,
    );
    let rpc_server = Arc::new(rpc_server);

    let (worker_scheduler_send, worker_scheduler_recv) = flume::unbounded();

    // Workers RPC server
    let mut workers_server = WorkersServer::new(
        kill_switch.clone(),
        public_key,
        private_key,
        zmq_ctx.clone(),
        &args.workers_request_listen,
        worker_scheduler_send,
    )
    .map_err(|e| eyre!("Failed to create workers server: {}", e))?;
    let workers_sender = workers_server.start().map_err(|e| {
        eyre!(
            "Failed to start workers server on {}: {}",
            args.workers_request_listen,
            e
        )
    })?;

    let workers_listen_addr = args.workers_response_listen.clone();
    std::thread::spawn(move || {
        if let Err(e) = workers_server.listen(&workers_listen_addr) {
            error!(
                "Workers server failed to listen on {}: {}",
                workers_listen_addr, e
            );
        }
    });

    // The pieces from core we're going to use:
    //   Our DB.
    //   Our scheduler.
    let scheduler = Scheduler::new(
        version,
        database,
        tasks_db,
        config.clone(),
        Arc::new(system_control),
        Some(workers_sender),
        Some(worker_scheduler_recv),
    );
    let scheduler_client = scheduler
        .client()
        .map_err(|e| eyre!("Failed to get scheduler client: {}", e))?;

    // The scheduler thread:
    let scheduler_rpc_server = rpc_server.clone();
    let scheduler_loop_jh = std::thread::Builder::new()
        .name("moor-scheduler".to_string())
        .spawn(move || scheduler.run(scheduler_rpc_server))?;

    // Background DB checkpoint thread.
    if let (Some(checkpoint_interval), Some(output_path)) = (
        config.import_export.checkpoint_interval,
        config.import_export.output_path.clone(),
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
                    if let Err(e) = checkpoint_scheduler_client.request_checkpoint() {
                        error!("Failed to submit checkpoint request: {}", e);
                    }
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
            if let Err(e) =
                rpc_server.request_loop(rpc_listen.clone(), rpc_loop_scheduler_client, task_monitor)
            {
                error!("RPC server failed on {}: {}", rpc_listen, e);
            }
        })?;

    signal_hook::flag::register(signal_hook::consts::SIGTERM, kill_switch.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, kill_switch.clone())?;
    info!(
        rpc_endpoint = args.rpc_listen,
        events_endpoint = args.events_listen,
        "Daemon started. Listening for RPC events."
    );
    if let Err(e) = rpc_loop_thread.join() {
        error!("RPC thread panicked: {:?}", e);
    }
    warn!("RPC thread exited. Departing...");

    if let Err(e) = scheduler_client.submit_shutdown("System shutting down") {
        error!("Failed to send shutdown signal to scheduler: {}", e);
    }

    if let Err(e) = scheduler_loop_jh.join() {
        error!("Scheduler thread panicked: {:?}", e);
    }

    info!("Done.");
    Ok(())
}
