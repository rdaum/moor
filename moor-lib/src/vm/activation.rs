use tracing::trace;

use moor_value::var::error::Error;
use moor_value::var::error::Error::E_VARNF;
use moor_value::var::objid::{Objid, NOTHING};
use moor_value::var::{v_int, v_list, v_none, v_objid, v_str, v_string, Var, VarType};

use crate::compiler::labels::{Label, Name};
use crate::model::permissions::PermissionsContext;
use crate::model::verbs::VerbInfo;
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::TaskId;
use crate::vm::opcode::{Binary, Op};
use crate::vm::ResolvedVerbCall;

// {this, verb-name, programmer, verb-loc, player, line-number}
#[derive(Clone)]
pub struct Caller {
    pub this: Objid,
    pub verb_name: String,
    pub perms: PermissionsContext,
    pub verb_loc: Objid,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Activation {
    pub(crate) task_id: TaskId,
    pub(crate) binary: Binary,
    pub(crate) environment: Vec<Var>,
    pub(crate) valstack: Vec<Var>,
    pub(crate) handler_stack: Vec<HandlerLabel>,
    pub(crate) pc: usize,
    pub(crate) temp: Var,
    pub(crate) this: Objid,
    pub(crate) player: Objid,
    pub(crate) permissions: PermissionsContext,
    pub(crate) verb_name: String,
    pub(crate) verb_info: VerbInfo,
    pub(crate) command: Option<ParsedCommand>,
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
            span_id,
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

    pub fn verb_definer(&self) -> Objid {
        self.verb_info.attrs.definer.unwrap()
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
