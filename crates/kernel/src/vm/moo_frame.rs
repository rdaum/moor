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
use daumtils::{BitArray, Bitset16};

use crate::vm::FinallyReason;
use moor_compiler::Name;
use moor_compiler::{GlobalName, Label, Op, Program};
use moor_values::var::Error::E_VARNF;
use moor_values::var::{v_none, Error, Var};

/// The MOO stack-frame specific portions of the activation:
///   the value stack, local variables, program, program counter, handler stack, etc.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MooStackFrame {
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
}

/// A scope is a record of the current size of the valstack when it was created, and are
/// enter and exit scopes.
/// On entry, the current size of the valstack is stored in `valstack_pos`.
/// On exit, the valstack is eaten back to that size.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub(crate) struct Scope {
    pub(crate) scope_type: ScopeType,
    pub(crate) valstack_pos: usize,
}

impl Encode for MooStackFrame {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.program.encode(encoder)?;
        self.pc.encode(encoder)?;

        // Environment is custom, is not bincodable, so we need to encode it manually, but we just
        // do it as an array of Option<Var>
        let mut env = vec![None; self.environment.len()];
        let env_iter = self.environment.iter();
        for (i, v) in env_iter {
            env[i] = Some(v.clone())
        }
        env.encode(encoder)?;
        self.valstack.encode(encoder)?;
        self.scope_stack.encode(encoder)?;
        self.temp.encode(encoder)?;
        self.catch_stack.encode(encoder)?;
        self.finally_stack.encode(encoder)
    }
}

impl Decode for MooStackFrame {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let program = Program::decode(decoder)?;
        let pc = usize::decode(decoder)?;

        let env: Vec<Option<Var>> = Vec::decode(decoder)?;
        let mut environment = BitArray::new();
        for (i, v) in env.iter().enumerate() {
            if let Some(v) = v {
                environment.set(i, v.clone());
            }
        }

        let valstack = Vec::decode(decoder)?;
        let scope_stack = Vec::decode(decoder)?;
        let temp = Var::decode(decoder)?;
        let catch_stack = Vec::decode(decoder)?;
        let finally_stack = Vec::decode(decoder)?;
        Ok(Self {
            program,
            pc,
            environment,
            valstack,
            scope_stack,
            temp,
            catch_stack,
            finally_stack,
        })
    }
}

impl<'de> BorrowDecode<'de> for MooStackFrame {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let program = Program::borrow_decode(decoder)?;
        let pc = usize::borrow_decode(decoder)?;

        let env: Vec<Option<Var>> = Vec::borrow_decode(decoder)?;
        let mut environment = BitArray::new();
        for (i, v) in env.iter().enumerate() {
            if let Some(v) = v {
                environment.set(i, v.clone());
            }
        }

        let valstack = Vec::borrow_decode(decoder)?;
        let scope_stack = Vec::borrow_decode(decoder)?;
        let temp = Var::borrow_decode(decoder)?;
        let catch_stack = Vec::borrow_decode(decoder)?;
        let finally_stack = Vec::borrow_decode(decoder)?;
        Ok(Self {
            program,
            pc,
            environment,
            valstack,
            scope_stack,
            temp,
            catch_stack,
            finally_stack,
        })
    }
}

impl MooStackFrame {
    pub(crate) fn new(program: Program) -> Self {
        let environment = BitArray::new();

        Self {
            program,
            environment,
            valstack: vec![],
            scope_stack: vec![],
            pc: 0,
            temp: v_none(),
            catch_stack: vec![],
            finally_stack: vec![],
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

    pub fn enter_scope(&mut self, scope: ScopeType) {
        self.scope_stack.push(Scope {
            scope_type: scope,
            valstack_pos: self.valstack.len(),
        });
    }

    pub fn pop_scope(&mut self) -> Option<Scope> {
        let Some(scope) = self.scope_stack.pop() else {
            return None;
        };

        self.valstack.truncate(scope.valstack_pos);
        Some(scope)
    }
}
