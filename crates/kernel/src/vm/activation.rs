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

use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use bytes::Bytes;
use lazy_static::lazy_static;
use uuid::Uuid;

use moor_compiler::Name;
use moor_compiler::Program;
use moor_compiler::{BuiltinId, GlobalName};
use moor_values::model::VerbArgsSpec;
use moor_values::model::VerbDef;
use moor_values::model::{BinaryType, VerbFlag};
use moor_values::util::BitEnum;
use moor_values::NOTHING;
use moor_values::{v_empty_list, v_int, v_objid, v_str, v_string, Var, VarType};
use moor_values::{v_empty_str, Error};
use moor_values::{v_list, Objid};
use moor_values::{AsByteBuffer, Symbol};

use crate::tasks::command_parse::ParsedCommand;
use crate::vm::moo_frame::MooStackFrame;
use crate::vm::vm_call::VerbProgram;
use crate::vm::VerbExecutionRequest;

lazy_static! {
    static ref EVAL_SYMBOL: Symbol = Symbol::mk("eval");
}

/// Activation frame for the call stack of verb executions.
/// Holds the current VM stack frame, along with the current verb activation information.
#[derive(Debug, Clone)]
pub(crate) struct Activation {
    /// The current stack frame, which holds the current execution state for the interpreter
    /// running this activation.
    pub(crate) frame: Frame,
    /// The object that is the receiver of the current verb call.
    pub(crate) this: Objid,
    /// The object that is the 'player' role; that is, the active user of this task.
    pub(crate) player: Objid,
    /// The arguments to the verb or bf being called.
    pub(crate) args: Vec<Var>,
    /// The name of the verb that is currently being executed.
    pub(crate) verb_name: Symbol,
    /// The extended information about the verb that is currently being executed.
    pub(crate) verbdef: VerbDef,
    /// This is the "task perms" for the current activation. It is the "who" the verb is acting on
    /// behalf-of in terms of permissions in the world.
    /// Set initially to verb owner ('programmer'). It is what set_task_perms() can override,
    /// and caller_perms() returns the value of this in the *parent* stack frame (or #-1 if none)
    pub(crate) permissions: Objid,
    /// The command that triggered this verb call, if any.
    pub(crate) command: Option<ParsedCommand>,
}

impl Encode for Activation {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        // Everything is standard bincodable except verbdef, which is a flatbuffer.
        // TODO: this is temporary, and should be replaced with a flatbuffer encoding.
        self.frame.encode(encoder)?;
        self.this.encode(encoder)?;
        self.player.encode(encoder)?;
        self.args.encode(encoder)?;
        self.verb_name.encode(encoder)?;
        self.permissions.encode(encoder)?;
        self.command.encode(encoder)?;

        // verbdef gets encoded as its raw bytes from the flatbuffer
        let verbdef_bytes = self.verbdef.as_bytes().unwrap();
        verbdef_bytes.encode(encoder)
    }
}

impl Decode for Activation {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let frame = Frame::decode(decoder)?;
        let this = Objid::decode(decoder)?;
        let player = Objid::decode(decoder)?;
        let args = Vec::<Var>::decode(decoder)?;
        let verb_name = Symbol::decode(decoder)?;
        let permissions = Objid::decode(decoder)?;
        let command = Option::<ParsedCommand>::decode(decoder)?;

        let verbdef_bytes = Vec::<u8>::decode(decoder)?;
        let verbdef_bytes = Bytes::from(verbdef_bytes);
        let verbdef = VerbDef::from_bytes(verbdef_bytes).unwrap();

        Ok(Self {
            frame,
            this,
            player,
            args,
            verb_name,
            verbdef,
            permissions,
            command,
        })
    }
}

impl<'de> BorrowDecode<'de> for Activation {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let frame = Frame::decode(decoder)?;
        let this = Objid::decode(decoder)?;
        let player = Objid::decode(decoder)?;
        let args = Vec::<Var>::decode(decoder)?;
        let verb_name = Symbol::decode(decoder)?;
        let permissions = Objid::decode(decoder)?;
        let command = Option::<ParsedCommand>::decode(decoder)?;

        let verbdef_bytes = Vec::<u8>::decode(decoder)?;
        let verbdef_bytes = Bytes::from(verbdef_bytes);
        let verbdef = VerbDef::from_bytes(verbdef_bytes).unwrap();

        Ok(Self {
            frame,
            this,
            player,
            args,
            verb_name,
            verbdef,
            permissions,
            command,
        })
    }
}
#[derive(Clone, Debug, Encode, Decode)]
pub enum Frame {
    Moo(MooStackFrame),
    Bf(BfFrame),
}

impl Frame {
    /// What is the line number of the currently executing stack frame, if any?
    pub fn find_line_no(&self) -> Option<usize> {
        match self {
            Frame::Moo(frame) => frame.find_line_no(frame.pc),
            Frame::Bf(_) => None,
        }
    }

    pub fn set_variable(&mut self, name: &Name, value: Var) -> Result<(), Error> {
        match self {
            Frame::Moo(frame) => frame.set_variable(name, value),
            Frame::Bf(_) => {
                panic!("set_variable called for a built-in function frame")
            }
        }
    }

    pub fn set_global_variable(&mut self, gname: GlobalName, value: Var) {
        match self {
            Frame::Moo(frame) => frame.set_gvar(gname, value),
            Frame::Bf(_) => {
                panic!("set_global_variable called for a built-in function frame")
            }
        }
    }

    pub fn set_return_value(&mut self, value: Var) {
        match self {
            Frame::Moo(ref mut frame) => {
                frame.push(value);
            }
            Frame::Bf(bf_frame) => {
                bf_frame.return_value = Some(value);
            }
        }
    }

    pub fn return_value(&self) -> Var {
        match self {
            Frame::Moo(ref frame) => frame.peek_top().clone(),
            Frame::Bf(bf_frame) => bf_frame
                .return_value
                .clone()
                .expect("No return value set for built-in function"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct BfFrame {
    /// The index of the built-in function being called.
    pub(crate) bf_id: BuiltinId,
    /// If the activation is a call to a built-in function, the per-bf unique # trampoline passed
    /// in, which can be used by the bf to figure out how to resume where it left off.
    pub(crate) bf_trampoline: Option<usize>,
    /// And an optional argument that can be passed with the above...
    pub(crate) bf_trampoline_arg: Option<Var>,

    /// Return value into this frame.
    pub(crate) return_value: Option<Var>,
}

/// Set global constants into stack frame.
fn set_constants(f: &mut Frame) {
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
        matches!(self.frame, Frame::Bf(_))
    }

    #[allow(irrefutable_let_patterns)] // We know this is a Moo frame. We're just making room
    pub fn for_call(verb_call_request: VerbExecutionRequest) -> Self {
        let program = verb_call_request.program;
        let verb_owner = verb_call_request.resolved_verb.owner();

        let VerbProgram::Moo(program) = program else {
            unimplemented!("Only MOO programs are supported")
        };
        let frame = MooStackFrame::new(program);
        let mut frame = Frame::Moo(frame);
        set_constants(&mut frame);
        frame.set_global_variable(GlobalName::this, v_objid(verb_call_request.call.this));
        frame.set_global_variable(GlobalName::player, v_objid(verb_call_request.call.player));
        frame.set_global_variable(GlobalName::caller, v_objid(verb_call_request.call.caller));
        frame.set_global_variable(
            GlobalName::verb,
            v_str(verb_call_request.call.verb_name.as_str()),
        );
        frame.set_global_variable(GlobalName::args, v_list(&verb_call_request.call.args));

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
            verbdef: verb_call_request.resolved_verb,
            verb_name: verb_call_request.call.verb_name,
            command: verb_call_request.command.clone(),
            args: verb_call_request.call.args.clone(),
            permissions: verb_owner,
        }
    }

    pub fn for_eval(permissions: Objid, player: Objid, program: Program) -> Self {
        let verbdef = VerbDef::new(
            Uuid::new_v4(),
            NOTHING,
            NOTHING,
            &["eval"],
            BitEnum::new_with(VerbFlag::Exec) | VerbFlag::Debug,
            BinaryType::None,
            VerbArgsSpec::this_none_this(),
        );

        let frame = MooStackFrame::new(program);
        let mut frame = Frame::Moo(frame);

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
            verbdef,
            verb_name: *EVAL_SYMBOL,
            command: None,
            args: vec![],
            permissions,
        }
    }

    pub fn for_bf_call(
        bf_id: BuiltinId,
        bf_name: Symbol,
        args: Vec<Var>,
        _verb_flags: BitEnum<VerbFlag>,
        player: Objid,
    ) -> Self {
        let verbdef = VerbDef::new(
            Uuid::new_v4(),
            NOTHING,
            NOTHING,
            &[bf_name.as_str()],
            BitEnum::new_with(VerbFlag::Exec),
            BinaryType::None,
            VerbArgsSpec::this_none_this(),
        );

        let bf_frame = BfFrame {
            bf_id,
            bf_trampoline: None,
            bf_trampoline_arg: None,
            return_value: None,
        };
        let frame = Frame::Bf(bf_frame);
        Self {
            frame,
            this: NOTHING,
            player,
            verbdef,
            verb_name: bf_name,
            command: None,
            args,
            permissions: NOTHING,
        }
    }

    pub fn verb_definer(&self) -> Objid {
        match self.frame {
            Frame::Bf(_) => NOTHING,
            _ => self.verbdef.location(),
        }
    }

    pub fn verb_owner(&self) -> Objid {
        self.verbdef.owner()
    }
}
