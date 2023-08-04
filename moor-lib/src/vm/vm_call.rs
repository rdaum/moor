use std::sync::Arc;

use anyhow::Context;
use moor_value::var::error::Error;
use tokio::sync::RwLock;
use tracing::{error, span, trace, Level};

use crate::compiler::builtins::BUILTINS;
use crate::model::ObjectError;

use crate::model::permissions::{PermissionsContext, Perms};
use crate::model::verbs::VerbInfo;
use crate::model::world_state::WorldState;
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::{Sessions, TaskId};

use crate::vm::activation::{Activation, Caller};
use crate::vm::builtin::BfCallState;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, VerbCallRequest, VM};
use moor_value::var::error::Error::{E_INVARG, E_INVIND, E_PERM, E_PROPNF, E_VARNF, E_VERBNF};
use moor_value::var::objid::{Objid, NOTHING};
use moor_value::var::variant::Variant;
use moor_value::var::Var;

impl VM {
    /// Entry point (from the scheduler) for beginning a command execution in this VM.
    pub fn start_call_command_verb(
        &mut self,
        task_id: TaskId,
        vi: VerbInfo,
        obj: Objid,
        this: Objid,
        player: Objid,
        permissions: PermissionsContext,
        command: ParsedCommand,
    ) -> Result<VerbCallRequest, Error> {
        let span = span!(
            Level::TRACE,
            "start_call_command_verb",
            task_id,
            ?this,
            verb = command.verb,
            verb_aliases = ?vi.names,
            ?player,
            args = ?command.args,
            permission = ?permissions,
        );
        let span_id = span.id();

        let call_request = VerbCallRequest {
            verb_info: vi,
            permissions,
            location: obj,
            verb_name: command.verb.to_string(),
            this,
            player,
            caller: NOTHING,
            args: command.args.to_vec(),
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
        verb_name: String,
        obj: Objid,
        this: Objid,
        player: Objid,
        args: Vec<Var>,
        permissions: PermissionsContext,
    ) -> Result<VerbCallRequest, Error> {
        // Find the callable verb ...
        let verb_info = match state
            .find_method_verb_on(permissions.clone(), this, verb_name.as_str())
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
            ?this,
            verb = verb_name,
            verb_aliases = ?verb_info.names,
            ?player,
            ?args,
            permission = ?permissions,
        );
        let span_id = span.id();

        let call_request = VerbCallRequest {
            verb_info,
            permissions,
            location: obj,
            verb_name,
            this,
            player,
            caller: NOTHING,
            args,
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
        let call_request = VerbCallRequest {
            verb_info,
            permissions,
            location: this,
            verb_name: verb_name.to_string(),
            this,
            player: self.top().player,
            caller: self.top().verb_definer(),
            args: args.to_vec(),
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

        let call_request = VerbCallRequest {
            verb_info: vi,
            permissions,
            location: parent,
            verb_name: verb.clone(),
            this: self.top().this,
            player: self.top().player,
            caller: self.top().verb_definer(),
            args: args.to_vec(),
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
        call_request: VerbCallRequest,
    ) -> Result<(), anyhow::Error> {
        let span = span!(Level::TRACE, "VC", task_id, ?call_request);
        let span_id = span.id();

        let mut callers = if self.stack.is_empty() {
            vec![]
        } else {
            self.top().callers.clone()
        };

        // Should this be necessary? Can't we just walk the stack?
        callers.push(Caller {
            this: call_request.this,
            verb_name: call_request.verb_name.clone(),
            perms: call_request.permissions.clone(),
            verb_loc: call_request.verb_info.attrs.definer.unwrap(),
            player: call_request.player,
            line_number: 0,
        });

        let a = Activation::for_call(task_id, call_request, callers, span_id.clone())?;

        self.stack.push(a);

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
            name: BUILTINS[bf_func_num],
            world_state: state,
            frame: self.top_mut(),
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
