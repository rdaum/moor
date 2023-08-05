use std::sync::Arc;

use anyhow::Context;
use tokio::sync::RwLock;
use tracing::{error, span, trace, Level};

use moor_value::var::error::Error;
use moor_value::var::error::Error::{E_INVARG, E_INVIND, E_PERM, E_PROPNF, E_VARNF, E_VERBNF};
use moor_value::var::objid::Objid;
use moor_value::var::variant::Variant;
use moor_value::var::{v_int, Var};

use crate::compiler::builtins::BUILTINS;
use crate::model::permissions::{PermissionsContext, Perms};
use crate::model::verbs::VerbInfo;
use crate::model::world_state::WorldState;
use crate::model::ObjectError;
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::{Sessions, TaskId, VerbCall};
use crate::vm::activation::Activation;
use crate::vm::builtin::BfCallState;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, ForkRequest, ResolvedVerbCall, VM};

impl VM {
    /// Entry point (from the scheduler) for beginning a command execution in this VM.
    pub fn start_call_command_verb(
        &mut self,
        task_id: TaskId,
        vi: VerbInfo,
        verb_call: VerbCall,
        command: ParsedCommand,
        permissions: PermissionsContext,
    ) -> Result<ResolvedVerbCall, Error> {
        let span = span!(
            Level::TRACE,
            "start_call_command_verb",
            task_id,
            this = ?verb_call.this,
            verb = command.verb,
            verb_aliases = ?vi.names,
            player = ?verb_call.player,
            args = ?verb_call.args,
            command = ?command,
            permission = ?permissions,
        );
        let span_id = span.id();

        let call_request = ResolvedVerbCall {
            permissions,
            resolved_verb: vi,
            call: verb_call,
            command: Some(command),
        };

        tracing_enter_span(&span_id, &None);

        Ok(call_request)
    }

    /// Entry point (from the scheduler) for beginning a verb execution in this VM.
    pub async fn start_call_method_verb(
        &mut self,
        state: &mut dyn WorldState,
        task_id: TaskId,
        verb_call: VerbCall,
        permissions: PermissionsContext,
    ) -> Result<ResolvedVerbCall, Error> {
        // Find the callable verb ...
        let verb_info = match state
            .find_method_verb_on(
                permissions.clone(),
                verb_call.this,
                verb_call.verb_name.as_str(),
            )
            .await
        {
            Ok(vi) => vi,
            Err(ObjectError::ObjectPermissionDenied) => {
                return Err(E_PERM);
            }
            Err(ObjectError::VerbPermissionDenied) => {
                return Err(E_PERM);
            }
            Err(ObjectError::VerbNotFound(_, _)) => {
                return Err(E_VERBNF);
            }
            Err(e) => {
                error!(error = ?e, "Error finding verb");
                return Err(E_INVIND);
            }
        };

        let span = span!(
            Level::TRACE,
            "start_call_method_verb",
            task_id,
            this = ?verb_call.this,
            verb = verb_call.verb_name,
            verb_aliases = ?verb_info.names,
            player = ?verb_call.player,
            args = ?verb_call.args,
            permission = ?permissions,
        );
        let span_id = span.id();

        let call_request = ResolvedVerbCall {
            permissions,
            resolved_verb: verb_info,
            call: verb_call,
            command: None,
        };

        tracing_enter_span(&span_id, &None);

        Ok(call_request)
    }
    /// Entry point for preparing a verb call for execution, invoked from the CallVerb opcode
    /// Seek the verb and prepare the call parameters.
    /// All parameters for player, caller, etc. are pulled off the stack.
    /// The call params will be returned back to the task in the scheduler, which will then dispatch
    /// back through to `do_method_call`
    pub(crate) async fn prepare_call_verb(
        &mut self,
        state: &mut dyn WorldState,
        this: Objid,
        verb_name: &str,
        args: &[Var],
    ) -> Result<ExecutionResult, anyhow::Error> {
        let call = VerbCall {
            verb_name: verb_name.to_string(),
            location: this,
            this,
            player: self.top().player,
            args: args.to_vec(),
            caller: self.top().permissions.caller_perms().obj,
        };
        let self_valid = state.valid(this).await?;
        if !self_valid {
            return self.push_error(E_INVIND);
        }
        // Find the callable verb ...
        let verb_info = match state
            .find_method_verb_on(self.top().permissions.clone(), this, verb_name)
            .await
        {
            Ok(vi) => vi,
            Err(ObjectError::ObjectPermissionDenied) => {
                return self.push_error(E_PERM);
            }
            Err(ObjectError::VerbPermissionDenied) => {
                return self.push_error(E_PERM);
            }
            Err(ObjectError::VerbNotFound(_, _)) => {
                return self.push_error_msg(E_VERBNF, format!("Verb \"{}\" not found", verb_name));
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!("Error finding verb \"{}\" on object {}", verb_name, this)
                })?;
            }
        };

        // Derive permissions for the new activation from the current one + the verb's owner
        // permissions.
        let verb_owner = verb_info.attrs.owner.unwrap();
        let next_task_perms = state.flags_of(verb_owner).await?;
        let permissions = self
            .top()
            .permissions
            .mk_child_perms(Perms::new(verb_owner, next_task_perms));

        // Construct the call request based on a combination of the current activation record and
        // the new values.
        let call_request = ResolvedVerbCall {
            permissions,
            resolved_verb: verb_info,
            call,
            command: self.top().command.clone(),
        };

        Ok(ExecutionResult::ContinueVerb(call_request))
    }

    /// Setup the VM to execute the verb of the same current name, but using the parent's
    /// version.
    pub(crate) async fn prepare_pass_verb(
        &mut self,
        state: &mut dyn WorldState,
        args: &[Var],
    ) -> Result<ExecutionResult, anyhow::Error> {
        // get parent of verb definer object & current verb name.
        let definer = self.top().verb_definer();
        let permissions = self.top().permissions.clone();
        let parent = state.parent_of(permissions.clone(), definer).await?;
        let verb = self.top().verb_name.to_string();

        // call verb on parent, but with our current 'this'
        trace!(task_id = self.top().task_id, verb, ?definer, ?parent);

        let Ok(vi) = state.find_method_verb_on(
            permissions.clone(),
            parent,
            verb.as_str(),
        ).await else {
            return self.raise_error(E_VERBNF);
        };

        let call = VerbCall {
            verb_name: verb,
            location: parent,
            this: self.top().this,
            player: self.top().player,
            args: args.to_vec(),
            caller: permissions.caller_perms().obj,
        };
        let call_request = ResolvedVerbCall {
            permissions,
            resolved_verb: vi,
            call,
            command: self.top().command.clone(),
        };

        Ok(ExecutionResult::ContinueVerb(call_request))
    }

    /// Entry point from scheduler for actually beginning the dispatch of a method execution
    /// (non-command) in this VM.
    /// Actually creates the activation record and puts it on the stack.
    pub async fn exec_call_request(
        &mut self,
        task_id: TaskId,
        call_request: ResolvedVerbCall,
    ) -> Result<(), anyhow::Error> {
        let span = span!(Level::TRACE, "VC", task_id, ?call_request);
        let span_id = span.id();

        let a = Activation::for_call(task_id, call_request, span_id.clone())?;

        self.stack.push(a);

        tracing_enter_span(&span_id, &None);

        Ok(())
    }

    /// Prepare a new stack & call hierarchy for invocation of a forked task.
    /// Called (ultimately) from the scheduler as the result of a fork() call.
    /// We get an activation record which is a copy of where it was borked from, and a new Binary
    /// which is the new task's code, derived from a fork vector in the original task.
    pub(crate) async fn exec_fork_vector(
        &mut self,
        fork_request: ForkRequest,
        task_id: usize,
    ) -> Result<(), anyhow::Error> {
        let span = span!(Level::TRACE, "FORK", task_id);
        let span_id = span.id();

        // Set the activation up with the new task ID, and the new code.
        let mut a = fork_request.activation;
        a.span_id = span_id.clone();
        a.task_id = task_id;
        a.binary.main_vector = a.binary.fork_vectors[fork_request.fork_vector_offset.0].clone();
        a.pc = 0;
        if let Some(task_id_name) = fork_request.task_id {
            a.set_var_offset(task_id_name, v_int(task_id as i64))
                .unwrap();
        }

        // TODO how to set the task_id in the parent activation, as we no longer have a reference
        // to it?
        self.stack = vec![a];

        tracing_enter_span(&span_id, &None);
        Ok(())
    }

    /// Call into a builtin function.
    pub(crate) async fn call_builtin_function(
        &mut self,
        bf_func_num: usize,
        args: &[Var],
        state: &mut dyn WorldState,
        client_connection: Arc<RwLock<dyn Sessions>>,
    ) -> Result<ExecutionResult, anyhow::Error> {
        if bf_func_num >= self.builtins.len() {
            return self.raise_error(E_VARNF);
        }
        let bf = self.builtins[bf_func_num].clone();

        let span = span!(
            Level::TRACE,
            "BF",
            bf_name = BUILTINS[bf_func_num],
            bf_func_num,
            ?args
        );
        span.follows_from(self.top().span_id.clone());

        let _guard = span.enter();
        // this is clearly wrong and we need to be passing in a reference...
        let mut bf_args = BfCallState {
            vm: self,
            name: BUILTINS[bf_func_num],
            world_state: state,
            sessions: client_connection,
            args: args.to_vec(),
        };
        match bf.call(&mut bf_args).await {
            Ok(result) => {
                if let Variant::Err(e) = result.variant() {
                    return self.push_error(*e);
                }
                self.push(&result);
                Ok(ExecutionResult::More)
            }
            Err(e) => match e.downcast_ref() {
                Some(ObjectError::ObjectNotFound(_)) => self.push_error(E_INVARG),
                Some(ObjectError::ObjectPermissionDenied) => self.push_error(E_PERM),
                Some(ObjectError::VerbNotFound(_, _)) => self.push_error(E_VERBNF),
                Some(ObjectError::VerbPermissionDenied) => self.push_error(E_PERM),
                Some(ObjectError::InvalidVerb(_)) => self.push_error(E_VERBNF),
                Some(ObjectError::PropertyNotFound(_, _)) => self.push_error(E_PROPNF),
                Some(ObjectError::PropertyPermissionDenied) => self.push_error(E_PERM),
                Some(ObjectError::PropertyDefinitionNotFound(_, _)) => self.push_error(E_PROPNF),
                _ => Err(e),
            },
        }
    }
}

/// Manually enter a tracing span by its Id.
fn tracing_enter_span(span_id: &Option<span::Id>, follows_span: &Option<span::Id>) {
    if let Some(span_id) = span_id {
        tracing::dispatcher::get_default(|d| {
            if let Some(follows_span) = follows_span {
                d.record_follows_from(span_id, follows_span);
            }
            d.enter(span_id);
        });
    }
}

/// Manually exit a tracing span by its Id.
pub(crate) fn tracing_exit_vm_span(
    span_id: &Option<span::Id>,
    finally_reason: &FinallyReason,
    return_value: &Var,
) {
    if let Some(span_id) = span_id {
        tracing::dispatcher::get_default(|d| {
            // TODO figure out how to get the return value & exit information into the span
            trace!(?finally_reason, ?return_value, "exiting VM span");
            d.exit(span_id);
        });
    }
}
