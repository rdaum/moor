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

use moor_db::DatabaseFlavour;
use pem::Pem;
use rand::rngs::OsRng;
use rusty_paseto::core::Key;
use tracing::{info, warn};

use moor_db_wiredtiger::WiredTigerDatabaseBuilder;
use moor_kernel::config::Config;
use moor_kernel::tasks::scheduler::Scheduler;
use moor_kernel::textdump::textdump_load;

use crate::rpc_server::zmq_loop;

#[cfg(feature = "relbox")]
use moor_db_relbox::RelBoxDatabaseBuilder;
use moor_kernel::tasks::NoopTasksDb;

mod connections;

#[cfg(feature = "relbox")]
mod connections_rb;
mod connections_wt;
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
        long,
        value_name = "textdump-output",
        help = "Path to textdump file to write on `dump_database()`, if any"
    )]
    textdump_out: Option<PathBuf>,

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

    #[arg(
        long,
        value_name = "num-io-threads",
        help = "Number of ZeroMQ IO threads to use",
        default_value = "8"
    )]
    num_io_threads: i32,

    #[arg(
        long,
        value_name = "max_buffer_pool_bytes",
        help = "Maximum size of the database buffer pool in bytes when using the `relbox` database. \
                Note that the system will mmap this quantity of memory via mmap, but the actual \
                memory usage (in RSS) will be less than this value. Further, the quantity of memory \
                requested is actually divided up into multiple different mmap regions for different \
                buffer 'size classes'. \
                The default very large number requires vm_overcommit to be set to enabled.",
        default_value = "1099511627776" // 1TB
    )]
    max_buffer_pool_bytes: usize,

    #[arg(
        long,
        value_name = "db-flavour",
        // For those that don't appreciate proper English.
        alias = "db-flavor",
        help = "The database flavour to use",
        default_value = "wiredtiger"
    )]
    db_flavour: DatabaseFlavour,

    #[arg(
        long,
        value_name = "checkpoint-interval-seconds",
        help = "Interval in seconds between database checkpoints",
        default_value = "10"
    )]
    checkpoint_interval_seconds: u16,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,
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
    let (db_source, freshly_made) = match args.db_flavour {
        DatabaseFlavour::WiredTiger => {
            let db_source_builder = WiredTigerDatabaseBuilder::new().with_path(args.db.clone());
            let (db_source, freshly_made) = db_source_builder.open_db().unwrap();
            info!(path = ?args.db, "Opened database");
            (db_source, freshly_made)
        }
        #[cfg(feature = "relbox")]
        DatabaseFlavour::RelBox => {
            let db_source_builder = RelBoxDatabaseBuilder::new()
                .with_path(args.db.clone())
                .with_memory_size(args.max_buffer_pool_bytes);
            let (db_source, freshly_made) = db_source_builder.open_db().unwrap();
            info!(path = ?args.db, "Opened database");
            (db_source, freshly_made)
        }
    };
    info!(path = ?args.db, "Opened database");

    // If the database already existed, do not try to import the textdump...
    if let Some(textdump) = args.textdump {
        if !freshly_made {
            info!("Database already exists, skipping textdump import");
        } else {
            info!("Loading textdump...");
            let start = std::time::Instant::now();
            let mut loader_interface = db_source
                .clone()
                .loader_client()
                .expect("Unable to get loader interface from database");
            textdump_load(loader_interface.as_ref(), textdump).unwrap();
            let duration = start.elapsed();
            info!("Loaded textdump in {:?}", duration);
            loader_interface
                .commit()
                .expect("Failure to commit loaded database...");
        }
    }

    let config = Config {
        textdump_output: args.textdump_out,
    };

    let tasks_db = Box::new(NoopTasksDb {});

    let state_source = db_source
        .clone()
        .world_state_source()
        .expect("Could not get world state source from db");
    // The pieces from core we're going to use:
    //   Our DB.
    //   Our scheduler.
    let scheduler = Scheduler::new(db_source, tasks_db, config);
    let scheduler_client = scheduler.client().expect("Failed to get scheduler client");

    // The scheduler thread:
    let scheduler_loop_jh = std::thread::Builder::new()
        .name("moor-scheduler".to_string())
        .spawn(move || scheduler.run())?;

    let kill_switch = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Background DB checkpoint thread.
    let checkpoint_kill_switch = kill_switch.clone();
    let checkpoint_state_source = state_source.clone();
    let _checkpoint_thread = std::thread::Builder::new()
        .name("moor-checkpoint".to_string())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(
                args.checkpoint_interval_seconds as u64,
            ));
            if checkpoint_kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            checkpoint_state_source
                .checkpoint()
                .expect("Checkpoint failed");
        })?;

    let rpc_kill_switch = kill_switch.clone();
    let rpc_listen = args.rpc_listen.clone();
    let rpc_narrative_listen = args.narrative_listen.clone();
    let rpc_scheduler_client = scheduler_client.clone();
    let rpc_loop_thread = std::thread::Builder::new()
        .name("moor-rpc".to_string())
        .spawn(move || {
            let _ = zmq_loop(
                keypair,
                args.connections_file,
                state_source,
                rpc_scheduler_client,
                rpc_listen,
                rpc_narrative_listen,
                Some(args.num_io_threads),
                rpc_kill_switch,
                args.db_flavour,
            );
        })?;

    signal_hook::flag::register(signal_hook::consts::SIGTERM, kill_switch.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, kill_switch.clone())?;
    info!(
        rpc_endpoint = args.rpc_listen,
        narrative_endpoint = args.narrative_listen,
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
