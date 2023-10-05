use std::path::PathBuf;

use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use strum::VariantNames;
use tmq::Multipart;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;

use moor_db::{DatabaseBuilder, DatabaseType};
use moor_kernel::tasks::scheduler::Scheduler;
use moor_kernel::textdump::load_db::textdump_load;
use rpc_common::{RpcRequestError, RpcResponse, RpcResult};

use crate::rpc_server::zmq_loop;

mod connections;
mod rpc_server;
mod rpc_session;

#[macro_export]
macro_rules! clap_enum_variants {
    ($e: ty) => {{
        use clap::builder::TypedValueParser;
        clap::builder::PossibleValuesParser::new(<$e>::VARIANTS).map(|s| s.parse::<$e>().unwrap())
    }};
}

/// Host for the moor runtime.
///   * Brings up the database
///   * Instantiates a scheduler
///   * Exposes RPC interface for session/connection management.

#[derive(Parser, Debug)] // requires `derive` feature
struct Args {
    #[arg(value_name = "db", help = "Path to database file to use or create", value_hint = ValueHint::FilePath)]
    db: PathBuf,

    #[arg(short, long, value_name = "textdump", help = "Path to textdump to import", value_hint = ValueHint::FilePath)]
    textdump: Option<PathBuf>,

    #[arg(
        short,
        long,
        value_name = "connections-file",
        help = "Path to connections list file to use or create",
        value_hint = ValueHint::FilePath,
        default_value = "connections.json"
    )]
    connections_file: PathBuf,

    #[arg(long,
        value_name = "db-type", help = "Type of database backend to use",
        value_parser = clap_enum_variants!(DatabaseType),
        default_value = "RocksDb"
    )]
    db_type: DatabaseType,

    #[arg(
        long,
        value_name = "rpc-listen",
        help = "RPC server address",
        default_value = "tcp://0.0.0.0:7899"
    )]
    rpc_listen: String,

    #[arg(
        long,
        value_name = "narrative-listen",
        help = "Narrative server address",
        default_value = "tcp://0.0.0.0:7898"
    )]
    narrative_listen: String,
}

pub(crate) fn make_response(result: Result<RpcResponse, RpcRequestError>) -> Multipart {
    let mut payload = Multipart::default();
    let rpc_result = match result {
        Ok(r) => RpcResult::Success(r),
        Err(e) => RpcResult::Failure(e),
    };
    payload.push_back(
        bincode::encode_to_vec(&rpc_result, bincode::config::standard())
            .unwrap()
            .into(),
    );
    payload
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    let builder = PrometheusBuilder::new();
    builder
        .install()
        .expect("failed to install Prometheus recorder");

    info!("Daemon starting...");
    let db_source_builder = DatabaseBuilder::new()
        .with_db_type(args.db_type)
        .with_path(args.db.clone());
    let (mut db_source, freshly_made) = db_source_builder.open_db().await.unwrap();
    info!(db_type = ?args.db_type, path = ?args.db, "Opened database");

    // If the database already existed, do not try to import the textdump...
    if let Some(textdump) = args.textdump {
        if !freshly_made {
            info!("Database already exists, skipping textdump import");
        } else {
            info!("Loading textdump...");
            let start = std::time::Instant::now();
            let mut loader_interface = db_source
                .loader_client()
                .expect("Unable to get loader interface from database");
            textdump_load(loader_interface.as_mut(), textdump.to_str().unwrap())
                .await
                .unwrap();
            let duration = start.elapsed();
            info!("Loaded textdump in {:?}", duration);
            loader_interface
                .commit()
                .await
                .expect("Failure to commit loaded database...");
        }
    }

    // Unix-signals for better exit behaviour.
    let mut hup_signal =
        signal(SignalKind::hangup()).expect("Unable to register HUP signal handler");
    let mut stop_signal =
        signal(SignalKind::interrupt()).expect("Unable to register STOP signal handler");

    // The pieces from core we're going to use:
    //   Our DB.
    let state_source = db_source.world_state_source().unwrap();
    //   Our scheduler.
    let scheduler = Scheduler::new(state_source.clone());

    // The scheduler thread:
    let loop_scheduler = scheduler.clone();
    let scheduler_loop = tokio::spawn(async move { loop_scheduler.run().await });

    let zmq_server_loop = zmq_loop(
        args.connections_file,
        state_source,
        scheduler.clone(),
        args.rpc_listen.as_str(),
        args.narrative_listen.as_str(),
    );

    info!(
        rpc_endpoint = args.rpc_listen,
        narrative_endpoint = args.narrative_listen,
        "Daemon started. Listening for RPC events."
    );
    select! {
        _ = scheduler_loop => {
            info!("Scheduler loop exited, stopping...");
            scheduler.stop().await.unwrap();
        },
        _ = zmq_server_loop => {
            info!("ZMQ server loop exited, stopping...");
            scheduler.stop().await.unwrap();
        }
        _ = hup_signal.recv() => {
            info!("HUP received, stopping...");
            scheduler.stop().await.unwrap();
        },
        _ = stop_signal.recv() => {
            info!("STOP received, stopping...");
            scheduler.stop().await.unwrap();
        }
    }
    info!("Done.");
}
