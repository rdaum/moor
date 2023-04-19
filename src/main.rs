extern crate core;
#[macro_use]
extern crate pest_derive;

use std::io;
use std::sync::Arc;

use clap::Parser;
use clap_derive::Parser;
use tokio::sync::Mutex;
use tracing::info;

use crate::db::inmem_db::ImDB;
use crate::db::inmem_db_worldstate::ImDbWorldStateSource;
use crate::model::objects::ObjAttrs;
use crate::model::var::Objid;
use crate::server::scheduler::Scheduler;
use crate::server::ws_server::{ws_server_start, WebSocketServer};
use crate::textdump::load_db::textdump_load;
use clap::builder::ValueHint;

pub mod compiler;
pub mod db;
pub mod model;
pub mod server;
pub mod textdump;
pub mod util;
pub mod vm;

#[derive(Parser, Debug)] // requires `derive` feature
struct Args {
    #[arg(value_name = "db", help = "Path to database file to use or create", value_hint = ValueHint::FilePath)]
    db: std::path::PathBuf,

    #[arg(value_name = "textdump", help = "Path to textdump to import", value_hint = ValueHint::FilePath)]
    textdump: Option<std::path::PathBuf>,

    #[arg(value_name = "listen", help = "Listen address")]
    listen_address: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    tracing_subscriber::fmt::init();

    let args: Args = Args::parse();

    info!("Moor");

    let mut src = ImDB::new();
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

    let state_src = Arc::new(Mutex::new(ImDbWorldStateSource::new(src)));
    let scheduler = Arc::new(Mutex::new(Scheduler::new(state_src.clone())));

    let addr = args
        .listen_address
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());

    let ws_server = Arc::new(Mutex::new(WebSocketServer::new(scheduler)));
    ws_server_start(ws_server, addr)
        .await
        .expect("Unable to run websocket server");

    info!("Done.");

    Ok(())
}
