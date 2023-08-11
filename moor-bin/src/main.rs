extern crate core;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::future::ready;

use axum::{routing::get, Extension, Router};
use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::RwLock;
use tower_http::trace;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};
use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::layer::SubscriberExt;

use moor_lib::db::rocksdb::server::RocksDbServer;
use moor_lib::tasks::scheduler::Scheduler;
use moor_lib::textdump::load_db::textdump_load;

use crate::server::ws_server::{ws_connect_handler, WebSocketServer};

mod server;

#[derive(Parser, Debug)] // requires `derive` feature
struct Args {
    #[arg(value_name = "db", help = "Path to database file to use or create", value_hint = ValueHint::FilePath)]
    db: PathBuf,

    // TODO likely this should be removed when we stabilize more.
    // (The reason this is here is because importing a textdump into an existing DB = bad.)
    #[arg(
        short,
        long,
        value_name = "discard_db",
        help = "DANGEROUS; discard existing database; typically used for development when loading from textdump"
    )]
    development_discard_db: bool,

    #[arg(short, long, value_name = "textdump", help = "Path to textdump to import", value_hint = ValueHint::FilePath)]
    textdump: Option<PathBuf>,

    #[arg(value_name = "listen", help = "Listen address")]
    listen_address: Option<String>,

    #[arg(
        long,
        value_name = "perfetto_tracing",
        help = "Enable perfetto/chromium tracing output"
    )]
    perfetto_tracing: Option<bool>,
}

fn setup_metrics_recorder() -> PrometheusHandle {
    PrometheusBuilder::new().install_recorder().unwrap()
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), anyhow::Error> {
    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    let _perfetto_guard = match args.perfetto_tracing {
        Some(true) => {
            let (chrome_layer, _guard) = ChromeLayerBuilder::new().include_args(true).build();

            let with_chrome_tracing = main_subscriber.with(chrome_layer);
            tracing::subscriber::set_global_default(with_chrome_tracing)?;
            Some(_guard)
        }
        _ => {
            tracing::subscriber::set_global_default(main_subscriber)?;
            None
        }
    };

    info!("Moor Server starting...");

    if args.development_discard_db {
        info!("Discarding existing database...");
        std::fs::remove_dir_all(&args.db).unwrap_or_else(|_| {
            info!("Failed to remove existing database, continuing...");
        });
    }

    let mut src = RocksDbServer::new(PathBuf::from(args.db.to_str().unwrap())).unwrap();
    if let Some(textdump) = args.textdump {
        info!("Loading textdump...");
        let start = std::time::Instant::now();
        textdump_load(&mut src, textdump.to_str().unwrap())
            .await
            .unwrap();
        let duration = start.elapsed();
        info!("Loaded textdump in {:?}", duration);
    }

    let state_src = Arc::new(RwLock::new(src));
    let scheduler = Scheduler::new(state_src.clone());

    let addr = args
        .listen_address
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());

    let (shutdown_sender, mut shutdown_receiver) = tokio::sync::mpsc::channel(1);

    let server_scheduler = scheduler.clone();
    let ws_server = Arc::new(RwLock::new(WebSocketServer::new(
        server_scheduler,
        shutdown_sender,
    )));
    let mut hup_signal =
        signal(SignalKind::hangup()).expect("Unable to register HUP signal handler");
    let mut stop_signal =
        signal(SignalKind::interrupt()).expect("Unable to register STOP signal handler");

    let mut loop_scheduler = scheduler.clone();
    let scheduler_loop = tokio::spawn(async move { loop_scheduler.run().await });

    let recorder_handle = setup_metrics_recorder();

    let web_router = Router::new()
        .route("/ws/connect/players/:player", get(ws_connect_handler))
        .layer(Extension(ws_server))
        .layer(
            TraceLayer::new_for_http().make_span_with(
                trace::DefaultMakeSpan::new()
                    .level(Level::TRACE)
                    .include_headers(true),
            ),
        )
        .route("/metrics", get(move || ready(recorder_handle.render())));

    let address = &addr.parse::<SocketAddr>().unwrap();
    info!(address=?address, "Listening");
    let axum_server = tokio::spawn(
        axum::Server::bind(address)
            .serve(web_router.into_make_service_with_connect_info::<SocketAddr>()),
    );

    loop {
        select! {
            _ = shutdown_receiver.recv() => {
                info!("Shutdown received, stopping...");
                scheduler.clone().stop().await.unwrap();
                info!("All tasks stopped.");
                axum_server.abort();
                break;
            }
            _ = scheduler_loop => {
                info!("Scheduler loop exited, stopping...");
                axum_server.abort();
                break;
            }
            _ = hup_signal.recv() => {
                info!("HUP received, stopping...");
                axum_server.abort();
                break;
            }
            _ = stop_signal.recv() => {
                info!("STOP received, stopping...");
                axum_server.abort();
                break;
            }
        }
    }
    info!("Done.");

    Ok(())
}
