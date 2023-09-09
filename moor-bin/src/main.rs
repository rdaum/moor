extern crate core;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use strum::VariantNames;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::RwLock;
use tracing::info;
use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::layer::SubscriberExt;

use crate::repl_session::ReplSession;
use moor_lib::db::{DatabaseBuilder, DatabaseType};
use moor_lib::tasks::scheduler::Scheduler;
use moor_lib::textdump::load_db::textdump_load;
use moor_value::model::world_state::WorldStateSource;
use moor_value::var::objid::Objid;

use crate::server::routes::mk_routes;
use crate::server::ws_server::WebSocketServer;

mod repl_session;
mod server;

#[macro_export]
macro_rules! clap_enum_variants {
    ($e: ty) => {{
        use clap::builder::TypedValueParser;
        clap::builder::PossibleValuesParser::new(<$e>::VARIANTS).map(|s| s.parse::<$e>().unwrap())
    }};
}

#[derive(Parser, Debug)] // requires `derive` feature
struct Args {
    #[arg(value_name = "db", help = "Path to database file to use or create", value_hint = ValueHint::FilePath)]
    db: PathBuf,

    #[arg(short, long, value_name = "textdump", help = "Path to textdump to import", value_hint = ValueHint::FilePath)]
    textdump: Option<PathBuf>,

    #[arg(value_name = "listen", help = "HTTP server listen address.")]
    listen_address: Option<String>,

    #[arg(long,
        value_name = "db-type", help = "Type of database backend to use",
        value_parser = clap_enum_variants!(DatabaseType),
        default_value = "RocksDb"
    )]
    db_type: DatabaseType,

    #[arg(
        long,
        value_name = "perfetto_tracing",
        help = "Enable perfetto/chromium tracing output"
    )]
    perfetto_tracing: Option<bool>,

    #[arg(
        long,
        value_name = "repl",
        help = "Start a Read-Eval-Print loop on the attached console"
    )]
    repl: Option<bool>,
}

async fn run_repl_if_enabled(
    run_repl: Option<bool>,
    scheduler: Scheduler,
    state_source: Arc<dyn WorldStateSource>,
) -> Option<()> {
    if let Some(true) = run_repl {
        info!("Starting Repl...");
        let repl_session = Arc::new(ReplSession {
            player: Objid(2),
            connect_time: std::time::Instant::now(),
            last_activity: RwLock::new(std::time::Instant::now()),
        });

        repl_session
            .session_loop(scheduler.clone(), state_source.clone())
            .await;
        info!("Repl exited.");
    }
    None
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
    let db_source_builder = DatabaseBuilder::new()
        .with_db_type(args.db_type)
        .with_path(args.db.clone());
    let (mut db_source, freshly_made) = db_source_builder.open_db().unwrap();
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

    let state_source = db_source.world_state_source().unwrap();
    let scheduler = Scheduler::new(state_source.clone());

    let (shutdown_sender, mut shutdown_receiver) = tokio::sync::mpsc::channel(1);

    let repl_run = run_repl_if_enabled(args.repl.clone(), scheduler.clone(), state_source.clone());

    let server_scheduler = scheduler.clone();
    let ws_server = WebSocketServer::new(server_scheduler, shutdown_sender);
    let mut hup_signal =
        signal(SignalKind::hangup()).expect("Unable to register HUP signal handler");
    let mut stop_signal =
        signal(SignalKind::interrupt()).expect("Unable to register STOP signal handler");

    let loop_scheduler = scheduler.clone();
    let scheduler_loop = tokio::spawn(async move { loop_scheduler.run().await });

    let main_router = mk_routes(state_source.clone(), ws_server);

    let addr = args
        .listen_address
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());
    let address = &addr.parse::<SocketAddr>().unwrap();
    info!(address=?address, "Listening");
    let axum_server = tokio::spawn(
        axum::Server::bind(address)
            .serve(main_router.into_make_service_with_connect_info::<SocketAddr>()),
    );

    select! {
        _ = shutdown_receiver.recv() => {
            info!("Shutdown received, stopping...");
            scheduler.clone().stop().await.unwrap();
            info!("All tasks stopped.");
            axum_server.abort();
        },
        Some(_) = repl_run => {
            info!("Readline loop exited");
        },
        _ = scheduler_loop => {
            info!("Scheduler loop exited, stopping...");
            axum_server.abort();
        },
        _ = hup_signal.recv() => {
            info!("HUP received, stopping...");
            axum_server.abort();
        },
        _ = stop_signal.recv() => {
            info!("STOP received, stopping...");
            axum_server.abort();
        }
    }
    info!("Done.");

    Ok(())
}
