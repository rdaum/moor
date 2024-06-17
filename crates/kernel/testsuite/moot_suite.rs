//! Moot is a simple text-based test format for testing the kernel.
//!
//! See example.moot for a full-fledged example

mod common;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    sync::{Arc, Once},
};

use common::{create_wiredtiger_db, testsuite_dir};
use eyre::Context;
use moor_db::Database;
use moor_kernel::{
    config::Config,
    tasks::{
        scheduler::{Scheduler, SchedulerError},
        scheduler_test_utils,
        sessions::{NoopClientSession, Session},
    },
};
use moor_moot::{MootRunner, MootState, WIZARD};
use moor_values::var::{v_none, Objid, Var};

#[cfg(feature = "relbox")]
use common::create_relbox_db;

#[derive(Clone)]
struct SchedulerMootRunner {
    scheduler: Arc<Scheduler>,
    session: Arc<dyn Session>,
}
impl SchedulerMootRunner {
    fn new(scheduler: Arc<Scheduler>, session: Arc<dyn Session>) -> Self {
        Self { scheduler, session }
    }
}
impl MootRunner for SchedulerMootRunner {
    type Value = Var;
    type Error = SchedulerError;

    fn eval<S: Into<String>>(&mut self, player: Objid, command: S) -> Result<Var, SchedulerError> {
        scheduler_test_utils::call_eval(
            self.scheduler.clone(),
            self.session.clone(),
            player,
            command.into(),
        )
    }

    fn command<S: AsRef<str>>(&mut self, player: Objid, command: S) -> Result<Var, SchedulerError> {
        scheduler_test_utils::call_command(
            self.scheduler.clone(),
            self.session.clone(),
            player,
            command.as_ref(),
        )
    }

    fn none(&self) -> Var {
        v_none()
    }
}

#[cfg(feature = "relbox")]
fn test_relbox(path: &Path) {
    test(create_relbox_db(), path);
}
#[cfg(feature = "relbox")]
test_each_file::test_each_path! { in "./crates/kernel/testsuite/moot" as relbox => test_relbox }

fn test_wiredtiger(path: &Path) {
    test(create_wiredtiger_db(), path);
}
test_each_file::test_each_path! { in "./crates/kernel/testsuite/moot" as wiredtiger => test_wiredtiger }

#[allow(dead_code)]
static LOGGING_INIT: Once = Once::new();
#[allow(dead_code)]
fn init_logging() {
    LOGGING_INIT.call_once(|| {
        let main_subscriber = tracing_subscriber::fmt()
            .compact()
            .with_ansi(true)
            .with_file(true)
            .with_line_number(true)
            .with_thread_names(true)
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .finish();
        tracing::subscriber::set_global_default(main_subscriber)
            .expect("Unable to set configure logging");
    });
}

fn test(db: Arc<dyn Database + Send + Sync>, path: &Path) {
    init_logging();
    if path.is_dir() {
        return;
    }
    eprintln!("Test definition: {}", path.display());
    let f = BufReader::new(File::open(path).unwrap());

    let scheduler = Arc::new(Scheduler::new(db, Config::default()));
    let loop_scheduler = scheduler.clone();
    let scheduler_loop_jh = std::thread::Builder::new()
        .name("moor-scheduler".to_string())
        .spawn(move || loop_scheduler.run())
        .unwrap();

    let mut state = MootState::new(
        SchedulerMootRunner::new(scheduler.clone(), Arc::new(NoopClientSession::new())),
        WIZARD,
    );
    for (line_no, line) in f.lines().enumerate() {
        state = state
            .process_line(line_no + 1, &line.unwrap())
            .context(format!("line {}", line_no + 1))
            .unwrap();
    }
    state.finalize().unwrap();

    scheduler
        .submit_shutdown(0, Some("Test is done".to_string()))
        .unwrap();
    scheduler_loop_jh.join().unwrap();
}

#[test]
#[ignore = "Useful for debugging; just run a single test"]
fn test_single() {
    // cargo test -p moor-kernel --test moot-suite test_single -- --ignored
    // CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --test moot-suite -- test_single --ignored
    test_wiredtiger(&testsuite_dir().join("moot/single.moot"));
}
