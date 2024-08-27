// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
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
use uuid::Uuid;
use EncodingMode::UTF8;

use moor_compiler::Program;
use moor_compiler::{compile, CompileOptions};
use moor_db::Database;
#[cfg(feature = "relbox")]
use moor_db_relbox::RelBoxWorldState;
use moor_db_wiredtiger::WiredTigerDB;
use moor_kernel::builtins::BuiltinRegistry;
use moor_kernel::tasks::sessions::NoopClientSession;
use moor_kernel::tasks::sessions::Session;
use moor_kernel::tasks::vm_test_utils;
use moor_kernel::tasks::vm_test_utils::ExecResult;
use moor_kernel::textdump::{textdump_load, EncodingMode};
use moor_moot::test_db_path;
use moor_values::model::CommitResult;
use moor_values::model::Named;
use moor_values::model::VerbArgsSpec;
use moor_values::model::WorldStateSource;
use moor_values::model::{BinaryType, VerbFlag};
use moor_values::Objid;
use moor_values::Symbol;
use moor_values::{AsByteBuffer, SYSTEM_OBJECT};

#[allow(dead_code)]
pub fn testsuite_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir).join("testsuite")
}

/// Create a minimal Db to support the test harness.
#[allow(dead_code)]
pub fn load_textdump(db: &dyn Database) {
    let mut tx = db.loader_client().unwrap();
    textdump_load(tx.as_ref(), test_db_path(), UTF8, CompileOptions::default())
        .expect("Could not load textdump");
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);
}

#[cfg(feature = "relbox")]
pub fn create_relbox_db() -> Box<dyn Database> {
    let (db, _) = RelBoxWorldState::open(None, 1 << 30);
    let db = Box::new(db);
    load_textdump(db.as_ref());
    db
}

pub fn create_wiredtiger_db() -> Box<dyn Database> {
    let (db, _) = WiredTigerDB::open(None);
    let db = Box::new(db);
    load_textdump(db.as_ref());
    db
}

#[allow(dead_code)]
pub fn compile_verbs(db: &dyn Database, verbs: &[(&str, &Program)]) {
    let mut tx = db.new_world_state().unwrap();
    for (verb_name, program) in verbs {
        let binary = program.make_copy_as_vec().unwrap();
        let verb_name = Symbol::mk(verb_name);
        tx.add_verb(
            Objid(3),
            SYSTEM_OBJECT,
            vec![verb_name],
            Objid(3),
            VerbFlag::rx(),
            VerbArgsSpec::this_none_this(),
            binary,
            BinaryType::LambdaMoo18X,
        )
        .unwrap();

        // Verify it was added.
        let verb = tx.get_verb(Objid(3), SYSTEM_OBJECT, verb_name).unwrap();
        assert!(verb.matches_name(verb_name));
    }
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);

    // And then verify that again in a new transaction.
    let mut tx = db.new_world_state().unwrap();
    for (verb_name, _) in verbs {
        let verb_name = Symbol::mk(verb_name);
        let verb = tx.get_verb(Objid(3), SYSTEM_OBJECT, verb_name).unwrap();
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
    let builtin_registry = Arc::new(BuiltinRegistry::new());
    let result = vm_test_utils::call_verb(
        state.as_mut(),
        Arc::new(NoopClientSession::new()),
        builtin_registry,
        &verb_uuid,
        vec![],
    );
    state.commit().unwrap();
    result
}

#[allow(dead_code)]
pub fn eval(
    db: Arc<dyn WorldStateSource>,
    player: Objid,
    expression: &str,
    session: Arc<dyn Session>,
) -> eyre::Result<ExecResult> {
    let binary = compile(expression, CompileOptions::default())?;
    let mut state = db.new_world_state()?;
    let builtin_registry = Arc::new(BuiltinRegistry::new());
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
