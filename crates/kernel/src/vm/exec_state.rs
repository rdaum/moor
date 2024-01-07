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

use crate::vm::activation::{Activation, Caller};
use moor_compiler::labels::{Label, Name};
use moor_compiler::opcode::Op;
use moor_values::var::objid::Objid;
use moor_values::var::Var;
use moor_values::NOTHING;
use std::time::SystemTime;

/// Represents the state of VM execution.
/// The actual "VM" remains stateless and could be potentially re-used for multiple tasks,
/// and swapped out at each level of the activation stack for different runtimes.
/// e.g. a MOO VM, a WASM VM, a JS VM, etc. but all having access to the same shared state.
pub struct VMExecState {
    /// The stack of activation records / stack frames.
    /// (For language runtimes that keep their own stack, this is simply the "entry" point
    ///  for the function invocation.)
    pub(crate) stack: Vec<Activation>,
    /// The number of ticks that have been executed so far.
    pub(crate) tick_count: usize,
    /// The time at which the task was started.
    pub(crate) start_time: Option<SystemTime>,
}

impl Default for VMExecState {
    fn default() -> Self {
        Self::new()
    }
}

impl VMExecState {
    pub fn new() -> Self {
        Self {
            stack: vec![],
            tick_count: 0,
            start_time: None,
        }
    }

    /// Return the callers stack, in the format expected by the `callers` built-in function.
    pub(crate) fn callers(&self) -> Vec<Caller> {
        let mut callers_iter = self.stack.iter().rev();
        callers_iter.next(); // skip the top activation, that's our current frame

        let mut callers = vec![];
        for activation in callers_iter {
            let verb_name = activation.verb_name.clone();
            let definer = activation.verb_definer();
            let player = activation.player;
            let line_number = 0; // TODO: fix after decompilation support
            let this = activation.this;
            let perms = activation.permissions;
            let programmer = if activation.bf_index.is_some() {
                NOTHING
            } else {
                perms
            };
            callers.push(Caller {
                verb_name,
                definer,
                player,
                line_number,
                this,
                programmer,
            });
        }
        callers
    }

    #[inline]
    pub(crate) fn top_mut(&mut self) -> &mut Activation {
        self.stack.last_mut().expect("activation stack underflow")
    }

    #[inline]
    pub(crate) fn top(&self) -> &Activation {
        self.stack.last().expect("activation stack underflow")
    }

    /// Return the object that called the current activation.
    pub(crate) fn caller(&self) -> Objid {
        let stack_iter = self.stack.iter().rev();
        for activation in stack_iter {
            if activation.bf_index.is_some() {
                continue;
            }
            return activation.this;
        }
        NOTHING
    }

    /// Return the activation record of the caller of the current activation.
    pub(crate) fn parent_activation_mut(&mut self) -> &mut Activation {
        let len = self.stack.len();
        self.stack
            .get_mut(len - 2)
            .expect("activation stack underflow")
    }

    /// Return the permissions of the caller of the current activation.
    pub(crate) fn caller_perms(&self) -> Objid {
        // Filter out builtins.
        let mut stack_iter = self.stack.iter().rev().filter(|a| a.bf_index.is_none());
        // caller is the frame just before us.
        stack_iter.next();
        stack_iter.next().map(|a| a.permissions).unwrap_or(NOTHING)
    }

    /// Return the permissions of the current task, which is the "starting"
    /// permissions of the current task, but note that this can be modified by
    /// the `set_task_perms` built-in function.
    pub(crate) fn task_perms(&self) -> Objid {
        let stack_top = self.stack.iter().rev().find(|a| a.bf_index.is_none());
        stack_top.map(|a| a.permissions).unwrap_or(NOTHING)
    }

    /// Update the permissions of the current task, as called by the `set_task_perms`
    /// built-in.
    pub(crate) fn set_task_perms(&mut self, perms: Objid) {
        self.top_mut().permissions = perms;
    }

    /// Pop a value off the value stack.
    #[inline]
    pub(crate) fn pop(&mut self) -> Var {
        self.top_mut().pop().unwrap_or_else(|| {
            panic!(
                "stack underflow, activation depth: {} PC: {}",
                self.stack.len(),
                self.top().pc
            )
        })
    }

    /// Push a value onto the value stack
    #[inline]
    pub(crate) fn push(&mut self, v: &Var) {
        self.top_mut().push(v.clone())
    }

    /// Non-destructively peek a range of values from the value stack.
    /// Returns top N values, where N is the given amount.
    #[inline]
    pub(crate) fn peek_range(&self, amt: usize) -> Vec<Var> {
        self.top().peek_range(amt)
    }

    /// Non-destructively return a value at a specific absolute offset in the value stack.
    /// Not computed relative to the top of the stack, used by the Length opcode, which
    /// has specific knowledge of the stack layout.
    #[inline]
    pub(crate) fn peek_abs(&self, amt: usize) -> &Var {
        self.top().peek_abs(amt)
    }

    /// Return the top two values on the value stack.
    #[inline]
    pub(crate) fn peek2(&self) -> (Var, Var) {
        self.top().peek2()
    }

    /// Update at a set offset in the value stack.
    #[inline]
    pub(crate) fn update(&mut self, amt: usize, v: &Var) {
        self.top_mut().update(amt, v.clone())
    }

    /// Return the top of the value stack.
    #[inline]
    pub(crate) fn peek_top(&self) -> Var {
        self.top().peek_top().expect("stack underflow")
    }

    /// Return the next opcode in the program stream.
    #[inline]
    pub(crate) fn next_op(&mut self) -> Op {
        self.top_mut().next_op()
    }

    /// Jump to the given label.
    #[inline]
    pub(crate) fn jump(&mut self, label: Label) {
        self.top_mut().jump(label)
    }

    /// Return the value of a local variable.
    #[inline]
    pub(crate) fn get_env(&self, id: Name) -> Option<&Var> {
        self.top().environment.get(id.0 as usize)
    }

    /// Set the value of a local variable.
    #[inline]
    pub(crate) fn set_env(&mut self, id: Name, v: &Var) {
        self.top_mut().environment.set(id.0 as usize, v.clone());
    }
}
