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

use common::{create_wiredtiger_db, testsuite_dir, NONPROGRAMMER, PROGRAMMER};
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
use moor_values::var::{v_none, Objid, Var};
use pretty_assertions::assert_eq;

use crate::common::WIZARD;

#[cfg(feature = "relbox")]
use common::create_relbox_db;

#[derive(Clone, Copy, Debug)]
enum CommandKind {
    Eval,
    Command,
}
impl From<char> for CommandKind {
    fn from(c: char) -> Self {
        match c {
            ';' => CommandKind::Eval,
            '%' => CommandKind::Command,
            _ => panic!("Unknown command kind: {}", c),
        }
    }
}

trait MootRunner {
    fn eval<S: Into<String>>(&mut self, player: Objid, command: S) -> Result<Var, SchedulerError>;
    fn command<S: AsRef<str>>(&mut self, player: Objid, command: S) -> Result<Var, SchedulerError>;
}

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
}

enum MootState<R: MootRunner> {
    Ready {
        runner: R,
        player: Objid,
    },
    ReadingCommand {
        runner: R,
        player: Objid,
        line_no: usize,
        command: String,
        command_kind: CommandKind,
    },
    ReadingExpectation {
        runner: R,
        player: Objid,
        line_no: usize,
        command: String,
        command_kind: CommandKind,
        expectation: String,
    },
}
impl<R: MootRunner> MootState<R> {
    fn new(runner: R, player: Objid) -> Self {
        MootState::Ready { runner, player }
    }

    fn into_runner(self) -> R {
        match self {
            MootState::Ready { runner, .. } => runner,
            MootState::ReadingCommand { runner, .. } => runner,
            MootState::ReadingExpectation { runner, .. } => runner,
        }
    }

    // Could implement this with `nom` I guess, but this seems simple enough, and it's probably easier to read.
    fn process_line(self, new_line_no: usize, line: &str) -> eyre::Result<Self> {
        let line = line.trim_end_matches('\n');
        match self {
            MootState::Ready { runner, player } => {
                if line.starts_with([';', '%']) {
                    Ok(MootState::ReadingCommand {
                        runner,
                        player,
                        line_no: new_line_no,
                        command: line[1..].trim_start().to_string(),
                        command_kind: line.chars().next().unwrap().into(),
                    })
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Ok(MootState::new(runner, Self::player(new_player)?))
                } else if line.is_empty() || line.starts_with("//") {
                    Ok(MootState::new(runner, player))
                } else {
                    Err(eyre::eyre!(
                        "Expected a command (starting `;`), a comment (starting `//`), a player switch (starting `@`), a command (starting `%`), or an empty line"
                    ))
                }
            }
            MootState::ReadingCommand {
                mut runner,
                player,
                line_no,
                mut command,
                command_kind,
            } => {
                if let Some(rest) = line.strip_prefix('>') {
                    command.push_str(rest);
                    Ok(MootState::ReadingCommand {
                        runner,
                        player,
                        line_no,
                        command,
                        command_kind,
                    })
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Self::execute_test(&mut runner, player, &command, command_kind, None, line_no)?;
                    Ok(MootState::new(runner, Self::player(new_player)?))
                } else if line.starts_with([';', '%']) || line.is_empty() {
                    Self::execute_test(&mut runner, player, &command, command_kind, None, line_no)?;
                    MootState::new(runner, player).process_line(new_line_no, line)
                } else {
                    let line = line.strip_prefix('<').unwrap_or(line);
                    Ok(MootState::ReadingExpectation {
                        runner,
                        player,
                        line_no,
                        command,
                        command_kind,
                        expectation: line.to_string(),
                    })
                }
            }
            MootState::ReadingExpectation {
                mut runner,
                player,
                line_no,
                command,
                command_kind,
                mut expectation,
            } => {
                if line.is_empty() || line.starts_with("//") || line.starts_with([';', '%']) {
                    Self::execute_test(
                        &mut runner,
                        player,
                        &command,
                        command_kind,
                        Some(&expectation),
                        line_no,
                    )?;
                }
                if line.is_empty() || line.starts_with("//") {
                    Ok(MootState::new(runner, player))
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Ok(MootState::new(runner, Self::player(new_player)?))
                } else if line.starts_with([';', '%']) {
                    MootState::new(runner, player).process_line(new_line_no, line)
                } else {
                    expectation.push('\n');
                    let line = line.strip_prefix('<').unwrap_or(line);
                    expectation.push_str(line);
                    Ok(MootState::ReadingExpectation {
                        runner,
                        player,
                        line_no,
                        command,
                        command_kind,
                        expectation,
                    })
                }
            }
        }
    }

    fn finalize(self) -> eyre::Result<()> {
        match self {
            MootState::Ready { .. } => Ok(()),
            MootState::ReadingCommand {
                mut runner,
                player,
                command,
                line_no,
                command_kind,
            } => Self::execute_test(&mut runner, player, &command, command_kind, None, line_no),
            MootState::ReadingExpectation {
                mut runner,
                player,
                line_no,
                command,
                command_kind,
                expectation,
            } => Self::execute_test(
                &mut runner,
                player,
                &command,
                command_kind,
                Some(&expectation),
                line_no,
            ),
        }
    }

    fn player(s: &str) -> eyre::Result<Objid> {
        match s {
            "wizard" => Ok(WIZARD),
            "programmer" => Ok(PROGRAMMER),
            "nonprogrammer" => Ok(NONPROGRAMMER),
            _ => Err(eyre::eyre!("Unknown player: {s}")),
        }
    }

    fn execute_test(
        runner: &mut R,
        player: Objid,
        command: &str,
        command_kind: CommandKind,
        expectation: Option<&str>,
        line_no: usize,
    ) -> eyre::Result<()> {
        let expected = if let Some(expectation) = expectation {
            runner
                .eval(WIZARD, format!("return {expectation};"))
                .context(format!("Failed to compile expected output: {expectation}"))?
        } else {
            v_none()
        };

        let actual = match command_kind {
            CommandKind::Eval => runner.eval(player, command),
            CommandKind::Command => runner.command(player, command),
        }?;
        assert_eq!(actual, expected, "Line {line_no}: {command}");
        Ok(())
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
