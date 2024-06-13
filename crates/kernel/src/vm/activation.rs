// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use daumtils::{BitArray, Bitset16, SliceRef};
use moor_values::var::{v_empty_str, List, Variant};
use moor_values::NOTHING;
use uuid::Uuid;

use moor_compiler::GlobalName;
use moor_values::model::VerbArgsSpec;
use moor_values::model::VerbDef;
use moor_values::model::VerbInfo;
use moor_values::model::{BinaryType, VerbFlag};
use moor_values::util::BitEnum;
use moor_values::var::Error;
use moor_values::var::Error::E_VARNF;
use moor_values::var::Objid;
use moor_values::var::{v_empty_list, v_int, v_none, v_objid, v_str, v_string, Var, VarType};

use crate::tasks::command_parse::ParsedCommand;
use crate::vm::VerbExecutionRequest;
use moor_compiler::Program;
use moor_compiler::{Label, Name};
use moor_compiler::{Op, EMPTY_PROGRAM};

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

/// The MOO stack-frame specific portions of the activation:
///   the value stack, local variables, program, program counter, handler stack, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
    /// The program of the verb that is currently being executed.
    pub(crate) program: Program,
    /// The program counter.
    pub(crate) pc: usize,
    // TODO: Language enhancement: Introduce lexical scopes to the MOO language:
    //      add a 'with' keyword to the language which introduces a new scope, similar to ML's "let":
    //              with x = 1 in
    //                     ...
    //              endlet
    //      Multiple variables can be introduced at once:
    //              with x = 1, y = 2 in ...
    //      Variables not declared with 'with' are verb-scoped as they are now
    //      'with' variables that shadow already-known verb-scoped variables override the verb-scope
    //      Add LetBegin and LetEnd opcodes to the language.
    //      Make the environment have a width, and expand and contract as scopes are entered and exited.
    //      Likewise, Names in Program should be scope delimited somehow
    /// The values of the variables currently in scope, by their offset.
    pub(crate) environment: BitArray<Var, 256, Bitset16<16>>,
    /// The value stack.
    pub(crate) valstack: Vec<Var>,
    /// A stack of active error handlers, each relative to a position in the valstack.
    pub(crate) handler_stack: Vec<HandlerLabel>,
    /// Scratch space for PushTemp and PutTemp opcodes.
    pub(crate) temp: Var,
}

/// Activation frame for the call stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Activation {
    /// Frame
    pub(crate) frame: Frame,
    /// The object that is the receiver of the current verb call.
    pub(crate) this: Objid,
    /// The object that is the 'player' role; that is, the active user of this task.
    pub(crate) player: Objid,
    /// The arguments to the verb or bf being called.
    pub(crate) args: List,
    /// The name of the verb that is currently being executed.
    pub(crate) verb_name: String,
    /// The extended information about the verb that is currently being executed.
    pub(crate) verb_info: VerbInfo,
    /// This is the "task perms" for the current activation. It is the "who" the verb is acting on
    /// behalf-of in terms of permissions in the world.
    /// Set initially to verb owner ('programmer'). It is what set_task_perms() can override,
    /// and caller_perms() returns the value of this in the *parent* stack frame (or #-1 if none)
    pub(crate) permissions: Objid,
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

impl Frame {
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

    #[inline]
    pub fn set_gvar(&mut self, gname: GlobalName, value: Var) {
        self.environment.set(gname as usize, value);
    }

    #[inline]
    pub fn set_env(&mut self, id: &Name, v: Var) {
        self.environment.set(id.0 as usize, v);
    }

    /// Return the value of a local variable.
    #[inline]
    pub(crate) fn get_env(&self, id: &Name) -> Option<&Var> {
        self.environment.get(id.0 as usize)
    }

    #[inline]
    pub fn set_var_offset(&mut self, offset: &Name, value: Var) -> Result<(), Error> {
        if offset.0 as usize >= self.environment.len() {
            return Err(E_VARNF);
        }
        self.environment.set(offset.0 as usize, value);
        Ok(())
    }

    #[inline]
    pub fn lookahead(&self) -> Option<Op> {
        self.program.main_vector.get(self.pc).cloned()
    }

    #[inline]
    pub fn skip(&mut self) {
        self.pc += 1;
    }

    #[inline]
    pub fn pop(&mut self) -> Var {
        self.valstack
            .pop()
            .unwrap_or_else(|| panic!("stack underflow @ PC: {}", self.pc))
    }

    #[inline]
    pub fn push(&mut self, v: Var) {
        self.valstack.push(v)
    }

    #[inline]
    pub fn peek_top(&self) -> &Var {
        self.valstack.last().expect("stack underflow")
    }

    #[inline]
    pub fn peek_top_mut(&mut self) -> &mut Var {
        self.valstack.last_mut().expect("stack underflow")
    }

    #[inline]
    pub fn peek_range(&self, width: usize) -> Vec<Var> {
        let l = self.valstack.len();
        Vec::from(&self.valstack[l - width..])
    }

    #[inline]
    pub(crate) fn peek_abs(&self, amt: usize) -> &Var {
        &self.valstack[amt]
    }

    #[inline]
    pub fn peek2(&self) -> (&Var, &Var) {
        let l = self.valstack.len();
        let (a, b) = (&self.valstack[l - 1], &self.valstack[l - 2]);
        (a, b)
    }

    #[inline]
    pub fn poke(&mut self, amt: usize, v: Var) {
        let l = self.valstack.len();
        self.valstack[l - amt - 1] = v;
    }

    #[inline]
    pub fn jump(&mut self, label_id: &Label) {
        let label = &self.program.jump_labels[label_id.0 as usize];
        self.pc = label.position.0 as usize;
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
}

/// Set global constants into stack frame.
fn set_constants(f: &mut Frame) {
    f.set_gvar(GlobalName::NUM, v_int(VarType::TYPE_INT as i64));
    f.set_gvar(GlobalName::OBJ, v_int(VarType::TYPE_OBJ as i64));
    f.set_gvar(GlobalName::STR, v_int(VarType::TYPE_STR as i64));
    f.set_gvar(GlobalName::ERR, v_int(VarType::TYPE_ERR as i64));
    f.set_gvar(GlobalName::LIST, v_int(VarType::TYPE_LIST as i64));
    f.set_gvar(GlobalName::INT, v_int(VarType::TYPE_INT as i64));
    f.set_gvar(GlobalName::FLOAT, v_int(VarType::TYPE_FLOAT as i64));
}

impl Activation {
    pub fn for_call(verb_call_request: VerbExecutionRequest) -> Self {
        let program = verb_call_request.program;
        let environment = BitArray::new();

        let verb_owner = verb_call_request.resolved_verb.verbdef().owner();
        let mut frame = Frame {
            program,
            environment,
            valstack: vec![],
            handler_stack: vec![],
            pc: 0,
            temp: v_none(),
        };

        set_constants(&mut frame);
        frame.set_gvar(GlobalName::this, v_objid(verb_call_request.call.this));
        frame.set_gvar(GlobalName::player, v_objid(verb_call_request.call.player));
        frame.set_gvar(GlobalName::caller, v_objid(verb_call_request.call.caller));
        frame.set_gvar(
            GlobalName::verb,
            v_str(verb_call_request.call.verb_name.as_str()),
        );
        frame.set_gvar(
            GlobalName::args,
            Var::new(Variant::List(verb_call_request.call.args.clone())),
        );

        // From the command, if any...
        if let Some(ref command) = verb_call_request.command {
            frame.set_gvar(GlobalName::argstr, v_string(command.argstr.clone()));
            frame.set_gvar(GlobalName::dobj, v_objid(command.dobj.unwrap_or(NOTHING)));
            frame.set_gvar(
                GlobalName::dobjstr,
                command
                    .dobjstr
                    .as_ref()
                    .map_or_else(v_empty_str, |s| v_string(s.clone())),
            );
            frame.set_gvar(
                GlobalName::prepstr,
                command
                    .prepstr
                    .as_ref()
                    .map_or_else(v_empty_str, |s| v_string(s.clone())),
            );
            frame.set_gvar(GlobalName::iobj, v_objid(command.iobj.unwrap_or(NOTHING)));
            frame.set_gvar(
                GlobalName::iobjstr,
                command
                    .iobjstr
                    .as_ref()
                    .map_or_else(v_empty_str, |s| v_string(s.clone())),
            );
        } else {
            frame.set_gvar(
                GlobalName::argstr,
                v_string(verb_call_request.call.argstr.clone()),
            );
            frame.set_gvar(GlobalName::dobj, v_objid(NOTHING));
            frame.set_gvar(GlobalName::dobjstr, v_str(""));
            frame.set_gvar(GlobalName::prepstr, v_str(""));
            frame.set_gvar(GlobalName::iobj, v_objid(NOTHING));
            frame.set_gvar(GlobalName::iobjstr, v_str(""));
        }

        Self {
            frame,
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
        }
    }

    pub fn for_eval(permissions: Objid, player: Objid, program: Program) -> Self {
        let environment = BitArray::new();

        let verb_info = VerbInfo::new(
            // Fake verbdef. Not sure how I feel about this. Similar to with BF calls.
            // Might need to clean up the requirement for a VerbInfo in Activation.
            VerbDef::new(
                Uuid::new_v4(),
                NOTHING,
                NOTHING,
                &["eval"],
                BitEnum::new_with(VerbFlag::Exec) | VerbFlag::Debug,
                BinaryType::None,
                VerbArgsSpec::this_none_this(),
            ),
            SliceRef::empty(),
        );

        let mut frame = Frame {
            program,
            environment,
            valstack: vec![],
            handler_stack: vec![],
            pc: 0,
            temp: v_none(),
        };
        set_constants(&mut frame);
        frame.set_gvar(GlobalName::this, v_objid(NOTHING));
        frame.set_gvar(GlobalName::player, v_objid(player));
        frame.set_gvar(GlobalName::caller, v_objid(player));
        frame.set_gvar(GlobalName::verb, v_empty_str());
        frame.set_gvar(GlobalName::args, v_empty_list());
        frame.set_gvar(GlobalName::argstr, v_empty_str());
        frame.set_gvar(GlobalName::dobj, v_objid(NOTHING));
        frame.set_gvar(GlobalName::dobjstr, v_empty_str());
        frame.set_gvar(GlobalName::prepstr, v_empty_str());
        frame.set_gvar(GlobalName::iobj, v_objid(NOTHING));
        frame.set_gvar(GlobalName::iobjstr, v_empty_str());

        Self {
            frame,
            this: player,
            player,
            verb_info,
            verb_name: "eval".to_string(),
            command: None,
            bf_index: None,
            bf_trampoline: None,
            bf_trampoline_arg: None,
            args: List::new(),
            permissions,
        }
    }
    pub fn for_bf_call(
        bf_index: usize,
        bf_name: &str,
        args: List,
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

        // Frame doesn't really matter.
        let frame = Frame {
            program: EMPTY_PROGRAM.clone(),
            environment: BitArray::new(),
            valstack: vec![],
            handler_stack: vec![],
            pc: 0,
            temp: v_none(),
        };
        Self {
            frame,
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
}
