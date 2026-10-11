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

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use ::tracing::{error, info, warn};
use clap::Parser;
use clap::builder::ValueHint;
use clap_derive::{Parser, ValueEnum};
use eyre::{Report, bail, eyre};
use mimalloc::MiMalloc;
use moor_common::{build, tracing, util::config_path};
use moor_daemon::{
    DaemonEndpoints, DaemonKeys, DaemonPaths, DaemonRuntime, DaemonRuntimeConfig,
    VERSION_BANNER_MSG, ensure_enrollment_token, generate_keypair,
    load_or_generate_daemon_curve_keypair,
};
use moor_db::DatabaseConfig;
use moor_kernel::config::{
    Config, FeaturesConfig, ImportExportConfig, ImportFormat, RuntimeConfig,
};
use moor_runtime_api::{api::HostServices, load_keypair};
use moor_telnet_host::{HostRuntime as TelnetHostRuntime, TelnetHostConfig, TelnetProtocolConfig};
use moor_web_host::{
    WebHostConfig,
    host::{OAuth2Config, WebRtcConfig},
    routes::{CorsConfig, RateLimitConfig},
};
use serde::{Deserialize, Serialize};
use tokio::{task::JoinHandle, time::timeout};

mod local_runtime;

use local_runtime::{LocalEventBus, LocalRuntimeClient, LocalRuntimeServices, LocalWorkerServices};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const INPROC_RPC_ENDPOINT: &str = "inproc://moor-services-rpc";
const INPROC_EVENTS_ENDPOINT: &str = "inproc://moor-services-events";
const INPROC_ENROLLMENT_ENDPOINT: &str = "inproc://moor-services-enrollment";

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Format {
    Textdump,
    Objdef,
}

impl From<Format> for ImportFormat {
    fn from(value: Format) -> Self {
        match value {
            Format::Textdump => ImportFormat::Textdump,
            Format::Objdef => ImportFormat::Objdef,
        }
    }
}

#[derive(Parser, Debug)]
#[command(version = build::PKG_VERSION)]
struct Args {
    #[arg(
        value_name = "data-dir",
        help = "Directory to store all database files under",
        value_hint = ValueHint::DirPath,
        default_value = "./moor-data"
    )]
    data_dir: PathBuf,

    #[arg(
        long,
        value_name = "config",
        help = "Path to combined moor configuration YAML",
        value_hint = ValueHint::FilePath
    )]
    config_file: Option<PathBuf>,

    #[arg(
        long,
        value_name = "db",
        help = "Main database filename (relative to data-dir if not absolute)",
        value_hint = ValueHint::FilePath,
        default_value = "world.db"
    )]
    db: PathBuf,

    #[arg(
        short,
        long,
        value_name = "import",
        help = "Path to a textdump or objdef directory to import",
        value_hint = ValueHint::FilePath
    )]
    import: Option<PathBuf>,

    #[arg(
        long,
        value_name = "export",
        help = "Path to export checkpoints into",
        value_hint = ValueHint::FilePath
    )]
    export: Option<PathBuf>,

    #[arg(
        long,
        value_name = "import-format",
        help = "Format to import from",
        value_enum
    )]
    import_format: Option<Format>,

    /// Prepare initial program baselines from HEAD's common ancestor with this remote-tracking ref.
    /// Uses local Git history only; fetch the remote first when needed.
    #[arg(
        long,
        env = "MOOR_GIT_UPSTREAM",
        conflicts_with = "baseline_objdef_dir"
    )]
    git_upstream: Option<String>,

    /// Use this objdef directory as the upstream baseline for the initial import.
    #[arg(long, env = "MOOR_BASELINE_OBJDEF_DIR")]
    baseline_objdef_dir: Option<PathBuf>,

    #[arg(
        short,
        long,
        value_name = "connections-db",
        help = "Path to connections database to use or create (relative to data-dir if not absolute)",
        value_hint = ValueHint::FilePath
    )]
    connections_file: Option<PathBuf>,

    #[arg(
        short = 'x',
        long,
        value_name = "tasks-db",
        help = "Path to persistent tasks database to use or create (relative to data-dir if not absolute)",
        value_hint = ValueHint::FilePath
    )]
    tasks_db: Option<PathBuf>,

    #[arg(
        short = 'e',
        long,
        value_name = "events-db",
        help = "Path to persistent events database to use or create (relative to data-dir if not absolute)",
        value_hint = ValueHint::FilePath
    )]
    events_db: Option<PathBuf>,

    #[arg(
        long,
        value_name = "public-key",
        help = "PEM encoded public key. Relative paths resolve under the XDG config directory.",
        value_hint = ValueHint::FilePath
    )]
    public_key: Option<PathBuf>,

    #[arg(
        long,
        value_name = "private-key",
        help = "PEM encoded private key. Relative paths resolve under the XDG config directory.",
        value_hint = ValueHint::FilePath
    )]
    private_key: Option<PathBuf>,

    #[arg(
        long,
        value_name = "enrollment-token-file",
        help = "Enrollment token file. Relative paths resolve under the XDG config directory.",
        value_hint = ValueHint::FilePath
    )]
    enrollment_token_file: Option<PathBuf>,

    #[arg(
        long,
        value_name = "num-io-threads",
        help = "Number of ZeroMQ IO threads to use",
        default_value = "8"
    )]
    num_io_threads: i32,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,

    #[arg(long, help = "Generate ED25519 keypair if it does not exist")]
    generate_keypair: bool,

    #[arg(long, help = "Disable the telnet listener in single-process mode")]
    no_telnet: bool,

    #[arg(long, help = "Disable the web listener in single-process mode")]
    no_web: bool,

    #[arg(long, help = "Enable the embedded curl worker")]
    enable_curl_worker: bool,

    #[arg(long, help = "Enable the embedded Git worker with isolated child jobs")]
    enable_git_worker: bool,

    #[arg(long, value_name = "address", help = "Telnet listener bind address")]
    telnet_address: Option<String>,

    #[arg(long, value_name = "port", help = "Telnet listener port")]
    telnet_port: Option<u16>,

    #[arg(long, value_name = "address", help = "Web listener bind address")]
    web_listen_address: Option<String>,

    #[cfg(feature = "trace_events")]
    #[arg(
        long,
        value_name = "trace-output",
        help = "Path to output Chrome trace events JSON file",
        value_hint = ValueHint::FilePath
    )]
    trace_output: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct CombinedConfig {
    database: Option<DatabaseConfig>,
    features: Arc<FeaturesConfig>,
    import_export: ImportExportConfig,
    runtime: RuntimeConfig,
    services: ServicesConfig,
}

impl Default for CombinedConfig {
    fn default() -> Self {
        let config = Config::default();
        Self {
            database: config.database,
            features: config.features,
            import_export: config.import_export,
            runtime: config.runtime,
            services: ServicesConfig::default(),
        }
    }
}

impl CombinedConfig {
    fn into_parts(self) -> (Arc<Config>, ServicesConfig) {
        let mut features = self.features.as_ref().clone();
        features.normalize_deprecated_flags();

        let config = Config {
            database: self.database,
            features: Arc::new(features),
            import_export: self.import_export,
            runtime: self.runtime,
        };

        (Arc::new(config), self.services)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[derive(Default)]
struct ServicesConfig {
    telnet: TelnetServiceConfig,
    web: WebServiceConfig,
    curl_worker: CurlWorkerServiceConfig,
    git_worker: GitWorkerServiceConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TelnetServiceConfig {
    enabled: bool,
    address: String,
    port: u16,
    health_check_port: u16,
    tls_port: Option<u16>,
    tls_cert: Option<PathBuf>,
    tls_key: Option<PathBuf>,
    protocols: TelnetProtocolConfig,
}

impl Default for TelnetServiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            address: "0.0.0.0".to_string(),
            port: 8888,
            health_check_port: 9888,
            tls_port: None,
            tls_cert: None,
            tls_key: None,
            protocols: TelnetProtocolConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WebServiceConfig {
    enabled: bool,
    listen_address: String,
    enable_webhooks: bool,
    oauth2: OAuth2Config,
    cors: CorsConfig,
    rate_limit: RateLimitConfig,
    trusted_proxy_cidrs: Vec<String>,
    webrtc: WebRtcConfig,
}

impl Default for WebServiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            listen_address: "0.0.0.0:8080".to_string(),
            enable_webhooks: true,
            oauth2: OAuth2Config::default(),
            cors: CorsConfig::default(),
            rate_limit: RateLimitConfig::default(),
            trusted_proxy_cidrs: vec![],
            webrtc: WebRtcConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[derive(Default)]
struct CurlWorkerServiceConfig {
    enabled: bool,
    health_check_port: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct GitWorkerServiceConfig {
    enabled: bool,
    work_dir: PathBuf,
    max_concurrent_requests: usize,
    limits: moor_git_worker::protocol::Limits,
}

impl Default for GitWorkerServiceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            work_dir: PathBuf::from("git-jobs"),
            max_concurrent_requests: 4,
            limits: Default::default(),
        }
    }
}

fn main() -> Result<(), Report> {
    // Dispatch isolated jobs before starting runtime threads or opening the world database.
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() == Some(std::ffi::OsStr::new("--git-job")) {
        let directory = arguments
            .next()
            .ok_or_else(|| eyre!("Missing Git job directory"))?;
        return moor_git_worker::job::child_main(std::path::Path::new(&directory));
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

async fn run() -> Result<(), Report> {
    color_eyre::install()?;

    let args = Args::parse();
    eprintln!("Initializing...\n{VERSION_BANNER_MSG}");
    tracing::init_tracing(args.debug).map_err(|e| eyre!("Unable to configure logging: {e}"))?;

    let version = semver::Version::parse(build::PKG_VERSION)
        .map_err(|e| eyre!("Invalid moor version '{}': {}", build::PKG_VERSION, e))?;
    let mut combined_config = moor_common::config::apply_yaml_config_file(
        CombinedConfig::default(),
        args.config_file.as_deref(),
    )?;
    apply_cli_overrides(&args, &mut combined_config);
    let (config, services_config) = combined_config.into_parts();
    config.validate().map_err(|e| eyre!(e))?;

    prepare_config_dir()?;
    std::fs::create_dir_all(&args.data_dir)?;

    // Validate limits and prepare job storage before starting the daemon or importing a world.
    let git_executor = if services_config.git_worker.enabled {
        Some(Arc::new(moor_git_worker::job::Executor::new(
            std::env::current_exe()?,
            resolve_data_path(&args.data_dir, &services_config.git_worker.work_dir),
            services_config.git_worker.limits.clone(),
            services_config.git_worker.max_concurrent_requests,
        )?))
    } else {
        None
    };

    let public_key_path = resolved_config_path(args.public_key.as_ref(), "moor-verifying-key.pem");
    let private_key_path = resolved_config_path(args.private_key.as_ref(), "moor-signing-key.pem");
    let enrollment_token_path =
        resolved_config_path(args.enrollment_token_file.as_ref(), "enrollment-token");

    let (private_key, public_key) =
        load_or_create_keypair(&public_key_path, &private_key_path, args.generate_keypair)?;
    let daemon_curve_keypair = load_or_generate_daemon_curve_keypair(&args.data_dir)?;
    let _enrollment_token = ensure_enrollment_token(&enrollment_token_path)?;

    let zmq_context = r0z::Context::new();

    let kill_switch = Arc::new(AtomicBool::new(false));
    let emergency_checkpoint = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, kill_switch.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, kill_switch.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGUSR1, emergency_checkpoint.clone())?;

    let (ready_sender, ready_receiver) = oneshot::channel();
    let (local_runtime_sender, local_runtime_receiver) = flume::bounded(1);
    let (local_worker_handler_sender, local_worker_handler_receiver) = flume::bounded(1);
    let ready_signal = Arc::new(Mutex::new(Some(ready_sender)));
    let local_event_bus = Arc::new(LocalEventBus::new());
    let runtime_config = DaemonRuntimeConfig {
        version,
        config,
        paths: DaemonPaths {
            data_dir: args.data_dir.clone(),
            db_path: resolve_data_path(&args.data_dir, &args.db),
            connections_db_path: Some(resolve_optional_data_path(
                &args.data_dir,
                args.connections_file.as_ref(),
                "connections.db",
            )),
            tasks_db_path: resolve_optional_data_path(
                &args.data_dir,
                args.tasks_db.as_ref(),
                "tasks.db",
            ),
            events_db_path: resolve_optional_data_path(
                &args.data_dir,
                args.events_db.as_ref(),
                "events.db",
            ),
        },
        endpoints: DaemonEndpoints {
            rpc_listen: INPROC_RPC_ENDPOINT.to_string(),
            events_listen: INPROC_EVENTS_ENDPOINT.to_string(),
            workers_request_listen: String::new(),
            workers_response_listen: String::new(),
            enrollment_listen: INPROC_ENROLLMENT_ENDPOINT.to_string(),
        },
        keys: DaemonKeys {
            private_key,
            public_key,
            curve_keypair: daemon_curve_keypair,
            enrollment_token_path,
            allowed_hosts_dir: resolved_allowed_hosts_dir(&args.data_dir),
        },
        num_io_threads: args.num_io_threads,
        workers_enabled: services_config.curl_worker.enabled || services_config.git_worker.enabled,
        #[cfg(feature = "trace_events")]
        trace_output_path: args
            .trace_output
            .as_ref()
            .map(|path| resolve_data_path(&args.data_dir, path)),
    };
    let daemon_runtime = DaemonRuntime {
        zmq_context: zmq_context.clone(),
        kill_switch: kill_switch.clone(),
        emergency_checkpoint: Some(emergency_checkpoint),
        ready_signal: Some(ready_signal),
        local_transport: Some(local_event_bus.clone()),
        local_runtime_sender: Some(local_runtime_sender),
        local_worker_handler_sender: (services_config.curl_worker.enabled
            || services_config.git_worker.enabled)
            .then_some(local_worker_handler_sender),
    };

    let daemon_handle =
        tokio::task::spawn_blocking(move || moor_daemon::run(runtime_config, daemon_runtime));

    wait_for_daemon_ready(&ready_receiver, &daemon_handle).await?;
    let local_runtime_init = local_runtime_receiver
        .recv()
        .map_err(|e| eyre!("Daemon did not publish local runtime services: {e}"))?;
    let local_runtime_services = LocalRuntimeServices {
        runtime_client: Arc::new(LocalRuntimeClient::new(
            local_runtime_init.runtime_api,
            local_runtime_init.scheduler_client,
        )),
        event_bus: local_event_bus,
    };
    let host_services = Arc::new(local_runtime_services) as Arc<dyn HostServices>;

    let mut host_tasks = Vec::new();
    if services_config.telnet.enabled {
        let telnet_config = TelnetHostConfig {
            telnet_address: services_config.telnet.address,
            telnet_port: services_config.telnet.port,
            health_check_port: services_config.telnet.health_check_port,
            tls_port: services_config.telnet.tls_port,
            tls_cert: services_config.telnet.tls_cert,
            tls_key: services_config.telnet.tls_key,
            protocols: services_config.telnet.protocols,
        };
        let telnet_runtime = TelnetHostRuntime {
            kill_switch: kill_switch.clone(),
        };
        let host_services = host_services.clone();
        host_tasks.push(tokio::spawn(async move {
            moor_telnet_host::run_with_services(telnet_config, telnet_runtime, host_services).await
        }));
    }

    if services_config.web.enabled {
        let web_config = WebHostConfig {
            listen_address: services_config.web.listen_address,
            enable_webhooks: services_config.web.enable_webhooks,
            oauth2: services_config.web.oauth2,
            cors: services_config.web.cors,
            rate_limit: services_config.web.rate_limit,
            trusted_proxy_cidrs: services_config.web.trusted_proxy_cidrs,
            webrtc: services_config.web.webrtc,
        };
        let web_runtime = moor_web_host::HostRuntime {
            kill_switch: kill_switch.clone(),
        };
        let host_services = host_services.clone();
        host_tasks.push(tokio::spawn(async move {
            moor_web_host::run_with_services(web_config, web_runtime, host_services).await
        }));
    }

    if services_config.curl_worker.enabled || services_config.git_worker.enabled {
        let worker_handler = local_worker_handler_receiver
            .recv()
            .map_err(|e| eyre!("Daemon did not publish local worker services: {e}"))?;
        let worker_services = Arc::new(LocalWorkerServices::new(worker_handler))
            as Arc<dyn moor_runtime_api::WorkerServices>;
        if services_config.curl_worker.enabled {
            let worker_services = worker_services.clone();
            let curl_worker_runtime = moor_curl_worker::LocalWorkerRuntime {
                kill_switch: kill_switch.clone(),
            };
            host_tasks.push(tokio::spawn(async move {
                moor_curl_worker::run_with_services(
                    services_config.curl_worker.health_check_port,
                    curl_worker_runtime,
                    worker_services,
                )
                .await
            }));
        }
        if let Some(executor) = git_executor {
            let kill_switch = kill_switch.clone();
            host_tasks.push(tokio::spawn(async move {
                moor_git_worker::run_with_services(executor, kill_switch, worker_services).await
            }));
        }
    }

    if host_tasks.is_empty() {
        warn!("No telnet, web, or worker services are enabled");
    }

    wait_for_exit(kill_switch.clone(), &daemon_handle, &host_tasks).await;
    kill_switch.store(true, Ordering::SeqCst);
    let host_shutdown = shutdown_hosts(host_tasks).await;
    let daemon_shutdown = shutdown_daemon(daemon_handle).await;
    host_shutdown?;
    daemon_shutdown?;

    info!("Done.");
    Ok(())
}

fn apply_cli_overrides(args: &Args, config: &mut CombinedConfig) {
    if let Some(import) = args.import.as_ref() {
        config.import_export.input_path = Some(import.clone());
    }
    if let Some(export) = args.export.as_ref() {
        config.import_export.output_path = Some(export.clone());
    }
    if let Some(import_format) = args.import_format {
        config.import_export.import_format = import_format.into();
    }
    if let Some(upstream) = &args.git_upstream {
        config.import_export.git_upstream = Some(upstream.clone());
        config.import_export.baseline_objdef_dir = None;
    }
    if let Some(directory) = &args.baseline_objdef_dir {
        config.import_export.baseline_objdef_dir = Some(directory.clone());
        config.import_export.git_upstream = None;
    }
    if args.no_telnet {
        config.services.telnet.enabled = false;
    }
    if args.no_web {
        config.services.web.enabled = false;
    }
    if args.enable_curl_worker {
        config.services.curl_worker.enabled = true;
    }
    if args.enable_git_worker {
        config.services.git_worker.enabled = true;
    }
    if let Some(telnet_address) = args.telnet_address.as_ref() {
        config.services.telnet.address = telnet_address.clone();
    }
    if let Some(telnet_port) = args.telnet_port {
        config.services.telnet.port = telnet_port;
    }
    if let Some(web_listen_address) = args.web_listen_address.as_ref() {
        config.services.web.listen_address = web_listen_address.clone();
    }
}

fn load_or_create_keypair(
    public_key_path: &PathBuf,
    private_key_path: &PathBuf,
    generate: bool,
) -> Result<(rusty_paseto::core::Key<64>, rusty_paseto::core::Key<32>), Report> {
    if public_key_path.exists() && private_key_path.exists() {
        info!(
            "Loading existing keypair from {} / {}",
            public_key_path.display(),
            private_key_path.display()
        );
        return load_keypair(public_key_path, private_key_path)
            .map_err(|e| eyre!("Unable to load keypair from public and private key files: {e}"));
    }

    if !generate {
        bail!(
            "Public ({:?}) and/or private ({:?}) key files must exist. Use --generate-keypair to create them.",
            public_key_path,
            private_key_path
        );
    }

    generate_keypair(public_key_path, private_key_path)?;
    info!(
        "Generated keypair to {} / {}",
        public_key_path.display(),
        private_key_path.display()
    );
    load_keypair(public_key_path, private_key_path)
        .map_err(|e| eyre!("Unable to load generated keypair: {e}"))
}

async fn wait_for_exit(
    kill_switch: Arc<AtomicBool>,
    daemon_handle: &JoinHandle<Result<(), Report>>,
    host_tasks: &[JoinHandle<Result<(), Report>>],
) {
    loop {
        if kill_switch.load(Ordering::Relaxed) {
            return;
        }
        if daemon_handle.is_finished() {
            warn!("Daemon task exited; shutting down combined process");
            return;
        }
        if host_tasks.iter().any(JoinHandle::is_finished) {
            warn!("Host task exited; shutting down combined process");
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_daemon_ready(
    ready_receiver: &oneshot::Receiver<()>,
    daemon_handle: &JoinHandle<Result<(), Report>>,
) -> Result<(), Report> {
    let started = std::time::Instant::now();
    let timeout = Duration::from_secs(15 * 60);

    loop {
        match ready_receiver.try_recv() {
            Ok(()) => return Ok(()),
            Err(oneshot::TryRecvError::Empty) => {}
            Err(oneshot::TryRecvError::Disconnected) => {
                return Err(eyre!("Daemon exited before reporting RPC readiness"));
            }
        }

        if daemon_handle.is_finished() {
            return Err(eyre!("Daemon exited before reporting RPC readiness"));
        }

        if started.elapsed() > timeout {
            return Err(eyre!("Timed out waiting for daemon RPC readiness"));
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn shutdown_hosts(host_tasks: Vec<JoinHandle<Result<(), Report>>>) -> Result<(), Report> {
    let mut shutdown_error = None;

    for task in host_tasks {
        match timeout(Duration::from_secs(10), task).await {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(e))) => {
                error!("Host task exited with error: {e}");
                if shutdown_error.is_none() {
                    shutdown_error = Some(e);
                }
            }
            Ok(Err(e)) => {
                error!("Host task join failed: {e}");
                if shutdown_error.is_none() {
                    shutdown_error = Some(eyre!("Host task join failed: {e}"));
                }
            }
            Err(_) => {
                warn!("Timed out waiting for host task shutdown");
                if shutdown_error.is_none() {
                    shutdown_error = Some(eyre!("Timed out waiting for host task shutdown"));
                }
            }
        }
    }

    match shutdown_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

async fn shutdown_daemon(daemon_handle: JoinHandle<Result<(), Report>>) -> Result<(), Report> {
    match timeout(Duration::from_secs(30), daemon_handle).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => Err(eyre!("Daemon task join failed: {e}")),
        Err(_) => Err(eyre!("Timed out waiting for daemon shutdown")),
    }
}

fn resolve_data_path(data_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        data_dir.join(path)
    }
}

fn resolve_optional_data_path(data_dir: &Path, path: Option<&PathBuf>, default: &str) -> PathBuf {
    match path {
        Some(path) => resolve_data_path(data_dir, path),
        None => data_dir.join(default),
    }
}

fn resolved_config_path(path: Option<&PathBuf>, default: &str) -> PathBuf {
    match path {
        Some(path) if path.is_absolute() => path.clone(),
        Some(path) => config_path(path),
        None => config_path(default),
    }
}

fn resolved_allowed_hosts_dir(data_dir: &Path) -> PathBuf {
    if let Ok(data) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(data).join("moor/allowed-hosts")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local/share/moor/allowed-hosts")
    } else {
        data_dir.join("allowed-hosts")
    }
}

fn prepare_config_dir() -> Result<(), Report> {
    let config_dir = moor_common::util::config_dir();
    std::fs::create_dir_all(&config_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&config_dir)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&config_dir, perms)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::CombinedConfig;
    use std::io::Write;

    fn load(yaml: &str) -> Result<CombinedConfig, eyre::Report> {
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(yaml.as_bytes())?;
        moor_common::config::apply_yaml_config_file(CombinedConfig::default(), Some(file.path()))
    }

    #[test]
    fn baseline_preparation_combines_cli_and_yaml() {
        use clap::Parser;
        let mut config = load(
            "import_export:\n  input_path: core\n  import_format: Objdef\n  git_upstream: origin/main\n",
        ).unwrap();
        let args = super::Args::try_parse_from(["moor", "--baseline-objdef-dir", "base"]).unwrap();
        super::apply_cli_overrides(&args, &mut config);
        let (config, _) = config.into_parts();
        config.validate().unwrap();
        assert!(config.import_export.git_upstream.is_none());
        assert_eq!(
            config.import_export.baseline_objdef_dir.as_deref(),
            Some(std::path::Path::new("base"))
        );

        let mut config = CombinedConfig::default();
        super::apply_cli_overrides(&args, &mut config);
        assert!(config.into_parts().0.validate().is_err());
    }

    #[test]
    fn embedded_git_configuration() {
        use clap::Parser;
        let mut config = CombinedConfig::default();
        assert!(!config.services.git_worker.enabled);
        let args = super::Args::try_parse_from(["moor", "--enable-git-worker"]).unwrap();
        super::apply_cli_overrides(&args, &mut config);
        assert!(config.services.git_worker.enabled);
        let config = load("services:\n  git_worker:\n    enabled: true\n    max_concurrent_requests: 2\n    limits:\n      max_seconds: 10\n").unwrap();
        assert!(config.services.git_worker.enabled);
        assert_eq!(config.services.git_worker.max_concurrent_requests, 2);
        assert_eq!(config.services.git_worker.limits.max_seconds, 10);
        assert_eq!(config.services.git_worker.limits.max_entries, 4096);
        assert!(load("services:\n  git_worker:\n    limits:\n      max_second: 10\n").is_err());
    }

    #[test]
    fn telnet_protocols_from_yaml() {
        let config = load(
            "services:\n  telnet:\n    protocols:\n      gmcp: true\n      naws: true\n      mssp_values:\n        NAME: Test\n",
        )
        .unwrap();
        let protocols = &config.services.telnet.protocols;
        assert!(protocols.gmcp && protocols.naws && !protocols.msdp);
        assert_eq!(
            protocols.mssp_values.get("NAME").map(String::as_str),
            Some("Test")
        );
    }

    #[test]
    fn telnet_protocols_unknown_field_rejected() {
        assert!(load("services:\n  telnet:\n    protocols:\n      gmpc: true\n").is_err());
        assert!(load("services:\n  telnet:\n    protcols: {}\n").is_err());
    }
}
