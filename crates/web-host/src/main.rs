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

mod host;

use crate::host::WebHost;
use std::collections::HashMap;

use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use clap_derive::Parser;

use axum::extract::State;
use axum::handler::HandlerWithoutStateExt;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use futures_util::future::OptionFuture;
use moor_values::{Obj, SYSTEM_OBJECT};
use rolldown::{
    Bundler, BundlerOptions, ExperimentalOptions, InputItem, OutputFormat, Platform, SourceMapType,
    Watcher,
};
use rpc_async_client::{
    make_host_token, proces_hosts_events, start_host_session, ListenersClient, ListenersMessage,
};
use rpc_common::client_args::RpcClientArgs;
use rpc_common::{load_keypair, HostType};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use tracing::{info, warn};

#[derive(Parser, Debug)]
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

    #[arg(
        long,
        value_name = "dist-directory",
        help = "Directory to serve static files from, and to compile/bundle them to",
        default_value = "./dist"
    )]
    dist_directory: PathBuf,

    #[arg(
        long,
        value_name = "watch-changes",
        help = "Watch for changes in the dist directory and recompile (for development)",
        default_value = "false"
    )]
    watch_changes: bool,

    // Where to find the client source files for the web bundler
    #[arg(
        long,
        value_name = "client-sources",
        help = "Directory for HTML/JS/CSS client source files for serving and compilation",
        default_value = "./crates/web-host/src/client"
    )]
    client_sources: PathBuf,
}

struct Listeners {
    listeners: HashMap<SocketAddr, Listener>,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    events_address: String,
    kill_switch: Arc<AtomicBool>,
    src_dir: PathBuf,
    dist_dir: PathBuf,
}

impl Listeners {
    pub fn new(
        zmq_ctx: tmq::Context,
        rpc_address: String,
        events_address: String,
        kill_switch: Arc<AtomicBool>,
        src_dir: PathBuf,
        dist_dir: PathBuf,
    ) -> (
        Self,
        tokio::sync::mpsc::Receiver<ListenersMessage>,
        ListenersClient,
    ) {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let listeners = Self {
            listeners: HashMap::new(),
            zmq_ctx,
            rpc_address,
            events_address,
            kill_switch,
            src_dir,
            dist_dir,
        };
        let listeners_client = ListenersClient::new(tx);
        (listeners, rx, listeners_client)
    }

    pub async fn run(
        &mut self,
        mut listeners_channel: tokio::sync::mpsc::Receiver<ListenersMessage>,
    ) {
        self.zmq_ctx
            .set_io_threads(8)
            .expect("Unable to set ZMQ IO threads");

        loop {
            if self.kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                info!("Host kill switch activated, stopping...");
                return;
            }

            match listeners_channel.recv().await {
                Some(ListenersMessage::AddListener(handler, addr)) => {
                    let ws_host = WebHost::new(
                        self.src_dir.clone(),
                        self.rpc_address.clone(),
                        self.events_address.clone(),
                        handler.clone(),
                    );
                    let main_router = match mk_routes(ws_host, &self.dist_dir) {
                        Ok(mr) => mr,
                        Err(e) => {
                            warn!(?e, "Unable to create main router");
                            return;
                        }
                    };

                    let listener = TcpListener::bind(addr)
                        .await
                        .expect("Unable to bind listener");
                    let (terminate_send, terminate_receive) = tokio::sync::watch::channel(false);
                    self.listeners
                        .insert(addr, Listener::new(terminate_send, handler));

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
                Some(ListenersMessage::RemoveListener(addr)) => {
                    let listener = self.listeners.remove(&addr);
                    info!(?addr, "Removing listener");
                    if let Some(listener) = listener {
                        listener
                            .terminate
                            .send(true)
                            .expect("Unable to send terminate message");
                    }
                }
                Some(ListenersMessage::GetListeners(tx)) => {
                    let listeners = self
                        .listeners
                        .iter()
                        .map(|(addr, listener)| (listener.handler_object.clone(), *addr))
                        .collect();
                    tx.send(listeners).expect("Unable to send listeners list");
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

async fn index_handler(State(state): State<WebHost>) -> impl IntoResponse {
    let mut headers = header::HeaderMap::new();
    // Read the index.html file out of state.root_path
    let Ok(index_html) = std::fs::read_to_string(state.root_path.join("index.html")) else {
        return (
            StatusCode::NOT_FOUND,
            headers,
            "Unable to read index.html".to_string(),
        );
    };
    // Return with content-type html
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/html"),
    );
    (StatusCode::OK, headers, index_html)
}

async fn css_handler(State(state): State<WebHost>) -> impl IntoResponse {
    let mut headers = header::HeaderMap::new();
    let Ok(css) = std::fs::read_to_string(state.root_path.join("moor.css")) else {
        return (
            StatusCode::NOT_FOUND,
            headers,
            "Unable to read moor.css".to_string(),
        );
    };
    // Return with content-type css
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/css"),
    );
    (StatusCode::OK, headers, css)
}

fn mk_routes(web_host: WebHost, dist_dir: &Path) -> eyre::Result<Router> {
    async fn handle_404() -> (StatusCode, &'static str) {
        (StatusCode::NOT_FOUND, "Not found")
    }

    let dist_service = ServeDir::new(dist_dir).not_found_service(handle_404.into_service());

    let webhost_router = Router::new()
        .route("/", get(index_handler))
        .route("/moor.css", get(css_handler))
        .route(
            "/ws/attach/connect/{token}",
            get(host::ws_connect_attach_handler),
        )
        .route(
            "/ws/attach/create/{token}",
            get(host::ws_create_attach_handler),
        )
        .route("/auth/connect", post(host::connect_auth_handler))
        .route("/auth/create", post(host::create_auth_handler))
        .route("/welcome", get(host::welcome_message_handler))
        .route("/eval", post(host::eval_handler))
        .route("/verbs", get(host::verbs_handler))
        .route("/verbs/{object}/{name}", get(host::verb_retrieval_handler))
        .route("/verbs/{object}/{name}", post(host::verb_program_handler))
        .route("/properties", get(host::properties_handler))
        // ?oid=1234 or ?sysobj=foo.bar.baz or ?match=foo
        .route("/objects/{object}", get(host::resolve_objref_handler))
        .route(
            "/properties/{object}/{name}",
            get(host::property_retrieval_handler),
        )
        .fallback_service(dist_service)
        .with_state(web_host);

    Ok(webhost_router)
}

fn mk_js_bundler(src_dir: &Path) -> Arc<Mutex<Bundler>> {
    // Find all .ts files in the src directory
    // moor.ts is always first
    let mut input = vec![src_dir.join("moor.ts").to_string_lossy().to_string().into()];

    for entry in std::fs::read_dir(src_dir).expect("Unable to read src directory") {
        let entry = entry.expect("Unable to read entry");
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "ts") {
            if path.file_name().map_or(false, |name| name == "moor.ts") {
                continue;
            }
            input.push(InputItem {
                import: path.to_string_lossy().to_string(),
                ..Default::default()
            });
        }
    }

    let bundler = Bundler::new(BundlerOptions {
        input: Some(input),
        sourcemap: Some(SourceMapType::File),
        minify: Some(false),
        format: Some(OutputFormat::Esm),
        platform: Some(Platform::Browser),
        experimental: Some(ExperimentalOptions {
            incremental_build: Some(true),
            ..Default::default()
        }),

        ..Default::default()
    });
    Arc::new(Mutex::new(bundler))
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install()?;
    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    let mut hup_signal =
        signal(SignalKind::hangup()).expect("Unable to register HUP signal handler");
    let mut stop_signal =
        signal(SignalKind::interrupt()).expect("Unable to register STOP signal handler");

    let kill_switch = Arc::new(AtomicBool::new(false));

    let (private_key, _public_key) =
        load_keypair(&args.client_args.public_key, &args.client_args.private_key)
            .expect("Unable to load keypair from public and private key files");
    let host_token = make_host_token(&private_key, HostType::TCP);

    let zmq_ctx = tmq::Context::new();

    let (mut listeners_server, listeners_channel, listeners) = Listeners::new(
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        args.client_args.events_address.clone(),
        kill_switch.clone(),
        args.client_sources.to_owned(),
        args.dist_directory.to_owned(),
    );
    let listeners_thread = tokio::spawn(async move {
        listeners_server.run(listeners_channel).await;
    });

    info!("Serving out of CWD {:?}", std::env::current_dir()?);
    let rpc_client = start_host_session(
        &host_token,
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
    )
    .await
    .expect("Unable to establish initial host session");

    listeners
        .add_listener(&SYSTEM_OBJECT, args.listen_address.parse().unwrap())
        .await
        .expect("Unable to start default listener");

    let host_listen_loop = proces_hosts_events(
        rpc_client,
        host_token,
        zmq_ctx.clone(),
        args.client_args.events_address.clone(),
        args.listen_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
        HostType::TCP,
    );

    let bundler = mk_js_bundler(&args.client_sources);

    // Delete any pre-existing content in the dist dir.
    if args.dist_directory.exists() {
        std::fs::remove_dir_all(&args.dist_directory)
            .expect("Unable to remove existing dist directory");
    }

    // Write initial bundle
    if let Err(e) = bundler.lock().await.write().await {
        warn!(?e, "Unable to write initial bundle");
        for msg in e.iter() {
            warn!("{:?}", msg);
        }
        panic!("Unable to write initial bundle");
    }
    info!("Bundle written to {:?}", args.dist_directory);

    // Start watching for changes, if such a thing is desired
    let _watcher_future: OptionFuture<_> = args
        .watch_changes
        .then(|| {
            let bundler = bundler.clone();
            let dist_dir = args.dist_directory.clone();
            tokio::spawn(async move {
                let watcher = Watcher::new(vec![bundler], None).unwrap();
                info!("Watching {:?} for changes...", dist_dir);
                watcher.start().await
            })
        })
        .into();

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
