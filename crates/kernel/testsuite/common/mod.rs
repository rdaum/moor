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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pretty_assertions::assert_eq;
use semver::Version;
use uuid::Uuid;

use moor_common::model::CommitResult;
use moor_common::model::Named;
use moor_common::model::VerbArgsSpec;
use moor_common::model::VerbFlag;
use moor_common::model::WorldStateSource;
use moor_common::program::ProgramType;
use moor_common::tasks::NoopClientSession;
use moor_common::tasks::Session;
use moor_compiler::Program;
use moor_compiler::{CompileOptions, compile};
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_kernel::testing::vm_test_utils;
use moor_kernel::testing::vm_test_utils::ExecResult;
use moor_kernel::vm::builtins::BuiltinRegistry;
use moor_moot::test_db_path;
use moor_textdump::textdump_load;
use moor_var::SYSTEM_OBJECT;
use moor_var::Symbol;
use moor_var::{List, Obj};

#[allow(dead_code)]
pub fn testsuite_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir).join("testsuite")
}

/// Create a minimal Db to support the test harness.
#[allow(dead_code)]
pub fn load_textdump(db: &dyn Database) {
    let mut tx = db.loader_client().unwrap();
    textdump_load(
        tx.as_mut(),
        test_db_path(),
        Version::new(0, 1, 0),
        CompileOptions::default(),
    )
    .expect("Could not load textdump");
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);
}

pub fn create_db() -> Box<dyn Database> {
    let (db, _) = TxDB::open(None, DatabaseConfig::default());
    let db = Box::new(db);
    load_textdump(db.as_ref());
    db
}

#[allow(dead_code)]
pub fn compile_verbs(db: &dyn Database, verbs: &[(&str, &Program)]) {
    let mut tx = db.new_world_state().unwrap();
    for (verb_name, program) in verbs {
        let verb_name = Symbol::mk(verb_name);
        tx.add_verb(
            &Obj::mk_id(3),
            &SYSTEM_OBJECT,
            vec![verb_name],
            &Obj::mk_id(3),
            VerbFlag::rx(),
            VerbArgsSpec::this_none_this(),
            ProgramType::MooR((*program).clone()),
        )
        .unwrap();

        // Verify it was added.
        let verb = tx
            .get_verb(&Obj::mk_id(3), &SYSTEM_OBJECT, verb_name)
            .unwrap();
        assert!(verb.matches_name(verb_name));
    }
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);

    // And then verify that again in a new transaction.
    let tx = db.new_world_state().unwrap();
    for (verb_name, _) in verbs {
        let verb_name = Symbol::mk(verb_name);
        let verb = tx
            .get_verb(&Obj::mk_id(3), &SYSTEM_OBJECT, verb_name)
            .unwrap();
        assert!(verb.matches_name(verb_name));
    }
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);
}

#[allow(dead_code)]
pub fn run_as_verb(db: &dyn Database, expression: &str) -> ExecResult {
    let binary = compile(expression, CompileOptions::default()).unwrap();
    let verb_uuid = Uuid::new_v4().to_string();
    compile_verbs(db, &[(&verb_uuid, &binary)]);
    let mut state = db.new_world_state().unwrap();
    let builtin_registry = BuiltinRegistry::new();
    let result = vm_test_utils::call_verb(
        state.as_mut(),
        Arc::new(NoopClientSession::new()),
        builtin_registry,
        &verb_uuid,
        List::mk_list(&[]),
    );
    state.commit().unwrap();
    result
}

#[allow(dead_code)]
pub fn eval(
    db: Arc<dyn WorldStateSource>,
    player: Obj,
    expression: &str,
    session: Arc<dyn Session>,
) -> eyre::Result<ExecResult> {
    let binary = compile(expression, CompileOptions::default())?;
    let mut state = db.new_world_state()?;
    let builtin_registry = BuiltinRegistry::new();
    let result =
        vm_test_utils::call_eval_builtin(state.as_mut(), session, builtin_registry, player, binary);
    state.commit()?;
    Ok(result)
}

#[allow(dead_code)]
pub trait AssertRunAsVerb {
    fn assert_run_as_verb<T: Into<ExecResult>, S: AsRef<str>>(&self, expression: S, expected: T);
}
impl AssertRunAsVerb for Box<dyn Database> {
    fn assert_run_as_verb<T: Into<ExecResult>, S: AsRef<str>>(&self, expression: S, expected: T) {
        let expected = expected.into();
        let actual = run_as_verb(self.as_ref(), expression.as_ref());
        assert_eq!(actual, expected, "{}", expression.as_ref());
    }
}
