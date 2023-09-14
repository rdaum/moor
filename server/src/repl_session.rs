use anyhow::Error;
use async_trait::async_trait;
use moor_core::compiler::decompile::program_to_tree;
use moor_core::compiler::unparse::unparse;
use moor_core::tasks::scheduler::{Scheduler, SchedulerError, TaskWaiterResult};
use moor_core::tasks::sessions::Session;
use moor_core::vm::opcode::Program;
use moor_values::model::defset::HasUuid;
use moor_values::model::verbs::BinaryType;
use moor_values::model::world_state::{WorldState, WorldStateSource};
use moor_values::model::NarrativeEvent;
use moor_values::var::objid::Objid;
use moor_values::var::variant::Variant;
use moor_values::AsByteBuffer;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::block_in_place;

// When repl lines begin with these fragments, we don't prepend a return or postfix a ;
const EVAL_BLOCK_WORDS: [&str; 7] = ["if ", "while ", "fork ", "for ", "try ", "return ", ";;"];

pub struct ReplSession {
    pub(crate) player: Objid,
    pub(crate) connect_time: std::time::Instant,
    pub(crate) last_activity: RwLock<std::time::Instant>,
}

impl ReplSession {
    pub async fn session_loop(
        self: Arc<Self>,
        scheduler: Scheduler,
        state_source: Arc<dyn WorldStateSource>,
    ) {
        let mut rl = DefaultEditor::new().unwrap();
        loop {
            let output = block_in_place(|| rl.readline("> "));
            match output {
                Ok(line) => {
                    rl.add_history_entry(line.clone())
                        .expect("Could not add history");
                    if let Err(e) = self
                        .clone()
                        .handle_input(Objid(2), scheduler.clone(), line, state_source.clone())
                        .await
                    {
                        println!("Error: {e:?}");
                    }
                }
                Err(ReadlineError::Eof) => {
                    println!("<EOF>");
                    break;
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    continue;
                }
                Err(e) => {
                    println!("Error: {e:?}");
                    break;
                }
            }
        }
    }

    pub async fn handle_input(
        self: Arc<Self>,
        player: Objid,
        scheduler: Scheduler,
        program: String,
        state_source: Arc<dyn WorldStateSource>,
    ) -> Result<(), anyhow::Error> {
        (*self.last_activity.write().await) = std::time::Instant::now();

        let mut command = program.trim().to_string();

        // Dump out the list of running tasks known to the scheduler.
        if command == "@tasks" {
            let tasks = scheduler.tasks().await?;
            if tasks.is_empty() {
                println!("No running background tasks.");
                return Ok(());
            }
            println!("Running background tasks:");
            for task in tasks {
                println!("TASK {task:?}");
            }
            return Ok(());
        }

        if let Some(command) = command.strip_prefix("@list ") {
            if let Err(err_str) =
                list_command(self.player, command.to_string(), state_source.clone()).await
            {
                println!("{err_str}");
            }
            return Ok(());
        }

        // Check EVAL_BLOCK_WORDS for a prefix, and if found, don't add a return or postfix a ;
        if !EVAL_BLOCK_WORDS.iter().any(|&s| command.starts_with(s)) {
            if command.starts_with(';') {
                command = command[1..].to_string();
            }
            if !command.starts_with("return ") {
                command = format!("return {command}");
            }
            if !command.ends_with(';') {
                command = format!("{command};");
            }
        }

        let task_id = scheduler
            .submit_eval_task(player, player, command, self.clone())
            .await?;

        let subscription = scheduler.subscribe_to_task(task_id).await?;
        match subscription.await? {
            TaskWaiterResult::Success(v) => {
                println!("=> {v}");
            }
            TaskWaiterResult::Error(SchedulerError::TaskAbortedException(e)) => {
                println!("EXCEPTION: {e:?}");
            }
            TaskWaiterResult::Error(SchedulerError::TaskAbortedLimit(a)) => {
                println!("TIMEOUT: {a:?}");
            }
            TaskWaiterResult::Error(SchedulerError::TaskAbortedCancelled) => {
                println!("CANCELLED");
            }
            TaskWaiterResult::Error(e) => {
                println!("ERROR: {e:?}");
            }
        }
        Ok(())
    }
}
#[async_trait]
impl Session for ReplSession {
    async fn commit(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn rollback(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, Error> {
        Ok(self)
    }

    async fn send_event(&self, _player: Objid, msg: NarrativeEvent) -> Result<(), Error> {
        println!("{}", msg.event());
        Ok(())
    }

    async fn send_system_msg(&self, _player: Objid, msg: &str) -> Result<(), Error> {
        println!("SYS_MSG: {msg}");
        Ok(())
    }

    async fn shutdown(&self, msg: Option<String>) -> Result<(), Error> {
        println!("SHUTDOWN: {}", msg.unwrap_or_default());
        exit(0);
    }

    async fn connection_name(&self, player: Objid) -> Result<String, Error> {
        Ok(format!("REPL:{player}"))
    }

    async fn disconnect(&self, player: Objid) -> Result<(), Error> {
        println!("DISCONNECT: {player}");
        Ok(())
    }

    async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
        Ok(vec![self.player])
    }

    async fn connected_seconds(&self, _player: Objid) -> Result<f64, Error> {
        let now = std::time::Instant::now();
        let duration = now.duration_since(self.connect_time);
        Ok(duration.as_secs_f64())
    }

    async fn idle_seconds(&self, _player: Objid) -> Result<f64, Error> {
        let now = std::time::Instant::now();
        let last_activity = self.last_activity.read().await;
        let duration = now.duration_since(*last_activity);
        Ok(duration.as_secs_f64())
    }
}

// Prop and verb names must be a single identifier, underscores and alpha only, no leading numbers.
fn valid_ident(id_str: &str) -> bool {
    id_str
        .matches(|c: char| c.is_alphanumeric() || c == '_' || c == '@')
        .count()
        == id_str.len()
        && !id_str.starts_with(char::is_numeric)
}

async fn parse_objref(perms: Objid, obj_str: &str, tx: &dyn WorldState) -> Result<Objid, String> {
    if let Some(obj_str) = obj_str.strip_prefix('#') {
        // Parse to number...
        match obj_str.parse::<u64>() {
            Ok(oid) => Ok(Objid(oid as i64)),
            Err(_) => Err(format!(
                "Bad object reference ({obj_str}); but must of form #123 or $name"
            )),
        }
    } else if let Some(obj_str) = obj_str.strip_prefix('$') {
        // TODO Must be a single identifier, underscores and alpha only, no leading numbers.
        if !valid_ident(obj_str) {
            return Err(format!(
                "Bad object reference ({obj_str}); but must of form #123 or $name"
            ));
        }
        // Look up on #0...
        let Ok(pvalue) = tx.retrieve_property(perms, Objid(0), obj_str).await else {
            return Err(format!(
                "Invalid $object reference; couldn't not access {obj_str}"
            ));
        };
        let Variant::Obj(o) = pvalue.variant() else {
            return Err(format!(
                "Invalid $object reference; not an object. ({obj_str}; {pvalue:?})"
            ));
        };
        Ok(*o)
    } else {
        return Err(format!(
            "Bad object reference ({obj_str}); but must of form #123 or $name"
        ));
    }
}

// Decompile a verb. A bit like a MOO core's @list.
// @list obj:verb
// Accepts only object numbers or $type names, as we don't have access to a matcher here.
async fn list_command(
    perms: Objid,
    command_args: String,
    state_source: Arc<dyn WorldStateSource>,
) -> Result<(), String> {
    let arguments = command_args.split(':').collect::<Vec<_>>();
    if arguments.len() != 2 {
        return Err("Usage: @list obj:verb".to_string());
    }
    let Ok(mut tx) = state_source.new_world_state().await else {
        return Err("Unable to get world state".to_string());
    };
    let (obj_str, verb_str) = (arguments[0], arguments[1]);
    let obj = parse_objref(perms, obj_str, tx.as_ref()).await?;

    if !valid_ident(verb_str) {
        return Err(format!("Invalid verb name {verb_str}"));
    }

    // Look up the verb.
    let Ok(verb) = tx.get_verb(Objid(2), obj, verb_str).await else {
        return Err(format!("Unable to find verb {verb_str} on object {obj}"));
    };

    // If it's not a MOO binary, we can't handle that.
    if verb.binary_type() != BinaryType::LambdaMoo18X {
        return Err(format!(
            "Verb {verb_str} on object {obj} is not a MOO binary"
        ));
    }

    let Ok(verb) = tx.retrieve_verb(Objid(2), obj, verb.uuid()).await else {
        return Err(format!(
            "Unable to retrieve verb {} with uuid {} on object {}",
            verb_str,
            verb.uuid(),
            obj
        ));
    };

    // If the binary is empty, just error out.
    if verb.binary().is_empty() {
        return Err(format!("Verb {verb_str} on object {obj} is empty"));
    }

    // Parse its binary as a program...
    let program = Program::from_sliceref(verb.binary());
    let decompiled = match program_to_tree(&program) {
        Ok(decompiled) => decompiled,
        Err(e) => {
            return Err(format!(
                "Unable to decompile verb {verb_str} on object {obj}: {e:?}"
            ));
        }
    };

    let unparsed = match unparse(&decompiled) {
        Ok(unparsed) => unparsed,
        Err(e) => {
            return Err(format!(
                "Unable to unparse verb {verb_str} on object {obj}: {e:?}"
            ));
        }
    };
    println!(
        "Verb {} on object {}:\n{}",
        verb_str,
        obj,
        unparsed.join("\n")
    );

    tx.rollback().await.unwrap();

    Ok(())
}
