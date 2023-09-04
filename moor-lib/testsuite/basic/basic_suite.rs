use anyhow::Error;
use async_trait::async_trait;
use moor_lib::compiler::codegen::compile;
use moor_lib::db::inmemtransient::InMemTransientDatabase;
use moor_lib::db::DbTxWorldState;
use moor_lib::tasks::{Sessions, VerbCall};
use moor_lib::textdump::load_db::textdump_load;
use moor_lib::vm::opcode::Program;
use moor_lib::vm::vm_execute::VmExecParams;
use moor_lib::vm::{ExecutionResult, VerbExecutionRequest, VM};
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::world_state::WorldState;
use moor_value::var::objid::Objid;
use moor_value::var::Var;
use moor_value::{AsByteBuffer, NOTHING, SYSTEM_OBJECT};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

struct NoopClientConnection {}
impl NoopClientConnection {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Sessions for NoopClientConnection {
    async fn send_text(&mut self, _player: Objid, _msg: &str) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn shutdown(&mut self, _msg: Option<String>) -> Result<(), Error> {
        Ok(())
    }

    async fn connection_name(&self, player: Objid) -> Result<String, Error> {
        Ok(format!("player-{}", player.0))
    }

    async fn disconnect(&mut self, _player: Objid) -> Result<(), Error> {
        Ok(())
    }

    fn connected_players(&self) -> Result<Vec<Objid>, Error> {
        Ok(vec![])
    }

    fn connected_seconds(&self, _player: Objid) -> Result<f64, Error> {
        Ok(0.0)
    }

    fn idle_seconds(&self, _player: Objid) -> Result<f64, Error> {
        Ok(0.0)
    }
}

fn testsuite_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir).join("testsuite")
}

// Create a minimal Db to support the test harness.
async fn test_db_with_verbs(
    db: &mut InMemTransientDatabase,
    verbs: &[(&str, &Program)],
) -> Box<DbTxWorldState> {
    let mut tx = db.tx().unwrap();
    textdump_load(
        &mut tx,
        testsuite_dir().join("Minimal.db").to_str().unwrap(),
    )
    .await
    .expect("Could not load textdump");

    for (verb_name, program) in verbs {
        let binary = program.make_copy_as_vec();
        tx.add_verb(
            Objid(3),
            SYSTEM_OBJECT,
            vec![verb_name.to_string()],
            Objid(3),
            VerbFlag::rx(),
            VerbArgsSpec::this_none_this(),
            binary,
            BinaryType::LambdaMoo18X,
        )
        .await
        .unwrap();
    }
    Box::new(tx)
}

async fn call_verb(state: &mut dyn WorldState, verb_name: &str, vm: &mut VM) {
    let o = Objid(0);

    let call = VerbCall {
        verb_name: verb_name.to_string(),
        location: o,
        this: o,
        player: o,
        args: vec![],
        caller: NOTHING,
    };
    let verb = state.find_method_verb_on(o, o, verb_name).await.unwrap();
    let program = Program::from_sliceref(verb.binary());
    let cr = VerbExecutionRequest {
        permissions: o,
        resolved_verb: verb,
        call,
        command: None,
        program,
    };
    assert!(vm.exec_call_request(0, cr).await.is_ok());
}

// TODO: this loop is starting to look like boilerplate copy of large parts of what's in Task, and
//  introduces significant possibility of functionality-drift from the real thing. We should factor
//  out the pieces from Task so they can be re-used here without pulling in all of scheduler.
//  This is also totally copy and pasted from the same in vm_test.rs
//  So the whole thing is majorly due for a cleanup.
async fn exec_vm(state: &mut dyn WorldState, vm: &mut VM) -> Var {
    let client_connection = Arc::new(RwLock::new(NoopClientConnection::new()));
    // Call repeatedly into exec until we ge either an error or Complete.

    loop {
        let (sched_send, _) = tokio::sync::mpsc::unbounded_channel();
        let vm_exec_params = VmExecParams {
            world_state: state,
            sessions: client_connection.clone(),
            scheduler_sender: sched_send.clone(),
            max_stack_depth: 50,
            ticks_left: 90_000,
            time_left: None,
        };
        match vm.exec(vm_exec_params, 1_000000).await {
            Ok(ExecutionResult::More) => continue,
            Ok(ExecutionResult::Complete(a)) => return a,
            Err(e) => panic!("error during execution: {:?}", e),
            Ok(ExecutionResult::Exception(e)) => {
                panic!("MOO exception {:?}", e);
            }
            Ok(ExecutionResult::ContinueVerb {
                permissions,
                resolved_verb,
                call,
                command,
                trampoline: _,
                trampoline_arg: _,
            }) => {
                let decoded_verb = Program::from_sliceref(resolved_verb.binary());
                let cr = VerbExecutionRequest {
                    permissions,
                    resolved_verb,
                    call,
                    command,
                    program: decoded_verb,
                };
                vm.exec_call_request(0, cr).await.unwrap();
            }
            Ok(ExecutionResult::PerformEval {
                permissions,
                player,
                program,
            }) => {
                vm.exec_eval_request(0, permissions, player, program)
                    .await
                    .expect("Could not set up VM for verb execution");
            }
            Ok(ExecutionResult::DispatchFork(_)) => {
                panic!("fork not implemented in test VM")
            }
            Ok(ExecutionResult::Suspend(_)) => {
                panic!("suspend not implemented in test VM")
            }
            Ok(ExecutionResult::ContinueBuiltin {
                bf_func_num: _,
                arguments: _,
            }) => {}
        }
    }
}

async fn eval(db: &mut InMemTransientDatabase, expression: &str) -> Result<Var, anyhow::Error> {
    let binary = compile(format!("return {};", expression).as_str()).unwrap();
    let mut state = test_db_with_verbs(db, &[("test", &binary)]).await;
    let mut vm = VM::new();
    let _args = binary.find_var("args");
    call_verb(state.as_mut(), "test", &mut vm).await;
    let result = exec_vm(state.as_mut(), &mut vm).await;
    state.commit().await?;
    Ok(result)
}

async fn run_basic_test(test_dir: &str) {
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

    // Frustratingly the tests are not independent, so we need to run them in a single database.
    let mut db = InMemTransientDatabase::new();

    for (line_num, (input, expected_output)) in zipped.enumerate() {
        let evaluated = eval(&mut db, input).await.unwrap();
        let output = eval(&mut db, expected_output).await.unwrap();
        assert_eq!(
            evaluated, output,
            "{}: line {}: {}",
            test_dir, line_num, input
        )
    }
}

fn main() {}
#[tokio::test]
async fn basic_arithmetic() {
    run_basic_test("arithmetic").await;
}

#[tokio::test]
async fn basic_value() {
    run_basic_test("value").await;
}

#[tokio::test]
async fn basic_string() {
    run_basic_test("string").await;
}

#[tokio::test]
async fn basic_list() {
    run_basic_test("list").await;
}

#[tokio::test]
async fn basic_property() {
    run_basic_test("property").await;
}

#[tokio::test]
async fn basic_object() {
    run_basic_test("object").await;
}
