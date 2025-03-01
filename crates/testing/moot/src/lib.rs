// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

mod parser;
pub mod telnet;

use std::{
    path::{Path, PathBuf},
    sync::Once,
};

use eyre::{ContextCompat, WrapErr, eyre};
use moor_var::Obj;

use parser::{MootBlock, MootBlockTest, MootBlockTestExpectedOutput, MootBlockTestKind};
use pretty_assertions::assert_eq;

#[allow(dead_code)]
pub const WIZARD: Obj = Obj::mk_id(3);
#[allow(dead_code)]
pub const PROGRAMMER: Obj = Obj::mk_id(4);
#[allow(dead_code)]
pub const NONPROGRAMMER: Obj = Obj::mk_id(5);

#[allow(dead_code)]
static LOGGING_INIT: Once = Once::new();
#[allow(dead_code)]
fn init_logging(options: &MootOptions) {
    if !options.init_logging {
        return;
    }
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
        tracing::subscriber::set_global_default(main_subscriber).ok();
    });
}
/// Look up the path to Test.db from any crate under the `moor` workspace
pub fn test_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../moot/Test.db")
}

pub trait MootRunner {
    type Value: PartialEq + std::fmt::Debug;

    fn eval<S: Into<String>>(&mut self, player: &Obj, command: S) -> eyre::Result<()>;
    fn command<S: AsRef<str>>(&mut self, player: &Obj, command: S) -> eyre::Result<()>;

    fn read_line(&mut self, player: &Obj) -> eyre::Result<Option<String>>;
    fn read_eval_result(&mut self, player: &Obj) -> eyre::Result<Option<Self::Value>>;
    fn read_command_result(&mut self, player: &Obj) -> eyre::Result<Option<Self::Value>>;

    fn none(&self) -> Self::Value;
}

pub struct MootOptions {
    /// Whether logging needs to be initialized, or if the host process has already initialized
    /// logging
    init_logging: bool,
    wizard_object: Obj,
    programmer_object: Obj,
    nonprogrammer_object: Obj,
}

impl Default for MootOptions {
    fn default() -> Self {
        Self {
            init_logging: false,
            wizard_object: WIZARD,
            programmer_object: PROGRAMMER,
            nonprogrammer_object: NONPROGRAMMER,
        }
    }
}

impl MootOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init_logging(mut self, yesno: bool) -> Self {
        self.init_logging = yesno;
        self
    }

    pub fn wizard_object(mut self, wizard_object: Obj) -> Self {
        self.wizard_object = wizard_object;
        self
    }

    pub fn programmer_object(mut self, programmer_object: Obj) -> Self {
        self.programmer_object = programmer_object;
        self
    }

    pub fn nonprogrammer_object(mut self, nonprogrammer_object: Obj) -> Self {
        self.nonprogrammer_object = nonprogrammer_object;
        self
    }
}

pub fn execute_moot_test<R: MootRunner, F: Fn() -> eyre::Result<()>>(
    mut runner: R,
    options: &MootOptions,
    path: &Path,
    validate_state: F,
) {
    init_logging(options);
    eprintln!("Test definition: {}", path.display());

    let test = std::fs::read_to_string(path)
        .wrap_err(format!("{}", path.display()))
        .unwrap();

    let mut player = options.wizard_object.clone();
    for span in parser::parse(&test).context("parse").unwrap() {
        eprintln!("{:?}", span);
        match &span.expr {
            MootBlock::ChangePlayer(change) => {
                player = handle_change_player(options, change.name)
                    .context("handle_change_player")
                    .unwrap();
            }
            MootBlock::Test(test) => {
                handle_test(
                    &mut runner,
                    &player,
                    span.line_no,
                    test,
                    &validate_state,
                    path,
                )
                .context("handle_test")
                .unwrap();
            }
        }
    }
}

fn handle_change_player(options: &MootOptions, name: &str) -> eyre::Result<Obj> {
    Ok(match name {
        "wizard" => options.wizard_object.clone(),
        "programmer" => options.programmer_object.clone(),
        "nonprogrammer" => options.nonprogrammer_object.clone(),
        _ => return Err(eyre!("Unknown player: {}", name)),
    })
}

fn handle_test<R: MootRunner, F: Fn() -> eyre::Result<()>>(
    runner: &mut R,
    player: &Obj,
    line_no: usize,
    test: &MootBlockTest,
    validate_state: F,
    path: &Path,
) -> eyre::Result<()> {
    execute_test_prog(runner, player, line_no, test, &validate_state)?;

    if test.expected_output.is_empty() {
        if test.kind == MootBlockTestKind::EvalBg {
            // Discard the result of the eval
            runner.read_eval_result(player)?;
        } else if test.kind == MootBlockTestKind::Eval {
            // Assert that we got the empty result
            assert_result(runner, player, &test.kind, None, path, line_no)?;
        }
    } else {
        for expectation in &test.expected_output {
            execute_test_expectation(
                runner,
                player,
                &validate_state,
                &test.kind,
                expectation,
                path,
            )?;
        }
    }

    Ok(())
}

fn execute_test_expectation<R: MootRunner, F: Fn() -> eyre::Result<()>>(
    runner: &mut R,
    player: &Obj,
    validate_state: &F,
    kind: &MootBlockTestKind,
    expectation: &MootBlockTestExpectedOutput,
    path: &Path,
) -> Result<(), eyre::Error> {
    validate_state().with_context(|| {
        format!(
            "Invalid state before processing line {}",
            expectation.line_no
        )
    })?;

    if expectation.verbatim {
        assert_raw_line(
            runner,
            player,
            Some(expectation.expected_output),
            path,
            expectation.line_no,
        )?;
    } else {
        assert_result(
            runner,
            player,
            kind,
            Some(expectation.expected_output),
            path,
            expectation.line_no,
        )?;
    }

    Ok(())
}

fn execute_test_prog<R: MootRunner, F: Fn() -> eyre::Result<()>>(
    runner: &mut R,
    player: &Obj,
    line_no: usize,
    test: &MootBlockTest,
    validate_state: F,
) -> Result<(), eyre::Error> {
    validate_state()
        .with_context(|| format!("Invalid state before processing line {}", line_no))?;

    let prog = test.prog();
    match test.kind {
        MootBlockTestKind::Eval | MootBlockTestKind::EvalBg => {
            runner.eval(player, format!("{prog} \"moot-line:{line_no}\";"))?;
        }
        MootBlockTestKind::Command => {
            runner.command(player, prog)?;
        }
    };

    Ok(())
}

fn assert_result<R: MootRunner>(
    runner: &mut R,
    player: &Obj,
    kind: &MootBlockTestKind,
    expectation: Option<&str>,
    path: &Path,
    line_no: usize,
) -> eyre::Result<()> {
    let err_prefix = || format!("assert_eval_result({player}, {expectation:?}, {line_no})");
    let actual = match kind {
        MootBlockTestKind::Eval | MootBlockTestKind::EvalBg => runner
            .read_eval_result(player)?
            .ok_or_else(|| eyre!("{}/actual: got no eval result", err_prefix()))?,
        MootBlockTestKind::Command => runner
            .read_command_result(player)?
            .ok_or_else(|| eyre!("{}/actual: got no command result", err_prefix()))?,
    };

    let expected = if let Some(expectation) = expectation {
        runner
            .eval(
                player,
                format!("return {expectation}; \"moot-expect-line:{line_no}\";"),
            )
            .wrap_err_with(|| {
                format!(
                    "{}/expected: Failed to compile expected output: {expectation}",
                    err_prefix()
                )
            })?;
        runner
            .read_eval_result(player)?
            .wrap_err_with(|| format!("{}/expected: got no eval result", err_prefix()))?
    } else {
        runner.none()
    };

    // Send the MOO values through the debug formatter, because MOO string comparison
    // is case-insensitive, but we want case-sensitive comparison in tests.
    assert_eq!(
        format!("{actual:?}"),
        format!("{expected:?}"),
        "{}:{line_no}",
        path.display()
    );
    Ok(())
}

fn assert_raw_line<R: MootRunner>(
    runner: &mut R,
    player: &Obj,
    expectation: Option<&str>,
    path: &Path,
    line_no: usize,
) -> eyre::Result<()> {
    let actual = runner.read_line(player)?;
    assert_eq!(
        actual.as_deref(),
        expectation,
        "{}:{line_no}",
        path.display()
    );
    Ok(())
}
