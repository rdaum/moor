#![recursion_limit = "256"]
// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use eyre::{bail, eyre};
use std::io::IsTerminal;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
};

use crate::{
    connections::ConnectionRegistryFactory,
    event_log::{EventLog, EventLogConfig, EventLogOps, NoOpEventLog},
    rpc::transport::RpcTransport,
    workers::WorkersServer,
};
use ::tracing::{debug, error, info, warn};
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::{
    SigningKey,
    pkcs8::{EncodePrivateKey, EncodePublicKey},
};
use eyre::Report;
use moor_common::{build, model::ObjectRef, tasks::SessionFactory, threading::spawn_efficient};
use moor_compiler::emit_compile_error;
use moor_db::{Database, TxDB};
use moor_kernel::{
    SchedulerClient,
    config::{Config, ImportFormat},
    tasks::{NoopTasksDb, TasksDb, scheduler::Scheduler},
};
use moor_objdef::ObjectDefinitionLoader;
use moor_textdump::{TextdumpImportOptions, textdump_load};
use moor_var::{List, Obj, SYSTEM_OBJECT, Symbol, v_empty_str};
use rand::RngExt;
use rusty_paseto::core::Key;
use sha2::{Digest, Sha256};

mod allowed_hosts;
pub mod connections;
mod curve_keys;
mod enrollment;
mod event_log;
mod rpc;
mod runtime;
mod system_control;
mod tasks;
#[cfg(test)]
mod testing;
mod workers;
mod zap_auth;

use crate::tasks::tasks_db_fjall::FjallTasksDB;
use moor_common::model::{CommitResult, CompileError, loader::LoaderInterface};

pub use curve_keys::CurveKeyPair;
pub use rpc::{MessageHandler, RpcServer, Transport};
pub use runtime::RuntimeApi;
pub use workers::WorkersMessageHandlerImpl;

pub type ReadySignal = Arc<Mutex<Option<oneshot::Sender<()>>>>;

pub(crate) fn signal_ready(ready_signal: &Option<ReadySignal>) {
    let Some(ready_signal) = ready_signal else {
        return;
    };

    let Ok(mut sender) = ready_signal.lock() else {
        return;
    };

    if let Some(sender) = sender.take() {
        let _ = sender.send(());
    }
}

#[derive(Clone)]
pub struct DaemonPaths {
    pub data_dir: PathBuf,
    pub connections_db_path: Option<PathBuf>,
    pub tasks_db_path: PathBuf,
    pub events_db_path: PathBuf,
}

#[derive(Clone)]
pub struct DaemonEndpoints {
    pub rpc_listen: String,
    pub events_listen: String,
    pub workers_request_listen: String,
    pub workers_response_listen: String,
    pub enrollment_listen: String,
}

#[derive(Clone)]
pub struct DaemonKeys {
    pub private_key: Key<64>,
    pub public_key: Key<32>,
    pub curve_keypair: CurveKeyPair,
    pub enrollment_token_path: PathBuf,
    pub allowed_hosts_dir: PathBuf,
}

#[derive(Clone)]
pub struct DaemonRuntimeConfig {
    pub version: semver::Version,
    pub config: Arc<Config>,
    pub storage: moor_db::StorageConfig,
    pub paths: DaemonPaths,
    pub endpoints: DaemonEndpoints,
    pub keys: DaemonKeys,
    pub num_io_threads: i32,
    pub workers_enabled: bool,
    #[cfg(feature = "trace_events")]
    pub trace_output_path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct DaemonRuntime {
    pub zmq_context: r0z::Context,
    pub kill_switch: Arc<AtomicBool>,
    pub emergency_checkpoint: Option<Arc<AtomicBool>>,
    pub ready_signal: Option<ReadySignal>,
    pub local_transport: Option<Arc<dyn Transport>>,
    pub local_runtime_sender: Option<flume::Sender<LocalRuntimeInit>>,
    pub local_worker_handler_sender: Option<flume::Sender<Arc<WorkersMessageHandlerImpl>>>,
}

impl Default for DaemonRuntime {
    fn default() -> Self {
        Self {
            zmq_context: r0z::Context::new(),
            kill_switch: Arc::new(AtomicBool::new(false)),
            emergency_checkpoint: None,
            ready_signal: None,
            local_transport: None,
            local_runtime_sender: None,
            local_worker_handler_sender: None,
        }
    }
}

pub struct LocalRuntimeInit {
    pub runtime_api: Arc<dyn RuntimeApi>,
    pub scheduler_client: SchedulerClient,
}

pub const VERSION_BANNER_MSG: &str = r#"
                                   ███████████
                                  ░░███░░░░░███
 █████████████    ██████   ██████  ░███    ░███
░░███░░███░░███  ███░░███ ███░░███ ░██████████
 ░███ ░███ ░███ ░███ ░███░███ ░███ ░███░░░░░███
 ░███ ░███ ░███ ░███ ░███░███ ░███ ░███    ░███
 █████░███ █████░░██████ ░░██████  █████   █████
░░░░░ ░░░ ░░░░░  ░░░░░░   ░░░░░░  ░░░░░   ░░░░░



        ████████        █████
       ███░░░░███     ███░░░███
      ░░░    ░███    ███   ░░███
         ███████    ░███    ░███
        ███░░░░     ░███    ░███
       ███      █   ░░███   ███
      ░██████████ ██ ░░░█████░
      ░░░░░░░░░░ ░░    ░░░░░░
 "#;

/// Acquire an exclusive lock on the data directory to prevent multiple daemon instances
/// from operating on the same data.
fn acquire_data_directory_lock(data_dir: &PathBuf) -> Result<File, Report> {
    // Create the data directory if it doesn't exist
    fs::create_dir_all(data_dir)?;

    // Create a lock file in the data directory
    let lock_file_path = data_dir.join(".moor-daemon.lock");

    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&lock_file_path)?;

    // Try to acquire exclusive lock
    match lock_file.try_lock() {
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
    import_path: &Path,
    mut loader_interface: Box<dyn LoaderInterface>,
    version: semver::Version,
) -> Result<bool, Report> {
    let start = std::time::Instant::now();
    // We have two ways of loading textdump.
    // legacy "textdump" format from LambdaMOO,
    // or our own exploded objdef format.
    let commit = match &config.import_export.import_format {
        ImportFormat::Objdef => {
            let mut od = ObjectDefinitionLoader::new(loader_interface.as_mut());
            let options = moor_objdef::ObjDefLoaderOptions::default();
            let prepared = moor_objdef::ObjDefSet::read_directory_with_baseline(
                &config.features.compile_options(),
                import_path,
                config.import_export.git_upstream.as_deref(),
                config.import_export.baseline_objdef_dir.as_deref(),
            );
            let results = match prepared.and_then(|set| od.load_objdef_set(set, options)) {
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
                import_path.to_path_buf(),
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
                info!("Import transaction published in {:?}", start.elapsed());
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
    Ok(commit)
}

/// Generate ED25519 keypair and write to PEM files
pub fn generate_keypair(
    public_key_path: &PathBuf,
    private_key_path: &PathBuf,
) -> Result<(), Report> {
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

pub fn load_or_generate_daemon_curve_keypair(
    data_dir: &std::path::Path,
) -> Result<CurveKeyPair, Report> {
    curve_keys::load_or_generate_daemon_keypair(data_dir)
}

pub fn ensure_enrollment_token(token_path: &std::path::Path) -> Result<String, Report> {
    enrollment::ensure_enrollment_token(token_path)
}

pub fn rotate_enrollment_token(token_path: &std::path::Path) -> Result<String, Report> {
    enrollment::rotate_enrollment_token(token_path)
}

/// Create the RPC transport layer for production use
fn create_rpc_transport(
    zmq_context: r0z::Context,
    kill_switch: Arc<AtomicBool>,
    events_listen: &str,
    rpc_listen: &str,
    curve_secret_key: Option<String>,
    ready_signal: Option<ReadySignal>,
) -> Result<Arc<dyn Transport>, Report> {
    let transport = Arc::new(
        RpcTransport::new(
            zmq_context,
            kill_switch,
            events_listen,
            rpc_listen,
            curve_secret_key,
            ready_signal,
        )
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
        v_empty_str(),
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
pub fn run(runtime_config: DaemonRuntimeConfig, runtime: DaemonRuntime) -> Result<(), Report> {
    let DaemonRuntime {
        zmq_context,
        kill_switch: shutdown_requested,
        emergency_checkpoint,
        ready_signal,
        local_transport,
        local_runtime_sender,
        local_worker_handler_sender,
    } = runtime;
    // External callers and signal handlers request an orderly shutdown. Components keep
    // running until the scheduler has cancelled and drained its active tasks.
    let kill_switch = Arc::new(AtomicBool::new(false));
    let DaemonRuntimeConfig {
        version,
        config,
        storage,
        paths,
        endpoints,
        keys,
        num_io_threads,
        workers_enabled,
        #[cfg(feature = "trace_events")]
        trace_output_path,
    } = runtime_config;
    config.validate().map_err(|e| eyre!(e))?;
    let persistence = config.storage.persistence_config()?;
    let DaemonKeys {
        private_key,
        public_key,
        curve_keypair: daemon_curve_keypair,
        enrollment_token_path,
        allowed_hosts_dir,
    } = keys;

    // Derive symmetric key for PASETO V4.Local tokens from the Ed25519 private key
    let mut hasher = Sha256::new();
    hasher.update(private_key.as_slice());
    let server_symmetric_key: [u8; 32] = hasher.finalize().into();

    // Initialize the server's symmetric key in the kernel
    moor_kernel::initialize_server_symmetric_key(server_symmetric_key)
        .map_err(|e| eyre!("Failed to initialize server symmetric key: {}", e))?;

    info!("Server symmetric key initialized for PASETO operations");

    // Acquire exclusive lock on the data directory to prevent multiple daemon instances
    let _data_dir_lock = acquire_data_directory_lock(&paths.data_dir)?;

    let allowed_hosts_registry = allowed_hosts::AllowedHostsRegistry::from_dir(&allowed_hosts_dir)?;

    let (phys_cores, logical_cores) = (
        moor_common::threading::physical_core_count()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        moor_common::threading::logical_core_count().to_string(),
    );

    let mut affinity_config = moor_common::threading::TaskPoolAffinityConfig::default();
    if let Some(pinning_mode) = config.runtime.task_pool_pinning {
        affinity_config.pinning_mode = pinning_mode;
    }
    affinity_config.service_perf_cores = config.runtime.service_perf_cores;
    moor_common::threading::set_task_pool_affinity_config(affinity_config);
    let reserved_worker_perf_cores = moor_common::threading::worker_performance_core_ids_ref();
    let reserved_service_perf_cores = moor_common::threading::service_performance_core_ids_ref();

    // Initialize tracing if trace output is specified and feature is enabled
    #[cfg(feature = "trace_events")]
    {
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

    info!(
        storage_backend = %storage.kind(),
        commit = build::short_commit(),
        "moor {version} daemon starting. {phys_cores} physical cores; {logical_cores} logical cores."
    );
    info!(
        worker_perf_cores = ?reserved_worker_perf_cores,
        service_perf_cores = ?reserved_service_perf_cores,
        task_pool_pinning = ?config.runtime.task_pool_pinning,
        configured_service_perf_cores = config.runtime.service_perf_cores,
        "Thread core reservations initialized"
    );
    let fjall_path = match &storage {
        moor_db::StorageConfig::Fjall(fjall) => {
            info!(path = ?fjall.path, "Opening Fjall world");
            fjall.path.clone()
        }
        #[cfg(feature = "postgres")]
        moor_db::StorageConfig::Postgres(postgres) => {
            info!(
                schema = postgres.schema.as_str(),
                "Opening PostgreSQL world"
            );
            None
        }
    };
    let (database, freshly_made) = TxDB::try_open(
        storage,
        config.database.clone().unwrap_or_default(),
        persistence,
    )?;
    let database = Box::new(database);
    info!("Opened world database");

    if let Some(import_path) = config.import_export.input_path.as_ref() {
        // If the database already existed, do not try to import the textdump...
        if !freshly_made {
            info!("Database already exists, skipping import");
        } else {
            let import_format_name = match &config.import_export.import_format {
                ImportFormat::Objdef => "objdef",
                ImportFormat::Textdump => "textdump",
            };
            info!("Loading {} from {:?}", import_format_name, import_path);
            let loader_interface = database
                .loader_client()
                .map_err(|e| eyre!("Unable to get loader interface from database: {}", e))?;

            let import_result = perform_import(
                config.as_ref(),
                import_path,
                loader_interface,
                version.clone(),
            )
            .and_then(|committed| {
                if !committed {
                    return Ok(());
                }
                info!("Waiting for imported world state to be applied to storage");
                database.wait_for_persistence().map_err(|error| {
                    eyre!("Failed to apply imported world state to storage: {error}")
                })?;
                info!("Imported world state applied to storage");
                Ok(())
            });

            if let Err(import_error) = import_result {
                error!("Import failed: {}", import_error);

                drop(database);
                // Only a newly created Fjall world belongs to this local cleanup path.
                if let Some(path) = fjall_path {
                    let cleanup_result = if path.is_dir() {
                        fs::remove_dir_all(&path)
                    } else {
                        fs::remove_file(&path)
                    };
                    if let Err(error) = cleanup_result {
                        return Err(import_error.wrap_err(format!(
                            "Failed to remove newly created world {path:?}: {error}"
                        )));
                    }
                    info!(?path, "Removed world after failed initial import");
                }

                return Err(import_error);
            }
        }
    }

    let tasks_db: Box<dyn TasksDb> = if config.features.persistent_tasks {
        Box::new(FjallTasksDB::open(&paths.tasks_db_path).0)
    } else {
        Box::new(NoopTasksDb {})
    };

    // We have to create the RpcServer before starting the scheduler because we need to pass it in
    // as a parameter to the scheduler for background session construction.
    let zmq_ctx = zmq_context;
    zmq_ctx.set_io_threads(num_io_threads).map_err(|e| {
        eyre!(
            "Failed to set number of IO threads to {}: {}",
            num_io_threads,
            e
        )
    })?;

    // Create the connections registry based on args/config
    let connections = match paths.connections_db_path.as_ref() {
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
    // Check if we need CURVE encryption (only for TCP endpoints, not IPC)
    // Endpoints can be comma-separated, so check if any of them is TCP
    let has_tcp_endpoint =
        |endpoints: &str| endpoints.split(',').any(|e| e.trim().starts_with("tcp://"));
    let use_curve =
        has_tcp_endpoint(&endpoints.rpc_listen) || has_tcp_endpoint(&endpoints.events_listen);
    let use_curve_for_workers = workers_enabled
        && (has_tcp_endpoint(&endpoints.workers_request_listen)
            || has_tcp_endpoint(&endpoints.workers_response_listen));

    if use_curve {
        // Start ZAP authentication handler for CURVE
        // IMPORTANT: This must be started before any CURVE-enabled sockets are created
        info!("TCP endpoints detected - enabling CURVE encryption with ZAP authentication");
        let zap_handler = zap_auth::ZapAuthHandler::new(
            zmq_ctx.clone(),
            kill_switch.clone(),
            allowed_hosts_registry.clone(),
        );
        spawn_efficient("moor-zap-auth", move || {
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

    let using_local_transport = local_transport.is_some();
    let rpc_transport = if let Some(local_transport) = local_transport {
        local_transport
    } else {
        create_rpc_transport(
            zmq_ctx.clone(),
            kill_switch.clone(),
            endpoints.events_listen.as_str(),
            endpoints.rpc_listen.as_str(),
            if use_curve {
                Some(daemon_curve_keypair.secret.clone())
            } else {
                None
            },
            ready_signal.clone(),
        )?
    };

    // Create the event log based on configuration
    let event_log: Arc<dyn EventLogOps> = if config.features.enable_eventlog {
        Arc::new(EventLog::with_config(
            EventLogConfig::default(),
            Some(&paths.events_db_path),
        ))
    } else {
        info!("Event logging is disabled - using no-op implementation");
        Arc::new(NoOpEventLog::new())
    };

    let (workers_sender, worker_scheduler_recv, worker_info_source) = if workers_enabled {
        let (worker_scheduler_send, worker_scheduler_recv) = flume::unbounded();
        let using_local_workers = local_worker_handler_sender.is_some();
        let workers_message_handler = if using_local_workers {
            Arc::new(workers::WorkersMessageHandlerImpl::new_local(
                worker_scheduler_send,
            ))
        } else {
            Arc::new(
                workers::WorkersMessageHandlerImpl::new_zmq(
                    zmq_ctx.clone(),
                    &endpoints.workers_request_listen,
                    worker_scheduler_send,
                    if use_curve_for_workers {
                        Some(daemon_curve_keypair.secret.clone())
                    } else {
                        None
                    },
                )
                .map_err(|e| eyre!("Failed to create workers message handler: {e}"))?,
            )
        };

        let mut workers_server = WorkersServer::new(
            kill_switch.clone(),
            zmq_ctx.clone(),
            workers_message_handler.clone(),
            if use_curve_for_workers {
                Some(daemon_curve_keypair.secret.clone())
            } else {
                None
            },
        );
        let workers_sender = workers_server.start().map_err(|e| {
            eyre!(
                "Failed to start workers server on {}: {}",
                endpoints.workers_request_listen,
                e
            )
        })?;

        if let Some(local_worker_handler_sender) = local_worker_handler_sender {
            let _ = local_worker_handler_sender.send(workers_message_handler.clone());
        } else {
            let workers_listen_addr = endpoints.workers_response_listen.clone();
            spawn_efficient("moor-workers-listen", move || {
                if let Err(e) = workers_server.listen(&workers_listen_addr) {
                    error!(
                        "Workers server failed to listen on {}: {}",
                        workers_listen_addr, e
                    );
                }
            })?;
        }

        (
            Some(workers_sender),
            Some(worker_scheduler_recv),
            workers_message_handler as Arc<dyn system_control::WorkerInfoSource>,
        )
    } else {
        info!("External worker transport is disabled");
        (
            None,
            None,
            Arc::new(system_control::NoopWorkerInfoSource)
                as Arc<dyn system_control::WorkerInfoSource>,
        )
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
        worker_info_source,
    );
    let rpc_server = Arc::new(rpc_server);

    // Enrollment server for host registration (only needed for CURVE/TCP)
    if use_curve {
        let enrollment_server = enrollment::EnrollmentServer::new(
            zmq_ctx.clone(),
            kill_switch.clone(),
            daemon_curve_keypair.public.clone(),
            allowed_hosts_registry.clone(),
            enrollment_token_path.clone(),
        );
        let enrollment_listen_addr = endpoints.enrollment_listen.clone();
        spawn_efficient("moor-enrollment", move || {
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
    let scheduler = Scheduler::new(
        version,
        database,
        tasks_db,
        config.clone(),
        Arc::new(system_control),
        workers_sender,
        worker_scheduler_recv,
    );
    let scheduler_client = scheduler.client().unwrap();

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

        spawn_efficient("moor-checkpoint", move || {
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

    // Start the scheduler (spawns timer + worker-response threads internally).
    let scheduler_loop_jh = scheduler
        .start(rpc_server.clone())
        .map_err(|e| eyre!("Failed to start scheduler: {e}"))?;

    let shutdown_watcher_requested = shutdown_requested.clone();
    let shutdown_watcher_kill_switch = kill_switch.clone();
    let shutdown_watcher_scheduler = scheduler_client.clone();
    spawn_efficient("moor-shutdown", move || {
        loop {
            if shutdown_watcher_kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            if shutdown_watcher_requested.load(std::sync::atomic::Ordering::Relaxed) {
                if let Err(e) = shutdown_watcher_scheduler.submit_shutdown("System shutting down") {
                    error!("Failed to submit shutdown request: {e}");
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    })?;

    // Invoke server_started hook if it exists
    invoke_server_started_hook(&scheduler_client, &rpc_server)?;

    if let Some(local_runtime_sender) = local_runtime_sender {
        let _ = local_runtime_sender.send(LocalRuntimeInit {
            runtime_api: rpc_server.runtime_api(),
            scheduler_client: scheduler_client.clone(),
        });
    }

    let rpc_loop_scheduler_client = scheduler_client.clone();
    let rpc_listen = endpoints.rpc_listen.clone();
    let rpc_loop_thread = spawn_efficient("moor-rpc", move || {
        if let Err(e) =
            rpc_server.request_loop(rpc_listen.clone(), rpc_loop_scheduler_client, task_monitor)
        {
            error!("RPC server failed on {}: {}", rpc_listen, e);
        }
    })?;

    if using_local_transport {
        signal_ready(&ready_signal);
    }

    if let Some(emergency_checkpoint) = emergency_checkpoint {
        let sigusr1_kill_switch = kill_switch.clone();
        let sigusr1_shutdown_requested = shutdown_requested.clone();
        let sigusr1_scheduler_client = scheduler_client.clone();
        spawn_efficient("moor-sigusr1", move || {
            loop {
                if sigusr1_kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                if emergency_checkpoint.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    warn!("SIGUSR1 received: Performing emergency checkpoint before shutdown...");
                    match sigusr1_scheduler_client.request_checkpoint_blocking() {
                        Ok(()) => {
                            info!("Emergency checkpoint completed successfully.");
                        }
                        Err(e) => {
                            error!(
                                "Emergency checkpoint failed: {}. Proceeding with shutdown anyway.",
                                e
                            );
                        }
                    }
                    sigusr1_shutdown_requested.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        })?;
    }

    info!(
        version = build::PKG_VERSION,
        commit = build::short_commit(),
        rpc_endpoint = endpoints.rpc_listen,
        events_endpoint = endpoints.events_listen,
        "Daemon started. Listening for RPC events."
    );
    if let Err(e) = rpc_loop_thread.join() {
        error!("RPC thread panicked: {:?}", e);
    }
    warn!("RPC thread exited. Departing...");

    // A transport can also stop independently of an external shutdown request. In that
    // case the scheduler is still running and must be stopped before its threads are joined.
    if scheduler_client.check_status().is_ok()
        && let Err(e) = scheduler_client.submit_shutdown("System shutting down")
    {
        error!("Failed to send shutdown signal to scheduler: {e}");
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

#[cfg(test)]
mod directory_lock_tests;

#[cfg(test)]
mod import_tests {
    use super::*;
    use moor_common::{
        model::{HasUuid, ObjFlag, TaskPermissions, WorldStateSource},
        util::BitEnum,
    };
    use moor_objdef::{ObjDefSet, fingerprint::program_fingerprint};
    use moor_var::{Associative, v_str};

    #[test]
    fn startup_import_uses_separate_baseline_and_preserves_live_program() {
        let source = "object #1 [import_export_id -> \"root\"]\nowner: #1\nwizard: true\nverb test (this none this) owner: #1 flags: \"rxd\"\nreturn 1;\nendverb\nendobject\n";
        let baseline = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        fs::write(baseline.path().join("root.moo"), source).unwrap();
        fs::write(
            local.path().join("root.moo"),
            source.replace("return 1", "return 2"),
        )
        .unwrap();
        let mut config = Config::default();
        config.import_export.input_path = Some(local.path().to_path_buf());
        config.import_export.import_format = ImportFormat::Objdef;
        config.import_export.baseline_objdef_dir = Some(baseline.path().to_path_buf());
        config.validate().unwrap();
        let db = TxDB::try_open_temporary(Default::default()).unwrap().0;
        assert!(
            perform_import(
                &config,
                local.path(),
                db.loader_client().unwrap(),
                semver::Version::new(2, 0, 0),
            )
            .unwrap()
        );

        let object = Obj::mk_id(1);
        let permissions = TaskPermissions::new(object, BitEnum::new_with(ObjFlag::Wizard));
        let world = db.new_world_state().unwrap();
        let verb = world
            .get_verb(&permissions, &object, Symbol::mk("test"))
            .unwrap();
        let hash = |path: &std::path::Path| {
            let set = ObjDefSet::read_directory(&config.features.compile_options(), path).unwrap();
            program_fingerprint(&set.graph().object_definitions()[&object].1.verbs[0].program)
                .unwrap()
        };
        let metadata = world
            .get_verb_metadata(
                &permissions,
                &object,
                verb.uuid(),
                Symbol::mk("objdef_base"),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            metadata.as_map().unwrap().get(&v_str("program")).unwrap(),
            v_str(&hash(baseline.path()))
        );
        let (program, _) = world
            .retrieve_verb(&permissions, &object, verb.uuid())
            .unwrap();
        assert_eq!(program_fingerprint(&program).unwrap(), hash(local.path()));
    }
}
