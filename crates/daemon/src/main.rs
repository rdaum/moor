use std::path::PathBuf;

use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use ed25519_dalek::SigningKey;
use metrics_exporter_prometheus::PrometheusBuilder;
use pem::Pem;
use rand::rngs::OsRng;
use rusty_paseto::core::Key;
use tmq::Multipart;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;

use moor_db::DatabaseBuilder;
use moor_kernel::tasks::scheduler::Scheduler;
use moor_kernel::textdump::load_db::textdump_load;
use rpc_common::{RpcRequestError, RpcResponse, RpcResult};

use crate::rpc_server::zmq_loop;

mod connections;
mod connections_tb;
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
        value_name = "connections-db",
        help = "Path to connections database to use or create",
        value_hint = ValueHint::FilePath,
        default_value = "connections.db"
    )]
    connections_file: PathBuf,

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

    #[arg(
        long,
        value_name = "public_key",
        help = "file containing a pkcs8 ed25519 public key, used for authenticating client connections",
        default_value = "public_key.pem"
    )]
    public_key: PathBuf,

    #[arg(
        long,
        value_name = "private_key",
        help = "file containing a pkcs8 ed25519 private key, used for authenticating client connections",
        default_value = "private_key.pem"
    )]
    private_key: PathBuf,

    #[arg(
        long,
        value_name = "generate-keypair",
        help = "Generate a new keypair and save it to the keypair files, if they don't exist already",
        default_value = "false"
    )]
    generate_keypair: bool,
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

    // Check the public/private keypair file to see if it exists. If it does, parse it and establish
    // the keypair from it...
    let keypair = if args.public_key.exists() && args.private_key.exists() {
        let privkey_pem = std::fs::read(args.private_key).expect("Unable to read private key");
        let pubkey_pem = std::fs::read(args.public_key).expect("Unable to read public key");

        let privkey_pem = pem::parse(privkey_pem).expect("Unable to parse private key");
        let pubkey_pem = pem::parse(pubkey_pem).expect("Unable to parse public key");

        let mut key_bytes = privkey_pem.contents().to_vec();
        key_bytes.extend_from_slice(pubkey_pem.contents());

        let key: Key<64> = Key::from(&key_bytes[0..64]);
        key
    } else {
        // Otherwise, check to see if --generate-keypair was passed. If it was, generate a new
        // keypair and save it to the file; otherwise, error out.

        if args.generate_keypair {
            let mut csprng = OsRng;
            let signing_key: SigningKey = SigningKey::generate(&mut csprng);
            let keypair: Key<64> = Key::from(signing_key.to_keypair_bytes());

            let privkey_pem = Pem::new("PRIVATE KEY", signing_key.to_bytes());
            let pubkey_pem = Pem::new("PUBLIC KEY", signing_key.verifying_key().to_bytes());

            // And write to the files...
            std::fs::write(args.private_key, pem::encode(&privkey_pem))
                .expect("Unable to write private key");
            std::fs::write(args.public_key, pem::encode(&pubkey_pem))
                .expect("Unable to write public key");

            keypair
        // Write
        } else {
            panic!(
                "Public/private keypair files do not exist, and --generate-keypair was not passed"
            );
        }
    };

    info!("Daemon starting...");
    let db_source_builder = DatabaseBuilder::new().with_path(args.db.clone());
    let (mut db_source, freshly_made) = db_source_builder.open_db().await.unwrap();
    info!(path = ?args.db, "Opened database");

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
        keypair,
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
