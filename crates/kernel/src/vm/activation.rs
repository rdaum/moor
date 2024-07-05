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

use bincode::{Decode, Encode};
use bytes::Bytes;
use lazy_static::lazy_static;
use moor_values::var::{v_empty_str, Error, List, Variant};
use moor_values::NOTHING;
use uuid::Uuid;

use moor_compiler::{GlobalName, Name};
use moor_values::model::VerbArgsSpec;
use moor_values::model::VerbDef;
use moor_values::model::VerbInfo;
use moor_values::model::{BinaryType, VerbFlag};
use moor_values::util::BitEnum;
use moor_values::var::Objid;
use moor_values::var::{v_empty_list, v_int, v_objid, v_str, v_string, Var, VarType};

use crate::tasks::command_parse::ParsedCommand;
use crate::vm::frame::Frame;
use crate::vm::vm_call::VerbProgram;
use crate::vm::VerbExecutionRequest;
use moor_compiler::Program;
use moor_values::var::Symbol;

lazy_static! {
    static ref EVAL_SYMBOL: Symbol = Symbol::mk("eval");
}

/// Activation frame for the call stack of verb executions.
/// Holds the current VM stack frame, along with the current verb activation information.
#[derive(Debug, Clone, Encode, Decode)]
pub(crate) struct Activation {
    /// Frame
    pub(crate) frame: VmStackFrame,
    /// The object that is the receiver of the current verb call.
    pub(crate) this: Objid,
    /// The object that is the 'player' role; that is, the active user of this task.
    pub(crate) player: Objid,
    /// The arguments to the verb or bf being called.
    pub(crate) args: List,
    /// The name of the verb that is currently being executed.
    pub(crate) verb_name: Symbol,
    /// The extended information about the verb that is currently being executed.
    pub(crate) verb_info: VerbInfo,
    /// This is the "task perms" for the current activation. It is the "who" the verb is acting on
    /// behalf-of in terms of permissions in the world.
    /// Set initially to verb owner ('programmer'). It is what set_task_perms() can override,
    /// and caller_perms() returns the value of this in the *parent* stack frame (or #-1 if none)
    pub(crate) permissions: Objid,
    /// The command that triggered this verb call, if any.
    pub(crate) command: Option<ParsedCommand>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub enum VmStackFrame {
    Moo(Frame),
    Bf(BfFrame),
}

impl VmStackFrame {
    /// What is the line number of the currently executing stack frame, if any?
    pub fn find_line_no(&self) -> Option<usize> {
        match self {
            VmStackFrame::Moo(frame) => frame.find_line_no(frame.pc),
            VmStackFrame::Bf(_) => None,
        }
    }

    pub fn set_variable(&mut self, name: &Name, value: Var) -> Result<(), Error> {
        match self {
            VmStackFrame::Moo(frame) => frame.set_var_offset(name, value),
            VmStackFrame::Bf(_) => {
                panic!("set_variable called for a built-in function frame")
            }
        }
    }

    pub fn set_global_variable(&mut self, gname: GlobalName, value: Var) {
        match self {
            VmStackFrame::Moo(frame) => frame.set_gvar(gname, value),
            VmStackFrame::Bf(_) => {
                panic!("set_global_variable called for a built-in function frame")
            }
        }
    }

    pub fn set_return_value(&mut self, value: Var) {
        match self {
            VmStackFrame::Moo(ref mut frame) => {
                frame.push(value);
            }
            VmStackFrame::Bf(bf_frame) => {
                bf_frame.return_value = Some(value);
            }
        }
    }

    pub fn return_value(&self) -> Var {
        match self {
            VmStackFrame::Moo(ref frame) => frame.peek_top().clone(),
            VmStackFrame::Bf(bf_frame) => bf_frame
                .return_value
                .clone()
                .expect("No return value set for built-in function"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct BfFrame {
    /// The index of the built-in function being called.
    pub(crate) bf_index: usize,
    /// If the activation is a call to a built-in function, the per-bf unique # trampoline passed
    /// in, which can be used by the bf to figure out how to resume where it left off.
    pub(crate) bf_trampoline: Option<usize>,
    /// And an optional argument that can be passed with the above...
    pub(crate) bf_trampoline_arg: Option<Var>,

    /// Return value into this frame.
    pub(crate) return_value: Option<Var>,
}

/// Set global constants into stack frame.
fn set_constants(f: &mut VmStackFrame) {
    f.set_global_variable(GlobalName::NUM, v_int(VarType::TYPE_INT as i64));
    f.set_global_variable(GlobalName::OBJ, v_int(VarType::TYPE_OBJ as i64));
    f.set_global_variable(GlobalName::STR, v_int(VarType::TYPE_STR as i64));
    f.set_global_variable(GlobalName::ERR, v_int(VarType::TYPE_ERR as i64));
    f.set_global_variable(GlobalName::LIST, v_int(VarType::TYPE_LIST as i64));
    f.set_global_variable(GlobalName::INT, v_int(VarType::TYPE_INT as i64));
    f.set_global_variable(GlobalName::FLOAT, v_int(VarType::TYPE_FLOAT as i64));
}

impl Activation {
    pub fn is_builtin_frame(&self) -> bool {
        matches!(self.frame, VmStackFrame::Bf(_))
    }

    #[allow(irrefutable_let_patterns)] // We know this is a Moo frame. We're just making room
    pub fn for_call(verb_call_request: VerbExecutionRequest) -> Self {
        let program = verb_call_request.program;
        let verb_owner = verb_call_request.resolved_verb.verbdef().owner();

        let VerbProgram::MOO(program) = program else {
            unimplemented!("Only MOO programs are supported")
        };
        let frame = Frame::new(program);
        let mut frame = VmStackFrame::Moo(frame);
        set_constants(&mut frame);
        frame.set_global_variable(GlobalName::this, v_objid(verb_call_request.call.this));
        frame.set_global_variable(GlobalName::player, v_objid(verb_call_request.call.player));
        frame.set_global_variable(GlobalName::caller, v_objid(verb_call_request.call.caller));
        frame.set_global_variable(
            GlobalName::verb,
            v_str(verb_call_request.call.verb_name.as_str()),
        );
        frame.set_global_variable(
            GlobalName::args,
            Var::new(Variant::List(verb_call_request.call.args.clone())),
        );

        // From the command, if any...
        if let Some(ref command) = verb_call_request.command {
            frame.set_global_variable(GlobalName::argstr, v_string(command.argstr.clone()));
            frame.set_global_variable(GlobalName::dobj, v_objid(command.dobj.unwrap_or(NOTHING)));
            frame.set_global_variable(
                GlobalName::dobjstr,
                command
                    .dobjstr
                    .as_ref()
                    .map_or_else(v_empty_str, |s| v_string(s.clone())),
            );
            frame.set_global_variable(
                GlobalName::prepstr,
                command
                    .prepstr
                    .as_ref()
                    .map_or_else(v_empty_str, |s| v_string(s.clone())),
            );
            frame.set_global_variable(GlobalName::iobj, v_objid(command.iobj.unwrap_or(NOTHING)));
            frame.set_global_variable(
                GlobalName::iobjstr,
                command
                    .iobjstr
                    .as_ref()
                    .map_or_else(v_empty_str, |s| v_string(s.clone())),
            );
        } else {
            frame.set_global_variable(
                GlobalName::argstr,
                v_string(verb_call_request.call.argstr.clone()),
            );
            frame.set_global_variable(GlobalName::dobj, v_objid(NOTHING));
            frame.set_global_variable(GlobalName::dobjstr, v_str(""));
            frame.set_global_variable(GlobalName::prepstr, v_str(""));
            frame.set_global_variable(GlobalName::iobj, v_objid(NOTHING));
            frame.set_global_variable(GlobalName::iobjstr, v_str(""));
        }

        Self {
            frame,
            this: verb_call_request.call.this,
            player: verb_call_request.call.player,
            verb_info: verb_call_request.resolved_verb,
            verb_name: verb_call_request.call.verb_name,
            command: verb_call_request.command.clone(),
            args: verb_call_request.call.args.clone(),
            permissions: verb_owner,
        }
    }

    pub fn for_eval(permissions: Objid, player: Objid, program: Program) -> Self {
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
            Bytes::new(),
        );

        let frame = Frame::new(program);
        let mut frame = VmStackFrame::Moo(frame);

        set_constants(&mut frame);
        frame.set_global_variable(GlobalName::this, v_objid(NOTHING));
        frame.set_global_variable(GlobalName::player, v_objid(player));
        frame.set_global_variable(GlobalName::caller, v_objid(player));
        frame.set_global_variable(GlobalName::verb, v_empty_str());
        frame.set_global_variable(GlobalName::args, v_empty_list());
        frame.set_global_variable(GlobalName::argstr, v_empty_str());
        frame.set_global_variable(GlobalName::dobj, v_objid(NOTHING));
        frame.set_global_variable(GlobalName::dobjstr, v_empty_str());
        frame.set_global_variable(GlobalName::prepstr, v_empty_str());
        frame.set_global_variable(GlobalName::iobj, v_objid(NOTHING));
        frame.set_global_variable(GlobalName::iobjstr, v_empty_str());

        Self {
            frame,
            this: player,
            player,
            verb_info,
            verb_name: *EVAL_SYMBOL,
            command: None,
            args: List::new(),
            permissions,
        }
    }

    pub fn for_bf_call(
        bf_index: usize,
        bf_name: Symbol,
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
                &[bf_name.as_str()],
                BitEnum::new_with(VerbFlag::Exec),
                BinaryType::None,
                VerbArgsSpec::this_none_this(),
            ),
            Bytes::new(),
        );

        let bf_frame = BfFrame {
            bf_index,
            bf_trampoline: None,
            bf_trampoline_arg: None,
            return_value: None,
        };
        let frame = VmStackFrame::Bf(bf_frame);
        Self {
            frame,
            this: NOTHING,
            player,
            verb_info,
            verb_name: bf_name,
            command: None,
            args,
            permissions: NOTHING,
        }
    }

    pub fn verb_definer(&self) -> Objid {
        match self.frame {
            VmStackFrame::Bf(_) => NOTHING,
            _ => self.verb_info.verbdef().location(),
        }
    }

    pub fn verb_owner(&self) -> Objid {
        self.verb_info.verbdef().owner()
    }
}
