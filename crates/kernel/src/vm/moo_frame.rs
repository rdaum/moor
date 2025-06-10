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
use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use moor_common::program::labels::Offset;
use moor_common::program::names::{GlobalName, Name};
use moor_compiler::{Label, Op, Program};
use moor_var::VarType::TYPE_NONE;
use moor_var::{Error, Var, v_none};
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
    pub(crate) environment: Vec<Vec<Var>>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Encode, Decode)]
pub enum PcType {
    Main,
    ForkVector(Offset),
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub enum CatchType {
    Any,
    Errors(Vec<Error>),
}

/// The kinds of block scopes that can be entered and exited, which far now are just catch and
/// finally blocks.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
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
    Block,
    Comprehension,
}

/// A scope is a record of the current size of the valstack when it was created, and are
/// enter and exit scopes.
/// On entry, the current size of the valstack is stored in `valstack_pos`.
/// On exit, the valstack is eaten back to that size.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub(crate) struct Scope {
    pub(crate) scope_type: ScopeType,
    pub(crate) valstack_pos: usize,
    pub(crate) end_pos: usize,
    /// True if this scope has a variable environment.
    pub(crate) environment: bool,
}

impl MooStackFrame {
    pub(crate) fn new(program: Program) -> Self {
        let width = max(program.var_names().global_width(), GlobalName::COUNT);
        let mut first_env = Vec::with_capacity(width);
        first_env.resize(width, v_none());
        let mut environment = Vec::with_capacity(16);
        environment.push(first_env);
        Self {
            program,
            environment,
            pc: 0,
            pc_type: PcType::Main,
            temp: v_none(),
            valstack: Default::default(),
            scope_stack: Default::default(),
            catch_stack: Default::default(),
            finally_stack: Default::default(),
        }
    }

    pub(crate) fn opcodes(&self) -> &[Op] {
        match self.pc_type {
            PcType::Main => self.program.main_vector(),
            PcType::ForkVector(fork_vector) => self.program.fork_vector(fork_vector),
        }
    }

    pub(crate) fn find_line_no(&self, pc: usize) -> Option<usize> {
        match self.pc_type {
            PcType::Main => Some(self.program.line_num_for_position(pc, 0)),
            PcType::ForkVector(fv) => Some(self.program.fork_line_num_for_position(fv, pc)),
        }
    }

    #[inline]
    pub fn set_gvar(&mut self, gname: GlobalName, value: Var) {
        let pos = gname as usize;
        self.environment[0][pos] = value;
    }

    #[inline]
    pub fn set_variable(&mut self, id: &Name, v: Var) {
        // This is a "trust us we know what we're doing" use of the explicit offset without check
        // into the names list like we did before. If the compiler produces garbage, it gets what
        // it deserves.
        assert_ne!(
            v.type_code(),
            TYPE_NONE,
            "Setting variable {:?} to TYPE_NONE",
            self.program.var_names().name_of(id)
        );
        let offset = id.0 as usize;
        let scope = id.1 as usize;
        self.environment[scope][offset] = v;
    }

    /// Return the value of a local variable.
    #[inline]
    pub(crate) fn get_env(&self, id: &Name) -> Option<&Var> {
        let v = &self.environment[id.1 as usize][id.0 as usize];
        if v.type_code() == TYPE_NONE {
            return None;
        }
        Some(v)
    }

    pub(crate) fn switch_to_fork_vector(&mut self, fork_vector: Offset) {
        self.pc_type = PcType::ForkVector(fork_vector);
        self.pc = 0;
    }

    #[inline]
    pub fn lookahead(&self) -> Option<Op> {
        match self.pc_type {
            PcType::Main => self.program.main_vector().get(self.pc).cloned(),
            PcType::ForkVector(fork_vector) => {
                self.program.fork_vector(fork_vector).get(self.pc).cloned()
            }
        }
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
        let label = &self.program.jump_label(*label_id);

        self.pc = label.position.0 as usize;
        // Pop all scopes until we find one whose end_pos is > our jump point
        while let Some(scope) = self.scope_stack.last() {
            if scope.end_pos > self.pc {
                break;
            }
            self.pop_scope();
        }
    }

    /// Enter a new lexical scope and/or try/catch handling block.
    pub fn push_scope(&mut self, scope: ScopeType, scope_width: u16, end_label: &Label) {
        let end_pos = self.program.jump_label(*end_label).position.0 as usize;
        self.scope_stack.push(Scope {
            scope_type: scope,
            valstack_pos: self.valstack.len(),
            end_pos,
            environment: true,
        });
        let mut new_scope = Vec::with_capacity(scope_width as usize);
        new_scope.resize(scope_width as usize, v_none());
        self.environment.push(new_scope);
    }

    /// Enter a scope which does not restrict stack of environment size, purely for catch expressions
    /// The scope is just used for unwinding to the catch handler purposes.
    pub fn push_non_var_scope(&mut self, scope: ScopeType, end_label: &Label) {
        let end_pos = self.program.jump_label(*end_label).position.0 as usize;
        self.scope_stack.push(Scope {
            scope_type: scope,
            valstack_pos: self.valstack.len(),
            end_pos,
            environment: false,
        });
    }

    pub fn pop_scope(&mut self) -> Option<Scope> {
        let scope = self.scope_stack.pop()?;
        if scope.environment {
            self.environment.pop();
        }
        self.valstack.truncate(scope.valstack_pos);
        Some(scope)
    }
}

impl Encode for MooStackFrame {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.program.encode(encoder)?;
        self.pc.encode(encoder)?;
        self.pc_type.encode(encoder)?;
        self.environment.encode(encoder)?;
        self.valstack.encode(encoder)?;
        self.scope_stack.encode(encoder)?;
        self.temp.encode(encoder)?;
        self.catch_stack.encode(encoder)?;
        self.finally_stack.encode(encoder)
    }
}

impl<C> Decode<C> for MooStackFrame {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let program = Program::decode(decoder)?;
        let pc = usize::decode(decoder)?;
        let pc_type = PcType::decode(decoder)?;
        let environment = Vec::decode(decoder)?;
        let valstack = Vec::decode(decoder)?;
        let scope_stack = Vec::decode(decoder)?;
        let temp = Var::decode(decoder)?;
        let catch_stack = Vec::decode(decoder)?;
        let finally_stack = Vec::decode(decoder)?;
        Ok(Self {
            program,
            pc,
            pc_type,
            environment,
            valstack,
            scope_stack,
            temp,
            catch_stack,
            finally_stack,
        })
    }
}

impl<'de, C> BorrowDecode<'de, C> for MooStackFrame {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let program = Program::borrow_decode(decoder)?;
        let pc = usize::borrow_decode(decoder)?;
        let pc_type = PcType::borrow_decode(decoder)?;
        let environment = Vec::borrow_decode(decoder)?;
        let valstack = Vec::borrow_decode(decoder)?;
        let scope_stack = Vec::borrow_decode(decoder)?;
        let temp = Var::borrow_decode(decoder)?;
        let catch_stack = Vec::borrow_decode(decoder)?;
        let finally_stack = Vec::borrow_decode(decoder)?;
        Ok(Self {
            program,
            pc,
            pc_type,
            environment,
            valstack,
            scope_stack,
            temp,
            catch_stack,
            finally_stack,
        })
    }
}
