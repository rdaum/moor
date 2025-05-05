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

use std::sync::Arc;

use crate::args::Args;
use crate::rpc_server::RpcServer;
use crate::workers_server::WorkersServer;
use clap::Parser;
use eyre::Report;
use moor_common::build;
use moor_db::{Database, TxDB};
use moor_kernel::config::ImportExportFormat;
use moor_kernel::objdef::ObjectDefinitionLoader;
use moor_kernel::tasks::scheduler::Scheduler;
use moor_kernel::tasks::{NoopTasksDb, TasksDb};
use moor_kernel::textdump::textdump_load;
use rpc_common::load_keypair;
use tracing::{debug, info, warn};
use tracing_subscriber::fmt::format::FmtSpan;

mod connections;

mod args;
mod connections_fjall;
mod rpc_hosts;
mod rpc_server;
mod rpc_session;
mod sys_ctrl;
mod tasks_fjall;
mod workers_server;

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
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    // Check the public/private keypair file to see if it exists. If it does, parse it and establish
    // the keypair from it...
    let (private_key, public_key) = if args.public_key.exists() && args.private_key.exists() {
        load_keypair(&args.public_key, &args.private_key)
            .expect("Unable to load keypair from public and private key files")
    } else {
        panic!(
            "Public ({:?}) and/or private ({:?}) key files must exist",
            args.public_key, args.private_key
        );
    };

    let config = args
        .config_file
        .as_ref()
        .map(|path| {
            let file = std::fs::File::open(path).expect("Unable to open config file");

            serde_json::from_reader(file).expect("Unable to parse config file")
        })
        .unwrap_or_default();
    let config = Arc::new(args.merge_config(config));

    if let Some(write_config) = args.write_merged_config.as_ref() {
        let merged_config_json =
            serde_json::to_string_pretty(config.as_ref()).expect("Unable to serialize config");
        debug!("Merged config: {}", merged_config_json);
        std::fs::write(write_config, merged_config_json).expect("Unable to write merged config");
    }

    info!(
        "moor {} daemon starting. Using database at {:?}",
        version, args.db_args.db
    );
    let (database, freshly_made) =
        TxDB::open(Some(&args.db_args.db), config.database_config.clone());
    let database = Box::new(database);
    info!(path = ?args.db_args.db, "Opened database");

    if let Some(import_path) = config.import_export_config.input_path.as_ref() {
        // If the database already existed, do not try to import the textdump...
        if !freshly_made {
            info!("Database already exists, skipping textdump import");
        } else {
            info!("Loading textdump from {:?}", import_path);
            let start = std::time::Instant::now();
            let mut loader_interface = database
                .loader_client()
                .expect("Unable to get loader interface from database");

            // We have two ways of loading textdump.
            // legacy "textdump" format from LambdaMOO,
            // or our own exploded objdef format.
            match &config.import_export_config.import_format {
                ImportExportFormat::Objdef => {
                    let mut od = ObjectDefinitionLoader::new(loader_interface.as_mut());
                    od.read_dirdump(config.features_config.clone(), import_path.as_ref())
                        .unwrap();
                }
                ImportExportFormat::Textdump => {
                    textdump_load(
                        loader_interface.as_mut(),
                        import_path.clone(),
                        version.clone(),
                        config.features_config.clone(),
                    )
                    .unwrap();
                }
            }

            let duration = start.elapsed();
            info!("Loaded textdump in {:?}", duration);
            loader_interface
                .commit()
                .expect("Failure to commit loaded database...");
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
    let rpc_server = Arc::new(RpcServer::new(
        public_key.clone(),
        private_key.clone(),
        args.connections_file,
        zmq_ctx.clone(),
        args.events_listen.as_str(),
        config.clone(),
    ));
    let kill_switch = rpc_server.kill_switch();

    let (worker_scheduler_send, worker_scheduler_recv) = crossbeam_channel::unbounded();

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
