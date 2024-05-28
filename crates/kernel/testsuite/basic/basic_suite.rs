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
use moor_db::odb::RelBoxWorldState;
use moor_db::Database;
use moor_kernel::tasks::sessions::NoopClientSession;
use moor_kernel::tasks::vm_test_utils::call_eval_builtin;
use moor_kernel::tasks::vm_test_utils::call_verb;
use moor_kernel::tasks::vm_test_utils::ExecResult;
use moor_kernel::textdump::textdump_load;
use moor_values::model::CommitResult;
use moor_values::model::Named;
use moor_values::model::VerbArgsSpec;
use moor_values::model::WorldStateSource;
use moor_values::model::{BinaryType, VerbFlag};
use moor_values::var::Objid;
use moor_values::{AsByteBuffer, SYSTEM_OBJECT};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

fn testsuite_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir).join("testsuite")
}

/// Create a minimal Db to support the test harness.
fn load_textdump(db: Arc<dyn Database>) {
    let tx = db.loader_client().unwrap();
    textdump_load(tx.clone(), testsuite_dir().join("Minimal.db")).expect("Could not load textdump");
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);
}

#[allow(unused)]
fn compile_verbs(db: Arc<dyn WorldStateSource>, verbs: &[(&str, &Program)]) {
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

#[allow(unused)]
fn run_as_verb(db: Arc<dyn WorldStateSource>, expression: &str) -> ExecResult {
    let binary = compile(format!("return {expression};").as_str()).unwrap();
    let verb_uuid = Uuid::new_v4().to_string();
    compile_verbs(db.clone(), &[(&verb_uuid, &binary)]);
    let mut state = db.new_world_state().unwrap();
    let result = call_verb(
        state.as_mut(),
        Arc::new(NoopClientSession::new()),
        &verb_uuid,
        vec![],
    );
    state.commit().unwrap();
    result
}

fn eval(db: Arc<dyn WorldStateSource>, expression: &str) -> ExecResult {
    let binary = compile(expression).unwrap();
    let mut state = db.new_world_state().unwrap();
    let result = call_eval_builtin(state.as_mut(), Arc::new(NoopClientSession::new()), binary);
    state.commit().unwrap();
    result
}

fn run_basic_test(test_dir: &str) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let basic_arith_dir = Path::new(manifest_dir)
        .join("testsuite")
        .join("basic")
        .join(test_dir);

    let test_in = basic_arith_dir.join("test.in");
    let test_out = basic_arith_dir.join("test.out");

    // Read the lines from both files, the first is an input expression, the second the
    // expected output. Both as MOO expressions. # of lines must be identical in each.
    let input = std::fs::read_to_string(test_in).unwrap();
    let in_lines = input.lines();
    let output = std::fs::read_to_string(test_out).unwrap();
    let out_lines = output.lines();
    assert_eq!(in_lines.clone().count(), out_lines.clone().count());

    // Zip
    let zipped = in_lines.zip(out_lines);

    // Frustratingly the individual test lines are not independent, so we need to run them in a
    // single database.
    let (db, _) = RelBoxWorldState::open(None, 1 << 30);
    let db = Arc::new(db);
    load_textdump(db.clone());
    for (line_num, (input, expected_str)) in zipped.enumerate() {
        let actual = eval(db.clone(), &format!("return {};", input.trim()));
        let expected = eval(db.clone(), &format!("return {};", expected_str.trim()));
        assert_eq!(
            actual,
            expected,
            "{test_dir}: line {}: {input}",
            line_num + 1
        );
    }
}

fn main() {}
#[test]
fn basic_arithmetic() {
    run_basic_test("arithmetic");
}

#[test]
fn basic_value() {
    run_basic_test("value");
}

#[test]
fn basic_string() {
    run_basic_test("string");
}

#[test]
fn basic_list() {
    run_basic_test("list");
}

#[test]
fn basic_property() {
    run_basic_test("property");
}

#[test]
fn basic_object() {
    run_basic_test("object");
}
