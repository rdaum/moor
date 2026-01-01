#![recursion_limit = "256"]
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

use crate::args::Args;
use eyre::{bail, eyre};
use fs2::FileExt;
use moor_common::tracing;
use std::io::IsTerminal;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    process::exit,
    sync::{Arc, atomic::AtomicBool},
};

use crate::{
    connections::ConnectionRegistryFactory,
    event_log::{EventLog, EventLogConfig, EventLogOps, NoOpEventLog},
    rpc::{RpcServer, Transport, transport::RpcTransport},
    workers::WorkersServer,
};
use ::tracing::{debug, error, info, warn};
use base64::{Engine as _, engine::general_purpose};
use clap::Parser;
use ed25519_dalek::{
    SigningKey,
    pkcs8::{EncodePrivateKey, EncodePublicKey},
};
use eyre::Report;
use mimalloc::MiMalloc;
use moor_common::{build, model::ObjectRef, tasks::SessionFactory};
use moor_compiler::emit_compile_error;
use moor_db::{Database, TxDB};
use moor_kernel::{
    SchedulerClient,
    config::{Config, ImportFormat},
    tasks::{NoopTasksDb, TasksDb, scheduler::Scheduler},
};
use moor_objdef::ObjectDefinitionLoader;
use moor_textdump::{TextdumpImportOptions, textdump_load};
use moor_var::{List, Obj, SYSTEM_OBJECT, Symbol};
use rand::Rng;
use rpc_common::load_keypair;
use sha2::{Digest, Sha256};

mod allowed_hosts;
mod args;
mod connections;
mod curve_keys;
mod enrollment;
mod event_log;
mod feature_args;
mod rpc;
mod system_control;
mod tasks;
#[cfg(test)]
mod testing;
mod workers;
mod zap_auth;

// main.rs
use crate::tasks::tasks_db_fjall::FjallTasksDB;
use moor_common::model::{CommitResult, CompileError, loader::LoaderInterface};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const BANNER_MSG: &str = r#"                                   ███████████      ████        █████   
                                  ▒▒███▒▒▒▒▒███    ▒▒███      ███▒▒▒███ 
 █████████████    ██████   ██████  ▒███    ▒███     ▒███     ███   ▒▒███
▒▒███▒▒███▒▒███  ███▒▒███ ███▒▒███ ▒██████████      ▒███    ▒███    ▒███
 ▒███ ▒███ ▒███ ▒███ ▒███▒███ ▒███ ▒███▒▒▒▒▒███     ▒███    ▒███    ▒███
 ▒███ ▒███ ▒███ ▒███ ▒███▒███ ▒███ ▒███    ▒███     ▒███    ▒▒███   ███ 
 █████▒███ █████▒▒██████ ▒▒██████  █████   █████    █████ ██ ▒▒▒█████▒  
▒▒▒▒▒ ▒▒▒ ▒▒▒▒▒  ▒▒▒▒▒▒   ▒▒▒▒▒▒  ▒▒▒▒▒   ▒▒▒▒▒    ▒▒▒▒▒ ▒▒    ▒▒▒▒▒▒   
                                                                        
                                                                        
                                                                        "#;

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

fn log_objdef_compile_error(path: &str, compile_error: &CompileError, verb_source: &str) {
    let (source, source_name) = if !verb_source.is_empty() {
        (
            Some(verb_source.to_string()),
            format!("{} (verb body)", path),
        )
    } else if path != "<string>" {
        match fs::read_to_string(path) {
            Ok(text) => (Some(text), path.to_string()),
            Err(err) => {
                error!("Failed to read {path} for diagnostic rendering: {err}");
                (None, path.to_string())
            }
        }
    } else {
        (None, path.to_string())
    };

    let use_color = io::stderr().is_terminal();
    eprintln!();
    emit_compile_error(compile_error, source.as_deref(), &source_name, use_color);
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
    let commit = match &config.import_export.import_format {
        ImportFormat::Objdef => {
            let mut od = ObjectDefinitionLoader::new(loader_interface.as_mut());
            let options = moor_objdef::ObjDefLoaderOptions::default();
            let results = match od.load_objdef_directory(
                config.features.compile_options(),
                import_path.as_ref(),
                options,
            ) {
                Ok(results) => results,
                Err(e) => {
                    if let Some((source, compile_error, verb_source)) = e.compile_error() {
                        log_objdef_compile_error(source, compile_error, verb_source);
                        return Err(eyre::eyre!("Failed to compile object definitions"));
                    }
                    return Err(Report::new(e));
                }
            };
            info!(
                "Imported {} objects w/ {} verbs, {} properties and {} property overrides",
                results.loaded_objects.len(),
                results.num_loaded_verbs,
                results.num_loaded_property_definitions,
                results.num_loaded_property_overrides
            );
            results.commit
        }
        ImportFormat::Textdump => {
            textdump_load(
                loader_interface.as_mut(),
                import_path.clone(),
                version.clone(),
                config.features.compile_options(),
                TextdumpImportOptions::default(),
            )?;
            true
        }
    };

    if commit {
        let result = loader_interface.commit()?;

        match result {
            CommitResult::Success { .. } => {
                info!("Import complete in {:?}", start.elapsed());
            }
            _ => {
                error!("Import failed due to commit failure: {:?}", result);
                bail!("Import failed");
            }
        }
    } else {
        warn!("Loaded requested rollback, not committing results");
        // Just dropping the transaction (LoaderInterface) is sufficient here.
    }
    Ok(())
}

/// Generate ED25519 keypair and write to PEM files
fn generate_keypair(public_key_path: &PathBuf, private_key_path: &PathBuf) -> Result<(), Report> {
    info!("Generating ED25519 keypair...");

    // Generate a new signing key
    let mut rng = rand::rng();
    let mut secret_key_bytes = [0u8; 32];
    rng.fill(&mut secret_key_bytes);
    let signing_key = SigningKey::from_bytes(&secret_key_bytes);
    let verifying_key = signing_key.verifying_key();

    // Convert to DER format first
    let private_der = signing_key
        .to_pkcs8_der()
        .map_err(|e| eyre!("Failed to encode private key to DER: {}", e))?;
    let public_der = verifying_key
        .to_public_key_der()
        .map_err(|e| eyre!("Failed to encode public key to DER: {}", e))?;

    // Convert DER to PEM using base64 encoding with proper line wrapping
    let private_b64 = general_purpose::STANDARD.encode(private_der.as_bytes());
    let public_b64 = general_purpose::STANDARD.encode(public_der.as_bytes());

    // Wrap base64 content at 64 characters per line
    let private_wrapped = private_b64
        .chars()
        .collect::<Vec<_>>()
        .chunks(64)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    let public_wrapped = public_b64
        .chars()
        .collect::<Vec<_>>()
        .chunks(64)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    let private_pem =
        format!("-----BEGIN PRIVATE KEY-----\n{private_wrapped}\n-----END PRIVATE KEY-----\n");
    let public_pem =
        format!("-----BEGIN PUBLIC KEY-----\n{public_wrapped}\n-----END PUBLIC KEY-----\n");

    // Write private key
    let mut private_file = File::create(private_key_path).map_err(|e| {
        eyre!(
            "Failed to create private key file {:?}: {}",
            private_key_path,
            e
        )
    })?;
    private_file
        .write_all(private_pem.as_bytes())
        .map_err(|e| {
            eyre!(
                "Failed to write private key to {:?}: {}",
                private_key_path,
                e
            )
        })?;

    // Write public key
    let mut public_file = File::create(public_key_path).map_err(|e| {
        eyre!(
            "Failed to create public key file {:?}: {}",
            public_key_path,
            e
        )
    })?;
    public_file
        .write_all(public_pem.as_bytes())
        .map_err(|e| eyre!("Failed to write public key to {:?}: {}", public_key_path, e))?;

    info!("Generated keypair:");
    info!("  Private key: {:?}", private_key_path);
    info!("  Public key: {:?}", public_key_path);

    Ok(())
}

/// Create the RPC transport layer for production use
fn create_rpc_transport(
    zmq_context: zmq::Context,
    kill_switch: Arc<AtomicBool>,
    events_listen: &str,
    curve_secret_key: Option<String>,
) -> Result<Arc<dyn Transport>, Report> {
    let transport = Arc::new(
        RpcTransport::new(zmq_context, kill_switch, events_listen, curve_secret_key)
            .map_err(|e| eyre!("Failed to create RPC transport: {}", e))?,
    ) as Arc<dyn Transport>;
    Ok(transport)
}

/// Invoke the server_started hook if it exists
fn invoke_server_started_hook(
    scheduler_client: &SchedulerClient,
    rpc_server: &Arc<RpcServer>,
) -> Result<(), Report> {
    let player = Obj::mk_id(-1);
    let Ok(session) = rpc_server.clone().mk_background_session(&player) else {
        error!("Failed to create background session for server_started hook");
        return Ok(());
    };

    let server_started_verb = Symbol::mk("server_started");
    let Ok(task_handle) = scheduler_client.submit_verb_task(
        &player,
        &ObjectRef::Id(SYSTEM_OBJECT),
        server_started_verb,
        List::mk_list(&[]),
        String::new(),
        &SYSTEM_OBJECT,
        session,
    ) else {
        debug!("No server_started verb found, skipping hook");
        return Ok(());
    };

    info!(
        "Server started hook submitted successfully, task_id: {:?}",
        task_handle.task_id()
    );

    // Poll for completion with timeout
    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);

    loop {
        let Ok((received_task_id, result)) = task_handle
            .receiver()
            .recv_timeout(std::time::Duration::from_millis(100))
        else {
            // Timeout, check if we've exceeded our overall timeout
            if start_time.elapsed() > timeout {
                warn!("Server started hook timed out after 30 seconds");
                break;
            }
            // Continue polling
            continue;
        };

        if received_task_id != task_handle.task_id() {
            continue;
        }

        match result {
            Ok(task_result) => match task_result {
                moor_kernel::tasks::TaskNotification::Result(value) => {
                    info!(
                        "Server started hook completed successfully with result: {:?}",
                        value
                    );
                }
                moor_kernel::tasks::TaskNotification::Suspended => {
                    // Ignore suspension notifications; keep waiting for completion.
                    continue;
                }
            },
            Err(e) => {
                warn!("Server started hook failed with error: {:?}", e);
            }
        }
        break;
    }
    Ok(())
}

/// Host for the moor runtime.
///   * Brings up the database
///   * Instantiates a scheduler
///   * Exposes RPC interface for session/connection management.
fn main() -> Result<(), Report> {
    color_eyre::install()?;

    let args = Args::parse();
    let enrollment_token_path = args.resolved_enrollment_token_path();

    let version = semver::Version::parse(build::PKG_VERSION)
        .map_err(|e| eyre!("Invalid moor version '{}': {}", build::PKG_VERSION, e))?;

    eprintln!("Initializing...\n{BANNER_MSG}");

    tracing::init_tracing(args.debug).map_err(|e| eyre!("Unable to configure logging: {}", e))?;

    // If rotate-enrollment-token flag is provided, rotate token and exit
    if args.rotate_enrollment_token {
        enrollment::rotate_enrollment_token(&enrollment_token_path)?;
        return Ok(());
    }

    // Resolve key paths using XDG config defaults and ensure config dir exists securely
    let public_key_path = args.resolved_public_key_path();
    let private_key_path = args.resolved_private_key_path();
    let config_dir = moor_common::util::config_dir();
    std::fs::create_dir_all(&config_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&config_dir)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&config_dir, perms)?;
    }
    // Check the public/private keypair file to see if it exists. If it does, parse it and establish
    // the keypair from it. If generate-keypair flag is set and keys don't exist, generate them.
    let (private_key, public_key) = if public_key_path.exists() && private_key_path.exists() {
        info!(
            "Loading existing keypair from {}/{}",
            public_key_path.display(),
            private_key_path.display()
        );
        load_keypair(&public_key_path, &private_key_path).map_err(|e| {
            eyre!(
                "Unable to load keypair from public and private key files: {}",
                e
            )
        })?
    } else if args.generate_keypair {
        // Generate keypair if flag is set and files don't exist
        generate_keypair(&public_key_path, &private_key_path)?;
        info!(
            "Generated keypair to {} / {}",
            public_key_path.display(),
            private_key_path.display()
        );
        load_keypair(&public_key_path, &private_key_path)
            .map_err(|e| eyre!("Unable to load generated keypair: {}", e))?
    } else {
        bail!(
            "Public ({:?}) and/or private ({:?}) key files must exist. Use --generate-keypair to create them.",
            public_key_path,
            private_key_path
        );
    };

    // Derive symmetric key for PASETO V4.Local tokens from the Ed25519 private key
    let mut hasher = Sha256::new();
    hasher.update(private_key.as_slice());
    let server_symmetric_key: [u8; 32] = hasher.finalize().into();

    // Initialize the server's symmetric key in the kernel
    moor_kernel::initialize_server_symmetric_key(server_symmetric_key)
        .map_err(|e| eyre!("Failed to initialize server symmetric key: {}", e))?;

    info!("Server symmetric key initialized for PASETO operations");

    // Acquire exclusive lock on the data directory to prevent multiple daemon instances
    let _data_dir_lock = acquire_data_directory_lock(&args.data_dir)?;

    // Generate or load CURVE keypair for daemon
    let daemon_curve_keypair = curve_keys::load_or_generate_daemon_keypair(&args.data_dir)?;
    info!("Daemon CURVE keys are initialized");

    // Ensure enrollment token exists (or generate it)
    let _enrollment_token = enrollment::ensure_enrollment_token(&enrollment_token_path)?;

    // Initialize allowed hosts registry
    let allowed_hosts_registry =
        allowed_hosts::AllowedHostsRegistry::from_dir(&args.resolved_allowed_hosts_dir())?;

    let (phys_cores, logical_cores) = (
        gdt_cpus::num_physical_cores()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        gdt_cpus::num_logical_cores()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
    );

    let config = args.load_config()?;

    // Initialize tracing if trace output is specified and feature is enabled
    #[cfg(feature = "trace_events")]
    {
        let trace_output_path = args.resolved_trace_output_path();
        if let Some(trace_path) = trace_output_path {
            if moor_kernel::init_tracing(Some(trace_path.clone())) {
                info!(
                    "Chrome trace events enabled, output will be written to: {:?}",
                    trace_path
                );
            } else {
                warn!("Failed to initialize Chrome trace events");
            }
        } else {
            info!("Chrome trace events disabled (no --trace-output specified)");
        }
    }

    let resolved_db_path = args.resolved_db_path();
    info!(
        "moor {version} (commit: {}) daemon starting. {phys_cores} physical cores; {logical_cores} logical cores. Using database at {resolved_db_path:?}",
        build::short_commit()
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
            let import_format_name = match &config.import_export.import_format {
                ImportFormat::Objdef => "objdef",
                ImportFormat::Textdump => "textdump",
            };
            info!("Loading {} from {:?}", import_format_name, import_path);
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

                // Delete only the database file/directory if the import fails since it was freshly created.
                // We don't want to delete the entire data directory as it may contain other databases
                // (connections, tasks, event logs) and system data (keys, enrollment tokens).
                let cleanup_result = if resolved_db_path.is_dir() {
                    std::fs::remove_dir_all(&resolved_db_path)
                } else {
                    std::fs::remove_file(&resolved_db_path)
                };

                if let Err(e) = cleanup_result {
                    panic!(
                        "Failed to remove database {:?} after import failure: {}",
                        resolved_db_path, e
                    );
                } else {
                    info!(
                        "Removed failed database {:?} after import failure",
                        resolved_db_path
                    );
                }

                exit(1);
            }
            // Import succeeded - mark all relations as fully loaded to skip provider I/O
            database.mark_all_fully_loaded();
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

    // Check if we need CURVE encryption (only for TCP endpoints, not IPC)
    let use_curve =
        args.rpc_listen.starts_with("tcp://") || args.events_listen.starts_with("tcp://");

    if use_curve {
        // Start ZAP authentication handler for CURVE
        // IMPORTANT: This must be started before any CURVE-enabled sockets are created
        info!("TCP endpoints detected - enabling CURVE encryption with ZAP authentication");
        let zap_handler = zap_auth::ZapAuthHandler::new(
            zmq_ctx.clone(),
            kill_switch.clone(),
            allowed_hosts_registry.clone(),
        );
        std::thread::Builder::new()
            .name("moor-zap-auth".to_string())
            .spawn(move || {
                if let Err(e) = zap_handler.run() {
                    error!(error = ?e, "ZAP authentication handler failed");
                }
            })
            .map_err(|e| eyre!("Failed to spawn ZAP authentication thread: {}", e))?;

        // Give the ZAP handler time to bind before creating CURVE sockets
        std::thread::sleep(std::time::Duration::from_millis(100));
    } else {
        info!("IPC endpoints detected - CURVE encryption disabled (using filesystem permissions)");
    }

    // Create the RPC transport with optional CURVE encryption
    let rpc_transport = create_rpc_transport(
        zmq_ctx.clone(),
        kill_switch.clone(),
        args.events_listen.as_str(),
        if use_curve {
            Some(daemon_curve_keypair.secret.clone())
        } else {
            None
        },
    )?;

    // Create the event log based on configuration
    let event_log: Arc<dyn EventLogOps> = if config.features.enable_eventlog {
        Arc::new(EventLog::with_config(
            EventLogConfig::default(),
            Some(&resolved_events_db_path),
        ))
    } else {
        info!("Event logging is disabled - using no-op implementation");
        Arc::new(NoOpEventLog::new())
    };

    let (rpc_server, task_monitor, system_control) = RpcServer::new(
        kill_switch.clone(),
        public_key.clone(),
        private_key.clone(),
        connections,
        event_log.clone(),
        rpc_transport,
        config.clone(),
        if use_curve {
            Some(enrollment_token_path.clone())
        } else {
            None
        },
    );
    let rpc_server = Arc::new(rpc_server);

    let (worker_scheduler_send, worker_scheduler_recv) = flume::unbounded();

    // Workers RPC server
    let mut workers_server = WorkersServer::new(
        kill_switch.clone(),
        zmq_ctx.clone(),
        &args.workers_request_listen,
        worker_scheduler_send,
        if use_curve {
            Some(daemon_curve_keypair.secret.clone())
        } else {
            None
        },
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

    // Enrollment server for host registration (only needed for CURVE/TCP)
    if use_curve {
        let enrollment_server = enrollment::EnrollmentServer::new(
            zmq_ctx.clone(),
            kill_switch.clone(),
            daemon_curve_keypair.public.clone(),
            allowed_hosts_registry.clone(),
            enrollment_token_path.clone(),
        );
        let enrollment_listen_addr = args.enrollment_listen.clone();
        std::thread::Builder::new()
            .name("moor-enrollment".to_string())
            .spawn(move || {
                if let Err(e) = enrollment_server.listen(&enrollment_listen_addr) {
                    error!(
                        "Enrollment server failed to listen on {}: {}",
                        enrollment_listen_addr, e
                    );
                }
            })?;
    }

    // The pieces from core we're going to use:
    //   Our DB.
    //   Our scheduler.
    let mut scheduler = Scheduler::new(
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

    // Background DB checkpoint thread
    (|| -> Result<(), Report> {
        let Some(output_path) = config.import_export.output_path.clone() else {
            info!("Checkpointing disabled - no output path configured.");
            return Ok(());
        };

        let Some(checkpoint_interval) =
            scheduler.get_checkpoint_interval(config.import_export.checkpoint_interval)
        else {
            info!("Checkpointing disabled - no interval configured.");
            return Ok(());
        };

        let checkpoint_kill_switch = kill_switch.clone();
        let checkpoint_scheduler_client = scheduler_client.clone();
        info!(
            "Checkpointing enabled to {}. Interval: {:?}",
            output_path.display(),
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
        Ok(())
    })()?;

    // The scheduler thread:
    let scheduler_rpc_server = rpc_server.clone();
    let scheduler_loop_jh = std::thread::Builder::new()
        .name("moor-scheduler".to_string())
        .spawn(move || scheduler.run(scheduler_rpc_server))?;

    // Invoke server_started hook if it exists
    invoke_server_started_hook(&scheduler_client, &rpc_server)?;

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
        version = build::PKG_VERSION,
        commit = build::short_commit(),
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

    // Shutdown tracing to flush any remaining events
    #[cfg(feature = "trace_events")]
    {
        moor_kernel::shutdown_tracing();
    }

    info!("Done.");
    Ok(())
}
