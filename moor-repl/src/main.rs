use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Error;
use async_trait::async_trait;
use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use rustyline_async::{Readline, ReadlineError, SharedWriter};

use tokio::sync::RwLock;
use tracing::{info, warn};

use moor_lib::db::rocksdb::server::RocksDbServer;
use moor_lib::db::rocksdb::LoaderInterface;
use moor_lib::tasks::scheduler::Scheduler;
use moor_lib::tasks::Sessions;
use moor_lib::textdump::load_db::textdump_load;
use moor_lib::values::objid::Objid;

#[derive(Parser, Debug)] // requires `derive` feature
struct Args {
    #[arg(value_name = "db", help = "Path to database file to use or create", value_hint = ValueHint::FilePath)]
    db: std::path::PathBuf,

    #[arg(value_name = "textdump", help = "Path to textdump to import", value_hint = ValueHint::FilePath)]
    textdump: Option<std::path::PathBuf>,
}

async fn do_eval(
    player: Objid,
    scheduler: Arc<RwLock<Scheduler>>,
    program: String,
    sessions: Arc<RwLock<ReplSession>>,
) -> Result<(), anyhow::Error> {
    let task_id = {
        let mut scheduler = scheduler.write().await;
        scheduler.submit_eval_task(player, program, sessions).await
    }?;
    info!("Submitted task {}", task_id);
    Ok(())
}

struct ReplSession {
    player: Objid,
    _console_writer: SharedWriter,
    connect_time: std::time::Instant,
    last_activity: std::time::Instant,
}

#[async_trait]
impl Sessions for ReplSession {
    async fn send_text(&mut self, _player: Objid, msg: &str) -> Result<(), Error> {
        warn!(msg, "NOTIFY");
        Ok(())
    }

    fn connected_players(&self) -> Result<Vec<Objid>, Error> {
        Ok(vec![self.player])
    }

    fn connected_seconds(&self, _player: Objid) -> Result<f64, Error> {
        let now = std::time::Instant::now();
        let duration = now.duration_since(self.connect_time);
        Ok(duration.as_secs_f64())
    }

    fn idle_seconds(&self, _player: Objid) -> Result<f64, Error> {
        let now = std::time::Instant::now();
        let duration = now.duration_since(self.last_activity);
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
        .with_max_level(tracing::Level::INFO)
        .with_writer(Mutex::new(stdout_clone))
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args: Args = Args::parse();

    info!("Moor REPL starting..");

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

    let mut scheduler_process_interval =
        tokio::time::interval(std::time::Duration::from_millis(100));

    let eval_sessions = Arc::new(RwLock::new(ReplSession {
        player: Objid(2),
        _console_writer: stdout.clone(),
        connect_time: std::time::Instant::now(),
        last_activity: std::time::Instant::now(),
    }));
    loop {
        tokio::select! {
            _ = scheduler_process_interval.tick() => {
                scheduler.write().await.do_process().await.unwrap();
            }
            cmd = rl.readline() => match cmd {
                Ok(line) => {
                    rl.add_history_entry(line.clone());
                    eval_sessions.write().await.last_activity = std::time::Instant::now();
                    if let Err(e) = do_eval(Objid(2), scheduler.clone(), line, eval_sessions.clone()).await {
                        writeln!(stdout, "Error: {:?}", e)?;
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
