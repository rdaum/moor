// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use std::path::PathBuf;
use std::sync::Arc;

use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use ed25519_dalek::SigningKey;
use eyre::Report;

use crate::rpc_server::RpcServer;
use moor_db::{Database, TxDB};
use moor_kernel::config::Config;
use moor_kernel::tasks::scheduler::Scheduler;
use moor_kernel::textdump::{textdump_load, EncodingMode};
use pem::Pem;
use rand::rngs::OsRng;
use rusty_paseto::core::Key;
use tracing::{info, warn};

use rpc_common::load_keypair;

mod connections;

mod connections_fjall;
mod rpc_hosts;
mod rpc_server;
mod rpc_session;
mod sys_ctrl;
mod tasks_fjall;

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
    #[arg(value_name = "db", help = "Path to database file to use or create", value_hint = ValueHint::FilePath
    )]
    db: PathBuf,

    #[arg(short, long, value_name = "textdump", help = "Path to textdump to import", value_hint = ValueHint::FilePath
    )]
    textdump: Option<PathBuf>,

    #[arg(
        long,
        value_name = "textdump-output",
        help = "Path to textdump file to write on `dump_database()`, if any"
    )]
    textdump_out: Option<PathBuf>,

    #[arg(
        long,
        value_name = "textdump-encoding",
        help = "Encoding to use for reading and writing textdump files. utf8 or iso8859-1. \
          LambdaMOO textdumps that contain 8-bit strings are written using iso8859-1, so for full compatibility, \
          choose iso8859-1.
          If you know your textdump contains no such strings, or if your textdump is from moor choose utf8,
          which is faster to read.",
        default_value = "utf8"
    )]
    textdump_encoding: EncodingMode,

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
        short = 'x',
        long,
        value_name = "tasks-db",
        help = "Path to persistent tasks database to use or create",
        value_hint = ValueHint::FilePath,
        default_value = "tasks.db"
    )]
    tasks_db: PathBuf,

    #[arg(
        long,
        value_name = "rpc-listen",
        help = "RPC server address",
        default_value = "ipc:///tmp/moor_rpc.sock"
    )]
    rpc_listen: String,

    #[arg(
        long,
        value_name = "events-listen",
        help = "Events publisher listen address",
        default_value = "ipc:///tmp/moor_events.sock"
    )]
    events_listen: String,

    #[arg(
        long,
        value_name = "public_key",
        help = "file containing a pkcs8 ed25519 public key, used for authenticating client & host connections",
        default_value = "public_key.pem"
    )]
    public_key: PathBuf,

    #[arg(
        long,
        value_name = "private_key",
        help = "file containing a pkcs8 ed25519 private key, used for authenticating client & host connections",
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

    #[arg(
        long,
        value_name = "num-io-threads",
        help = "Number of ZeroMQ IO threads to use",
        default_value = "8"
    )]
    num_io_threads: i32,

    #[arg(
        long,
        value_name = "checkpoint-interval-seconds",
        help = "Interval in seconds between database checkpoints",
        default_value = "240"
    )]
    checkpoint_interval_seconds: u16,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,

    /// Whether to allow notify() to send arbitrary MOO common to players. The interpretation of
    /// the common varies depending on host/client.
    /// If this is false, only strings are allowed, as in LambdaMOO.
    #[arg(
        long,
        help = "Enable rich_notify, allowing notify() to send arbitrary MOO common to players. \
                The interpretation of the common varies depending on host/client. \
                If this is false, only strings are allowed, as in LambdaMOO.",
        default_value = "true"
    )]
    rich_notify: bool,

    #[arg(
        long,
        help = "Enable block-level lexical scoping in programs. \
                Adds the `begin`/`end` syntax for creating lexical scopes, and `let` and `global`
                for declaring variables. \
                This is a feature that is not present in LambdaMOO, so if you need backwards compatibility, turn this off.",
        default_value = "true"
    )]
    lexical_scopes: bool,

    #[arg(
        long,
        help = "Enable the Map datatype ([ k -> v, .. ]) compatible with Stunt/ToastStunt",
        default_value = "true"
    )]
    map_type: bool,

    #[arg(
        long,
        help = "Enable primitive-type verb dispatching. E.g. \"test\":reverse() becomes $string:reverse(\"test\")",
        default_value = "true"
    )]
    type_dispatch: bool,

    #[arg(
        long,
        help = "Enable flyweight types. Flyweights are a lightweight, object delegate",
        default_value = "true"
    )]
    flyweight_type: bool,
}

fn main() -> Result<(), Report> {
    color_eyre::install()?;

    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_max_level(if args.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    // Check the public/private keypair file to see if it exists. If it does, parse it and establish
    // the keypair from it...
    let keypair = if args.public_key.exists() && args.private_key.exists() {
        load_keypair(&args.public_key, &args.private_key)
            .expect("Unable to load keypair from public and private key files")
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

    info!("Daemon starting. Using database at {:?}", args.db);
    let (database, freshly_made) = TxDB::open(Some(&args.db));
    let database = Box::new(database);
    info!(path = ?args.db, "Opened database");

    let config = Arc::new(Config {
        textdump_output: args.textdump_out,
        textdump_encoding: args.textdump_encoding,
        rich_notify: args.rich_notify,
        lexical_scopes: args.lexical_scopes,
        map_type: args.map_type,
        type_dispatch: args.type_dispatch,
        flyweight_type: args.flyweight_type,
    });

    // If the database already existed, do not try to import the textdump...
    if let Some(textdump) = args.textdump {
        if !freshly_made {
            info!("Database already exists, skipping textdump import");
        } else {
            info!("Loading textdump...");
            let start = std::time::Instant::now();
            let loader_interface = database
                .loader_client()
                .expect("Unable to get loader interface from database");
            textdump_load(
                loader_interface.as_ref(),
                textdump,
                args.textdump_encoding,
                config.compile_options(),
            )
            .unwrap();
            let duration = start.elapsed();
            info!("Loaded textdump in {:?}", duration);
            loader_interface
                .commit()
                .expect("Failure to commit loaded database...");
        }
    }

    let (tasks_db, _) = tasks_fjall::FjallTasksDB::open(&args.tasks_db);

    // We have to create the RpcServer before starting the scheduler because we need to pass it in
    // as a parameter to the scheduler for background session construction.

    let zmq_ctx = zmq::Context::new();
    zmq_ctx
        .set_io_threads(args.num_io_threads)
        .expect("Failed to set number of IO threads");
    let rpc_server = Arc::new(RpcServer::new(
        keypair,
        args.connections_file,
        zmq_ctx.clone(),
        args.events_listen.as_str(),
        config.clone(),
    ));
    let kill_switch = rpc_server.kill_switch();

    // The pieces from core we're going to use:
    //   Our DB.
    //   Our scheduler.
    let scheduler = Scheduler::new(database, Box::new(tasks_db), config, rpc_server.clone());
    let scheduler_client = scheduler.client().expect("Failed to get scheduler client");

    // The scheduler thread:
    let scheduler_rpc_server = rpc_server.clone();
    let scheduler_loop_jh = std::thread::Builder::new()
        .name("moor-scheduler".to_string())
        .spawn(move || scheduler.run(scheduler_rpc_server))?;

    // Background DB checkpoint thread.
    let checkpoint_kill_switch = kill_switch.clone();
    let checkpoint_scheduler_client = scheduler_client.clone();
    let _checkpoint_thread = std::thread::Builder::new()
        .name("moor-checkpoint".to_string())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(
                args.checkpoint_interval_seconds as u64,
            ));
            if checkpoint_kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            checkpoint_scheduler_client
                .request_checkpoint()
                .expect("Failed to submit checkpoint");
        })?;

    let rpc_loop_scheduler_client = scheduler_client.clone();
    let rpc_listen = args.rpc_listen.clone();
    let rpc_loop_thread = std::thread::Builder::new()
        .name("moor-rpc".to_string())
        .spawn(move || {
            rpc_server
                .request_loop(rpc_listen, rpc_loop_scheduler_client)
                .expect("RPC thread failed");
        })?;

    signal_hook::flag::register(signal_hook::consts::SIGTERM, kill_switch.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, kill_switch.clone())?;
    info!(
        rpc_endpoint = args.rpc_listen,
        events_endpoint = args.events_listen,
        "Daemon started. Listening for RPC events."
    );
    rpc_loop_thread.join().expect("RPC thread panicked");
    warn!("RPC thread exited. Departing...");

    scheduler_client
        .submit_shutdown("System shutting down")
        .expect("Scheduler thread failed to stop");
    scheduler_loop_jh.join().expect("Scheduler thread panicked");
    info!("Done.");

    Ok(())
}
