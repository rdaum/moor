use std::path::PathBuf;
use std::sync::Arc;

use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use strum::VariantNames;
use tokio::sync::RwLock;
use tokio::task::block_in_place;
use tracing::info;

use moor_lib::db::{DatabaseBuilder, DatabaseType};
use moor_lib::tasks::scheduler::Scheduler;
use moor_lib::textdump::load_db::textdump_load;
use moor_value::var::objid::Objid;

use crate::repl_session::ReplSession;

mod repl_session;

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
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), anyhow::Error> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let args: Args = Args::parse();

    info!("Moor REPL starting...");

    let db_source_builder = DatabaseBuilder::new()
        .with_db_type(args.db_type)
        .with_path(args.db.clone());
    let (mut db_source, freshly_made) = db_source_builder.open_db().unwrap();
    info!(db_type = ?args.db_type, path = ?args.db, "Opened database");

    if let Some(textdump) = args.textdump {
        if !freshly_made {
            info!("Database already exists, skipping textdump import");
        } else {
            info!("Loading textdump. Please wait...");

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

    let state_source = db_source
        .world_state_source()
        .expect("Unable to get world state source from database");
    let scheduler = Scheduler::new(state_source.clone());

    let repl_session = Arc::new(ReplSession {
        player: Objid(2),
        connect_time: std::time::Instant::now(),
        last_activity: RwLock::new(std::time::Instant::now()),
    });

    let scheduler_clone = scheduler.clone();
    let scheduler_loop = tokio::spawn(async move { scheduler_clone.run().await });

    let readline_loop = tokio::spawn(async move {
        let mut rl = DefaultEditor::new().unwrap();
        loop {
            let output = block_in_place(|| rl.readline("> "));
            match output {
                Ok(line) => {
                    rl.add_history_entry(line.clone())
                        .expect("Could not add history");
                    if let Err(e) = repl_session
                        .clone()
                        .handle_input(Objid(2), scheduler.clone(), line, state_source.clone())
                        .await
                    {
                        println!("Error: {e:?}");
                    }
                }
                Err(ReadlineError::Eof) => {
                    println!("<EOF>");
                    break;
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    continue;
                }
                Err(e) => {
                    println!("Error: {e:?}");
                    break;
                }
            }
        }
    });

    tokio::select! {
        _ = scheduler_loop => {
           info!("Scheduler loop exited, stopping...");
        }
        _ = readline_loop => {
           info!("Readline loop exited, stopping...");
        }
    }

    Ok(())
}
