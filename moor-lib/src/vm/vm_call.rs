use std::sync::Arc;

use anyhow::{bail, Context};
use tokio::sync::RwLock;
use tracing::{span, trace, Level};

use crate::compiler::builtins::BUILTINS;
use crate::model::ObjectError;

use crate::model::permissions::{PermissionsContext, Perms};
use crate::model::verbs::VerbInfo;
use crate::model::world_state::WorldState;
use crate::model::ObjectError::VerbNotFound;
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::{Sessions, TaskId};

use crate::values::error::Error::{E_INVARG, E_INVIND, E_PERM, E_PROPNF, E_VARNF, E_VERBNF};
use crate::values::objid::{Objid, NOTHING};
use crate::values::var::{v_objid, v_str, v_string, Var};
use crate::values::variant::Variant;
use crate::vm::activation::{Activation, Caller};
use crate::vm::builtin::BfCallState;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, VM};

impl VM {
    /// Entry point from scheduler for setting up a command execution in this VM.
    pub fn setup_verb_command(
        &mut self,
        task_id: TaskId,
        vi: VerbInfo,
        obj: Objid,
        this: Objid,
        player: Objid,
        permissions: PermissionsContext,
        command: &ParsedCommand,
    ) -> Result<(), anyhow::Error> {
        let Some(binary) = vi.attrs.program.clone() else {
            bail!(VerbNotFound(obj, command.verb.to_string()))
        };

        let span = span!(
            Level::TRACE,
            "setup_verb_command",
            task_id,
            ?this,
            verb = command.verb,
            verb_aliases = ?vi.names,
            ?player,
            args = ?command.args,
            permission = ?permissions,
        );
        let span_id = span.id();

        let mut a = Activation::new_for_method(
            task_id,
            binary,
            NOTHING,
            this,
            player,
            permissions,
            command.verb.as_str(),
            vi,
            &command.args,
            vec![],
            span_id.clone(),
        )?;

        // TODO use pre-set constant offsets for these like LambdaMOO does.
        a.set_var("argstr", v_string(command.argstr.clone()))
            .unwrap();
        a.set_var("dobj", v_objid(command.dobj)).unwrap();
        a.set_var("dobjstr", v_string(command.dobjstr.clone()))
            .unwrap();
        a.set_var("prepstr", v_string(command.prepstr.clone()))
            .unwrap();
        a.set_var("iobj", v_objid(command.iobj)).unwrap();
        a.set_var("iobjstr", v_string(command.iobjstr.clone()))
            .unwrap();

        self.stack.push(a);
        trace!(
            ?this,
            command.verb,
            ?command.args,
            command.argstr,
            ?command.dobj,
            command.dobjstr,
            ?command.prepstr,
            ?command.iobj,
            command.iobjstr,
            "start command"
        );

        tracing_enter_span(&span_id, &None);
        Ok(())
    }

    /// Entry point from scheduler for setting up a method execution (non-command) in this VM.
    pub async fn setup_verb_method_call(
        &mut self,
        task_id: TaskId,
        state: &mut dyn WorldState,
        permissions: PermissionsContext,
        obj: Objid,
        verb_name: &str,
        this: Objid,
        player: Objid,
        args: &[Var],
    ) -> Result<(), anyhow::Error> {
        let vi = state
            .find_method_verb_on(permissions.clone(), obj, verb_name)
            .await?;

        let Some(binary) = vi.attrs.program.clone() else {
            bail!(VerbNotFound(obj, verb_name.to_string()))
        };

        let span = span!(
            Level::TRACE,
            "setup_verb_method_call",
            task_id,
            ?this,
            verb = verb_name,
            verb_aliases = ?vi.names,
            ?player,
            ?args,
            permissions = ?permissions,
        );
        let span_id = span.id();

        let mut a = Activation::new_for_method(
            task_id,
            binary,
            NOTHING,
            this,
            player,
            permissions,
            verb_name,
            vi,
            args,
            vec![],
            span_id.clone(),
        )?;

        a.set_var("argstr", v_str("")).unwrap();
        a.set_var("dobj", v_objid(NOTHING)).unwrap();
        a.set_var("dobjstr", v_str("")).unwrap();
        a.set_var("prepstr", v_str("")).unwrap();
        a.set_var("iobj", v_objid(NOTHING)).unwrap();
        a.set_var("iobjstr", v_str("")).unwrap();

        self.stack.push(a);

        trace!(?this, verb_name, ?args, "method call");
        tracing_enter_span(&span_id, &None);

        Ok(())
    }

    /// Entry point for VM setting up a method call from the Op::CallVerb instruction.
    pub(crate) async fn call_verb(
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
        // find callable verb
        let verbinfo = match state
            .find_method_verb_on(self.top().permissions.clone(), this, verb_name)
            .await
        {
            Ok(vi) => vi,
            Err(ObjectError::ObjectPermissionDenied) => {
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
        let Some(binary) = verbinfo.attrs.program.clone() else {
            return self.push_error_msg(
                E_VERBNF,
                format!("Verb \"{}\" is not a program", verb_name),
            );
        };

        let caller = self.top().this;

        let top = self.top();
        let mut callers = top.callers.to_vec();
        let task_id = top.task_id;

        let follows_span = self.top().span_id.clone();

        // Should this be necessary? Can't we just walk the stack?
        callers.push(Caller {
            this,
            verb_name: verb_name.to_string(),
            perms: top.permissions.clone(),
            verb_loc: top.verb_definer(),
            player: top.player,
            line_number: 0,
        });

        // Derive permissions for the new activation from the current one + the verb's owner
        // permissions.
        let verb_owner = verbinfo.attrs.owner.unwrap();
        let next_task_perms = state.flags_of(verb_owner).await?;
        let new_perms = top
            .permissions
            .mk_child_perms(Perms::new(verb_owner, next_task_perms));

        let span = span!(
            Level::TRACE,
            "VC",
            task_id,
            ?this,
            verb_name,
            verb_aliases = ?verbinfo.names,
            player = ?top.player,
            ?args,
            permissions = ?new_perms,
        );
        let span_id = span.id();

        let mut a = Activation::new_for_method(
            task_id,
            binary,
            caller,
            this,
            top.player,
            new_perms,
            verb_name,
            verbinfo,
            args,
            callers,
            span_id.clone(),
        )?;

        // TODO use pre-set constant offsets for these like LambdaMOO does.
        let argstr = self.top().get_var("argstr");
        let dobj = self.top().get_var("dobj");
        let dobjstr = self.top().get_var("dobjstr");
        let prepstr = self.top().get_var("prepstr");
        let iobj = self.top().get_var("iobj");
        let iobjstr = self.top().get_var("iobjstr");

        a.set_var("argstr", argstr.unwrap()).unwrap();
        a.set_var("dobj", dobj.unwrap()).unwrap();
        a.set_var("dobjstr", dobjstr.unwrap()).unwrap();
        a.set_var("prepstr", prepstr.unwrap()).unwrap();
        a.set_var("iobj", iobj.unwrap()).unwrap();
        a.set_var("iobjstr", iobjstr.unwrap()).unwrap();

        self.stack.push(a);
        trace!(?this, verb_name, ?args, ?caller, "call_verb");

        tracing_enter_span(&span_id, &follows_span);

        Ok(ExecutionResult::More)
    }

    /// Setup the VM to execute the verb of the same current name, but using the parent's
    /// version.
    pub(crate) async fn pass_verb(
        &mut self,
        state: &mut dyn WorldState,
        args: &[Var],
    ) -> Result<ExecutionResult, anyhow::Error> {
        // get parent of verb definer object & current verb name.
        // TODO probably need verb definer right on Activation, this is gross.
        let definer = self.top().verb_definer();
        let parent = state
            .parent_of(self.top().permissions.clone(), definer)
            .await?;
        let verb = self.top().verb_name.to_string();

        // call verb on parent, but with our current 'this'
        let task_id = self.top().task_id;
        trace!(task_id, verb, ?definer, ?parent);
        self.setup_verb_method_call(
            task_id,
            state,
            self.top().permissions.clone(),
            parent,
            verb.as_str(),
            self.top().this,
            self.top().player,
            args,
        )
        .await?;
        Ok(ExecutionResult::More)
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
