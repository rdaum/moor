use std::sync::Arc;
use std::time::SystemTime;

use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, error, instrument, trace, warn};
use uuid::Uuid;

use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::{Objid, NOTHING};
use moor_value::var::variant::Variant;
use moor_value::var::{v_int, Var};

use crate::db::CommitResult;
use crate::model::permissions::PermissionsContext;
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{VerbFlag, VerbInfo};
use crate::model::world_state::WorldState;
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::scheduler::{SchedulerControlMsg, TaskDescription};
use crate::tasks::{Sessions, TaskId, VerbCall};
use crate::vm::opcode::Binary;
use crate::vm::vm_execute::VmExecParams;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, ForkRequest, VM};

/// Messages sent to tasks from the scheduler to tell the task to do things.
pub(crate) enum TaskControlMsg {
    /// The scheduler is telling the task to run a verb from a command.
    StartCommandVerb {
        player: Objid,
        vloc: Objid,
        verbinfo: VerbInfo,
        command: ParsedCommand,
    },
    /// The scheduler is telling the task to run a (method) verb.
    StartVerb {
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
    },
    /// The scheduler is telling the task to run a forked task.
    StartFork {
        task_id: TaskId,
        fork_request: ForkRequest,
        // If we're starting in a suspended state. If this is true, an explicit Resume from the
        // scheduler will be required to start the task.
        suspended: bool,
    },
    /// The scheduler is telling the task to evaluate a specific program.
    StartEval { player: Objid, binary: Binary },
    /// The scheduler is telling the task to resume execution. Use the given world state
    /// (transaction) and permissions when doing so.
    Resume(Box<dyn WorldState>, PermissionsContext, Var),
    /// The scheduler is asking the task to describe itself.
    /// This causes deadlock if the task requesting the description is the task being described,
    /// so I need to rethink this.
    Describe(oneshot::Sender<TaskDescription>),
    /// The scheduler is telling the task to abort itself.
    Abort,
}

pub(crate) struct Task {
    pub(crate) task_id: TaskId,
    /// When this task will begin execution.
    /// For currently execution tasks this is when the task actually began running.
    /// For tasks in suspension, this is when they will wake up.
    /// If the task is in indefinite suspension, this is None.
    pub(crate) start_time: Option<SystemTime>,
    pub(crate) task_control_receiver: UnboundedReceiver<TaskControlMsg>,
    pub(crate) scheduler_control_sender: UnboundedSender<SchedulerControlMsg>,
    pub(crate) player: Objid,
    pub(crate) vm: VM,
    pub(crate) sessions: Arc<RwLock<dyn Sessions>>,
    pub(crate) world_state: Box<dyn WorldState>,
    pub(crate) perms: PermissionsContext,
    pub(crate) running_method: bool,
    pub(crate) tmp_verb: Option<(Objid, String)>,
}

impl Task {
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

                        self.scheduler_control_sender
                            .send(SchedulerControlMsg::TaskAbortError(err))
                            .expect("Could not send error response");
                        return;
                    }
                };
                match vm_exec_result {
                    SchedulerControlMsg::TaskSuccess(ref result) => {
                        drop_tmp_verb(self.world_state.as_mut(), &self.perms, &self.tmp_verb).await;

                        trace!(self.task_id, result = ?result, "Task complete");
                        self.world_state.commit().await.unwrap();

                        self.scheduler_control_sender
                            .send(vm_exec_result)
                            .expect("Could not send success response");
                        return;
                    }
                    _ => {
                        trace!(task_id = self.task_id, "Task end");
                        self.world_state.rollback().await.unwrap();

                        self.scheduler_control_sender
                            .send(vm_exec_result)
                            .expect("Could not send success response");
                        return;
                    }
                }
            }

            // If we're not running a method, we block here instead.
            let control_msg = if self.running_method {
                match self.task_control_receiver.try_recv() {
                    Ok(control_msg) => control_msg,
                    Err(TryRecvError::Empty) => continue,
                    Err(TryRecvError::Disconnected) => {
                        error!(task_id = self.task_id, "Task control channel disconnected");
                        self.scheduler_control_sender
                            .send(SchedulerControlMsg::TaskAbortCancelled)
                            .expect("Could not send abort response");
                        return;
                    }
                }
            } else {
                self.task_control_receiver.recv().await.unwrap()
            };

            match self.handle_control_message(control_msg).await {
                Ok(None) => continue,
                Ok(Some(response)) => {
                    self.scheduler_control_sender
                        .send(response)
                        .expect("Could not send response");
                    return;
                }
                Err(e) => {
                    warn!(task_id = self.task_id, error = ?e, "Task mailbox receive error");
                    return;
                }
            };
        }
    }

    async fn handle_control_message(
        &mut self,
        msg: TaskControlMsg,
    ) -> Result<Option<SchedulerControlMsg>, anyhow::Error> {
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
                self.start_time = Some(SystemTime::now());
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
                self.start_time = Some(SystemTime::now());
                self.vm.exec_call_request(self.task_id, cr).await?;
                self.running_method = true;
            }
            TaskControlMsg::StartFork {
                task_id,
                fork_request,
                suspended,
            } => {
                assert!(!self.running_method);
                trace!(?task_id, "Setting up fork");
                self.start_time = Some(SystemTime::now());
                self.vm.exec_fork_vector(fork_request, task_id).await?;
                self.running_method = !suspended;
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
                self.start_time = Some(SystemTime::now());
                self.vm.exec_call_request(self.task_id, cr).await?;
                self.running_method = true;

                // Set up to remove the eval verb later...
                self.tmp_verb = Some((player, tmp_name.clone()));
                return Ok(None);
            }
            TaskControlMsg::Resume(world_state, permissions, value) => {
                // We're back. Get a new world state and resume.
                debug!(
                    task_id = self.task_id,
                    "Resuming task, getting new transaction"
                );
                self.world_state = world_state;
                self.perms = permissions;
                // suspend needs a return value.
                self.vm.top_mut().push(value);
                self.start_time = Some(SystemTime::now());
                debug!(task_id = self.task_id, "Resuming task...");
                self.running_method = true;
                return Ok(None);
            }
            // We've been asked to die.
            TaskControlMsg::Abort => {
                trace!("Aborting task");
                self.world_state.rollback().await?;

                return Ok(Some(SchedulerControlMsg::TaskAbortCancelled));
            }
            TaskControlMsg::Describe(reply_sender) => {
                trace!("Received and responding to describe request");
                let description = TaskDescription {
                    task_id: self.task_id,
                    start_time: self.start_time.clone(),
                    permissions: self.vm.top().permissions.clone(),
                    verb_name: self.vm.top().verb_name.clone(),
                    verb_definer: self.vm.top().verb_definer(),
                    // TODO: when we have proper decompilation support
                    line_number: 0,
                    this: self.vm.top().this,
                };
                reply_sender
                    .send(description)
                    .expect("Could not send task description");
                trace!("Sent task description back to scheduler");
                return Ok(None);
            }
        }
        Ok(None)
    }

    async fn exec_interpreter(&mut self) -> Result<Option<SchedulerControlMsg>, anyhow::Error> {
        if !self.running_method {
            return Ok(None);
        }
        let exec_params = VmExecParams {
            world_state: self.world_state.as_mut(),
            sessions: self.sessions.clone(),
            scheduler_sender: self.scheduler_control_sender.clone(),
        };
        let result = self.vm.exec(exec_params).await?;
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
                self.scheduler_control_sender
                    .send(SchedulerControlMsg::TaskRequestFork(fork_request, send))
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
            ExecutionResult::Suspend(delay) => {
                trace!(task_id = self.task_id, delay = ?delay, "Task suspend");
                // Attempt commit...
                // TODO: what to do on conflict? The whole thing needs to be retried, but we have
                // not implemented that at any other level yet, so we'll just abort for now.
                let commit_result = self
                    .world_state
                    .commit()
                    .await
                    .expect("Could not commit world state before suspend");
                if let CommitResult::ConflictRetry = commit_result {
                    error!("Conflict during commit before suspend");
                    return Ok(Some(SchedulerControlMsg::TaskAbortCancelled));
                }

                // Let the scheduler know about our suspension, which can be of the form:
                //      * Indefinite, wake-able only with Resume
                //      * Scheduled, a duration is given, and we'll wake up after that duration
                // In both cases we'll rely on the scheduler to wake us up in its processing loop
                // rather than sleep here, which would make this thread unresponsive to other
                // messages.
                let resume_time = delay.map(|delay| SystemTime::now() + delay);
                self.scheduler_control_sender
                    .send(SchedulerControlMsg::TaskSuspend(resume_time))
                    .expect("Could not send suspend response");

                // Turn off VM execution and now only listen on messages.
                self.running_method = false;
                Ok(None)
            }
            ExecutionResult::Complete(a) => {
                trace!(task_id = self.task_id, result = ?a, "Task complete");
                Ok(Some(SchedulerControlMsg::TaskSuccess(a)))
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

                        Ok(Some(SchedulerControlMsg::TaskAbortCancelled))
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

                        Ok(Some(SchedulerControlMsg::TaskException(fr)))
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
