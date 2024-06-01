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

use common::{create_relbox_db, create_wiretiger_db, testsuite_dir, NONPROGRAMMER, PROGRAMMER};
use eyre::Context;
use moor_db::Database;
use moor_kernel::tasks::{
    scheduler_test_utils,
    sessions::{NoopClientSession, Session},
};
use moor_values::var::{v_none, Objid};
use pretty_assertions::assert_eq;

use crate::common::WIZARD;

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

enum MootState {
    Ready {
        session: Arc<dyn Session>,
        player: Objid,
    },
    ReadingCommand {
        session: Arc<dyn Session>,
        player: Objid,
        line_no: usize,
        command: String,
        command_kind: CommandKind,
    },
    ReadingExpectation {
        session: Arc<dyn Session>,
        player: Objid,
        line_no: usize,
        command: String,
        command_kind: CommandKind,
        expectation: String,
    },
}
impl MootState {
    fn new(session: Arc<dyn Session>, player: Objid) -> Self {
        MootState::Ready { session, player }
    }

    // Could implement this with `nom` I guess, but this seems simple enough, and it's probably easier to read.
    fn process_line(
        self,
        new_line_no: usize,
        line: &str,
        db: Arc<dyn Database + Send + Sync>,
    ) -> eyre::Result<Self> {
        let line = line.trim_end_matches('\n');
        match self {
            MootState::Ready {
                ref session,
                player,
            } => {
                if line.starts_with([';', '%']) {
                    Ok(MootState::ReadingCommand {
                        session: session.clone(),
                        player,
                        line_no: new_line_no,
                        command: line[1..].trim_start().to_string(),
                        command_kind: line.chars().next().unwrap().into(),
                    })
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Ok(MootState::new(session.clone(), Self::player(new_player)?))
                } else if line.is_empty() || line.starts_with("//") {
                    Ok(self)
                } else {
                    Err(eyre::eyre!(
                        "Expected a command (starting `;`), a comment (starting `//`), a player switch (starting `@`), a command (starting `%`), or an empty line"
                    ))
                }
            }
            MootState::ReadingCommand {
                session,
                player,
                line_no,
                mut command,
                command_kind,
            } => {
                if let Some(rest) = line.strip_prefix('>') {
                    command.push_str(rest);
                    Ok(MootState::ReadingCommand {
                        session,
                        player,
                        line_no,
                        command,
                        command_kind,
                    })
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Self::execute_test(
                        &command,
                        command_kind,
                        None,
                        line_no,
                        db.clone(),
                        session.clone(),
                        player,
                    )?;
                    Ok(MootState::new(session, Self::player(new_player)?))
                } else if line.starts_with([';', '%']) || line.is_empty() {
                    Self::execute_test(
                        &command,
                        command_kind,
                        None,
                        line_no,
                        db.clone(),
                        session.clone(),
                        player,
                    )?;
                    MootState::new(session, player).process_line(new_line_no, line, db)
                } else {
                    Ok(MootState::ReadingExpectation {
                        session,
                        player,
                        line_no,
                        command,
                        command_kind,
                        expectation: line.to_string(),
                    })
                }
            }
            MootState::ReadingExpectation {
                session,
                player,
                line_no,
                command,
                command_kind,
                mut expectation,
            } => {
                if line.is_empty() || line.starts_with("//") || line.starts_with([';', '%']) {
                    Self::execute_test(
                        &command,
                        command_kind,
                        Some(&expectation),
                        line_no,
                        db.clone(),
                        session.clone(),
                        player,
                    )?;
                }
                if line.is_empty() || line.starts_with("//") {
                    Ok(MootState::new(session, player))
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Ok(MootState::new(session, Self::player(new_player)?))
                } else if line.starts_with([';', '%']) {
                    MootState::new(session, player).process_line(new_line_no, line, db)
                } else {
                    expectation.push_str(line);
                    Ok(MootState::ReadingExpectation {
                        session,
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

    fn finalize(self, db: Arc<dyn Database + Send + Sync>) -> eyre::Result<()> {
        match self {
            MootState::Ready { .. } => Ok(()),
            MootState::ReadingCommand {
                session,
                player,
                line_no,
                command,
                command_kind,
            } => Self::execute_test(&command, command_kind, None, line_no, db, session, player),
            MootState::ReadingExpectation {
                session,
                player,
                line_no,
                command,
                command_kind,
                expectation,
            } => Self::execute_test(
                &command,
                command_kind,
                Some(&expectation),
                line_no,
                db,
                session,
                player,
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
        command: &str,
        command_kind: CommandKind,
        expectation: Option<&str>,
        line_no: usize,
        db: Arc<dyn Database + Send + Sync>,
        session: Arc<dyn Session>,
        player: Objid,
    ) -> eyre::Result<()> {
        let expected = if let Some(expectation) = expectation {
            scheduler_test_utils::call_eval(
                db.clone(),
                session.clone(),
                WIZARD,
                format!("return {expectation};"),
            )
            .context(format!("Failed to compile expected output: {expectation}"))?
        } else {
            v_none()
        };

        let actual = match command_kind {
            CommandKind::Eval => {
                scheduler_test_utils::call_eval(db, session, player, command.into())
            }
            CommandKind::Command => {
                scheduler_test_utils::call_command(db, session, player, command)
            }
        }?;
        assert_eq!(actual, expected, "Line {line_no}: {command}");
        Ok(())
    }
}

test_each_file::test_each_path! { in "./crates/kernel/testsuite/moot" as relbox => test_relbox }
test_each_file::test_each_path! { in "./crates/kernel/testsuite/moot" as wiretiger => test_wiretiger }

fn test_relbox(path: &Path) {
    test(create_relbox_db(), path);
}

fn test_wiretiger(path: &Path) {
    test(create_wiretiger_db(), path);
}

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
            .with_max_level(tracing::Level::TRACE)
            .with_test_writer()
            .finish();
        tracing::subscriber::set_global_default(main_subscriber)
            .expect("Unable to set configure logging");
    });
}

fn test(db: Arc<dyn Database + Send + Sync>, path: &Path) {
    // Uncomment to get server logs for debugging; usually too noisy
    init_logging();
    if path.is_dir() {
        return;
    }
    eprintln!("Test definition: {}", path.display());
    let f = BufReader::new(File::open(path).unwrap());

    let mut state = MootState::new(Arc::new(NoopClientSession::new()), WIZARD);
    for (line_no, line) in f.lines().enumerate() {
        state = state
            .process_line(line_no + 1, &line.unwrap(), db.clone())
            .context(format!("line {}", line_no + 1))
            .unwrap();
    }
    state.finalize(db).unwrap();
}

#[test]
#[ignore = "Useful for debugging; just run a single test"]
fn test_single() {
    // cargo test -p moor-kernel --test moot-suite test_single -- --ignored
    // CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --test moot-suite -- test_single --ignored
    test_relbox(&testsuite_dir().join("moot/example.moot"));
}
