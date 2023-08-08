use tracing::trace;

use moor_value::util::bitenum::BitEnum;
use moor_value::var::error::Error;
use moor_value::var::error::Error::E_VARNF;
use moor_value::var::objid::{Objid, NOTHING};
use moor_value::var::{v_int, v_list, v_none, v_objid, v_str, v_string, Var, VarType};

use crate::compiler::labels::{Label, Name};
use crate::model::permissions::PermissionsContext;
use crate::model::verbs::{VerbAttrs, VerbFlag, VerbInfo};
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::TaskId;
use crate::vm::opcode::{Binary, Op, EMPTY_PROGRAM};
use crate::vm::ResolvedVerbCall;

// {this, verb-name, programmer, verb-loc, player, line-number}
#[derive(Clone)]
pub struct Caller {
    pub this: Objid,
    pub verb_name: String,
    pub perms: PermissionsContext,
    // For verb calls this should usually be the same as perms.task_perms.objid. But for builtins
    // this is #-1, which is one of the way that MOO cores can tell that a call is a builtin. Sigh.
    pub programmer: Objid,
    pub definer: Objid,
    pub player: Objid,
    pub line_number: usize,
}

// A Label that exists in a separate stack but is *relevant* only for the `valstack_pos`
// That is:
//   when created, the stack's current size is stored in `valstack_pos`
//   when popped off in unwind, the valstack's size is eaten back to pos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HandlerType {
    Catch(usize),
    CatchLabel(Label),
    Finally(Label),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HandlerLabel {
    pub(crate) handler_type: HandlerType,
    pub(crate) valstack_pos: usize,
}

/// Activation frame for the call stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Activation {
    /// The task ID of the task that owns this VM and this stack of activations.
    pub(crate) task_id: TaskId,
    /// The program of the verb that is currently being executed.
    pub(crate) binary: Binary,
    /// The object that is the receiver of the current verb call.
    pub(crate) this: Objid,
    /// The object that is the 'player' role; that is, the active user of this task.
    pub(crate) player: Objid,
    /// The arguments to the verb or bf being called.
    pub(crate) args: Vec<Var>,
    /// The name of the verb that is currently being executed.
    pub(crate) verb_name: String,
    /// The extended information about the verb that is currently being executed.
    pub(crate) verb_info: VerbInfo,
    /// The values of the variables currently in scope, by their offset.
    pub(crate) environment: Vec<Var>,
    /// The value stack.
    pub(crate) valstack: Vec<Var>,
    /// A stack of active error handlers, each relative to a position in the valstack.
    pub(crate) handler_stack: Vec<HandlerLabel>,
    /// The program counter.
    pub(crate) pc: usize,
    /// Scratch space for PushTemp and PutTemp opcodes.
    pub(crate) temp: Var,
    /// The permissions context for this verb call.
    pub(crate) permissions: PermissionsContext,
    /// The command that triggered this verb call, if any.
    pub(crate) command: Option<ParsedCommand>,
    /// If the activation is a call to a built-in function, the index of that function, in which
    /// case "verb_name", "verb_info", etc. are meaningless
    pub(crate) bf_index: Option<usize>,
    /// If the activation is a call to a built-in function, the per-bf unique # trampoline passed
    /// in, which can be used by the bf to figure out how to resume where it left off.
    pub(crate) bf_trampoline: Option<usize>,
    /// The tracing span ID for this verb call, if any.
    pub(crate) span_id: Option<tracing::span::Id>,
}

impl Activation {
    pub fn for_call(
        task_id: TaskId,
        verb_call_request: ResolvedVerbCall,
        span_id: Option<tracing::span::Id>,
    ) -> Result<Self, anyhow::Error> {
        let binary = verb_call_request
            .resolved_verb
            .attrs
            .program
            .clone()
            .unwrap();
        let environment = vec![v_none(); binary.var_names.width()];

        let mut a = Self {
            task_id,
            binary,
            environment,
            valstack: vec![],
            handler_stack: vec![],
            pc: 0,
            temp: v_none(),
            this: verb_call_request.call.this,
            player: verb_call_request.call.player,
            permissions: verb_call_request.permissions,
            verb_info: verb_call_request.resolved_verb,
            verb_name: verb_call_request.call.verb_name.clone(),
            command: verb_call_request.command.clone(),
            bf_index: None,
            bf_trampoline: None,
            span_id,
            args: verb_call_request.call.args.clone(),
        };

        // TODO use pre-set constant offsets for these like LambdaMOO does.
        a.set_var("this", v_objid(verb_call_request.call.this))
            .unwrap();
        a.set_var("player", v_objid(verb_call_request.call.player))
            .unwrap();
        a.set_var("caller", v_objid(verb_call_request.call.caller))
            .unwrap();
        a.set_var("NUM", v_int(VarType::TYPE_INT as i64)).unwrap();
        a.set_var("OBJ", v_int(VarType::TYPE_OBJ as i64)).unwrap();
        a.set_var("STR", v_int(VarType::TYPE_STR as i64)).unwrap();
        a.set_var("ERR", v_int(VarType::TYPE_ERR as i64)).unwrap();
        a.set_var("LIST", v_int(VarType::TYPE_LIST as i64)).unwrap();
        a.set_var("INT", v_int(VarType::TYPE_INT as i64)).unwrap();
        a.set_var("FLOAT", v_int(VarType::TYPE_FLOAT as i64))
            .unwrap();
        a.set_var("verb", v_str(verb_call_request.call.verb_name.as_str()))
            .unwrap();
        a.set_var("args", v_list(verb_call_request.call.args))
            .unwrap();

        // From the command, if any...
        if let Some(command) = verb_call_request.command {
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
        } else {
            a.set_var("argstr", v_str("")).unwrap();
            a.set_var("dobj", v_objid(NOTHING)).unwrap();
            a.set_var("dobjstr", v_str("")).unwrap();
            a.set_var("prepstr", v_str("")).unwrap();
            a.set_var("iobj", v_objid(NOTHING)).unwrap();
            a.set_var("iobjstr", v_str("")).unwrap();
        }
        Ok(a)
    }

    pub fn for_bf_call(
        task_id: TaskId,
        bf_index: usize,
        bf_name: &str,
        args: Vec<Var>,
        verb_flags: BitEnum<VerbFlag>,
        player: Objid,
        permissions: PermissionsContext,
        span_id: Option<tracing::span::Id>,
    ) -> Self {
        let verb_info = VerbInfo {
            names: vec![],
            attrs: VerbAttrs {
                definer: None,
                owner: None,
                flags: Some(verb_flags),
                args_spec: None,
                program: None,
            },
        };

        Self {
            task_id,
            binary: EMPTY_PROGRAM.clone(),
            environment: vec![],
            valstack: vec![],
            handler_stack: vec![],
            pc: 0,
            temp: v_none(),
            this: NOTHING,
            player,
            permissions,
            verb_info,
            verb_name: bf_name.to_string(),
            command: None,
            bf_index: Some(bf_index),
            bf_trampoline: None,
            span_id,
            args,
        }
    }

    pub fn verb_definer(&self) -> Objid {
        if self.bf_index.is_none() {
            self.verb_info.attrs.definer.unwrap()
        } else {
            NOTHING
        }
    }

    pub fn verb_owner(&self) -> Objid {
        self.verb_info.attrs.owner.unwrap()
    }

    pub fn set_var(&mut self, name: &str, value: Var) -> Result<(), Error> {
        let n = self.binary.var_names.find_name_offset(name);
        if let Some(n) = n {
            self.environment[n] = value;
            Ok(())
        } else {
            Err(E_VARNF)
        }
    }

    pub fn set_var_offset(&mut self, offset: Name, value: Var) -> Result<(), Error> {
        if offset.0 as usize >= self.environment.len() {
            return Err(E_VARNF);
        }
        self.environment[offset.0 as usize] = value;
        Ok(())
    }

    pub fn next_op(&mut self) -> Option<Op> {
        if !self.pc < self.binary.main_vector.len() {
            return None;
        }
        let op = self.binary.main_vector[self.pc].clone();
        self.pc += 1;
        Some(op)
    }

    pub fn lookahead(&self) -> Option<Op> {
        self.binary.main_vector.get(self.pc).cloned()
    }

    pub fn skip(&mut self) {
        self.pc += 1;
    }

    pub fn pop(&mut self) -> Option<Var> {
        self.valstack.pop()
    }

    pub fn push(&mut self, v: Var) {
        self.valstack.push(v)
    }

    pub fn push_handler_label(&mut self, handler_type: HandlerType) {
        self.handler_stack.push(HandlerLabel {
            handler_type,
            valstack_pos: self.valstack.len(),
        });
    }

    pub fn pop_applicable_handler(&mut self) -> Option<HandlerLabel> {
        if self.handler_stack.is_empty() {
            return None;
        }
        if self.handler_stack[self.handler_stack.len() - 1].valstack_pos != self.valstack.len() {
            return None;
        }
        self.handler_stack.pop()
    }

    pub fn peek_top(&self) -> Option<Var> {
        self.valstack.last().cloned()
    }

    pub fn peek(&self, width: usize) -> Vec<Var> {
        let l = self.valstack.len();
        Vec::from(&self.valstack[l - width..])
    }

    pub fn jump(&mut self, label_id: Label) {
        let label = &self.binary.jump_labels[label_id.0 as usize];
        trace!("Jump to {}", label.position.0);
        self.pc = label.position.0;
    }
}
