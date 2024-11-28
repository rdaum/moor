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

use std::time::{Duration, SystemTime};

use bincode::{Decode, Encode};
use daumtils::PhantomUnsync;

use moor_values::Var;
use moor_values::NOTHING;
use moor_values::{Obj, Symbol};

use crate::vm::activation::{Activation, Frame};
use moor_values::tasks::TaskId;

// {this, verb-name, programmer, verb-loc, player, line-number}
#[derive(Clone)]
pub struct Caller {
    pub this: Obj,
    pub verb_name: Symbol,
    pub programmer: Obj,
    pub definer: Obj,
    pub player: Obj,
    pub line_number: usize,
}

/// Represents the state of VM execution.
/// The actual "VM" remains stateless and could be potentially re-used for multiple tasks,
/// and swapped out at each level of the activation stack for different runtimes.
/// e.g. a MOO VM, a WASM VM, a JS VM, etc. but all having access to the same shared state.
#[derive(Encode, Decode)]
pub struct VMExecState {
    /// The task ID of the task that for current stack of activations.
    pub(crate) task_id: TaskId,
    /// The stack of activation records / stack frames.
    /// (For language runtimes that keep their own stack, this is simply the "entry" point
    ///  for the function invocation.)
    pub(crate) stack: Vec<Activation>,
    /// The tick slice for the current execution.
    pub(crate) tick_slice: usize,
    /// The total number of ticks that the task is allowed to run.
    pub(crate) max_ticks: usize,
    /// The number of ticks that have been executed so far.
    pub(crate) tick_count: usize,
    /// The time at which the task was started.
    pub(crate) start_time: Option<SystemTime>,
    /// The amount of time the task is allowed to run.
    pub(crate) maximum_time: Option<Duration>,

    unsync: PhantomUnsync,
}

impl VMExecState {
    pub fn new(task_id: TaskId, max_ticks: usize) -> Self {
        Self {
            task_id,
            stack: vec![],
            tick_count: 0,
            start_time: None,
            max_ticks,
            tick_slice: 0,
            maximum_time: None,
            unsync: Default::default(),
        }
    }

    /// Return the callers stack, in the format expected by the `callers` built-in function.
    pub(crate) fn callers(&self) -> Vec<Caller> {
        let mut callers_iter = self.stack.iter().rev();
        callers_iter.next(); // skip the top activation, that's our current frame

        let mut callers = vec![];
        for activation in callers_iter {
            let verb_name = activation.verb_name;
            let definer = activation.verb_definer();
            let player = activation.player.clone();
            let line_number = activation.frame.find_line_no().unwrap_or(0);
            let this = activation.this.clone();
            let perms = activation.permissions.clone();
            let programmer = match activation.frame {
                Frame::Bf(_) => NOTHING,
                _ => perms,
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
    pub(crate) fn caller(&self) -> Obj {
        let stack_iter = self.stack.iter().rev();

        // Skip builtin-frames (for now?)
        for activation in stack_iter {
            if let Frame::Bf(_) = activation.frame {
                continue;
            }
            return activation.this.clone();
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
    pub(crate) fn caller_perms(&self) -> Obj {
        // Filter out builtin frames, and then take the next one.
        let mut stack_iter = self.stack.iter().rev().filter(|a| !a.is_builtin_frame());
        // caller is the frame just before us.
        stack_iter.next();
        stack_iter
            .next()
            .map(|a| a.permissions.clone())
            .unwrap_or(NOTHING)
    }

    /// Return the permissions of the current task, which is the "starting"
    /// permissions of the current task, but note that this can be modified by
    /// the `set_task_perms` built-in function.
    pub(crate) fn task_perms(&self) -> Obj {
        let stack_top = self.stack.iter().rev().find(|a| !a.is_builtin_frame());
        stack_top.map(|a| a.permissions.clone()).unwrap_or(NOTHING)
    }

    pub(crate) fn this(&self) -> Obj {
        let stack_top = self.stack.iter().rev().find(|a| !a.is_builtin_frame());
        stack_top.map(|a| a.this.clone()).unwrap_or(NOTHING)
    }

    /// Update the permissions of the current task, as called by the `set_task_perms`
    /// built-in.
    pub(crate) fn set_task_perms(&mut self, perms: Obj) {
        // Copy the stack perms up to the last non-builtin. That is, make sure builtin-frames
        // get the permissions update, and the first non-builtin, too.
        for activation in self.stack.iter_mut().rev() {
            activation.permissions = perms.clone();
            if !activation.is_builtin_frame() {
                break;
            }
        }
    }

    /// Push a value onto the value stack
    #[inline]
    pub(crate) fn set_return_value(&mut self, v: Var) {
        self.top_mut().frame.set_return_value(v);
    }

    pub(crate) fn time_left(&self) -> Option<Duration> {
        let max_time = self.maximum_time?;
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(self.start_time.expect("No start time for task?"))
            .unwrap();

        max_time.checked_sub(elapsed)
    }
}
