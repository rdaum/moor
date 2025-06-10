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
use std::fs::{File, OpenOptions};

use crate::args::Args;
use fs2::FileExt;
use eyre::{eyre, bail};

// Helper functions for resolving database paths relative to data_dir
impl Args {
    /// Resolve the main database path relative to data_dir
    fn resolved_db_path(&self) -> PathBuf {
        if self.db_args.db.is_absolute() {
            self.db_args.db.clone()
        } else {
            self.data_dir.join(&self.db_args.db)
        }
    }

    /// Resolve the tasks database path relative to data_dir
    fn resolved_tasks_db_path(&self) -> PathBuf {
        match &self.tasks_db {
            Some(path) => {
                if path.is_absolute() {
                    path.clone()
                } else {
                    self.data_dir.join(path)
                }
            }
            None => self.data_dir.join("tasks.db")
        }
    }

    /// Resolve the connections database path relative to data_dir
    fn resolved_connections_db_path(&self) -> Option<PathBuf> {
        match &self.connections_file {
            Some(path) => {
                if path.is_absolute() {
                    Some(path.clone())
                } else {
                    Some(self.data_dir.join(path))
                }
            }
            None => Some(self.data_dir.join("connections.db"))
        }
    }

    /// Resolve the events database path relative to data_dir
    fn resolved_events_db_path(&self) -> PathBuf {
        match &self.events_db {
            Some(path) => {
                if path.is_absolute() {
                    path.clone()
                } else {
                    self.data_dir.join(path)
                }
            }
            None => self.data_dir.join("events.db")
        }
    }
}
use crate::connections::ConnectionRegistryFactory;
use crate::rpc_server::RpcServer;
use crate::workers_server::WorkersServer;
use clap::Parser;
use eyre::Report;
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
mod event_log;
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

    let version = semver::Version::parse(build::PKG_VERSION)
        .map_err(|e| eyre!("Invalid moor version '{}': {}", build::PKG_VERSION, e))?;

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
    tracing::subscriber::set_global_default(main_subscriber)
        .map_err(|e| eyre!("Unable to configure logging: {}", e))?;

    // Check the public/private keypair file to see if it exists. If it does, parse it and establish
    // the keypair from it...
    let (private_key, public_key) = if args.public_key.exists() && args.private_key.exists() {
        load_keypair(&args.public_key, &args.private_key)
            .map_err(|e| eyre!("Unable to load keypair from public and private key files: {}", e))?
    } else {
        bail!(
            "Public ({:?}) and/or private ({:?}) key files must exist",
            args.public_key, args.private_key
        );
    };

    let config = match args.config_file.as_ref() {
        Some(path) => {
            let file = std::fs::File::open(path)
                .map_err(|e| eyre!("Unable to open config file {}: {}", path.display(), e))?;

            serde_json::from_reader(file)
                .map_err(|e| eyre!("Unable to parse config file {}: {}", path.display(), e))?
        }
        None => Default::default(),
    };
    let config = Arc::new(args.merge_config(config));

    if let Some(write_config) = args.write_merged_config.as_ref() {
        let merged_config_json = serde_json::to_string_pretty(config.as_ref())
            .map_err(|e| eyre!("Unable to serialize config: {}", e))?;
        debug!("Merged config: {}", merged_config_json);
        std::fs::write(write_config, &merged_config_json)
            .map_err(|e| eyre!("Unable to write merged config to {}: {}", write_config.display(), e))?;
    }

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

    let resolved_db_path = args.resolved_db_path();
    info!(
        "moor {} daemon starting. {phys_cores} physical cores; {logical_cores} logical cores. Using database at {:?}",
        version, resolved_db_path
    );
    let (database, freshly_made) = TxDB::open(
        Some(&resolved_db_path),
        config.database_config.clone().unwrap_or_default(),
    );
    let database = Box::new(database);
    info!(path = ?resolved_db_path, "Opened database");

    if let Some(import_path) = config.import_export_config.input_path.as_ref() {
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
                // If import failed, we need to get the old database file out of the way. We don't want to
                // leave a dirty empty database file around, or it will be picked up next time.
                // We could just delete the databse, but we might have regrets about that,
                // So let's move the whole directory to XXX.timestamp ...
                error!("Import failed: {}", import_error);
                
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                    
                let destination_str = resolved_db_path
                    .to_str()
                    .ok_or_else(|| eyre!("Database path contains invalid UTF-8"))?;
                let destination = format!("{}.{}", destination_str, timestamp);
                
                match std::fs::rename(&resolved_db_path, &destination) {
                    Ok(()) => {
                        info!("Moved (likely empty) database to {:?}", destination);
                    }
                    Err(e) => {
                        error!("Failed to rename database from {:?} to {}: {}", resolved_db_path, destination, e);
                    }
                }
                
                bail!("Import failed: {}", import_error);
            }
        }
    }

    let resolved_tasks_db_path = args.resolved_tasks_db_path();
    let tasks_db: Box<dyn TasksDb> = if config.features_config.persistent_tasks {
        Box::new(tasks_fjall::FjallTasksDB::open(&resolved_tasks_db_path).0)
    } else {
        Box::new(NoopTasksDb {})
    };

    // We have to create the RpcServer before starting the scheduler because we need to pass it in
    // as a parameter to the scheduler for background session construction.
    let zmq_ctx = zmq::Context::new();
    zmq_ctx
        .set_io_threads(args.num_io_threads)
        .map_err(|e| eyre!("Failed to set number of IO threads to {}: {}", args.num_io_threads, e))?;

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

    let resolved_events_db_path = args.resolved_events_db_path();
    let rpc_server = Arc::new(RpcServer::new(
        public_key.clone(),
        private_key.clone(),
        connections,
        zmq_ctx.clone(),
        args.events_listen.as_str(),
        config.clone(),
        &resolved_events_db_path,
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
        .map_err(|e| eyre!("Failed to start workers server on {}: {}", args.workers_request_listen, e))?;

    let workers_listen_addr = args.workers_response_listen.clone();
    std::thread::spawn(move || {
        if let Err(e) = workers_server.listen(&workers_listen_addr) {
            error!("Workers server failed to listen on {}: {}", workers_listen_addr, e);
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
        rpc_server.clone(),
        Some(workers_sender),
        Some(worker_scheduler_recv),
    );
    let scheduler_client = scheduler.client()
        .map_err(|e| eyre!("Failed to get scheduler client: {}", e))?;

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
            if let Err(e) = rpc_server.request_loop(rpc_listen.clone(), rpc_loop_scheduler_client) {
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
