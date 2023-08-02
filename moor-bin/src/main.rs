extern crate core;

use std::path::PathBuf;
use std::sync::Arc;

use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::RwLock;
use tracing::info;
use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::layer::SubscriberExt;

use moor_lib::db::rocksdb::server::RocksDbServer;
use moor_lib::db::rocksdb::LoaderInterface;
use moor_lib::tasks::scheduler::Scheduler;
use moor_lib::textdump::load_db::textdump_load;
use moor_lib::values::objid::Objid;

use crate::server::ws_server::{ws_server_start, WebSocketServer};

mod server;

#[derive(Parser, Debug)] // requires `derive` feature
struct Args {
    #[arg(value_name = "db", help = "Path to database file to use or create", value_hint = ValueHint::FilePath)]
    db: std::path::PathBuf,

    #[arg(short, long, value_name = "textdump", help = "Path to textdump to import", value_hint = ValueHint::FilePath)]
    textdump: Option<std::path::PathBuf>,

    #[arg(value_name = "listen", help = "Listen address")]
    listen_address: Option<String>,

    #[arg(
        long,
        value_name = "perfetto_tracing",
        help = "Enable perfetto/chromium tracing output"
    )]
    perfetto_tracing: Option<bool>,
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
        .with_max_level(tracing::Level::TRACE)
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

    let mut src = RocksDbServer::new(PathBuf::from(args.db.to_str().unwrap())).unwrap();
    if let Some(textdump) = args.textdump {
        info!("Loading textdump...");
        let start = std::time::Instant::now();
        textdump_load(&mut src, textdump.to_str().unwrap()).unwrap();
        let duration = start.elapsed();
        info!("Loaded textdump in {:?}", duration);
    }

    let tx = src.start_transaction().unwrap();

    // Move wizard (#2) into first room (#70) for purpose of testing, so that there's something to
    // match against.
    tx.set_object_location(Objid(2), Objid(70)).unwrap();
    tx.commit().unwrap();

    let state_src = Arc::new(RwLock::new(src));
    let scheduler = Arc::new(RwLock::new(Scheduler::new(state_src.clone())));

    let addr = args
        .listen_address
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());

    let ws_server = Arc::new(RwLock::new(WebSocketServer::new(scheduler.clone())));
    let mut hup_signal =
        signal(SignalKind::hangup()).expect("Unable to register HUP signal handler");
    let mut stop_signal =
        signal(SignalKind::interrupt()).expect("Unable to register STOP signal handler");

    let mut scheduler_process_interval =
        tokio::time::interval(std::time::Duration::from_millis(100));
    let ws_server_future = tokio::spawn(ws_server_start(ws_server.clone(), addr));

    loop {
        select! {
            _ = scheduler_process_interval.tick() => {
                scheduler.write().await.do_process().await.unwrap();
            }
            _ = hup_signal.recv() => {
                info!("HUP received, stopping...");
                ws_server_future.abort();
                break;
            }
            _ = stop_signal.recv() => {
                info!("STOP received, stopping...");
                ws_server_future.abort();
                break;
            }
        }
    }
    info!("Done.");

    Ok(())
}
