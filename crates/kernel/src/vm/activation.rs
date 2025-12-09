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

use lazy_static::lazy_static;
use moor_compiler::BUILTINS;
use std::cmp::max;
use strum::EnumCount;
use uuid::Uuid;

use moor_common::{
    model::{VerbArgsSpec, VerbDef, VerbFlag},
    util::BitEnum,
};
use moor_compiler::{BuiltinId, Program, ScatterLabel};
use moor_var::{
    Error, Lambda, List, NOTHING, Obj, Symbol, Var, v_arc_string, v_empty_list, v_empty_str,
    v_list, v_obj, v_str,
};

use crate::vm::{moo_frame::MooStackFrame, scatter_assign::scatter_assign};
use moor_var::program::{
    ProgramType,
    names::{GlobalName, Name},
};

lazy_static! {
    static ref EVAL_SYMBOL: Symbol = Symbol::mk("eval");
}

/// Helper function to perform scatter assignment for lambda parameter binding.
/// Uses the shared scatter assignment logic and handles lambda-specific defaults.
/// Environment uses v_none() as sentinel for uninitialized slots.
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
            // For lambdas with defaults, the lambda program should start with code that checks
            // each optional parameter and evaluates its default if the parameter is v_int(0).
            // We set unassigned optionals to v_int(0) as a sentinel value.
            if result.needs_defaults && result.first_default_index.is_some() {
                let first_idx = result.first_default_index.unwrap();
                // Only set sentinel for parameters at or after the first one needing defaults
                for (idx, label) in scatter_args.labels.iter().enumerate() {
                    if idx >= first_idx
                        && let ScatterLabel::Optional(id, _) = label
                    {
                        let name_idx = id.0 as usize;
                        if name_idx < environment.len() && !assigned_params.contains(&name_idx) {
                            // Set to 0 as sentinel - lambda program will check this and evaluate default
                            environment[name_idx] = v_int(0);
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
}

// Boxing MooStackFrame would add pointer indirection on every opcode dispatch,
// which is unacceptable for performance. The size difference is intentional.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
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
            Frame::Bf(bf_frame) => {
                let Some(return_value) = bf_frame.return_value.as_ref() else {
                    panic!(
                        "missing return value for frame for built-in function '{}/{}'",
                        BUILTINS.name_of(bf_frame.bf_id).unwrap(),
                        bf_frame.bf_id.0
                    )
                };
                return_value.clone()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
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

    /// Optional override for what caller_perms() should return when called from verbs invoked
    /// by this builtin. When set, caller_perms() returns this value instead of filtering out
    /// this frame. Used by dispatch_command_verb to make dispatched verbs see the player as
    /// the caller rather than the wizard who invoked dispatch_command_verb.
    pub(crate) caller_perms_override: Option<Obj>,
}

impl Activation {
    pub fn is_builtin_frame(&self) -> bool {
        matches!(self.frame, Frame::Bf(_))
    }

    #[allow(irrefutable_let_patterns)] // We know this is a Moo frame
    #[allow(clippy::too_many_arguments)]
    pub fn for_call(
        _permissions: Obj,
        resolved_verb: VerbDef,
        verb_name: Symbol,
        this: Var,
        player: Obj,
        args: List,
        caller: Var,
        argstr: Var,
        current_activation: Option<&Activation>,
        program: ProgramType,
    ) -> Self {
        let verb_owner = resolved_verb.owner();

        let ProgramType::MooR(program) = program else {
            unimplemented!("Only MOO programs are supported")
        };

        // Check if we have a Moo frame to inherit parsing globals from
        let source_frame = current_activation.and_then(|a| match &a.frame {
            Frame::Moo(frame) => Some(frame),
            Frame::Bf(_) => None,
        });

        let moo_frame = if let Some(source) = source_frame {
            // Direct construction for nested calls
            MooStackFrame::new_with_globals_from_source(
                program,
                v_obj(player),
                this.clone(),
                caller,
                v_arc_string(verb_name.as_arc_string()),
                args.clone().into(),
                source,
            )
        } else {
            // Direct construction for top-level calls
            MooStackFrame::new_with_all_globals(
                program,
                v_obj(player),
                this.clone(),
                caller,
                v_arc_string(verb_name.as_arc_string()),
                args.clone().into(),
                argstr,
            )
        };
        let frame = Frame::Moo(moo_frame);

        Self {
            frame,
            this,
            player,
            verbdef: resolved_verb,
            verb_name,
            args,
            permissions: verb_owner,
        }
    }

    /// Create an activation for lambda execution.
    /// Inherits context from current activation but uses lambda's program.
    /// Uses v_none() as sentinel for uninitialized slots.
    pub fn for_lambda_call(
        lambda: &Lambda,
        current_activation: &Activation,
        args: Vec<Var>,
    ) -> Result<Self, Error> {
        use moor_var::v_none;

        // Build environment with v_none() as sentinel for uninitialized slots
        let width = max(lambda.0.body.var_names().global_width(), GlobalName::COUNT);
        let mut temp_env: Vec<Vec<Var>> = vec![vec![v_none(); width]];

        // Merge captured variables into the environment
        if !lambda.0.captured_env.is_empty() {
            // Ensure the environment has at least as many scopes as the captured environment
            while temp_env.len() < lambda.0.captured_env.len() {
                temp_env.push(vec![]);
            }

            // Merge captured variables from each scope
            for (scope_idx, captured_scope) in lambda.0.captured_env.iter().enumerate() {
                if scope_idx < temp_env.len() {
                    let target_scope = &mut temp_env[scope_idx];

                    // Ensure target scope has enough slots
                    if target_scope.len() < captured_scope.len() {
                        target_scope.resize(captured_scope.len(), v_none());
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
        let param_depths: Vec<usize> = lambda
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
            .collect();
        let max_scope_depth = param_depths.iter().copied().max().unwrap_or(0);

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
            while temp_env.len() <= scope_depth {
                temp_env.push(vec![]);
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
            if temp_env[scope_depth].len() <= max_offset {
                temp_env[scope_depth].resize(max_offset + 1, v_none());
            }

            // Create a ScatterArgs for just this scope
            let scope_scatter = moor_var::program::opcode::ScatterArgs {
                labels: labels.clone(),
                done: lambda.0.params.done,
            };

            // Perform parameter binding for this scope
            lambda_scatter_assign(&scope_scatter, &args, &mut temp_env[scope_depth])?;
        }

        // Handle self-reference for recursive lambdas
        if let Some(self_var_name) = lambda.0.self_var {
            let var_idx = self_var_name.0 as usize;

            // Create a deep copy of the lambda to avoid cycles
            let self_lambda = lambda.for_self_reference();
            // Environment already uses Var with v_none() as sentinel
            let lambda_var = moor_var::Var::mk_lambda(
                self_lambda.0.params.clone(),
                self_lambda.0.body.clone(),
                temp_env.clone(),
                self_lambda.0.self_var,
            );

            // Now set the self-reference in the environment
            if let Some(env) = temp_env.last_mut()
                && var_idx < env.len()
            {
                env[var_idx] = lambda_var;
            }
        }

        // Create frame with the built environment
        let moo_frame = MooStackFrame::with_environment(lambda.0.body.clone(), temp_env);
        let mut frame = Frame::Moo(moo_frame);

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

        Ok(Self {
            frame,
            this: current_activation.this.clone(),
            player: current_activation.player,
            verbdef: current_activation.verbdef.clone(),
            verb_name: Symbol::mk(&lambda_name),
            args: args.iter().cloned().collect(),
            permissions: current_activation.permissions,
        })
    }

    pub fn for_eval(permissions: Obj, player: &Obj, program: Program) -> Self {
        let verbdef = VerbDef::new(
            Uuid::new_v4(),
            NOTHING,
            NOTHING,
            &[*EVAL_SYMBOL],
            BitEnum::new_with(VerbFlag::Exec) | VerbFlag::Debug,
            VerbArgsSpec::this_none_this(),
        );

        let moo_frame = MooStackFrame::new_with_all_globals(
            program,
            v_obj(*player),        // player
            v_obj(NOTHING),        // this
            v_obj(*player),        // caller
            v_empty_str(),         // verb
            v_empty_list(), // args
            v_empty_str(),         // argstr
        );
        let frame = Frame::Moo(moo_frame);

        Self {
            frame,
            this: v_obj(*player),
            player: *player,
            verbdef,
            verb_name: *EVAL_SYMBOL,
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
            caller_perms_override: None,
        };
        let frame = Frame::Bf(bf_frame);
        Self {
            frame,
            this: v_obj(NOTHING),
            player,
            verbdef,
            verb_name: bf_name,
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
