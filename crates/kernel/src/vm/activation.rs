use moor_values::NOTHING;
use sized_chunks::SparseChunk;
use tracing::trace;
use uuid::Uuid;

use moor_compiler::GlobalName;
use moor_values::model::r#match::VerbArgsSpec;
use moor_values::model::verb_info::VerbInfo;
use moor_values::model::verbdef::VerbDef;
use moor_values::model::verbs::{BinaryType, VerbFlag};
use moor_values::util::bitenum::BitEnum;
use moor_values::util::slice_ref::SliceRef;
use moor_values::var::error::Error;
use moor_values::var::error::Error::E_VARNF;
use moor_values::var::objid::Objid;
use moor_values::var::{
    v_empty_list, v_int, v_list, v_none, v_objid, v_str, v_string, Var, VarType,
};

use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::TaskId;
use crate::vm::VerbExecutionRequest;
use moor_compiler::labels::{Label, Name};
use moor_compiler::opcode::{Op, Program, EMPTY_PROGRAM};

// {this, verb-name, programmer, verb-loc, player, line-number}
#[derive(Clone)]
pub struct Caller {
    pub this: Objid,
    pub verb_name: String,
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
    pub(crate) program: Program,
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
    /// This is the "task perms" for the current activation. It is the "who" the verb is acting on
    /// behalf-of in terms of permissions in the world.
    /// Set initially to verb owner ('programmer'). It is what set_task_perms() can override,
    /// and caller_perms() returns the value of this in the *parent* stack frame (or #-1 if none)
    pub(crate) permissions: Objid,
    /// The values of the variables currently in scope, by their offset.
    pub(crate) environment: SparseChunk<Var>,
    /// The value stack.
    pub(crate) valstack: Vec<Var>,
    /// A stack of active error handlers, each relative to a position in the valstack.
    pub(crate) handler_stack: Vec<HandlerLabel>,
    /// The program counter.
    pub(crate) pc: usize,
    /// Scratch space for PushTemp and PutTemp opcodes.
    pub(crate) temp: Var,
    /// The command that triggered this verb call, if any.
    pub(crate) command: Option<ParsedCommand>,
    /// If the activation is a call to a built-in function, the index of that function, in which
    /// case "verb_name", "verb_info", etc. are meaningless
    pub(crate) bf_index: Option<usize>,
    /// If the activation is a call to a built-in function, the per-bf unique # trampoline passed
    /// in, which can be used by the bf to figure out how to resume where it left off.
    pub(crate) bf_trampoline: Option<usize>,
    /// And an optional argument that can be passed with the above...
    pub(crate) bf_trampoline_arg: Option<Var>,
}

impl Activation {
    pub fn for_call(task_id: TaskId, verb_call_request: VerbExecutionRequest) -> Self {
        let program = verb_call_request.program;
        let environment = SparseChunk::new();

        let verb_owner = verb_call_request.resolved_verb.verbdef().owner();
        let mut a = Self {
            task_id,
            program,
            environment,
            valstack: vec![],
            handler_stack: vec![],
            pc: 0,
            temp: v_none(),
            this: verb_call_request.call.this,
            player: verb_call_request.call.player,
            verb_info: verb_call_request.resolved_verb,
            verb_name: verb_call_request.call.verb_name.clone(),
            command: verb_call_request.command.clone(),
            bf_index: None,
            bf_trampoline: None,
            bf_trampoline_arg: None,
            args: verb_call_request.call.args.clone(),
            permissions: verb_owner,
        };

        a.set_gvar(GlobalName::this, v_objid(verb_call_request.call.this));
        a.set_gvar(GlobalName::player, v_objid(verb_call_request.call.player));
        a.set_gvar(GlobalName::caller, v_objid(verb_call_request.call.caller));
        a.set_gvar(GlobalName::NUM, v_int(VarType::TYPE_INT as i64));
        a.set_gvar(GlobalName::OBJ, v_int(VarType::TYPE_OBJ as i64));
        a.set_gvar(GlobalName::STR, v_int(VarType::TYPE_STR as i64));
        a.set_gvar(GlobalName::ERR, v_int(VarType::TYPE_ERR as i64));
        a.set_gvar(GlobalName::LIST, v_int(VarType::TYPE_LIST as i64));
        a.set_gvar(GlobalName::INT, v_int(VarType::TYPE_INT as i64));
        a.set_gvar(GlobalName::FLOAT, v_int(VarType::TYPE_FLOAT as i64));
        a.set_gvar(
            GlobalName::verb,
            v_str(verb_call_request.call.verb_name.as_str()),
        );
        a.set_gvar(GlobalName::args, v_list(&verb_call_request.call.args));

        // From the command, if any...
        if let Some(command) = verb_call_request.command {
            a.set_gvar(GlobalName::argstr, v_string(command.argstr.clone()));
            a.set_gvar(GlobalName::dobj, v_objid(command.dobj));
            a.set_gvar(GlobalName::dobjstr, v_string(command.dobjstr.clone()));
            a.set_gvar(GlobalName::prepstr, v_string(command.prepstr.clone()));
            a.set_gvar(GlobalName::iobj, v_objid(command.iobj));
            a.set_gvar(GlobalName::iobjstr, v_string(command.iobjstr.clone()));
        } else {
            a.set_gvar(
                GlobalName::argstr,
                v_string(verb_call_request.call.argstr.clone()),
            );
            a.set_gvar(GlobalName::dobj, v_objid(NOTHING));
            a.set_gvar(GlobalName::dobjstr, v_str(""));
            a.set_gvar(GlobalName::prepstr, v_str(""));
            a.set_gvar(GlobalName::iobj, v_objid(NOTHING));
            a.set_gvar(GlobalName::iobjstr, v_str(""));
        }
        a
    }

    pub fn for_eval(task_id: TaskId, permissions: Objid, player: Objid, program: Program) -> Self {
        let environment = SparseChunk::new();

        let verb_info = VerbInfo::new(
            // Fake verbdef. Not sure how I feel about this. Similar to with BF calls.
            // Might need to clean up the requirement for a VerbInfo in Activation.
            VerbDef::new(
                Uuid::new_v4(),
                NOTHING,
                NOTHING,
                &["eval"],
                BitEnum::new_with(VerbFlag::Exec),
                BinaryType::None,
                VerbArgsSpec::this_none_this(),
            ),
            SliceRef::empty(),
        );

        let mut a = Self {
            task_id,
            program,
            environment,
            valstack: vec![],
            handler_stack: vec![],
            pc: 0,
            temp: v_none(),
            this: player,
            player,
            verb_info,
            verb_name: "eval".to_string(),
            command: None,
            bf_index: None,
            bf_trampoline: None,
            bf_trampoline_arg: None,
            args: vec![],
            permissions,
        };

        a.set_gvar(GlobalName::this, v_objid(player));
        a.set_gvar(GlobalName::player, v_objid(player));
        a.set_gvar(GlobalName::caller, v_objid(player));
        a.set_gvar(GlobalName::NUM, v_int(VarType::TYPE_INT as i64));
        a.set_gvar(GlobalName::OBJ, v_int(VarType::TYPE_OBJ as i64));
        a.set_gvar(GlobalName::STR, v_int(VarType::TYPE_STR as i64));
        a.set_gvar(GlobalName::ERR, v_int(VarType::TYPE_ERR as i64));
        a.set_gvar(GlobalName::LIST, v_int(VarType::TYPE_LIST as i64));
        a.set_gvar(GlobalName::INT, v_int(VarType::TYPE_INT as i64));
        a.set_gvar(GlobalName::FLOAT, v_int(VarType::TYPE_FLOAT as i64));
        a.set_gvar(GlobalName::verb, v_str("eval"));
        a.set_gvar(GlobalName::args, v_empty_list());
        a.set_gvar(GlobalName::argstr, v_str(""));
        a.set_gvar(GlobalName::dobj, v_objid(NOTHING));
        a.set_gvar(GlobalName::dobjstr, v_str(""));
        a.set_gvar(GlobalName::prepstr, v_str(""));
        a.set_gvar(GlobalName::iobj, v_objid(NOTHING));
        a.set_gvar(GlobalName::iobjstr, v_str(""));
        a
    }
    pub fn for_bf_call(
        task_id: TaskId,
        bf_index: usize,
        bf_name: &str,
        args: Vec<Var>,
        _verb_flags: BitEnum<VerbFlag>,
        player: Objid,
    ) -> Self {
        let verb_info = VerbInfo::new(
            // Fake verbdef. Not sure how I feel about this.
            VerbDef::new(
                Uuid::new_v4(),
                NOTHING,
                NOTHING,
                &[bf_name],
                BitEnum::new_with(VerbFlag::Exec),
                BinaryType::None,
                VerbArgsSpec::this_none_this(),
            ),
            SliceRef::empty(),
        );

        trace!(bf_name, bf_index, ?args, "for_bf_call");
        Self {
            task_id,
            program: EMPTY_PROGRAM.clone(),
            environment: SparseChunk::new(),
            valstack: vec![],
            handler_stack: vec![],
            pc: 0,
            temp: v_none(),
            this: NOTHING,
            player,
            verb_info,
            verb_name: bf_name.to_string(),
            command: None,
            bf_index: Some(bf_index),
            bf_trampoline: None,
            bf_trampoline_arg: None,
            args,
            permissions: NOTHING,
        }
    }

    pub(crate) fn find_line_no(&self, pc: usize) -> Option<usize> {
        if self.program.line_number_spans.is_empty() {
            return None;
        }
        // Seek through the line # spans looking for the first offset (first part of tuple) which is
        // equal to or higher than `pc`. If we don't find one, return the last one.
        let mut last_line_num = 1;
        for (offset, line_no) in &self.program.line_number_spans {
            if *offset >= pc {
                return Some(last_line_num);
            }
            last_line_num = *line_no
        }
        Some(last_line_num)
    }

    pub fn verb_definer(&self) -> Objid {
        if self.bf_index.is_none() {
            self.verb_info.verbdef().location()
        } else {
            NOTHING
        }
    }

    pub fn verb_owner(&self) -> Objid {
        self.verb_info.verbdef().owner()
    }

    pub fn set_gvar(&mut self, gname: GlobalName, value: Var) {
        self.environment.insert(gname as usize, value);
    }

    pub fn set_var_offset(&mut self, offset: Name, value: Var) -> Result<(), Error> {
        if offset.0 as usize >= self.environment.len() {
            return Err(E_VARNF);
        }
        self.environment[offset.0 as usize] = value;
        Ok(())
    }

    pub fn next_op(&mut self) -> Option<Op> {
        if !self.pc < self.program.main_vector.len() {
            return None;
        }
        let op = self.program.main_vector[self.pc].clone();
        self.pc += 1;
        Some(op)
    }

    pub fn lookahead(&self) -> Option<Op> {
        self.program.main_vector.get(self.pc).cloned()
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
        let label = &self.program.jump_labels[label_id.0 as usize];
        trace!("Jump to {}", label.position.0);
        self.pc = label.position.0;
    }
}
