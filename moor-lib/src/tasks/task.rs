use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{bail, Error};
use metrics_macros::increment_counter;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, error, instrument, trace, warn};
use uuid::Uuid;

use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::variant::Variant;
use moor_value::var::{v_int, Var};

use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::scheduler::{AbortLimitReason, SchedulerControlMsg, TaskDescription};
use crate::tasks::{Sessions, TaskId, VerbCall};
use crate::vm::opcode::Program;
use crate::vm::vm_execute::VmExecParams;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, ForkRequest, VerbExecutionRequest, VM};
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verb_info::VerbInfo;
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::world_state::WorldState;
use moor_value::model::CommitResult;
use moor_value::util::slice_ref::SliceRef;
use moor_value::AsByteBuffer;
use moor_value::NOTHING;

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
    StartEval { player: Objid, program: Program },
    /// The scheduler is telling the task to resume execution. Use the given world state
    /// (transaction) and permissions when doing so.
    Resume(Box<dyn WorldState>, Var),
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
    pub(crate) scheduled_start_time: Option<SystemTime>,
    pub(crate) task_control_receiver: UnboundedReceiver<TaskControlMsg>,
    pub(crate) scheduler_control_sender: UnboundedSender<SchedulerControlMsg>,
    pub(crate) player: Objid,
    pub(crate) vm: VM,
    pub(crate) sessions: Arc<RwLock<dyn Sessions>>,
    pub(crate) world_state: Box<dyn WorldState>,
    pub(crate) perms: Objid,
    pub(crate) running_method: bool,
    pub(crate) tmp_verb: Option<(Objid, String)>,
    /// The maximum stack detph for this task
    pub(crate) max_stack_depth: usize,
    /// The amount of ticks (opcode executions) allotted to this task
    pub(crate) max_ticks: usize,
    /// The maximum amount of time allotted to this task
    pub(crate) max_time: Duration,
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
                        increment_counter!("tasks.error.exec");
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
                        increment_counter!("tasks.success_complete");
                        drop_tmp_verb(self.world_state.as_mut(), self.perms, &self.tmp_verb).await;

                        // TODO: restart the whole task on conflict.
                        let CommitResult::Success = self
                            .world_state
                            .commit()
                            .await
                            .expect("Could not attempt commit")
                        else {
                            unimplemented!("Task restart on conflict")
                        };
                        trace!(self.task_id, result = ?result, "Task complete, committed");

                        self.scheduler_control_sender
                            .send(vm_exec_result)
                            .expect("Could not send success response");
                        return;
                    }
                    _ => {
                        increment_counter!("tasks.error.unknown");
                        trace!(task_id = self.task_id, "Task end, error");
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
                increment_counter!("task.start_command");

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
                let cr = self.start_call_command_verb(verbinfo, call, command, self.perms)?;
                self.scheduled_start_time = None;
                self.vm.start_time = Some(SystemTime::now());
                self.vm.tick_count = 0;
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
                increment_counter!("task.start_verb");
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
                let cr = self.start_call_method_verb(call).await?;
                self.scheduled_start_time = None;
                self.vm.start_time = Some(SystemTime::now());
                self.vm.tick_count = 0;
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
                self.scheduled_start_time = None;
                self.vm.start_time = Some(SystemTime::now());
                self.vm.tick_count = 0;
                self.vm.exec_fork_vector(fork_request, task_id).await?;
                self.running_method = !suspended;
            }
            TaskControlMsg::StartEval { player, program } => {
                increment_counter!("task.start_eval");

                assert!(!self.running_method);
                trace!(?player, ?program, "Starting eval");
                // Stick the binary into the player object under a temp name.
                let tmp_name = Uuid::new_v4().to_string();
                self.world_state
                    .add_verb(
                        self.perms,
                        player,
                        vec![tmp_name.clone()],
                        player,
                        BitEnum::new_with(VerbFlag::Read) | VerbFlag::Exec | VerbFlag::Debug,
                        VerbArgsSpec::this_none_this(),
                        Self::encode_program(&program)?,
                        BinaryType::LambdaMoo18X,
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
                let cr = self.start_call_method_verb(call).await?;
                self.scheduled_start_time = None;
                self.vm.start_time = Some(SystemTime::now());
                self.vm.tick_count = 0;
                self.vm.exec_call_request(self.task_id, cr).await?;
                self.running_method = true;

                // Set up to remove the eval verb later...
                self.tmp_verb = Some((player, tmp_name.clone()));
                return Ok(None);
            }
            TaskControlMsg::Resume(world_state, value) => {
                increment_counter!("task.resume");

                // We're back. Get a new world state and resume.
                debug!(
                    task_id = self.task_id,
                    "Resuming task, getting new transaction"
                );
                self.world_state = world_state;
                // suspend needs a return value.
                self.vm.top_mut().push(value);
                self.scheduled_start_time = None;
                self.vm.start_time = Some(SystemTime::now());
                self.vm.tick_count = 0;
                debug!(task_id = self.task_id, "Resuming task...");
                self.running_method = true;
                return Ok(None);
            }
            // We've been asked to die.
            TaskControlMsg::Abort => {
                increment_counter!("task.abort");

                trace!("Aborting task");
                self.world_state.rollback().await?;

                return Ok(Some(SchedulerControlMsg::TaskAbortCancelled));
            }
            TaskControlMsg::Describe(reply_sender) => {
                increment_counter!("task.describe");

                trace!("Received and responding to describe request");
                let description = TaskDescription {
                    task_id: self.task_id,
                    start_time: self.scheduled_start_time,
                    permissions: self.vm.top().permissions,
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

        // Check ticks and seconds, and abort the task if we've exceeded the limits.
        let time_left = match self.vm.start_time {
            Some(start_time) => {
                let elapsed = start_time.elapsed()?;
                if elapsed > self.max_time {
                    return Ok(Some(SchedulerControlMsg::TaskAbortLimitsReached(
                        AbortLimitReason::Time(elapsed),
                    )));
                }
                Some(self.max_time - elapsed)
            }
            None => None,
        };
        if self.vm.tick_count >= self.max_ticks {
            return Ok(Some(SchedulerControlMsg::TaskAbortLimitsReached(
                AbortLimitReason::Ticks(self.vm.tick_count),
            )));
        }
        let exec_params = VmExecParams {
            world_state: self.world_state.as_mut(),
            sessions: self.sessions.clone(),
            scheduler_sender: self.scheduler_control_sender.clone(),
            max_stack_depth: self.max_stack_depth,
            ticks_left: self.max_ticks - self.vm.tick_count,
            time_left,
        };
        let pre_exec_tick_count = self.vm.tick_count;
        let mut result = self.vm.exec(exec_params, self.max_ticks).await?;
        let post_exec_tick_count = self.vm.tick_count;
        trace!(
            task_id = self.task_id,
            executed_ticks = post_exec_tick_count - pre_exec_tick_count,
            ?result,
            "Executed ticks",
        );
        loop {
            match result {
                ExecutionResult::More => return Ok(None),
                ExecutionResult::ContinueVerb {
                    permissions,
                    resolved_verb,
                    call,
                    command,
                    trampoline,
                    trampoline_arg,
                } => {
                    trace!(task_id = self.task_id, call = ?call, "Task continue, call into verb");

                    self.vm.top_mut().bf_trampoline_arg = trampoline_arg;
                    self.vm.top_mut().bf_trampoline = trampoline;

                    let program = Self::decode_program(
                        resolved_verb.verbdef().binary_type(),
                        resolved_verb.binary().as_slice(),
                    )?;

                    let call_request = VerbExecutionRequest {
                        permissions,
                        resolved_verb,
                        call,
                        command,
                        program,
                    };

                    self.vm
                        .exec_call_request(self.task_id, call_request)
                        .await
                        .expect("Could not set up VM for command execution");
                    return Ok(None);
                }
                ExecutionResult::ContinueBuiltin {
                    bf_func_num: bf_offset,
                    arguments: args,
                } => {
                    let mut exec_params = VmExecParams {
                        world_state: self.world_state.as_mut(),
                        sessions: self.sessions.clone(),
                        scheduler_sender: self.scheduler_control_sender.clone(),
                        max_stack_depth: self.max_stack_depth,
                        ticks_left: self.max_ticks - self.vm.tick_count,
                        time_left,
                    };
                    // Ask the VM to execute the builtin function.
                    // This will push the result onto the stack.
                    // After this we will loop around and check the result.
                    result = self
                        .vm
                        .call_builtin_function(bf_offset, &args, &mut exec_params)
                        .await
                        .expect("Could not perform builtin execution");
                    continue;
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
                    return Ok(None);
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
                    return Ok(None);
                }
                ExecutionResult::Complete(a) => {
                    trace!(task_id = self.task_id, result = ?a, "Task complete");
                    return Ok(Some(SchedulerControlMsg::TaskSuccess(a)));
                }
                ExecutionResult::Exception(fr) => {
                    trace!(task_id = self.task_id, result = ?fr, "Task exception");

                    return match &fr {
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
                    };
                }
            }
        }
    }

    fn decode_program(
        binary_type: BinaryType,
        binary_bytes: &[u8],
    ) -> Result<Program, anyhow::Error> {
        match binary_type {
            BinaryType::LambdaMoo18X => {
                Ok(Program::from_sliceref(SliceRef::from_bytes(binary_bytes)))
            }
            _ => bail!("Unsupported binary type {:?}", binary_type),
        }
    }

    fn encode_program(binary: &Program) -> Result<Vec<u8>, anyhow::Error> {
        Ok(binary.make_copy_as_vec())
    }

    /// Entry point (from the scheduler) for beginning a command execution in this VM.
    fn start_call_command_verb(
        &mut self,
        vi: VerbInfo,
        verb_call: VerbCall,
        command: ParsedCommand,
        permissions: Objid,
    ) -> Result<VerbExecutionRequest, Error> {
        let binary = Self::decode_program(vi.verbdef().binary_type(), vi.binary().as_slice())?;

        let call_request = VerbExecutionRequest {
            permissions,
            resolved_verb: vi,
            call: verb_call,
            command: Some(command),
            program: binary,
        };

        Ok(call_request)
    }

    /// Entry point (from the scheduler) for beginning a verb execution in this VM.
    pub async fn start_call_method_verb(
        &mut self,
        verb_call: VerbCall,
    ) -> Result<VerbExecutionRequest, Error> {
        // Find the callable verb ...
        let verb_info = self
            .world_state
            .find_method_verb_on(self.perms, verb_call.this, verb_call.verb_name.as_str())
            .await?;

        let binary = Self::decode_program(
            verb_info.verbdef().binary_type(),
            verb_info.binary().as_slice(),
        )?;

        let call_request = VerbExecutionRequest {
            permissions: self.perms,
            resolved_verb: verb_info.clone(),
            call: verb_call,
            command: None,
            program: binary,
        };

        Ok(call_request)
    }
}

async fn drop_tmp_verb(
    state: &mut dyn WorldState,
    perms: Objid,
    tmp_verb: &Option<(Objid, String)>,
) {
    if let Some((player, verb_name)) = tmp_verb {
        if let Err(e) = state.remove_verb(perms, *player, verb_name.as_str()).await {
            error!(error = ?e, "Could not remove temp verb");
        }
    }
}
