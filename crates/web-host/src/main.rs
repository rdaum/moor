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

mod host;

use crate::host::{OAuth2Config, OAuth2Manager, OAuth2State, PendingOAuth2Store, WebHost};
use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post, put},
};
use clap::Parser;
use clap_derive::Parser;

use figment::{
    Figment,
    providers::{Format, Serialized, Yaml},
};
use ipnet::IpNet;
use moor_var::{Obj, SYSTEM_OBJECT};
use rpc_async_client::{
    ListenerInfo, ListenersClient, ListenersError, ListenersMessage, process_hosts_events,
    start_host_session,
};
use rpc_common::{HostType, client_args::RpcClientArgs};
use serde_derive::{Deserialize, Serialize};
use std::{
    net::{IpAddr, SocketAddr},
    sync::atomic::{AtomicBool, AtomicU64},
};
use tokio::{
    net::TcpListener,
    select,
    signal::unix::{SignalKind, signal},
};
use tower_governor::{GovernorLayer, errors::GovernorError, governor::GovernorConfigBuilder};
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tracing::{error, info, warn};
use uuid::Uuid;

use once_cell::sync::Lazy;

static VERSION_STRING: Lazy<String> = Lazy::new(|| {
    format!(
        "{} (commit: {})",
        env!("CARGO_PKG_VERSION"),
        moor_common::build::short_commit()
    )
});

/// Rate-limit key extractor that only trusts forwarding headers from configured
/// trusted proxy CIDRs.  Falls back to peer IP when no trusted proxy is present
/// or the peer is not in the trusted list.
#[derive(Clone, Debug)]
struct TrustedProxyKeyExtractor {
    trusted_cidrs: Arc<Vec<IpNet>>,
}

impl tower_governor::key_extractor::KeyExtractor for TrustedProxyKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, GovernorError> {
        let peer_ip = req
            .extensions()
            .get::<axum::extract::ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip())
            .ok_or(GovernorError::UnableToExtractKey)?;

        // Only trust forwarding headers when the direct peer is a known proxy.
        if !self.trusted_cidrs.is_empty()
            && self
                .trusted_cidrs
                .iter()
                .any(|cidr| cidr.contains(&peer_ip))
        {
            if let Some(ip) = req
                .headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.trim().parse::<IpAddr>().ok())
            {
                return Ok(ip);
            }
            if let Some(ip) = req
                .headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<IpAddr>().ok())
            {
                return Ok(ip);
            }
        }

        Ok(peer_ip)
    }
}

/// CORS middleware configuration.
/// Disabled by default; when enabled, explicit origins must be provided.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Allowed origins (e.g. ["http://localhost:3000", "https://mygame.example.com"]).
    /// Required when enabled; wildcard "*" is only permitted when allow_credentials is false.
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    #[serde(default)]
    pub allow_credentials: bool,
    /// HTTP methods to allow (e.g. ["GET", "POST"]). Defaults to GET, POST, PUT, DELETE, OPTIONS.
    #[serde(default)]
    pub allowed_methods: Vec<String>,
    /// Headers to allow. Defaults to common set including X-Moor-Auth-Token.
    #[serde(default)]
    pub allowed_headers: Vec<String>,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_origins: Vec::new(),
            allow_credentials: false,
            allowed_methods: Vec::new(),
            allowed_headers: Vec::new(),
        }
    }
}

/// Rate limiting configuration for auth endpoints.
/// Uses a token-bucket algorithm keyed by client IP.
/// Single-instance scope — not shared across multiple web-host processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Sustained requests per second (token refill rate).
    #[serde(default = "RateLimitConfig::default_rps")]
    pub requests_per_second: u64,
    /// Burst size (bucket capacity).
    #[serde(default = "RateLimitConfig::default_burst")]
    pub burst_size: u32,
}

impl RateLimitConfig {
    fn default_rps() -> u64 {
        5
    }
    fn default_burst() -> u32 {
        10
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            requests_per_second: Self::default_rps(),
            burst_size: Self::default_burst(),
        }
    }
}

#[derive(Parser, Debug, Serialize, Deserialize)]
#[command(version = VERSION_STRING.as_str())]
struct Args {
    #[command(flatten)]
    client_args: RpcClientArgs,

    #[arg(
        long,
        value_name = "listen-address",
        help = "HTTP listen address",
        default_value = "0.0.0.0:8080"
    )]
    listen_address: String,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    pub debug: bool,

    #[arg(long, help = "Yaml config file to use, overrides values in CLI args")]
    config_file: Option<String>,

    #[serde(default)]
    #[arg(skip)]
    pub oauth2: OAuth2Config,

    #[arg(long, help = "Enable webhooks", default_value = "true")]
    pub enable_webhooks: bool,

    #[serde(default)]
    #[arg(skip)]
    pub cors: CorsConfig,

    #[serde(default)]
    #[arg(skip)]
    pub rate_limit: RateLimitConfig,

    /// Trusted proxy CIDRs. Only connections from these CIDRs will have
    /// X-Forwarded-For / X-Real-IP headers honoured. Default empty = trust nothing.
    #[serde(default)]
    #[arg(skip)]
    pub trusted_proxy_cidrs: Vec<String>,
}

struct Listeners {
    host_id: Uuid,
    listeners: HashMap<SocketAddr, Listener>,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    events_address: String,
    kill_switch: Arc<AtomicBool>,
    oauth2_manager: Option<Arc<OAuth2Manager>>,
    curve_keys: Option<(String, String, String)>, // (client_secret, client_public, server_public) - Z85 encoded
    enable_webhooks: bool,
    last_daemon_ping: Arc<AtomicU64>,
    cors_config: CorsConfig,
    rate_limit_config: RateLimitConfig,
    trusted_proxy_cidrs: Arc<Vec<IpNet>>,
}

impl Listeners {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        host_id: Uuid,
        zmq_ctx: tmq::Context,
        rpc_address: String,
        events_address: String,
        kill_switch: Arc<AtomicBool>,
        oauth2_manager: Option<Arc<OAuth2Manager>>,
        curve_keys: Option<(String, String, String)>,
        enable_webhooks: bool,
        last_daemon_ping: Arc<AtomicU64>,
        cors_config: CorsConfig,
        rate_limit_config: RateLimitConfig,
        trusted_proxy_cidrs: Arc<Vec<IpNet>>,
    ) -> (
        Self,
        tokio::sync::mpsc::Receiver<ListenersMessage>,
        ListenersClient,
    ) {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let listeners = Self {
            host_id,
            listeners: HashMap::new(),
            zmq_ctx,
            rpc_address,
            events_address,
            kill_switch,
            oauth2_manager,
            curve_keys,
            enable_webhooks,
            last_daemon_ping,
            cors_config,
            rate_limit_config,
            trusted_proxy_cidrs,
        };
        let listeners_client = ListenersClient::new(tx);
        (listeners, rx, listeners_client)
    }

    pub async fn run(
        &mut self,
        mut listeners_channel: tokio::sync::mpsc::Receiver<ListenersMessage>,
    ) {
        if let Err(e) = self.zmq_ctx.set_io_threads(8) {
            error!("Unable to set ZMQ IO threads: {}", e);
            return;
        }

        loop {
            if self.kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                info!("Host kill switch activated, stopping...");
                return;
            }

            match listeners_channel.recv().await {
                Some(ListenersMessage::AddTlsListener(handler, addr, reply)) => {
                    // Web-host doesn't support TLS directly - TLS is handled at the reverse proxy level
                    error!(?addr, "TLS listeners not supported by web-host");
                    let _ = reply.send(Err(ListenersError::AddListenerFailed(handler, addr)));
                }
                Some(ListenersMessage::AddListener(handler, addr, reply)) => {
                    let listener = match TcpListener::bind(addr).await {
                        Ok(listener) => listener,
                        Err(e) => {
                            let _ =
                                reply.send(Err(ListenersError::AddListenerFailed(handler, addr)));
                            error!(?addr, "Unable to bind listener: {}", e);
                            continue;
                        }
                    };

                    let local_addr = match listener.local_addr() {
                        Ok(addr) => addr,
                        Err(e) => {
                            error!(?addr, "Unable to get local address: {}", e);
                            return;
                        }
                    };

                    let ws_host = WebHost::new(
                        self.rpc_address.clone(),
                        self.events_address.clone(),
                        handler,
                        local_addr.port(),
                        self.curve_keys.clone(),
                        self.host_id,
                        self.last_daemon_ping.clone(),
                        self.trusted_proxy_cidrs.clone(),
                    );

                    // Create OAuth2State if OAuth2 is enabled
                    let oauth2_state = self.oauth2_manager.as_ref().map(|manager| {
                        let pending = Arc::new(PendingOAuth2Store::new());
                        // Spawn a background task to reap expired CSRF tokens and auth codes
                        let reap_pending = Arc::clone(&pending);
                        tokio::spawn(async move {
                            let mut interval =
                                tokio::time::interval(std::time::Duration::from_secs(60));
                            loop {
                                interval.tick().await;
                                reap_pending.reap_expired();
                            }
                        });
                        OAuth2State {
                            manager: Arc::clone(manager),
                            web_host: ws_host.clone(),
                            pending,
                        }
                    });

                    let main_router = match mk_routes(
                        ws_host,
                        oauth2_state,
                        self.enable_webhooks,
                        &self.cors_config,
                        &self.rate_limit_config,
                        &self.trusted_proxy_cidrs,
                    ) {
                        Ok(mr) => mr,
                        Err(e) => {
                            warn!(?e, "Unable to create main router");
                            return;
                        }
                    };
                    let (terminate_send, terminate_receive) = tokio::sync::watch::channel(false);
                    self.listeners
                        .insert(addr, Listener::new(terminate_send, handler));

                    // Signal that the listener is successfully bound
                    let _ = reply.send(Ok(()));

                    // One task per listener.
                    tokio::spawn(async move {
                        let mut term_receive = terminate_receive.clone();
                        select! {
                            _ = term_receive.changed() => {
                                info!("Listener terminated, stopping...");
                            }
                            _ = Listener::serve(listener, main_router) => {
                                info!("Listener exited, restarting...");
                            }
                        }
                    });
                }
                Some(ListenersMessage::RemoveListener(addr, reply)) => {
                    let listener = self.listeners.remove(&addr);
                    info!(?addr, "Removing listener");
                    if let Some(listener) = listener {
                        if let Err(e) = listener.terminate.send(true) {
                            error!("Unable to send terminate message: {}", e);
                        }
                        let _ = reply.send(Ok(()));
                    } else {
                        let _ = reply.send(Err(ListenersError::RemoveListenerFailed(addr)));
                    }
                }
                Some(ListenersMessage::GetListeners(tx)) => {
                    let listeners = self
                        .listeners
                        .iter()
                        .map(|(addr, listener)| ListenerInfo {
                            handler: listener.handler_object,
                            addr: *addr,
                            is_tls: false,
                        })
                        .collect();
                    if let Err(e) = tx.send(listeners) {
                        error!("Unable to send listeners list: {:?}", e);
                    }
                }
                None => {
                    warn!("Listeners channel closed, stopping...");
                    return;
                }
            }
        }
    }
}
pub struct Listener {
    pub(crate) handler_object: Obj,
    pub(crate) terminate: tokio::sync::watch::Sender<bool>,
}

impl Listener {
    pub fn new(terminate: tokio::sync::watch::Sender<bool>, handler_object: Obj) -> Self {
        Self {
            handler_object,
            terminate,
        }
    }

    pub async fn serve(listener: TcpListener, main_router: Router) -> eyre::Result<()> {
        let addr = listener.local_addr()?;
        info!("Listening on {:?}", addr);
        axum::serve(
            listener,
            main_router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
        info!("Done listening on {:?}", addr);
        Ok(())
    }
}

fn build_cors_layer(config: &CorsConfig) -> eyre::Result<Option<CorsLayer>> {
    if !config.enabled {
        return Ok(None);
    }

    if config.allowed_origins.is_empty() {
        return Err(eyre::eyre!(
            "CORS enabled but no allowed_origins configured"
        ));
    }

    let origins = if config.allowed_origins.len() == 1 && config.allowed_origins[0] == "*" {
        if config.allow_credentials {
            return Err(eyre::eyre!(
                "CORS: wildcard origin '*' cannot be used with allow_credentials=true"
            ));
        }
        AllowOrigin::any()
    } else {
        let origins: Vec<axum::http::HeaderValue> = config
            .allowed_origins
            .iter()
            .map(|o| {
                o.parse()
                    .map_err(|_| eyre::eyre!("Invalid CORS origin: {}", o))
            })
            .collect::<eyre::Result<Vec<_>>>()?;
        AllowOrigin::list(origins)
    };

    let methods = if config.allowed_methods.is_empty() {
        AllowMethods::list([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
    } else {
        let methods: Vec<axum::http::Method> = config
            .allowed_methods
            .iter()
            .map(|m| {
                m.parse()
                    .map_err(|_| eyre::eyre!("Invalid CORS method: {}", m))
            })
            .collect::<eyre::Result<Vec<_>>>()?;
        AllowMethods::list(methods)
    };

    let headers = if config.allowed_headers.is_empty() {
        AllowHeaders::list([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
            axum::http::HeaderName::from_static("x-moor-auth-token"),
            axum::http::HeaderName::from_static("x-moor-client-token"),
            axum::http::HeaderName::from_static("x-moor-client-id"),
        ])
    } else {
        let headers: Vec<axum::http::HeaderName> = config
            .allowed_headers
            .iter()
            .map(|h| {
                h.parse()
                    .map_err(|_| eyre::eyre!("Invalid CORS header: {}", h))
            })
            .collect::<eyre::Result<Vec<_>>>()?;
        AllowHeaders::list(headers)
    };

    let mut layer = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(methods)
        .allow_headers(headers);

    if config.allow_credentials {
        layer = layer.allow_credentials(true);
    }

    Ok(Some(layer))
}

fn mk_routes(
    web_host: WebHost,
    oauth2_state: Option<OAuth2State>,
    enable_webhooks: bool,
    cors_config: &CorsConfig,
    rate_limit_config: &RateLimitConfig,
    trusted_proxy_cidrs: &Arc<Vec<IpNet>>,
) -> eyre::Result<Router> {
    // Build auth routes with optional rate limiting and tight body limit
    let mut auth_routes = Router::new()
        .route("/auth/connect", post(host::connect_auth_handler))
        .route("/auth/create", post(host::create_auth_handler))
        .layer(DefaultBodyLimit::max(64 * 1024)) // 64 KB — small form bodies
        .with_state(web_host.clone());

    if rate_limit_config.enabled {
        let key_extractor = TrustedProxyKeyExtractor {
            trusted_cidrs: Arc::clone(trusted_proxy_cidrs),
        };
        let governor_conf = GovernorConfigBuilder::default()
            .per_second(rate_limit_config.requests_per_second)
            .burst_size(rate_limit_config.burst_size)
            .key_extractor(key_extractor)
            .finish()
            .ok_or_else(|| eyre::eyre!("Failed to build rate limiter config"))?;
        auth_routes = auth_routes.layer(GovernorLayer {
            config: Arc::new(governor_conf),
        });
        info!(
            "Rate limiting enabled on auth endpoints: {}/s burst={}",
            rate_limit_config.requests_per_second, rate_limit_config.burst_size
        );
    }

    let mut webhost_router = Router::new()
        .route("/ws/attach/connect", get(host::ws_connect_attach_handler))
        .route("/ws/attach/create", get(host::ws_create_attach_handler))
        .route("/auth/validate", get(host::validate_auth_handler))
        .route("/auth/logout", post(host::logout_handler))
        .route(
            "/v1/system_property/{*path}",
            get(host::system_property_handler),
        )
        .route("/v1/eval", post(host::eval_handler))
        .route("/v1/features", get(host::features_handler))
        .route("/health", get(host::health_handler))
        .route("/version", get(host::version_handler))
        .route("/openapi.yaml", get(host::openapi_handler))
        .route(
            "/v1/invoke_welcome_message",
            get(host::invoke_welcome_message_handler),
        )
        .route(
            "/v1/verbs/{object}/{name}",
            post(host::verb_program_handler),
        )
        .route("/v1/verbs/{object}", get(host::verbs_handler))
        .route(
            "/v1/verbs/{object}/{name}",
            get(host::verb_retrieval_handler),
        )
        .route(
            "/v1/verbs/{object}/{name}/invoke",
            post(host::invoke_verb_handler),
        )
        .route("/v1/properties/{object}", get(host::properties_handler))
        .route(
            "/v1/properties/{object}/{name}",
            get(host::property_retrieval_handler),
        )
        .route(
            "/v1/properties/{object}/{name}",
            post(host::update_property_handler),
        )
        .route("/v1/objects", get(host::list_objects_handler))
        .route("/v1/objects/{object}", get(host::resolve_objref_handler))
        .route("/v1/history", get(host::history_handler))
        .route("/v1/presentations", get(host::presentations_handler))
        .route(
            "/v1/presentations/{presentation_id}",
            axum::routing::delete(host::dismiss_presentation_handler),
        )
        .route("/v1/event-log/pubkey", get(host::get_pubkey_handler))
        .route("/v1/event-log/pubkey", put(host::set_pubkey_handler))
        .route(
            "/v1/event-log/history",
            axum::routing::delete(host::delete_history_handler),
        )
        .with_state(web_host.clone());

    // Merge rate-limited auth routes
    webhost_router = webhost_router.merge(auth_routes);

    // Add OAuth2 routes if OAuth2 is enabled
    if let Some(oauth2_state) = oauth2_state {
        let oauth2_router = Router::new()
            .route("/v1/oauth2/config", get(host::oauth2_config_handler))
            .route(
                "/auth/oauth2/{provider}/authorize",
                get(host::oauth2_authorize_handler),
            )
            .route(
                "/auth/oauth2/{provider}/callback",
                get(host::oauth2_callback_handler),
            )
            .route(
                "/auth/oauth2/account",
                post(host::oauth2_account_choice_handler),
            )
            .route("/auth/oauth2/exchange", post(host::oauth2_exchange_handler))
            .layer(DefaultBodyLimit::max(64 * 1024)) // 64 KB — small form/JSON bodies
            .with_state(oauth2_state);

        webhost_router = webhost_router.merge(oauth2_router);
    }

    // 1 MB default body size limit for all non-webhook routes.
    // Applied before merging webhooks so the global limit doesn't override
    // the webhook-specific limit.
    webhost_router = webhost_router.layer(DefaultBodyLimit::max(1024 * 1024));

    // Add webhook routes only if enabled (2 MB limit for external payloads)
    if enable_webhooks {
        let webhook_router = Router::new()
            .route(
                "/webhooks/{*path}",
                axum::routing::any(host::web_hook_handler),
            )
            .layer(DefaultBodyLimit::max(2 * 1024 * 1024)) // 2 MB — external payloads
            .with_state(web_host.clone());
        webhost_router = webhost_router.merge(webhook_router);
    }

    // CORS layer (applied outermost so preflight OPTIONS work correctly)
    if let Some(cors_layer) = build_cors_layer(cors_config)? {
        info!("CORS policy enabled");
        webhost_router = webhost_router.layer(cors_layer);
    }

    Ok(webhost_router)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install()?;
    let cli_args = Args::parse();
    let config_file = cli_args.config_file.clone();
    let mut args_figment = Figment::new().merge(Serialized::defaults(cli_args));
    if let Some(config_file) = config_file {
        args_figment = args_figment.merge(Yaml::file(config_file));
    }
    let args = match args_figment.extract::<Args>() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Unable to parse arguments/configuration: {e}");
            std::process::exit(1);
        }
    };

    moor_common::tracing::init_tracing(args.debug).unwrap_or_else(|e| {
        eprintln!("Unable to configure logging: {e}");
        std::process::exit(1);
    });

    let mut hup_signal = match signal(SignalKind::hangup()) {
        Ok(signal) => signal,
        Err(e) => {
            error!("Unable to register HUP signal handler: {}", e);
            std::process::exit(1);
        }
    };
    let mut stop_signal = match signal(SignalKind::interrupt()) {
        Ok(signal) => signal,
        Err(e) => {
            error!("Unable to register STOP signal handler: {}", e);
            std::process::exit(1);
        }
    };

    let kill_switch = Arc::new(AtomicBool::new(false));

    // Setup CURVE encryption if using TCP endpoint
    let curve_keys = match rpc_async_client::enrollment_client::setup_curve_auth(
        &args.client_args.rpc_address,
        &args.client_args.enrollment_address,
        args.client_args.enrollment_token_file.as_deref(),
        "web-host",
        &args.client_args.data_dir,
    ) {
        Ok(keys) => keys,
        Err(e) => {
            error!("Failed to setup CURVE authentication: {}", e);
            std::process::exit(1);
        }
    };

    let zmq_ctx = tmq::Context::new();

    // Initialize OAuth2Manager if enabled
    let oauth2_manager = if args.oauth2.enabled {
        match OAuth2Manager::new(args.oauth2.clone()) {
            Ok(manager) => {
                info!(
                    "OAuth2 enabled with {} providers",
                    manager.available_providers().len()
                );
                Some(Arc::new(manager))
            }
            Err(e) => {
                error!("Failed to initialize OAuth2Manager: {}", e);
                error!("OAuth2 authentication will be disabled");
                None
            }
        }
    } else {
        info!("OAuth2 authentication is disabled");
        None
    };

    // Parse trusted proxy CIDRs
    let trusted_proxy_cidrs: Vec<IpNet> = args
        .trusted_proxy_cidrs
        .iter()
        .filter_map(|cidr| match cidr.parse::<IpNet>() {
            Ok(net) => Some(net),
            Err(e) => {
                error!("Invalid trusted proxy CIDR '{}': {}", cidr, e);
                None
            }
        })
        .collect();
    if !trusted_proxy_cidrs.is_empty() {
        info!(
            "Trusted proxy CIDRs: {:?}",
            trusted_proxy_cidrs
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
        );
    }
    let trusted_proxy_cidrs = Arc::new(trusted_proxy_cidrs);

    let host_id = Uuid::new_v4();
    let last_daemon_ping = Arc::new(AtomicU64::new(0));
    let (mut listeners_server, listeners_channel, listeners) = Listeners::new(
        host_id,
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        args.client_args.events_address.clone(),
        kill_switch.clone(),
        oauth2_manager,
        curve_keys.clone(),
        args.enable_webhooks,
        last_daemon_ping.clone(),
        args.cors.clone(),
        args.rate_limit.clone(),
        trusted_proxy_cidrs,
    );
    info!("Starting up listener thread...");
    let listeners_thread = tokio::spawn(async move {
        listeners_server.run(listeners_channel).await;
    });
    listeners
        .add_listener(
            &SYSTEM_OBJECT,
            match args.listen_address.parse() {
                Ok(addr) => addr,
                Err(e) => {
                    error!(
                        "Unable to parse listen address {}: {}",
                        args.listen_address, e
                    );
                    std::process::exit(1);
                }
            },
        )
        .await
        .unwrap_or_else(|e| {
            error!("Unable to start default listener: {}", e);
            std::process::exit(1);
        });

    info!("Starting host session....");
    let (rpc_client, host_id) = match start_host_session(
        host_id,
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
        curve_keys.clone(),
    )
    .await
    {
        Ok((client, id)) => (client, id),
        Err(e) => {
            error!("Unable to establish initial host session: {}", e);
            std::process::exit(1);
        }
    };

    let host_listen_loop = process_hosts_events(
        rpc_client,
        host_id,
        zmq_ctx.clone(),
        args.client_args.events_address.clone(),
        args.listen_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
        HostType::TCP,
        curve_keys,
        Some(last_daemon_ping),
    );

    select! {
        _ = host_listen_loop => {
            info!("Host events loop exited.");
        },
        _ = listeners_thread => {
            info!("Listener set exited.");
        }
        _ = hup_signal.recv() => {
            info!("HUP received, stopping...");
            kill_switch.store(true, std::sync::atomic::Ordering::SeqCst);
        },
        _ = stop_signal.recv() => {
            info!("STOP received, stopping...");
            kill_switch.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    info!("Done.");

    Ok(())
}
