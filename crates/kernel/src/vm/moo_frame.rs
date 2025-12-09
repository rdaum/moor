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

use crate::vm::FinallyReason;
use crate::vm::environment::Environment;
use moor_compiler::{Label, Op, Program};
use moor_var::{
    Error, NOTHING, Var,
    VarType::TYPE_NONE,
    program::{
        labels::Offset,
        names::{GlobalName, Name},
    },
    v_none, v_obj, v_str, v_string,
};
use std::cmp::max;
use strum::EnumCount;

/// The MOO stack-frame specific portions of the activation:
///   the value stack, local variables, program, program counter, handler stack, etc.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MooStackFrame {
    /// The program of the verb that is currently being executed.
    pub(crate) program: Program,
    /// The program counter.
    pub(crate) pc: usize,
    /// Where is the PC pointing to?
    pub(crate) pc_type: PcType,
    /// The values of the variables currently in scope, by their offset.
    pub(crate) environment: Environment,
    /// The value stack.
    pub(crate) valstack: Vec<Var>,
    /// A stack of active scopes. Used for catch and finally blocks and in the future for lexical
    /// scoping as well.
    pub(crate) scope_stack: Vec<Scope>,
    /// Scratch space for PushTemp and PutTemp opcodes.
    pub(crate) temp: Var,
    /// Scratch space for constructing the catch handlers for a forthcoming try scope.
    pub(crate) catch_stack: Vec<(CatchType, Label)>,
    /// Scratch space for holding finally-reasons to be popped off the stack when a finally block
    /// is ended.
    pub(crate) finally_stack: Vec<FinallyReason>,
    /// Stack for captured variables during lambda creation
    pub(crate) capture_stack: Vec<(Name, Var)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PcType {
    Main,
    ForkVector(Offset),
    Lambda(Offset),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatchType {
    Any,
    Errors(Vec<Error>),
}

/// The kinds of block scopes that can be entered and exited, which far now are just catch and
/// finally blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScopeType {
    /// A scope that attempts to execute a block of code, and then executes the block of code at
    /// "Label" regardless of whether the block of code succeeded or failed.
    /// Note that `return` and `exit` are not considered failures.
    TryFinally(Label),
    TryCatch(Vec<(CatchType, Label)>),
    If,
    Eif,
    While,
    For,
    /// For-sequence iteration state stored in scope instead of on stack
    /// For sequences: current_index tracks position, current_key is None
    /// For maps: current_key tracks the current key for efficient iteration
    ForSequence {
        sequence: moor_var::Var,
        current_index: usize,
        current_key: Option<moor_var::Var>,
        value_bind: moor_var::program::names::Name,
        key_bind: Option<moor_var::program::names::Name>,
        end_label: moor_compiler::Label,
    },
    /// For-range iteration state stored in scope instead of on stack
    ForRange {
        current_value: Var,
        end_value: Var,
        loop_variable: moor_var::program::names::Name,
        end_label: moor_compiler::Label,
    },
    Block,
    Comprehension,
}

/// A scope is a record of the current size of the valstack when it was created, and are
/// enter and exit scopes.
/// On entry, the current size of the valstack is stored in `valstack_pos`.
/// On exit, the valstack is eaten back to that size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scope {
    pub(crate) scope_type: ScopeType,
    pub(crate) valstack_pos: usize,
    pub(crate) start_pos: usize,
    pub(crate) end_pos: usize,
    /// True if this scope has a variable environment.
    pub(crate) environment: bool,
}

impl MooStackFrame {
    /// Create a builder for constructing a new MOO stack frame.
    /// This is the preferred way to create frames as it provides type-safe initialization.
    pub(crate) fn builder(program: Program) -> MooStackFrameBuilder {
        MooStackFrameBuilder::new(program)
    }

    /// Create a new MOO stack frame.
    /// Consider using `builder()` instead for safer initialization.
    #[allow(dead_code)]
    pub(crate) fn new(program: Program) -> Self {
        let width = max(program.var_names().global_width(), GlobalName::COUNT);

        // Create environment
        let mut environment = Environment::new();
        environment.push_scope(width);

        let valstack = Vec::with_capacity(16);
        let scope_stack = Vec::with_capacity(8);
        Self {
            program,
            environment,
            pc: 0,
            pc_type: PcType::Main,
            temp: v_none(),
            valstack,
            scope_stack,
            catch_stack: Default::default(),
            finally_stack: Default::default(),
            capture_stack: Default::default(),
        }
    }

    /// Create a new frame with a pre-built environment (for lambdas)
    pub(crate) fn with_environment(program: Program, environment: Vec<Vec<Option<Var>>>) -> Self {
        let mut env = Environment::new();

        // Ensure global scope exists with proper width for global variables
        let global_width = max(program.var_names().global_width(), GlobalName::COUNT);
        if environment.is_empty() {
            // No captured environment - just create global scope
            env.push_scope(global_width);
        } else {
            // Merge captured environment, ensuring scope 0 has enough room for globals
            for (scope_idx, scope) in environment.into_iter().enumerate() {
                let width = if scope_idx == 0 {
                    max(scope.len(), global_width)
                } else {
                    scope.len()
                };
                env.push_scope(width);
                for (i, var) in scope.into_iter().enumerate() {
                    if let Some(v) = var {
                        env.set(env.len() - 1, i, v);
                    }
                }
            }
        }

        let valstack = Vec::with_capacity(16);
        let scope_stack = Vec::with_capacity(8);
        Self {
            program,
            environment: env,
            pc: 0,
            pc_type: PcType::Main,
            temp: v_none(),
            valstack,
            scope_stack,
            catch_stack: Default::default(),
            finally_stack: Default::default(),
            capture_stack: Default::default(),
        }
    }

    pub(crate) fn opcodes(&self) -> &[Op] {
        match self.pc_type {
            PcType::Main => self.program.main_vector(),
            PcType::ForkVector(fork_vector) => self.program.fork_vector(fork_vector),
            PcType::Lambda(lambda_offset) => {
                self.program.lambda_program(lambda_offset).main_vector()
            }
        }
    }

    pub(crate) fn find_line_no(&self, pc: usize) -> Option<usize> {
        match self.pc_type {
            PcType::Main => Some(self.program.line_num_for_position(pc, 0)),
            PcType::ForkVector(fv) => Some(self.program.fork_line_num_for_position(fv, pc)),
            PcType::Lambda(lambda_offset) => {
                // For lambdas, use the lambda program's own line number spans
                let lambda_program = self.program.lambda_program(lambda_offset);
                Some(lambda_program.line_num_for_position(pc, 0))
            }
        }
    }

    pub fn set_gvar(&mut self, gname: GlobalName, value: Var) {
        let pos = gname as usize;
        self.environment.set(0, pos, value);
    }

    pub fn set_variable(&mut self, id: &Name, v: Var) {
        // This is a "trust us we know what we're doing" use of the explicit offset without check
        // into the names list like we did before. If the compiler produces garbage, it gets what
        // it deserves.
        debug_assert_ne!(
            v.type_code(),
            TYPE_NONE,
            "Setting variable {:?} to TYPE_NONE",
            self.program.var_names().ident_for_name(id)
        );
        let offset = id.0 as usize;
        let scope = id.1 as usize;
        self.environment.set(scope, offset, v);
    }

    /// Return the value of a local variable.
    pub(crate) fn get_env(&self, id: &Name) -> Option<&Var> {
        let scope_idx = id.1 as usize;
        let var_idx = id.0 as usize;

        self.environment.get(scope_idx, var_idx)
    }

    pub(crate) fn switch_to_fork_vector(&mut self, fork_vector: Offset) {
        self.pc_type = PcType::ForkVector(fork_vector);
        self.pc = 0;
    }

    pub fn lookahead(&self) -> Option<Op> {
        match self.pc_type {
            PcType::Main => self.program.main_vector().get(self.pc).cloned(),
            PcType::ForkVector(fork_vector) => {
                self.program.fork_vector(fork_vector).get(self.pc).cloned()
            }
            PcType::Lambda(lambda_offset) => self
                .program
                .lambda_program(lambda_offset)
                .main_vector()
                .get(self.pc)
                .cloned(),
        }
    }

    pub fn skip(&mut self) {
        self.pc += 1;
    }

    pub fn pop(&mut self) -> Var {
        self.valstack
            .pop()
            .unwrap_or_else(|| panic!("stack underflow @ PC: {}", self.pc))
    }

    pub fn push(&mut self, v: Var) {
        self.valstack.push(v)
    }

    pub fn peek_top(&self) -> &Var {
        self.valstack.last().expect("stack underflow")
    }

    pub fn peek_top_mut(&mut self) -> &mut Var {
        self.valstack.last_mut().expect("stack underflow")
    }

    pub fn peek_range(&self, width: usize) -> Vec<Var> {
        let l = self.valstack.len();
        Vec::from(&self.valstack[l - width..])
    }

    pub(crate) fn peek_abs(&self, amt: usize) -> &Var {
        &self.valstack[amt]
    }

    pub fn peek2(&self) -> (&Var, &Var) {
        let l = self.valstack.len();
        let (a, b) = (&self.valstack[l - 1], &self.valstack[l - 2]);
        (a, b)
    }

    pub fn poke(&mut self, amt: usize, v: Var) {
        let l = self.valstack.len();
        self.valstack[l - amt - 1] = v;
    }

    pub fn jump(&mut self, label_id: &Label) {
        let label = &self.program.jump_label(*label_id);

        self.pc = label.position.0 as usize;
        // Pop all scopes that the jump target is outside of
        while let Some(scope) = self.scope_stack.last() {
            // If jump target is within the scope range, keep the scope
            if self.pc >= scope.start_pos && self.pc < scope.end_pos {
                break;
            }

            // Jump target is outside scope range - pop it
            self.pop_scope();
        }
    }

    /// Enter a new lexical scope and/or try/catch handling block.
    pub fn push_scope(&mut self, scope: ScopeType, scope_width: u16, end_label: &Label) {
        let end_pos = self.program.jump_label(*end_label).position.0 as usize;
        let start_pos = self.pc;
        self.scope_stack.push(Scope {
            scope_type: scope,
            valstack_pos: self.valstack.len(),
            start_pos,
            end_pos,
            environment: true,
        });
        self.environment.push_scope(scope_width as usize);
    }

    /// Enter a scope which does not restrict stack of environment size, purely for catch expressions
    /// The scope is just used for unwinding to the catch handler purposes.
    pub fn push_non_var_scope(&mut self, scope: ScopeType, end_label: &Label) {
        let end_pos = self.program.jump_label(*end_label).position.0 as usize;
        let start_pos = self.pc;
        self.scope_stack.push(Scope {
            scope_type: scope,
            valstack_pos: self.valstack.len(),
            start_pos,
            end_pos,
            environment: false,
        });
    }

    pub fn pop_scope(&mut self) -> Option<Scope> {
        let scope = self.scope_stack.pop()?;
        if scope.environment {
            self.environment.pop_scope();
        }
        self.valstack.truncate(scope.valstack_pos);
        Some(scope)
    }

    /// Enter a ForSequence scope that holds iteration state
    pub fn push_for_sequence_scope(
        &mut self,
        sequence: moor_var::Var,
        value_bind: moor_var::program::names::Name,
        key_bind: Option<moor_var::program::names::Name>,
        end_label: &moor_compiler::Label,
        environment_width: u16,
    ) {
        let end_pos = self.program.jump_label(*end_label).position.0 as usize;
        let start_pos = self.pc;
        let scope_type = ScopeType::ForSequence {
            sequence,
            current_index: 0,
            current_key: None,
            value_bind,
            key_bind,
            end_label: *end_label,
        };
        self.scope_stack.push(Scope {
            scope_type,
            valstack_pos: self.valstack.len(),
            start_pos,
            end_pos,
            environment: true,
        });
        self.environment.push_scope(environment_width as usize);
    }

    /// Get the current ForSequence scope for iteration
    pub fn get_for_sequence_scope_mut(&mut self) -> Option<&mut ScopeType> {
        // Scan upwards through scope stack to find ForSequence scope
        for scope in self.scope_stack.iter_mut().rev() {
            if matches!(scope.scope_type, ScopeType::ForSequence { .. }) {
                return Some(&mut scope.scope_type);
            }
        }
        None
    }

    /// Enter a ForRange scope that holds iteration state
    pub fn push_for_range_scope(
        &mut self,
        start_value: &Var,
        end_value: &Var,
        loop_variable: moor_var::program::names::Name,
        end_label: &moor_compiler::Label,
        environment_width: u16,
    ) {
        let end_pos = self.program.jump_label(*end_label).position.0 as usize;
        let start_pos = self.pc;
        let scope_type = ScopeType::ForRange {
            current_value: start_value.clone(),
            end_value: end_value.clone(),
            loop_variable,
            end_label: *end_label,
        };
        self.scope_stack.push(Scope {
            scope_type,
            valstack_pos: self.valstack.len(),
            start_pos,
            end_pos,
            environment: true,
        });
        self.environment.push_scope(environment_width as usize);
    }

    /// Get the current ForRange scope for iteration
    pub fn get_for_range_scope_mut(&mut self) -> Option<&mut ScopeType> {
        // Scan upwards through scope stack to find ForRange scope
        for scope in self.scope_stack.iter_mut().rev() {
            if matches!(scope.scope_type, ScopeType::ForRange { .. }) {
                return Some(&mut scope.scope_type);
            }
        }
        None
    }
}

/// Builder for constructing MooStackFrame with safe, ergonomic initialization.
/// Ensures global variables are initialized during construction, avoiding the window
/// where slots contain uninitialized None values.
pub(crate) struct MooStackFrameBuilder {
    program: Program,
    environment: Environment,
    valstack: Vec<Var>,
    scope_stack: Vec<Scope>,
}

impl MooStackFrameBuilder {
    /// Create a new builder with an allocated scope.
    pub(crate) fn new(program: Program) -> Self {
        let width = max(program.var_names().global_width(), GlobalName::COUNT);

        // Create environment
        let mut environment = Environment::new();
        environment.push_scope(width);

        Self {
            program,
            environment,
            valstack: Vec::with_capacity(16),
            scope_stack: Vec::with_capacity(8),
        }
    }

    /// Initialize a global variable by moving the value directly into the environment.
    /// This avoids copying and ensures the slot is initialized exactly once.
    pub(crate) fn with_global(mut self, gname: GlobalName, value: Var) -> Self {
        let pos = gname as usize;
        self.environment.set(0, pos, value);
        self
    }

    /// Bulk initialize the core globals (player, this, caller, verb, args).
    /// Order matches GlobalName enum: player=0, this=1, caller=2, verb=3, args=4.
    #[inline]
    pub(crate) fn with_core_globals(
        mut self,
        this: Var,
        player: Var,
        caller: Var,
        verb: Var,
        args: Var,
    ) -> Self {
        self.environment.set_range(
            0,
            GlobalName::player as usize,
            [player, this, caller, verb, args],
        );
        self
    }

    /// Bulk initialize all parsing-related globals (argstr, dobj, dobjstr, prepstr, iobj, iobjstr).
    /// Uses a single slice copy when inheriting from a source frame.
    #[inline]
    pub(crate) fn with_parsing_globals(
        mut self,
        current_activation: Option<&crate::vm::activation::Activation>,
        argstr: String,
    ) -> Self {
        use crate::vm::activation::Frame;

        // Check once if we have a Moo frame to inherit from
        let source_frame = current_activation.and_then(|a| match &a.frame {
            Frame::Moo(frame) => Some(frame),
            Frame::Bf(_) => None,
        });

        if let Some(frame) = source_frame {
            // Bulk copy parsing globals (indices 5-10) in one slice operation
            self.environment.copy_range_from(
                &frame.environment,
                0,
                GlobalName::argstr as usize,
                GlobalName::iobjstr as usize,
            );
        } else {
            // No source frame - set all defaults with a single range operation
            // Order matches GlobalName enum: argstr=5, dobj=6, dobjstr=7, prepstr=8, iobj=9, iobjstr=10
            self.environment.set_range(
                0,
                GlobalName::argstr as usize,
                [
                    v_string(argstr),
                    v_obj(NOTHING),
                    v_str(""),
                    v_str(""),
                    v_obj(NOTHING),
                    v_str(""),
                ],
            );
        }
        self
    }

    /// Consume the builder and produce the final MooStackFrame.
    pub(crate) fn build(self) -> MooStackFrame {
        MooStackFrame {
            program: self.program,
            environment: self.environment,
            pc: 0,
            pc_type: PcType::Main,
            temp: v_none(),
            valstack: self.valstack,
            scope_stack: self.scope_stack,
            catch_stack: Default::default(),
            finally_stack: Default::default(),
            capture_stack: Default::default(),
        }
    }
}
