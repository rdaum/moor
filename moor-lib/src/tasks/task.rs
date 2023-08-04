use std::sync::Arc;

use anyhow::Error;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tracing::{debug, error, instrument, trace, warn};
use uuid::Uuid;

use crate::model::permissions::PermissionsContext;
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{VerbFlag, VerbInfo};
use crate::model::world_state::WorldState;
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::{Sessions, TaskId};
use crate::vm::opcode::Binary;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, VM};
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::variant::Variant;
use moor_value::var::Var;

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
    perms: PermissionsContext,
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
        perms: PermissionsContext,
    ) -> Self {
        Self {
            task_id,
            control_receiver,
            response_sender,
            player,
            vm,
            sessions,
            state,
            perms,
        }
    }

    #[instrument(skip(self), name="task_run", fields(task_id = task_id))]
    pub(crate) async fn run(&mut self, task_id: TaskId) {
        let mut running_method = false;

        let mut tmp_verb = None;

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
                        let result = self.vm.start_call_command_verb(
                            self.task_id,
                            verbinfo,
                            vloc,
                            vloc,
                            player,
                            self.perms.clone(),
                            command,
                        );

                        let Ok(cr) = result else {
                            error!(result = ?result, "Unable to prepare verb call for command");
                            break;
                        };
                        self.vm
                            .exec_call_request(task_id, cr)
                            .await
                            .expect("Unable to exec verb");
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

                        let result = self
                            .vm
                            .start_call_method_verb(
                                self.state.as_mut(),
                                self.task_id,
                                verb,
                                vloc,
                                vloc,
                                player,
                                args,
                                self.perms.clone(),
                            )
                            .await;
                        let Ok(cr) = result else {
                            error!(result = ?result, "Unable to prepare verb call");
                            break;
                        };
                        self.vm
                            .exec_call_request(task_id, cr)
                            .await
                            .expect("Unable to exec verb");
                        running_method = true;
                    }
                    TaskControlMsg::StartEval { player, binary } => {
                        assert!(!running_method);
                        trace!(?player, ?binary, "Starting eval");
                        // Stick the binary into the player object under a temp name.
                        let tmp_name = Uuid::new_v4().to_string();
                        self.state
                            .add_verb(
                                self.perms.clone(),
                                player,
                                vec![tmp_name.clone()],
                                player,
                                BitEnum::new_with(VerbFlag::Read)
                                    | VerbFlag::Exec
                                    | VerbFlag::Debug,
                                VerbArgsSpec::this_none_this(),
                                binary.clone(),
                            )
                            .await
                            .expect("Could not add temp verb");

                        let result = self
                            .vm
                            .start_call_method_verb(
                                self.state.as_mut(),
                                self.task_id,
                                tmp_name.clone(),
                                player,
                                player,
                                player,
                                vec![],
                                self.perms.clone(),
                            )
                            .await;
                        let Ok(cr) = result else {
                            error!(result = ?result, "Unable to prepare verb call");
                            break;
                        };
                        self.vm
                            .exec_call_request(task_id, cr)
                            .await
                            .expect("Unable to exec verb");
                        running_method = true;

                        // Set up to remove the eval verb later...
                        tmp_verb = Some((player, tmp_name.clone()));
                    }
                    // We've been asked to die.
                    TaskControlMsg::Abort => {
                        trace!("Aborting task");
                        self.state.rollback().await.unwrap();

                        self.response_sender
                            .send((self.task_id, TaskControlResponse::AbortCancelled))
                            .expect("Could not send abort response");
                        break;
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
                Ok(ExecutionResult::More) => {
                    continue;
                }
                Ok(ExecutionResult::ContinueVerb(call_request)) => {
                    trace!(task_id, call_request = ?call_request, "Task continue, call into verb");
                    self.vm
                        .exec_call_request(self.task_id, call_request)
                        .await
                        .expect("Could not set up VM for command execution");
                    continue;
                }
                Ok(ExecutionResult::Complete(a)) => {
                    drop_tmp_verb(self.state.as_mut(), &self.perms, &tmp_verb).await;

                    trace!(task_id, result = ?a, "Task complete");
                    self.state.commit().await.unwrap();

                    debug!(
                        task_id, result = ?a, "Task complete"
                    );

                    self.response_sender
                        .send((self.task_id, TaskControlResponse::Success(a)))
                        .expect("Could not send success response");

                    return;
                }
                Ok(ExecutionResult::Exception(fr)) => {
                    drop_tmp_verb(self.state.as_mut(), &self.perms, &tmp_verb).await;

                    trace!(task_id, result = ?fr, "Task exception");
                    self.state.rollback().await.unwrap();

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
                    drop_tmp_verb(self.state.as_mut(), &self.perms, &tmp_verb).await;

                    self.state.rollback().await.unwrap();
                    error!(task_id, error = ?e, "Task error");

                    self.response_sender
                        .send((self.task_id, TaskControlResponse::AbortError(e)))
                        .expect("Could not send error response");
                }
            }
        }
    }
}

async fn drop_tmp_verb(
    state: &mut dyn WorldState,
    perms: &PermissionsContext,
    tmp_verb: &Option<(Objid, String)>,
) {
    if let Some((player, verb_name)) = tmp_verb {
        if let Err(e) = state
            .remove_verb(perms.clone(), *player, verb_name.as_str())
            .await
        {
            error!(error = ?e, "Could not remove temp verb");
        }
    }
}
