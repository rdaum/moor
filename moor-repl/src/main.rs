use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::sync::{Arc, Mutex};

use anyhow::Error;
use async_trait::async_trait;
use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use rustyline_async::{Readline, ReadlineError, SharedWriter};
use strum::VariantNames;

use moor_lib::db::{DatabaseBuilder, DatabaseType};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use moor_lib::tasks::scheduler::Scheduler;
use moor_lib::tasks::sessions::Session;
use moor_lib::textdump::load_db::textdump_load;

use moor_value::var::objid::Objid;

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

async fn do_eval(
    player: Objid,
    scheduler: Scheduler,
    program: String,
    sessions: Arc<ReplSession>,
) -> Result<(), anyhow::Error> {
    let task_id = scheduler
        .submit_eval_task(player, player, program, sessions)
        .await?;
    info!("Submitted task {}", task_id);
    Ok(())
}

struct ReplSession {
    player: Objid,
    _console_writer: SharedWriter,
    connect_time: std::time::Instant,
    last_activity: RwLock<std::time::Instant>,
}

#[async_trait]
impl Session for ReplSession {
    async fn commit(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn rollback(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, Error> {
        Ok(self.clone())
    }

    async fn send_text(&self, _player: Objid, msg: &str) -> Result<(), Error> {
        info!(msg, "NOTIFY");
        Ok(())
    }

    async fn send_system_msg(&self, _player: Objid, msg: &str) -> Result<(), Error> {
        warn!(msg, "SYSMSG");
        Ok(())
    }

    async fn shutdown(&self, msg: Option<String>) -> Result<(), Error> {
        error!(msg, "SHUTDOWN");
        exit(0);
    }

    async fn connection_name(&self, player: Objid) -> Result<String, Error> {
        Ok(format!("REPL:{player}"))
    }

    async fn disconnect(&self, player: Objid) -> Result<(), Error> {
        error!(?player, "DISCONNECT");
        exit(0);
    }

    async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
        Ok(vec![self.player])
    }

    async fn connected_seconds(&self, _player: Objid) -> Result<f64, Error> {
        let now = std::time::Instant::now();
        let duration = now.duration_since(self.connect_time);
        Ok(duration.as_secs_f64())
    }

    async fn idle_seconds(&self, _player: Objid) -> Result<f64, Error> {
        let now = std::time::Instant::now();
        let last_activity = self.last_activity.read().await;
        let duration = now.duration_since(*last_activity);
        Ok(duration.as_secs_f64())
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), anyhow::Error> {
    let (mut rl, mut stdout) = Readline::new("> ".to_owned()).unwrap();

    let stdout_clone = stdout.clone();
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(Mutex::new(stdout_clone))
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args: Args = Args::parse();

    info!("Moor REPL starting..");
    let db_source_builder = DatabaseBuilder::new()
        .with_db_type(args.db_type)
        .with_path(args.db.clone());
    let (mut db_source, freshly_made) = db_source_builder.open_db().unwrap();
    info!(db_type = ?args.db_type, path = ?args.db, "Opened database");

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
    let state_source = db_source
        .world_state_source()
        .expect("Unable to get world state source from database");
    let scheduler = Scheduler::new(state_source);

    let eval_sessions = Arc::new(ReplSession {
        player: Objid(2),
        _console_writer: stdout.clone(),
        connect_time: std::time::Instant::now(),
        last_activity: RwLock::new(std::time::Instant::now()),
    });

    loop {
        let loop_scheduler = scheduler.clone();
        let scheduler_loop = tokio::spawn(async move { loop_scheduler.run().await });

        tokio::select! {
            _ = scheduler_loop => {
               writeln!(stdout, "Scheduler loop exited, stopping...")?;
               break;
            }
            cmd = rl.readline() => match cmd {
                Ok(line) => {
                    rl.add_history_entry(line.clone());
                    (*eval_sessions.last_activity.write().await) = std::time::Instant::now();
                    if let Err(e) = do_eval(Objid(2), scheduler.clone(), line, eval_sessions.clone()).await {
                        writeln!(stdout, "Error: {e:?}")?;
                    }
                }
                Err(ReadlineError::Eof) => {
                    writeln!(stdout, "<EOF>")?;
                    break;
                }
                Err(ReadlineError::Interrupted) => {writeln!(stdout, "^C")?; continue; }
                Err(e) => {
                    writeln!(stdout, "Error: {e:?}")?;
                    break;
                }
            }
        }
    }
    rl.flush()?;
    Ok(())
}
