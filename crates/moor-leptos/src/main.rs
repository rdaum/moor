#[cfg(feature = "ssr")]
pub mod server_side {
    use crate::server_side;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::Router;
    use clap::Parser;
    use clap_derive::Parser;
    use eyre::{bail, eyre};
    use figment::providers::{Format, Serialized, Yaml};
    use figment::Figment;
    use leptos::config::get_configuration;
    use leptos::context::provide_context;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use moor_leptos::app::{shell, App};
    use moor_leptos::Context;
    use moor_var::{Obj, SYSTEM_OBJECT};
    use rpc_async_client::rpc_client::RpcSendClient;
    use rpc_async_client::{make_host_token, process_hosts_events, start_host_session};
    use rpc_async_client::{ListenersClient, ListenersMessage};
    use rpc_common::client_args::RpcClientArgs;
    use rpc_common::HostClientToDaemonMessage::{Attach, ConnectionEstablish};
    use rpc_common::{load_keypair, ClientToken, DaemonToClientReply, HostType, ReplyResult};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tmq::request;
    use tokio::net::TcpListener;
    use tokio::select;
    use tokio::signal::unix::SignalKind;
    use tracing::{debug, error, info, warn};
    use tracing_subscriber::fmt::format::FmtSpan;
    use uuid::Uuid;

    #[derive(Parser, Debug, Serialize, Deserialize)]
    pub(crate) struct Args {
        #[command(flatten)]
        pub(crate) client_args: RpcClientArgs,

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

        #[arg(long, help = "Enable debug logging", default_value = "false")]
        pub debug: bool,

        #[arg(long, help = "Yaml config file to use, overrides values in CLI args")]
        pub(crate) config_file: Option<String>,
    }

    pub(crate) struct Listeners {
        pub(crate) listeners: HashMap<SocketAddr, Listener>,
        pub(crate) zmq_ctx: tmq::Context,
        pub(crate) rpc_address: String,
        pub(crate) events_address: String,
        pub(crate) kill_switch: Arc<AtomicBool>,
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
                        let (terminate_send, terminate_receive) =
                            tokio::sync::watch::channel(false);
                        self.listeners
                            .insert(addr, Listener::new(terminate_send, handler));

                        let context = Context {
                            zmq_ctx: self.zmq_ctx.clone(),
                            rpc_address: self.rpc_address.clone(),
                            events_address: self.events_address.clone(),
                            listen_address: addr,
                        };
                        // One task per listener.
                        tokio::spawn(async move {
                            let mut term_receive = terminate_receive.clone();
                            select! {
                                _ = term_receive.changed() => {
                                    info!("Listener terminated, stopping...");
                                }
                                _ = Listener::serve(listener, context.clone()) => {
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

        pub async fn serve(listener: TcpListener, context: Context) -> std::io::Result<()> {
            let conf = get_configuration(None).unwrap();
            let leptos_options = conf.leptos_options;
            let routes = generate_route_list(App);
            let app = Router::new()
                .leptos_routes_with_context(
                    &leptos_options,
                    routes,
                    move || {
                        info!("Providing context for request");
                        provide_context(context.clone())
                    },
                    {
                        let leptos_options = leptos_options.clone();
                        move || shell(leptos_options.clone())
                    },
                )
                .fallback(leptos_axum::file_and_error_handler(shell))
                .with_state(leptos_options);

            // run our app with hyper
            // `axum::Server` is a re-export of `hyper::Server`
            axum::serve(listener, app.into_make_service()).await
        }
    }

    pub async fn main() -> color_eyre::Result<()> {
        color_eyre::install()?;
        let cli_args = server_side::Args::parse();
        let config_file = cli_args.config_file.clone();
        let mut args_figment = Figment::new().merge(Serialized::defaults(cli_args));
        if let Some(config_file) = config_file {
            args_figment = args_figment.merge(Yaml::file(config_file));
        }
        let args = args_figment.extract::<Args>().unwrap();

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

        let mut hup_signal = match tokio::signal::unix::signal(SignalKind::hangup()) {
            Ok(signal) => signal,
            Err(e) => {
                error!("Unable to register HUP signal handler: {}", e);
                std::process::exit(1);
            }
        };
        let mut stop_signal = match tokio::signal::unix::signal(SignalKind::interrupt()) {
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
        let conf = get_configuration(None).unwrap();
        listeners
            .add_listener(&SYSTEM_OBJECT, conf.leptos_options.site_addr)
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
            conf.leptos_options.site_addr.to_string(),
            kill_switch.clone(),
            listeners.clone(),
            HostType::TCP,
        );

        select! {
            _ = hup_signal.recv() => {
                warn!("Received HUP signal, reloading configuration...");
            }
            _ = stop_signal.recv() => {
                warn!("Received STOP signal, shutting down...");
                // Perform any cleanup or shutdown tasks here if needed
            }
            _ = host_listen_loop => {
                info!("Host events loop exited.");
            },
            _ = listeners_thread => {
                info!("Listener set exited.");
            }
        }
        Ok(())
    }
}

#[tokio::main]
#[cfg(feature = "ssr")]
async fn main() -> color_eyre::Result<()> {
    use crate::server_side::main;
    main().await
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
