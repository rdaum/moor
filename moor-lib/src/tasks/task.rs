use std::sync::Arc;
use std::time::SystemTime;

use metrics_macros::increment_counter;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tracing::{debug, error, instrument, trace, warn};

use moor_value::model::verb_info::VerbInfo;
use moor_value::model::world_state::WorldState;
use moor_value::model::CommitResult;
use moor_value::var::objid::Objid;
use moor_value::var::variant::Variant;
use moor_value::var::{v_int, Var};
use moor_value::NOTHING;

use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::moo_vm_host::MooVmHost;
use crate::tasks::scheduler::{SchedulerControlMsg, TaskDescription};
use crate::tasks::sessions::Session;
use crate::tasks::vm_host::{VMHost, VMHostResponse};
use crate::tasks::{TaskId, VerbCall};
use crate::vm::opcode::Program;
use crate::vm::ForkRequest;

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

/// A task is a concurrent, transactionally isolated, thread of execution. It starts with the
/// execution of a 'verb' (or 'command verb' or 'eval' etc) and runs through to completion or
/// suspension or abort.
/// Within the task many verbs may be executed as subroutine calls from the root verb/command
/// Each task has its own VM host which is responsible for executing the program.
/// Each task has its own isolated transactional world state.
/// Each task is given a semi-isolated "session" object through which I/O is performed.
/// When a task fails, both the world state and I/O should be rolled back.
/// A task is generally tied 1:1 with a player connection, and usually come from one command, but
/// they can also be 'forked' from other tasks.
pub(crate) struct Task {
    /// My unique task id.
    pub(crate) task_id: TaskId,
    /// When this task will begin execution.
    /// For currently execution tasks this is when the task actually began running.
    /// For tasks in suspension, this is when they will wake up.
    /// If the task is in indefinite suspension, this is None.
    pub(crate) scheduled_start_time: Option<SystemTime>,
    /// The channel to receive control messages from the scheduler.
    pub(crate) task_control_receiver: UnboundedReceiver<TaskControlMsg>,
    /// The channel to send control messages to the scheduler.
    /// This sender is unique for our task, but is passed around all over the place down into the
    /// VM host and into the VM itself.
    pub(crate) scheduler_control_sender: UnboundedSender<SchedulerControlMsg>,
    /// The 'player' this task is running as.
    pub(crate) player: Objid,
    /// The session object for connection mgmt and sending messages to players
    pub(crate) session: Arc<dyn Session>,
    /// The transactionally isolated world state for this task.
    pub(crate) world_state: Box<dyn WorldState>,
    /// The permissions of the task -- the object on behalf of which all permissions are evaluated.
    pub(crate) perms: Objid,
    /// The actual VM host which is managing the execution of this task.
    pub(crate) vm_host: MooVmHost,
}

impl Task {
    #[instrument(skip(self), name = "task_run")]
    pub(crate) async fn run(mut self) {
        loop {
            // We have two potential sources of concurrent action here:
            //    * The VM host, which is running the VM and may need to suspend or abort or do
            //      something else.
            //    * The reception channel from the scheduler, which may be telling us to do
            //      something
            //
            // Ideally we'd use tokio::select! to wait on both futures simultaneously
            // but this leads to a concurrent borrowing nightmare for two mutable references to
            // `self` and everything it contains.
            // There are probably ways around this by further splitting up 'Task' into more
            // constituent pieces. but for now I will just run the futures in sequence
            // in priority order: execute as many VM opcodes in the loop as we can, and then
            // wait for a message from the scheduler.
            // This would not work for VM hosts that block. They would need to live on their own
            // thread.
            if self.vm_host.is_running() {
                let vm_exec_result = self
                    .vm_host
                    .exec_interpreter(self.task_id, self.world_state.as_mut())
                    .await;
                match vm_exec_result {
                    Ok(VMHostResponse::DispatchFork(fork_request)) => {
                        trace!(task_id = self.task_id, ?fork_request, "Task fork");
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
                            self.vm_host
                                .set_variable(task_id_var, v_int(task_id as i64));
                        }
                    }
                    Ok(VMHostResponse::Suspend(delay)) => {
                        trace!(task_id = self.task_id, delay = ?delay, "Task suspend");

                        // VMHost is now suspended for execution, and we'll be waiting for a Resume

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
                            self.scheduler_control_sender
                                .send(SchedulerControlMsg::TaskAbortCancelled)
                                .expect("Could not send suspend response");

                            // TODO: We terminate by exiting the loop here... is this right?
                            return;
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
                    }
                    Ok(VMHostResponse::ContinueOk) => continue,
                    Ok(VMHostResponse::CompleteSuccess(result)) => {
                        trace!(task_id = self.task_id, result = ?result, "Task complete, success");

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

                        self.session
                            .commit()
                            .await
                            .expect("Could not commit session...");

                        self.scheduler_control_sender
                            .send(SchedulerControlMsg::TaskSuccess(result))
                            .expect("Could not send success response");
                        return;
                    }
                    Ok(VMHostResponse::CompleteAbort) => {
                        error!(task_id = self.task_id, "Task aborted");
                        if let Err(send_error) = self
                            .session
                            .send_system_msg(self.player, format!("Aborted.").as_str())
                            .await
                        {
                            warn!("Could not send abort message to player: {:?}", send_error);
                        };

                        self.world_state
                            .rollback()
                            .await
                            .expect("Could not rollback world state transaction");
                        self.session
                            .rollback()
                            .await
                            .expect("Could not rollback connection...");

                        self.scheduler_control_sender
                            .send(SchedulerControlMsg::TaskAbortCancelled)
                            .expect("Could not send abort response");
                    }
                    Ok(VMHostResponse::CompleteException(exception)) => {
                        // Compose a string out of the backtrace
                        let mut traceback = vec![];
                        for frame in exception.backtrace.iter() {
                            let Variant::Str(s) = frame.variant() else {
                                continue;
                            };
                            traceback.push(format!("{:}\n", s));
                        }

                        for l in traceback.iter() {
                            if let Err(send_error) =
                                self.session.send_text(self.player, l.as_str()).await
                            {
                                warn!("Could not send traceback to player: {:?}", send_error);
                            }
                        }

                        // Commands that end in exceptions are still expected to be committed, to
                        // conform with MOO's expectations.
                        // We may revisit this later.
                        self.world_state.commit().await.expect("Could not commit");
                        self.session
                            .commit()
                            .await
                            .expect("Could not commit connection output");

                        self.scheduler_control_sender
                            .send(SchedulerControlMsg::TaskException(exception))
                            .expect("Could not send abort response");
                        return;
                    }
                    Err(err) => {
                        error!(task_id = self.task_id, error = ?err, "Task error; rollback");
                        increment_counter!("tasks.error.exec");
                        self.world_state
                            .rollback()
                            .await
                            .expect("Could not rollback world state");
                        self.session
                            .rollback()
                            .await
                            .expect("Could not rollback connection output");

                        self.scheduler_control_sender
                            .send(SchedulerControlMsg::TaskAbortError(err))
                            .expect("Could not send error response");
                        return;
                    }
                    Ok(VMHostResponse::AbortLimit(reason)) => {
                        self.world_state
                            .rollback()
                            .await
                            .expect("Could not rollback world state");
                        self.session
                            .rollback()
                            .await
                            .expect("Could not rollback connection output");
                        self.scheduler_control_sender
                            .send(SchedulerControlMsg::TaskAbortLimitsReached(reason))
                            .expect("Could not send error response");
                        return;
                    }
                };
            }

            // If we're not running a method, we block here instead.
            let control_msg = if self.vm_host.is_running() {
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
                assert!(!self.vm_host.is_running());
                trace!(?command, ?player, ?vloc, ?verbinfo, "Starting command");
                let call = VerbCall {
                    verb_name: command.verb.clone(),
                    location: vloc,
                    this: vloc,
                    player,
                    args: command.args.clone(),
                    caller: NOTHING,
                };
                self.vm_host
                    .start_call_command_verb(self.task_id, verbinfo, call, command, self.perms)
                    .await?;
            }

            TaskControlMsg::StartVerb {
                player,
                vloc,
                verb,
                args,
            } => {
                increment_counter!("task.start_verb");
                // We should never be asked to start a command while we're already running one.
                assert!(!self.vm_host.is_running());
                trace!(?verb, ?player, ?vloc, ?args, "Starting verb");

                let verb_call = VerbCall {
                    verb_name: verb,
                    location: vloc,
                    this: vloc,
                    player,
                    args,
                    caller: NOTHING,
                };
                // Find the callable verb ...
                let verb_info = self
                    .world_state
                    .find_method_verb_on(self.perms, verb_call.this, verb_call.verb_name.as_str())
                    .await?;

                self.vm_host
                    .start_call_method_verb(self.task_id, self.perms, verb_info, verb_call)
                    .await?;
            }
            TaskControlMsg::StartFork {
                task_id,
                fork_request,
                suspended,
            } => {
                assert!(!self.vm_host.is_running());
                trace!(?task_id, "Setting up fork");
                self.scheduled_start_time = None;

                self.vm_host
                    .start_fork(task_id, fork_request, suspended)
                    .await?;
            }
            TaskControlMsg::StartEval { player, program } => {
                increment_counter!("task.start_eval");

                assert!(!self.vm_host.is_running());

                self.scheduled_start_time = None;
                self.vm_host
                    .start_eval(self.task_id, player, program)
                    .await?;
            }
            TaskControlMsg::Resume(world_state, value) => {
                increment_counter!("task.resume");

                // We're back. Get a new world state and resume.
                debug!(
                    task_id = self.task_id,
                    "Resuming task, getting new transaction"
                );
                self.world_state = world_state;
                self.scheduled_start_time = None;
                self.vm_host.resume_execution(value).await?;
                return Ok(None);
            }
            TaskControlMsg::Abort => {
                // We've been asked to die. Go tell the VM host to abort, and roll back the
                // transaction.
                increment_counter!("task.abort");
                trace!(task_id = self.task_id, "Aborting task");
                self.vm_host.stop();
                self.world_state.rollback().await?;

                // And now tell the scheduler we're done, as we exit.
                return Ok(Some(SchedulerControlMsg::TaskAbortCancelled));
            }
            TaskControlMsg::Describe(reply_sender) => {
                increment_counter!("task.describe");

                let description = TaskDescription {
                    task_id: self.task_id,
                    start_time: self.scheduled_start_time,
                    permissions: self.vm_host.permissions(),
                    verb_name: self.vm_host.verb_name(),
                    verb_definer: self.vm_host.verb_definer(),
                    line_number: self.vm_host.line_number(),
                    this: self.vm_host.this(),
                };
                reply_sender
                    .send(description)
                    .expect("Could not send task description");
                return Ok(None);
            }
        }
        Ok(None)
    }
}
