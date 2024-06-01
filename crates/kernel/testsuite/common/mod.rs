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

use moor_compiler::compile;
use moor_compiler::Program;
use moor_db::Database;
use moor_db_relbox::RelBoxWorldState;
use moor_kernel::tasks::sessions::NoopClientSession;
use moor_kernel::tasks::sessions::Session;
use moor_kernel::tasks::vm_test_utils;
use moor_kernel::tasks::vm_test_utils::ExecResult;
use moor_kernel::textdump::textdump_load;
use moor_values::model::CommitResult;
use moor_values::model::Named;
use moor_values::model::VerbArgsSpec;
use moor_values::model::WorldStateSource;
use moor_values::model::{BinaryType, VerbFlag};
use moor_values::var::Objid;
use moor_values::{AsByteBuffer, SYSTEM_OBJECT};
use pretty_assertions::assert_eq;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;
use wtdb::WireTigerWorldState;

#[allow(dead_code)]
pub const WIZARD: Objid = Objid(3);
#[allow(dead_code)]
pub const PROGRAMMER: Objid = Objid(4);
#[allow(dead_code)]
pub const NONPROGRAMMER: Objid = Objid(5);

pub fn testsuite_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir).join("testsuite")
}

/// Create a minimal Db to support the test harness.
pub fn load_textdump(db: Arc<dyn Database>) {
    let tx = db.loader_client().unwrap();
    textdump_load(tx.clone(), testsuite_dir().join("Test.db")).expect("Could not load textdump");
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);
}

pub fn create_relbox_db() -> Arc<dyn Database + Send + Sync> {
    let (db, _) = RelBoxWorldState::open(None, 1 << 30);
    let db = Arc::new(db);
    load_textdump(db.clone());
    db
}

pub fn create_wiretiger_db() -> Arc<dyn Database + Send + Sync> {
    let (db, _) = WireTigerWorldState::open(None);
    let db = Arc::new(db);
    load_textdump(db.clone());
    db
}

#[allow(dead_code)]
pub fn compile_verbs(db: Arc<dyn WorldStateSource>, verbs: &[(&str, &Program)]) {
    let mut tx = db.new_world_state().unwrap();
    for (verb_name, program) in verbs {
        let binary = program.make_copy_as_vec().unwrap();
        tx.add_verb(
            Objid(3),
            SYSTEM_OBJECT,
            vec![(*verb_name).to_string()],
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
        let verb = tx.get_verb(Objid(3), SYSTEM_OBJECT, verb_name).unwrap();
        assert!(verb.matches_name(verb_name));
    }
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);
}

#[allow(dead_code)]
pub fn run_as_verb(db: Arc<dyn WorldStateSource>, expression: &str) -> ExecResult {
    let binary = compile(expression).unwrap();
    let verb_uuid = Uuid::new_v4().to_string();
    compile_verbs(db.clone(), &[(&verb_uuid, &binary)]);
    let mut state = db.new_world_state().unwrap();
    let result = vm_test_utils::call_verb(
        state.as_mut(),
        Arc::new(NoopClientSession::new()),
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
    let binary = compile(expression)?;
    let mut state = db.new_world_state()?;
    let result = vm_test_utils::call_eval_builtin(state.as_mut(), session, player, binary);
    state.commit()?;
    Ok(result)
}

#[allow(dead_code)]
pub trait AssertRunAsVerb {
    fn assert_run_as_verb<T: Into<ExecResult>, S: AsRef<str>>(&self, expression: S, expected: T);
}
impl AssertRunAsVerb for Arc<dyn Database + Send + Sync> {
    fn assert_run_as_verb<T: Into<ExecResult>, S: AsRef<str>>(&self, expression: S, expected: T) {
        let expected = expected.into();
        let actual = run_as_verb(
            self.clone().world_state_source().unwrap(),
            expression.as_ref(),
        );
        assert_eq!(actual, expected, "{}", expression.as_ref());
    }
}
