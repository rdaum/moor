//! Moot is a simple text-based test format for testing the kernel.
//!
//! Example test:
//!
//! # This is a comment.
//! ; return 42;
//! 42
//!
//! # Empty lines are ignored
//!
//! # Both thrown and returned errors can be matched with a simple error value
//! ; eval();
//! E_ARGS
//!
//! # Multi-line commands: continuation with `>`.
//! ; return 1 + 2 +
//! > 3;
//! 6
//!

mod common;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    sync::Arc,
};

use common::create_db;
use eyre::Context;
use moor_db::odb::RelBoxWorldState;
use moor_kernel::tasks::vm_test_utils::ExecResult;
use moor_values::var::v_none;
use pretty_assertions::assert_eq;

use crate::common::WIZARD;

#[derive(Debug, Default)]
enum MootState {
    #[default]
    Ready,
    ReadingCommand {
        line_no: usize,
        command: String,
    },
    ReadingExpectation {
        line_no: usize,
        command: String,
        expectation: String,
    },
}
impl MootState {
    // Could implement this with `nom` I guess, but this seems simple enough, and it's probably easier to read.
    fn process_line(
        self,
        new_line_no: usize,
        line: &str,
        db: Arc<RelBoxWorldState>,
    ) -> eyre::Result<Self> {
        let line = line.trim_end_matches('\n');
        match self {
            MootState::Ready => {
                if let Some(rest) = line.strip_prefix(';') {
                    Ok(MootState::ReadingCommand {
                        line_no: new_line_no,
                        command: rest.trim_start().to_string(),
                    })
                } else if line.is_empty() || line.starts_with('#') {
                    Ok(self)
                } else {
                    Err(eyre::eyre!(
                        "Expected a command (starting `;`), a comment (starting `#`), or an empty line"
                    ))
                }
            }
            MootState::ReadingCommand {
                line_no,
                mut command,
            } => {
                if let Some(rest) = line.strip_prefix('>') {
                    command.push_str(rest);
                    Ok(MootState::ReadingCommand { line_no, command })
                } else if line.starts_with(';') || line.is_empty() {
                    Self::execute_test(&command, None, line_no, db.clone())?;
                    MootState::Ready.process_line(new_line_no, line, db)
                } else {
                    Ok(MootState::ReadingExpectation {
                        line_no,
                        command,
                        expectation: line.to_string(),
                    })
                }
            }
            MootState::ReadingExpectation {
                line_no,
                command,
                mut expectation,
            } => {
                if line.is_empty() || line.starts_with('#') {
                    Self::execute_test(&command, Some(&expectation), line_no, db)?;
                    Ok(MootState::Ready)
                } else if line.starts_with(';') {
                    Self::execute_test(&command, Some(&expectation), line_no, db.clone())?;
                    MootState::Ready.process_line(new_line_no, line, db)
                } else {
                    expectation.push_str(line);
                    Ok(MootState::ReadingExpectation {
                        line_no,
                        command,
                        expectation,
                    })
                }
            }
        }
    }

    fn execute_test(
        command: &str,
        expectation: Option<&str>,
        line_no: usize,
        db: Arc<RelBoxWorldState>,
    ) -> eyre::Result<()> {
        let expected = if let Some(expectation) = expectation {
            common::eval(db.clone(), WIZARD, &format!("return {expectation};"))?.unwrap()
        } else {
            v_none()
        };
        let actual_exec_result = common::eval(db, WIZARD, command)?;
        let actual = match actual_exec_result {
            ExecResult::Success(v) => v,
            ExecResult::Exception(e) => e.code.into(),
        };
        assert_eq!(actual, expected, "Line {line_no}: {command}");
        Ok(())
    }
}

test_each_file::test_each_path! { in "./crates/kernel/testsuite/moot" as moot => test }

fn test(path: &Path) {
    if path.is_dir() {
        return;
    }
    eprintln!("Test definition: {}", path.display());
    let f = BufReader::new(File::open(path).unwrap());
    let db = create_db();

    let mut state = MootState::default();
    for (line_no, line) in f.lines().enumerate() {
        state = state
            .process_line(line_no + 1, &line.unwrap(), db.clone())
            .context(format!("line {}", line_no + 1))
            .unwrap();
    }
    state.process_line(0, "", db).unwrap();
}
