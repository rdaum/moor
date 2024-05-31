//! Moot is a simple text-based test format for testing the kernel.
//!
//! See example.moot for a full-fledged example

mod common;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    sync::Arc,
};

use common::{create_relbox_db, create_wiretiger_db, NONPROGRAMMER, PROGRAMMER};
use eyre::Context;
use moor_kernel::tasks::sessions::{NoopClientSession, Session};
use moor_values::{
    model::WorldStateSource,
    var::{v_none, Objid},
};
use pretty_assertions::assert_eq;

use crate::common::WIZARD;

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
    },
    ReadingExpectation {
        session: Arc<dyn Session>,
        player: Objid,
        line_no: usize,
        command: String,
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
        db: Arc<dyn WorldStateSource>,
    ) -> eyre::Result<Self> {
        let line = line.trim_end_matches('\n');
        match self {
            MootState::Ready {
                ref session,
                player,
            } => {
                if let Some(rest) = line.strip_prefix(';') {
                    Ok(MootState::ReadingCommand {
                        session: session.clone(),
                        player,
                        line_no: new_line_no,
                        command: rest.trim_start().to_string(),
                    })
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Ok(MootState::new(session.clone(), Self::player(new_player)?))
                } else if line.is_empty() || line.starts_with("//") {
                    Ok(self)
                } else {
                    Err(eyre::eyre!(
                        "Expected a command (starting `;`), a comment (starting `//`), a player switch (starting `@`), or an empty line"
                    ))
                }
            }
            MootState::ReadingCommand {
                session,
                player,
                line_no,
                mut command,
            } => {
                if let Some(rest) = line.strip_prefix('>') {
                    command.push_str(rest);
                    Ok(MootState::ReadingCommand {
                        session,
                        player,
                        line_no,
                        command,
                    })
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Self::execute_test(
                        &command,
                        None,
                        line_no,
                        db.clone(),
                        session.clone(),
                        player,
                    )?;
                    Ok(MootState::new(session, Self::player(new_player)?))
                } else if line.starts_with(';') || line.is_empty() {
                    Self::execute_test(
                        &command,
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
                        expectation: line.to_string(),
                    })
                }
            }
            MootState::ReadingExpectation {
                session,
                player,
                line_no,
                command,
                mut expectation,
            } => {
                if line.is_empty() || line.starts_with("//") || line.starts_with(';') {
                    Self::execute_test(
                        &command,
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
                } else if line.starts_with(';') {
                    MootState::new(session, player).process_line(new_line_no, line, db)
                } else {
                    expectation.push_str(line);
                    Ok(MootState::ReadingExpectation {
                        session,
                        player,
                        line_no,
                        command,
                        expectation,
                    })
                }
            }
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
        expectation: Option<&str>,
        line_no: usize,
        db: Arc<dyn WorldStateSource>,
        session: Arc<dyn Session>,
        player: Objid,
    ) -> eyre::Result<()> {
        let expected = if let Some(expectation) = expectation {
            common::eval(
                db.clone(),
                WIZARD,
                &format!("return {expectation};"),
                session.clone(),
            )??
        } else {
            v_none()
        };

        let actual_exec_result = common::eval(db, player, command, session)?;
        let actual = match actual_exec_result {
            Ok(v) => v,
            Err(e) => e.code.into(),
        };
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

fn test(db: Arc<dyn WorldStateSource>, path: &Path) {
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
    state.process_line(0, "", db).unwrap();
}
