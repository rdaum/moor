extern crate core;

use std::io;
use std::sync::Arc;

use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::RwLock;
use tracing::info;

use moor_lib::db::moor_db::MoorDB;
use moor_lib::db::moor_db_worldstate::MoorDbWorldStateSource;
use moor_lib::model::objects::ObjAttrs;
use moor_lib::tasks::scheduler::Scheduler;
use moor_lib::textdump::load_db::textdump_load;
use moor_lib::var::Objid;

use crate::server::ws_server::{WebSocketServer, ws_server_start};

mod server;

#[derive(Parser, Debug)] // requires `derive` feature
struct Args {
    #[arg(value_name = "db", help = "Path to database file to use or create", value_hint = ValueHint::FilePath)]
    db: std::path::PathBuf,

    #[arg(value_name = "textdump", help = "Path to textdump to import", value_hint = ValueHint::FilePath)]
    textdump: Option<std::path::PathBuf>,

    #[arg(value_name = "listen", help = "Listen address")]
    listen_address: Option<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), io::Error> {
    tracing_subscriber::fmt::init();

    let args: Args = Args::parse();

    info!("Moor");

    let mut src = MoorDB::new();
    if let Some(textdump) = args.textdump {
        info!("Loading textdump...");
        let start = std::time::Instant::now();
        textdump_load(&mut src, textdump.to_str().unwrap()).unwrap();
        let duration = start.elapsed();
        info!("Loaded textdump in {:?}", duration);
    }

    let mut tx = src.do_begin_tx().unwrap();

    // Move wizard (#2) into first room (#70) for purpose of testing, so that there's something to
    // match against.
    src.object_set_attrs(
        &mut tx,
        Objid(2),
        ObjAttrs {
            owner: None,
            name: None,
            parent: None,
            location: Some(Objid(70)),
            flags: None,
        },
    )
    .unwrap();
    src.do_commit_tx(&mut tx).unwrap();

    let state_src = Arc::new(RwLock::new(MoorDbWorldStateSource::new(src)));
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
