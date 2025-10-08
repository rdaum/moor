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

use axum::{
    Router,
    routing::{get, post, put},
};
use clap::Parser;
use clap_derive::Parser;

use figment::{
    Figment,
    providers::{Format, Serialized, Yaml},
};
use moor_var::{Obj, SYSTEM_OBJECT};
use rpc_async_client::{
    ListenersClient, ListenersMessage, process_hosts_events, start_host_session,
};
use rpc_common::{HostType, client_args::RpcClientArgs, load_keypair, make_host_token};
use serde_derive::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    sync::{Arc, atomic::AtomicBool},
};
use tokio::{
    net::TcpListener,
    select,
    signal::unix::{SignalKind, signal},
};
use tracing::{error, info, warn};

#[derive(Parser, Debug, Serialize, Deserialize)]
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
}

struct Listeners {
    listeners: HashMap<SocketAddr, Listener>,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    events_address: String,
    kill_switch: Arc<AtomicBool>,
}

impl Listeners {
    pub fn new(
        zmq_ctx: tmq::Context,
        rpc_address: String,
        events_address: String,
        kill_switch: Arc<AtomicBool>,
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
                    let listener = match TcpListener::bind(addr).await {
                        Ok(listener) => listener,
                        Err(e) => {
                            error!(?addr, "Unable to bind listener: {}", e);
                            return;
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
                    );
                    let main_router = match mk_routes(ws_host) {
                        Ok(mr) => mr,
                        Err(e) => {
                            warn!(?e, "Unable to create main router");
                            return;
                        }
                    };
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
                        .map(|(addr, listener)| (listener.handler_object, *addr))
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

fn mk_routes(web_host: WebHost) -> eyre::Result<Router> {
    let webhost_router = Router::new()
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
        .route(
            "/fb/system_property/{*path}",
            get(host::system_property_handler),
        )
        .route("/fb/eval", post(host::eval_handler))
        .route(
            "/fb/verbs/{object}/{name}",
            post(host::verb_program_handler),
        )
        .route("/fb/verbs/{object}", get(host::verbs_handler))
        .route(
            "/fb/verbs/{object}/{name}",
            get(host::verb_retrieval_handler),
        )
        .route(
            "/fb/verbs/{object}/{name}/invoke",
            post(host::invoke_verb_handler),
        )
        .route("/fb/properties/{object}", get(host::properties_handler))
        .route(
            "/fb/properties/{object}/{name}",
            get(host::property_retrieval_handler),
        )
        .route("/fb/objects/{object}", get(host::resolve_objref_handler))
        .route("/fb/api/history", get(host::history_handler))
        .route("/fb/api/presentations", get(host::presentations_handler))
        .route(
            "/api/presentations/{presentation_id}",
            axum::routing::delete(host::dismiss_presentation_handler),
        )
        .route("/api/event-log/pubkey", get(host::get_pubkey_handler))
        .route("/api/event-log/pubkey", put(host::set_pubkey_handler))
        .with_state(web_host);

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
    let args = args_figment.extract::<Args>().unwrap();

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

    let (private_key, _public_key) =
        match load_keypair(&args.client_args.public_key, &args.client_args.private_key) {
            Ok(keypair) => keypair,
            Err(e) => {
                error!(
                    "Unable to load keypair from public and private key files: {}",
                    e
                );
                std::process::exit(1);
            }
        };
    let host_token = make_host_token(&private_key, HostType::TCP);

    let zmq_ctx = tmq::Context::new();

    let (mut listeners_server, listeners_channel, listeners) = Listeners::new(
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        args.client_args.events_address.clone(),
        kill_switch.clone(),
    );
    let listeners_thread = tokio::spawn(async move {
        listeners_server.run(listeners_channel).await;
    });

    info!("Serving out of CWD {:?}", std::env::current_dir()?);
    let rpc_client = match start_host_session(
        &host_token,
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
    )
    .await
    {
        Ok(client) => client,
        Err(e) => {
            error!("Unable to establish initial host session: {}", e);
            std::process::exit(1);
        }
    };

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

    let host_listen_loop = process_hosts_events(
        rpc_client,
        host_token,
        zmq_ctx.clone(),
        args.client_args.events_address.clone(),
        args.listen_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
        HostType::TCP,
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
