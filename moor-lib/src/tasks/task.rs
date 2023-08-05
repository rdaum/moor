use std::sync::Arc;

use anyhow::Error;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tracing::{error, instrument, trace, warn};
use uuid::Uuid;

use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::{Objid, NOTHING};
use moor_value::var::variant::Variant;
use moor_value::var::{v_int, Var};

use crate::model::permissions::PermissionsContext;
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{VerbFlag, VerbInfo};
use crate::model::world_state::WorldState;
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::{Sessions, TaskId, VerbCall};
use crate::vm::opcode::Binary;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, ForkRequest, VM};

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
    StartFork {
        task_id: TaskId,
        fork_request: ForkRequest,
    },
    StartEval {
        player: Objid,
        binary: Binary,
    },
    Abort,
}

pub(crate) enum TaskControlResponse {
    Success(Var),
    Exception(FinallyReason),
    AbortError(Error),
    RequestFork(ForkRequest, tokio::sync::oneshot::Sender<TaskId>),
    AbortCancelled,
}

pub(crate) struct Task {
    task_id: TaskId,
    control_receiver: UnboundedReceiver<TaskControlMsg>,
    response_sender: UnboundedSender<TaskControlResponse>,
    player: Objid,
    vm: VM,
    sessions: Arc<RwLock<dyn Sessions>>,
    world_state: Box<dyn WorldState>,
    perms: PermissionsContext,
    running_method: bool,
    tmp_verb: Option<(Objid, String)>,
}

impl Task {
    pub fn new(
        task_id: TaskId,
        control_receiver: UnboundedReceiver<TaskControlMsg>,
        response_sender: UnboundedSender<TaskControlResponse>,
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
            world_state: state,
            perms,
            running_method: false,
            tmp_verb: None,
        }
    }

    #[instrument(skip(self), name = "task_run")]
    pub(crate) async fn run(mut self) {
        loop {
            // Ideally we'd use tokio::select! to wait on both futures simultaneously, but this
            // leads to a concurrent borrowing issue for two mutable references to `self` and
            // everything it contains.
            // I could get around that by shoving everything into Arc<RwLock or similar, but it
            // starts to get gross.
            // For now I will just run the futures in sequence (in priority)
            if self.running_method {
                let vm_exec_result = self.exec_interpreter().await;
                let vm_exec_result = match vm_exec_result {
                    Ok(Some(result)) => result,
                    Ok(None) => continue,
                    Err(err) => {
                        self.world_state.rollback().await.unwrap();
                        error!(task_id = self.task_id, error = ?err, "Task error");

                        self.response_sender
                            .send(TaskControlResponse::AbortError(err))
                            .expect("Could not send error response");
                        return;
                    }
                };
                match vm_exec_result {
                    TaskControlResponse::Success(ref result) => {
                        drop_tmp_verb(self.world_state.as_mut(), &self.perms, &self.tmp_verb).await;

                        trace!(self.task_id, result = ?result, "Task complete");
                        self.world_state.commit().await.unwrap();

                        self.response_sender
                            .send(vm_exec_result)
                            .expect("Could not send success response");
                        return;
                    }
                    _ => {
                        trace!(task_id = self.task_id, "Task end");
                        self.world_state.rollback().await.unwrap();

                        self.response_sender
                            .send(vm_exec_result)
                            .expect("Could not send success response");
                        return;
                    }
                }
            }

            match self.control_receiver.try_recv() {
                Ok(control_msg) => match self.handle_control_message(control_msg).await {
                    Ok(None) => continue,
                    Ok(Some(response)) => {
                        self.response_sender
                            .send(response)
                            .expect("Could not send response");
                        return;
                    }
                    Err(e) => {
                        error!(task_id = self.task_id, error = ?e, "Task error");
                        self.response_sender
                            .send(TaskControlResponse::AbortError(e))
                            .expect("Could not send error response");
                        return;
                    }
                },
                Err(TryRecvError::Empty) => continue,
                Err(TryRecvError::Disconnected) => {
                    error!(task_id = self.task_id, "Task control channel disconnected");
                    self.response_sender
                        .send(TaskControlResponse::AbortCancelled)
                        .expect("Could not send abort response");
                    return;
                }
            }
        }
    }

    async fn handle_control_message(
        &mut self,
        msg: TaskControlMsg,
    ) -> Result<Option<TaskControlResponse>, anyhow::Error> {
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
                assert!(!self.running_method);
                trace!(?command, ?player, ?vloc, ?verbinfo, "Starting command");
                let call = VerbCall {
                    verb_name: command.verb.clone(),
                    location: vloc,
                    this: vloc,
                    player,
                    args: command.args.clone(),
                    caller: NOTHING,
                };
                let cr = self.vm.start_call_command_verb(
                    self.task_id,
                    verbinfo,
                    call,
                    command,
                    self.perms.clone(),
                )?;
                self.vm
                    .exec_call_request(self.task_id, cr)
                    .await
                    .expect("Unable to exec verb");
                self.running_method = true;
            }

            TaskControlMsg::StartVerb {
                player,
                vloc,
                verb,
                args,
            } => {
                // We should never be asked to start a command while we're already running one.
                assert!(!self.running_method);
                trace!(?verb, ?player, ?vloc, ?args, "Starting verb");

                let call = VerbCall {
                    verb_name: verb,
                    location: vloc,
                    this: vloc,
                    player,
                    args,
                    caller: NOTHING,
                };
                let cr = self
                    .vm
                    .start_call_method_verb(
                        self.world_state.as_mut(),
                        self.task_id,
                        call,
                        self.perms.clone(),
                    )
                    .await?;
                self.vm.exec_call_request(self.task_id, cr).await?;
                self.running_method = true;
            }
            TaskControlMsg::StartFork {
                task_id,
                fork_request,
            } => {
                assert!(!self.running_method);
                trace!(?task_id, "Setting up fork");
                self.vm.exec_fork_vector(fork_request, task_id).await?;
                self.running_method = true;
            }
            TaskControlMsg::StartEval { player, binary } => {
                assert!(!self.running_method);
                trace!(?player, ?binary, "Starting eval");
                // Stick the binary into the player object under a temp name.
                let tmp_name = Uuid::new_v4().to_string();
                self.world_state
                    .add_verb(
                        self.perms.clone(),
                        player,
                        vec![tmp_name.clone()],
                        player,
                        BitEnum::new_with(VerbFlag::Read) | VerbFlag::Exec | VerbFlag::Debug,
                        VerbArgsSpec::this_none_this(),
                        binary.clone(),
                    )
                    .await?;

                let call = VerbCall {
                    verb_name: tmp_name.clone(),
                    location: player,
                    this: player,
                    player,
                    args: vec![],
                    caller: NOTHING,
                };
                let cr = self
                    .vm
                    .start_call_method_verb(
                        self.world_state.as_mut(),
                        self.task_id,
                        call,
                        self.perms.clone(),
                    )
                    .await?;
                self.vm.exec_call_request(self.task_id, cr).await?;
                self.running_method = true;

                // Set up to remove the eval verb later...
                self.tmp_verb = Some((player, tmp_name.clone()));
            }
            // We've been asked to die.
            TaskControlMsg::Abort => {
                trace!("Aborting task");
                self.world_state.rollback().await?;

                return Ok(Some(TaskControlResponse::AbortCancelled));
            }
        }
        Ok(None)
    }

    async fn exec_interpreter(&mut self) -> Result<Option<TaskControlResponse>, anyhow::Error> {
        if !self.running_method {
            return Ok(None);
        }
        let result = self
            .vm
            .exec(self.world_state.as_mut(), self.sessions.clone())
            .await?;
        match result {
            ExecutionResult::More => Ok(None),
            ExecutionResult::ContinueVerb(call_request) => {
                trace!(task_id = self.task_id, call_request = ?call_request, "Task continue, call into verb");
                self.vm
                    .exec_call_request(self.task_id, call_request)
                    .await
                    .expect("Could not set up VM for command execution");
                Ok(None)
            }
            ExecutionResult::DispatchFork(fork_request) => {
                // To fork a new task, we need to get the scheduler to do some work for us. So we'll
                // send a message back asking it to fork the task and return the new task id on a
                // reply channel.
                // We will then take the new task id and send it back to the caller.
                let (send, reply) = tokio::sync::oneshot::channel();
                let task_id_var = fork_request.task_id;
                self.response_sender
                    .send(TaskControlResponse::RequestFork(fork_request, send))
                    .expect("Could not send fork request");
                let task_id = reply.await.expect("Could not get fork reply");
                if let Some(task_id_var) = task_id_var {
                    self.vm
                        .top_mut()
                        .set_var_offset(task_id_var, v_int(task_id as i64))
                        .expect("Could not set forked task id");
                }
                Ok(None)
            }
            ExecutionResult::Complete(a) => {
                trace!(task_id = self.task_id, result = ?a, "Task complete");
                Ok(Some(TaskControlResponse::Success(a)))
            }
            ExecutionResult::Exception(fr) => {
                trace!(task_id = self.task_id, result = ?fr, "Task exception");

                match &fr {
                    FinallyReason::Abort => {
                        error!(task_id = self.task_id, "Task aborted");
                        if let Err(send_error) = self
                            .sessions
                            .write()
                            .await
                            .send_text(self.player, format!("Aborted: {:?}", fr).as_str())
                            .await
                        {
                            warn!("Could not send abort message to player: {:?}", send_error);
                        };

                        Ok(Some(TaskControlResponse::AbortCancelled))
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

                        Ok(Some(TaskControlResponse::Exception(fr)))
                    }
                    _ => {
                        unreachable!(
                            "Invalid FinallyReason {:?} reached for task {} in scheduler",
                            fr, self.task_id
                        );
                    }
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
