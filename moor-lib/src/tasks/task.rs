use std::sync::Arc;

use anyhow::Error;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tracing::{debug, error, instrument, trace, warn};
use uuid::Uuid;

use crate::db::state::WorldState;
use crate::model::objects::ObjFlag;
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{VerbFlag, VerbInfo};
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::{Sessions, TaskId};
use crate::util::bitenum::BitEnum;
use crate::values::objid::Objid;
use crate::values::var::Var;
use crate::values::variant::Variant;
use crate::vm::opcode::Binary;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, VM};

#[derive(Debug)]
pub(crate) enum TaskControlMsg {
    StartCommandVerb {
        player: Objid,
        vloc: Objid,
        verbinfo: VerbInfo,
        command: ParsedCommand,
    },
    StartVerb {
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
    },
    StartEval {
        player: Objid,
        binary: Binary,
    },
    Abort,
}

#[derive(Debug)]
pub(crate) enum TaskControlResponse {
    Success(Var),
    Exception(FinallyReason),
    AbortError(Error),
    AbortCancelled,
}

pub(crate) struct Task {
    task_id: TaskId,
    control_receiver: UnboundedReceiver<TaskControlMsg>,
    response_sender: UnboundedSender<(TaskId, TaskControlResponse)>,
    player: Objid,
    vm: VM,
    sessions: Arc<RwLock<dyn Sessions>>,
    state: Box<dyn WorldState>,
}

pub(crate) struct TaskControl {
    pub(crate) control_sender: UnboundedSender<TaskControlMsg>,
}

impl Task {
    pub fn new(
        task_id: TaskId,
        control_receiver: UnboundedReceiver<TaskControlMsg>,
        response_sender: UnboundedSender<(TaskId, TaskControlResponse)>,
        player: Objid,
        vm: VM,
        sessions: Arc<RwLock<dyn Sessions>>,
        state: Box<dyn WorldState>,
    ) -> Self {
        Self {
            task_id,
            control_receiver,
            response_sender,
            player,
            vm,
            sessions,
            state,
        }
    }

    #[instrument(skip(self), name="task_run", fields(task_id = task_id))]
    pub(crate) async fn run(&mut self, task_id: TaskId) {
        let mut running_method = false;

        // Special flag for 'eval' to get it to rollback on completion instead of commit.
        let mut rollback_on_complete = false;
        loop {
            // If not running a method, wait for a control message, otherwise continue to execute
            // opcodes but pick up messages if we happen to have one.
            let msg = if running_method {
                match self.control_receiver.try_recv() {
                    Ok(msg) => Some(msg),
                    Err(TryRecvError::Empty) => None,
                    Err(_) => panic!("Task control channel closed"),
                }
            } else {
                self.control_receiver.recv().await
            };

            if let Some(msg) = msg {
                // Check for control messages.
                match msg {
                    // We've been asked to start a command.
                    // We need to set up the VM and then execute it.
                    TaskControlMsg::StartCommandVerb {
                        player,
                        vloc,
                        verbinfo,
                        command,
                    } => {
                        // We should never be asked to start a command while we're already running one.
                        assert!(!running_method);
                        trace!(?command, ?player, ?vloc, ?verbinfo, "Starting command");
                        self.vm
                            .setup_verb_command(
                                self.task_id,
                                verbinfo,
                                vloc,
                                vloc,
                                player,
                                BitEnum::new_with(ObjFlag::Wizard),
                                &command,
                            )
                            .expect("Could not set up VM for command execution");
                        running_method = true;
                    }

                    TaskControlMsg::StartVerb {
                        player,
                        vloc,
                        verb,
                        args,
                    } => {
                        // We should never be asked to start a command while we're already running one.
                        assert!(!running_method);
                        trace!(?verb, ?player, ?vloc, ?args, "Starting verb");
                        self.vm
                            .setup_verb_method_call(
                                self.task_id,
                                self.state.as_mut(),
                                vloc,
                                verb.as_str(),
                                vloc,
                                player,
                                BitEnum::new_with(ObjFlag::Wizard),
                                &args,
                            )
                            .expect("Could not set up VM for command execution");
                        running_method = true;
                    }
                    TaskControlMsg::StartEval { player, binary } => {
                        assert!(!running_method);
                        trace!(?player, ?binary, "Starting eval");
                        // Stick the binary into the player object under a temp name.
                        let tmp_name = Uuid::new_v4().to_string();
                        self.state
                            .add_verb(
                                player,
                                vec![tmp_name.clone()],
                                player,
                                BitEnum::new_with(VerbFlag::Read)
                                    | VerbFlag::Exec
                                    | VerbFlag::Debug,
                                VerbArgsSpec::this_none_this(),
                                binary.clone(),
                            )
                            .expect("Could not add temp verb");
                        rollback_on_complete = true;
                        running_method = true;

                        // Now execute it.
                        self.vm
                            .setup_verb_method_call(
                                self.task_id,
                                self.state.as_mut(),
                                player,
                                tmp_name.as_str(),
                                player,
                                player,
                                BitEnum::new_with(ObjFlag::Wizard),
                                &[],
                            )
                            .expect("Could not set up VM for command execution");
                    }
                    // We've been asked to die.
                    TaskControlMsg::Abort => {
                        trace!("Aborting task");
                        self.state.rollback().unwrap();

                        self.response_sender
                            .send((self.task_id, TaskControlResponse::AbortCancelled))
                            .expect("Could not send abort response");
                        return;
                    }
                }
            };

            if !running_method {
                continue;
            }
            let result = self
                .vm
                .exec(self.state.as_mut(), self.sessions.clone())
                .await;
            match result {
                Ok(ExecutionResult::More) => {}
                Ok(ExecutionResult::Complete(a)) => {
                    trace!(task_id, result = ?a, "Task complete");
                    if rollback_on_complete {
                        self.state.rollback().unwrap();
                    } else {
                        self.state.commit().unwrap();
                    }

                    debug!(
                        "Task {} complete with result: {:?}; rollback? {}",
                        task_id, a, rollback_on_complete
                    );

                    self.response_sender
                        .send((self.task_id, TaskControlResponse::Success(a)))
                        .expect("Could not send success response");
                    return;
                }
                Ok(ExecutionResult::Exception(fr)) => {
                    trace!(task_id, result = ?fr, "Task exception");
                    self.state.rollback().unwrap();

                    match &fr {
                        FinallyReason::Abort => {
                            error!("Task {} aborted", task_id);
                            if let Err(send_error) = self
                                .sessions
                                .write()
                                .await
                                .send_text(self.player, format!("Aborted: {:?}", fr).as_str())
                                .await
                            {
                                warn!("Could not send abort message to player: {:?}", send_error);
                            };

                            if let Err(send_error) = self
                                .response_sender
                                .send((self.task_id, TaskControlResponse::AbortCancelled))
                            {
                                warn!("Could not send abort cancelled response: {:?}", send_error);
                            }
                        }
                        FinallyReason::Uncaught {
                            code: _,
                            msg: _,
                            value: _,
                            stack: _,
                            backtrace,
                        } => {
                            // Compose a string out of the backtrace
                            let mut traceback = vec![];
                            for frame in backtrace.iter() {
                                let Variant::Str(s) = frame.variant() else {
                                    continue;
                                };
                                traceback.push(format!("{:}\n", s));
                            }

                            for l in traceback.iter() {
                                if let Err(send_error) = self
                                    .sessions
                                    .write()
                                    .await
                                    .send_text(self.player, l.as_str())
                                    .await
                                {
                                    warn!("Could not send traceback to player: {:?}", send_error);
                                }
                            }

                            if let Err(send_error) = self
                                .response_sender
                                .send((self.task_id, TaskControlResponse::Exception(fr)))
                            {
                                warn!("Could not send exception response: {:?}", send_error);
                            }
                        }
                        _ => {
                            self.response_sender
                                .send((self.task_id, TaskControlResponse::Exception(fr.clone())))
                                .expect("Could not send exception response");
                            unreachable!(
                                "Invalid FinallyReason {:?} reached for task {} in scheduler",
                                fr, task_id
                            )
                        }
                    }

                    return;
                }
                Err(e) => {
                    self.state.rollback().unwrap();
                    error!(task_id, error = ?e, "Task error");

                    self.response_sender
                        .send((self.task_id, TaskControlResponse::AbortError(e)))
                        .expect("Could not send error response");
                    return;
                }
            }
        }
    }
}
