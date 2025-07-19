// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
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
use byteview::ByteView;
use lazy_static::lazy_static;
use uuid::Uuid;

use moor_common::model::VerbArgsSpec;
use moor_common::model::VerbDef;
use moor_common::model::VerbFlag;
use moor_common::util::BitEnum;
use moor_compiler::BuiltinId;
use moor_compiler::Program;
use moor_compiler::ScatterLabel;
use moor_var::Lambda;
use moor_var::{AsByteBuffer, Symbol};
use moor_var::{Error, v_empty_str};
use moor_var::{List, NOTHING};
use moor_var::{Obj, v_arc_string};
use moor_var::{Var, v_empty_list, v_list, v_obj, v_str, v_string};

use crate::vm::VerbExecutionRequest;
use crate::vm::moo_frame::MooStackFrame;
use crate::vm::scatter_assign::scatter_assign;
use moor_common::matching::ParsedCommand;
use moor_var::program::ProgramType;
use moor_var::program::names::{GlobalName, Name};

lazy_static! {
    static ref EVAL_SYMBOL: Symbol = Symbol::mk("eval");
}

/// Helper function to perform scatter assignment for lambda parameter binding
/// Uses the shared scatter assignment logic and handles lambda-specific defaults
fn lambda_scatter_assign(
    scatter_args: &moor_var::program::opcode::ScatterArgs,
    args: &[Var],
    environment: &mut [Var],
) -> Result<(), Error> {
    use moor_var::v_int;
    use std::collections::HashSet;

    // Track which parameters were actually assigned
    let mut assigned_params = HashSet::new();

    // Use the shared scatter assignment logic
    let result = scatter_assign(scatter_args, args, |name, value| {
        let name_idx = name.0 as usize;
        if name_idx < environment.len() {
            environment[name_idx] = value;
            assigned_params.insert(name_idx);
        }
    });

    match result.result {
        Err(e) => Err(e),
        Ok(()) => {
            // For lambdas, optional parameters that weren't provided should be set to 0 (false)
            // This is different from VM execution where jump-to-default logic is used
            if result.needs_defaults {
                for label in scatter_args.labels.iter() {
                    if let ScatterLabel::Optional(id, _) = label {
                        let name_idx = id.0 as usize;
                        if name_idx < environment.len() && !assigned_params.contains(&name_idx) {
                            environment[name_idx] = v_int(0); // MOO false value
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

/// Activation frame for the call stack of verb executions.
/// Holds the current VM stack frame, along with the current verb activation information.
#[derive(Debug, Clone)]
pub(crate) struct Activation {
    /// The current stack frame, which holds the current execution state for the interpreter
    /// running this activation.
    pub(crate) frame: Frame,
    /// The object that is the receiver of the current verb call.
    pub(crate) this: Var,
    /// The object that is the 'player' role; that is, the active user of this task.
    pub(crate) player: Obj,
    /// The arguments to the verb or bf being called.
    pub(crate) args: List,
    /// The name of the verb that is currently being executed.
    pub(crate) verb_name: Symbol,
    /// The extended information about the verb that is currently being executed.
    pub(crate) verbdef: VerbDef,
    /// This is the "task perms" for the current activation. It is the "who" the verb is acting on
    /// behalf-of in terms of permissions in the world.
    /// Set initially to verb owner ('programmer'). It is what set_task_perms() can override,
    /// and caller_perms() returns the value of this in the *parent* stack frame (or #-1 if none)
    pub(crate) permissions: Obj,
    /// The command that triggered this verb call, if any.
    pub(crate) command: Option<Box<ParsedCommand>>,
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

impl<C> Decode<C> for Activation {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let frame = Frame::decode(decoder)?;
        let this = Var::decode(decoder)?;
        let player = Obj::decode(decoder)?;
        let args = Vec::<Var>::decode(decoder)?;
        let verb_name = Symbol::decode(decoder)?;
        let permissions = Obj::decode(decoder)?;
        let command = Option::<Box<ParsedCommand>>::decode(decoder)?;

        let verbdef_bytes = Vec::<u8>::decode(decoder)?;
        let verbdef_bytes = ByteView::from(verbdef_bytes);
        let verbdef = VerbDef::from_bytes(verbdef_bytes).unwrap();

        Ok(Self {
            frame,
            this,
            player,
            args: List::mk_list(&args),
            verb_name,
            verbdef,
            permissions,
            command,
        })
    }
}

impl<'de, C> BorrowDecode<'de, C> for Activation {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let frame = Frame::borrow_decode(decoder)?;
        let this = Var::borrow_decode(decoder)?;
        let player = Obj::borrow_decode(decoder)?;
        let args = Vec::<Var>::borrow_decode(decoder)?;
        let verb_name = Symbol::borrow_decode(decoder)?;
        let permissions = Obj::borrow_decode(decoder)?;
        let command = Option::<Box<ParsedCommand>>::borrow_decode(decoder)?;

        let verbdef_bytes = Vec::<u8>::borrow_decode(decoder)?;
        let verbdef_bytes = ByteView::from(verbdef_bytes);
        let verbdef = VerbDef::from_bytes(verbdef_bytes).unwrap();

        Ok(Self {
            frame,
            this,
            player,
            args: List::mk_list(&args),
            verb_name,
            verbdef,
            permissions,
            command,
        })
    }
}
#[derive(Clone, Debug, Encode, Decode)]
pub enum Frame {
    Moo(Box<MooStackFrame>),
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
            Frame::Moo(frame) => {
                frame.set_variable(name, value);
                Ok(())
            }
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
            Frame::Moo(frame) => {
                frame.push(value);
            }
            Frame::Bf(bf_frame) => {
                bf_frame.return_value = Some(value);
            }
        }
    }

    pub fn return_value(&self) -> Var {
        match self {
            Frame::Moo(frame) => frame.peek_top().clone(),
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

impl Activation {
    pub fn is_builtin_frame(&self) -> bool {
        matches!(self.frame, Frame::Bf(_))
    }

    #[allow(irrefutable_let_patterns)] // We know this
    #[allow(clippy::boxed_local)] // It gets called w/ a Box so shut up, I have no choice, clippy
    // is a Moo frame. We're just making room
    pub fn for_call(verb_call_request: Box<VerbExecutionRequest>) -> Self {
        let program = verb_call_request.program;
        let verb_owner = verb_call_request.resolved_verb.owner();

        let ProgramType::MooR(program) = program else {
            unimplemented!("Only MOO programs are supported")
        };
        let frame = Box::new(MooStackFrame::new(program));
        let mut frame = Frame::Moo(frame);
        frame.set_global_variable(GlobalName::this, verb_call_request.call.this.clone());
        frame.set_global_variable(GlobalName::player, v_obj(verb_call_request.call.player));
        frame.set_global_variable(GlobalName::caller, verb_call_request.call.caller.clone());
        frame.set_global_variable(
            GlobalName::verb,
            v_arc_string(verb_call_request.call.verb_name.as_arc_string()),
        );
        frame.set_global_variable(GlobalName::args, verb_call_request.call.args.clone().into());

        // From the command, if any...
        if let Some(ref command) = verb_call_request.command {
            frame.set_global_variable(GlobalName::argstr, v_string(command.argstr.clone()));
            frame.set_global_variable(GlobalName::dobj, v_obj(command.dobj.unwrap_or(NOTHING)));
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
            frame.set_global_variable(GlobalName::iobj, v_obj(command.iobj.unwrap_or(NOTHING)));
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
            frame.set_global_variable(GlobalName::dobj, v_obj(NOTHING));
            frame.set_global_variable(GlobalName::dobjstr, v_str(""));
            frame.set_global_variable(GlobalName::prepstr, v_str(""));
            frame.set_global_variable(GlobalName::iobj, v_obj(NOTHING));
            frame.set_global_variable(GlobalName::iobjstr, v_str(""));
        }

        Self {
            frame,
            this: verb_call_request.call.this.clone(),
            player: verb_call_request.call.player,
            verbdef: verb_call_request.resolved_verb,
            verb_name: verb_call_request.call.verb_name,
            command: verb_call_request.command.clone(),
            args: verb_call_request.call.args.clone(),
            permissions: verb_owner,
        }
    }

    /// Create an activation for lambda execution
    /// Inherits context from current activation but uses lambda's program
    pub fn for_lambda_call(
        lambda: &Lambda,
        current_activation: &Activation,
        args: Vec<Var>,
    ) -> Result<Self, Error> {
        // Create new frame with lambda's program
        let mut frame = Box::new(MooStackFrame::new(lambda.0.body.clone()));

        // Merge captured variables into the fresh environment
        // The MooStackFrame::new already created a proper global environment
        if !lambda.0.captured_env.is_empty() {
            // Ensure the environment has at least as many scopes as the captured environment
            while frame.environment.len() < lambda.0.captured_env.len() {
                frame.environment.push(vec![]);
            }

            // Merge captured variables from each scope
            for (scope_idx, captured_scope) in lambda.0.captured_env.iter().enumerate() {
                if scope_idx < frame.environment.len() {
                    let target_scope = &mut frame.environment[scope_idx];

                    // Ensure target scope has enough slots
                    if target_scope.len() < captured_scope.len() {
                        target_scope.resize(captured_scope.len(), moor_var::v_none());
                    }

                    for (var_idx, captured_var) in captured_scope.iter().enumerate() {
                        if var_idx < target_scope.len() && !captured_var.is_none() {
                            target_scope[var_idx] = captured_var.clone();
                        }
                    }
                }
            }
        }

        // Lambda parameters go into their designated scopes
        // Group parameters by scope depth using a Vec (scope depths are sequential from 0)
        let max_scope_depth = lambda
            .0
            .params
            .labels
            .iter()
            .map(|label| {
                let name = match label {
                    moor_var::program::opcode::ScatterLabel::Required(name) => name,
                    moor_var::program::opcode::ScatterLabel::Optional(name, _) => name,
                    moor_var::program::opcode::ScatterLabel::Rest(name) => name,
                };
                name.1 as usize
            })
            .max()
            .unwrap_or(0);

        let mut scope_params: Vec<Vec<moor_var::program::opcode::ScatterLabel>> =
            vec![Vec::new(); max_scope_depth + 1];

        for label in &lambda.0.params.labels {
            let name = match label {
                moor_var::program::opcode::ScatterLabel::Required(name) => name,
                moor_var::program::opcode::ScatterLabel::Optional(name, _) => name,
                moor_var::program::opcode::ScatterLabel::Rest(name) => name,
            };

            let scope_depth = name.1 as usize;
            scope_params[scope_depth].push(label.clone());
        }

        // For each scope that has parameters, ensure it exists and bind parameters
        for (scope_depth, labels) in scope_params.iter().enumerate() {
            if labels.is_empty() {
                continue;
            }
            // Ensure we have enough scopes
            while frame.environment.len() <= scope_depth {
                frame.environment.push(vec![]);
            }

            // Find the maximum offset needed for this scope
            let max_offset = labels
                .iter()
                .map(|label| match label {
                    moor_var::program::opcode::ScatterLabel::Required(name) => name.0 as usize,
                    moor_var::program::opcode::ScatterLabel::Optional(name, _) => name.0 as usize,
                    moor_var::program::opcode::ScatterLabel::Rest(name) => name.0 as usize,
                })
                .max()
                .unwrap_or(0);

            // Ensure the scope has enough space
            if frame.environment[scope_depth].len() <= max_offset {
                frame.environment[scope_depth].resize(max_offset + 1, moor_var::v_none());
            }

            // Create a ScatterArgs for just this scope
            let scope_scatter = moor_var::program::opcode::ScatterArgs {
                labels: labels.clone(),
                done: lambda.0.params.done,
            };

            // Perform parameter binding for this scope
            lambda_scatter_assign(&scope_scatter, &args, &mut frame.environment[scope_depth])?;
        }

        // Handle self-reference for recursive lambdas
        if let Some(self_var_name) = lambda.0.self_var {
            // Extract the information we need without borrowing frame.environment
            let current_env = frame.environment.clone();
            let var_idx = self_var_name.0 as usize;

            // Create a deep copy of the lambda to avoid cycles
            let self_lambda = lambda.for_self_reference();
            let lambda_var = moor_var::Var::mk_lambda(
                self_lambda.0.params.clone(),
                self_lambda.0.body.clone(),
                current_env,
                self_lambda.0.self_var,
            );

            // Now set the self-reference in the environment
            if let Some(env) = frame.environment.last_mut() {
                if var_idx < env.len() {
                    env[var_idx] = lambda_var;
                }
            }
        }

        let mut frame = Frame::Moo(frame);

        // Inherit global variables from current activation (this, player, etc.)
        frame.set_global_variable(GlobalName::this, current_activation.this.clone());
        frame.set_global_variable(GlobalName::player, v_obj(current_activation.player));
        frame.set_global_variable(GlobalName::caller, current_activation.this.clone());
        // Format verb name: show function name if available, otherwise just <fn>
        let lambda_name = if let Some(self_var) = lambda.0.self_var {
            // Get the variable name from the lambda's program
            if let Some(var_name) = lambda.0.body.var_names().ident_for_name(&self_var) {
                format!(
                    "{}.{}",
                    current_activation.verb_name.as_string(),
                    var_name.as_string()
                )
            } else {
                format!("{}.<fn>", current_activation.verb_name.as_string())
            }
        } else {
            format!("{}.<fn>", current_activation.verb_name.as_string())
        };
        frame.set_global_variable(GlobalName::verb, v_str(&lambda_name));
        frame.set_global_variable(GlobalName::args, v_list(&args));

        // Copy command context if available
        if let Some(ref command) = current_activation.command {
            frame.set_global_variable(GlobalName::argstr, v_string(command.argstr.clone()));
            frame.set_global_variable(GlobalName::dobj, v_obj(command.dobj.unwrap_or(NOTHING)));
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
            frame.set_global_variable(GlobalName::iobj, v_obj(command.iobj.unwrap_or(NOTHING)));
            frame.set_global_variable(
                GlobalName::iobjstr,
                command
                    .iobjstr
                    .as_ref()
                    .map_or_else(v_empty_str, |s| v_string(s.clone())),
            );
        }

        Ok(Self {
            frame,
            this: current_activation.this.clone(),
            player: current_activation.player,
            verbdef: current_activation.verbdef.clone(),
            verb_name: Symbol::mk(&lambda_name),
            command: current_activation.command.clone(),
            args: args.iter().cloned().collect(),
            permissions: current_activation.permissions,
        })
    }

    pub fn for_eval(permissions: Obj, player: &Obj, program: Program) -> Self {
        let verbdef = VerbDef::new(
            Uuid::new_v4(),
            NOTHING,
            NOTHING,
            &[Symbol::mk("eval")],
            BitEnum::new_with(VerbFlag::Exec) | VerbFlag::Debug,
            VerbArgsSpec::this_none_this(),
        );

        let frame = Box::new(MooStackFrame::new(program));
        let mut frame = Frame::Moo(frame);

        frame.set_global_variable(GlobalName::this, v_obj(NOTHING));
        frame.set_global_variable(GlobalName::player, v_obj(*player));
        frame.set_global_variable(GlobalName::caller, v_obj(*player));
        frame.set_global_variable(GlobalName::verb, v_empty_str());
        frame.set_global_variable(GlobalName::args, v_empty_list());
        frame.set_global_variable(GlobalName::argstr, v_empty_str());
        frame.set_global_variable(GlobalName::dobj, v_obj(NOTHING));
        frame.set_global_variable(GlobalName::dobjstr, v_empty_str());
        frame.set_global_variable(GlobalName::prepstr, v_empty_str());
        frame.set_global_variable(GlobalName::iobj, v_obj(NOTHING));
        frame.set_global_variable(GlobalName::iobjstr, v_empty_str());

        Self {
            frame,
            this: v_obj(*player),
            player: *player,
            verbdef,
            verb_name: *EVAL_SYMBOL,
            command: None,
            args: List::mk_list(&[]),
            permissions,
        }
    }

    pub fn for_bf_call(
        bf_id: BuiltinId,
        bf_name: Symbol,
        args: List,
        _verb_flags: BitEnum<VerbFlag>,
        player: Obj,
    ) -> Self {
        let verbdef = VerbDef::new(
            Uuid::new_v4(),
            NOTHING,
            NOTHING,
            &[bf_name],
            BitEnum::new_with(VerbFlag::Exec),
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
            this: v_obj(NOTHING),
            player,
            verbdef,
            verb_name: bf_name,
            command: None,
            args,
            permissions: NOTHING,
        }
    }

    pub fn verb_definer(&self) -> Obj {
        match self.frame {
            Frame::Bf(_) => NOTHING,
            _ => self.verbdef.location(),
        }
    }
}
