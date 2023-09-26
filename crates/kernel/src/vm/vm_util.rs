use moor_values::NOTHING;
use tracing::debug;

use moor_values::model::world_state::WorldState;
use moor_values::var::error::Error::{E_INVIND, E_TYPE};
use moor_values::var::objid::Objid;
use moor_values::var::variant::Variant;
use moor_values::var::Var;

use crate::compiler::labels::{Label, Name};
use crate::vm::activation::{Activation, Caller};
use crate::vm::opcode::Op;
use crate::vm::{ExecutionResult, VM};

impl VM {
    /// VM-level property resolution.
    pub(crate) async fn resolve_property(
        &mut self,
        state: &mut dyn WorldState,
        propname: Var,
        obj: Var,
    ) -> ExecutionResult {
        let Variant::Str(propname) = propname.variant() else {
            return self.push_error(E_TYPE);
        };

        let Variant::Obj(obj) = obj.variant() else {
            return self.push_error(E_INVIND);
        };

        let result = state
            .retrieve_property(self.top().permissions, *obj, propname.as_str())
            .await;
        let v = match result {
            Ok(v) => v,
            Err(e) => {
                debug!(obj = ?obj, propname = propname.as_str(), "Error resolving property");
                return self.push_error(e.to_error_code());
            }
        };
        self.push(&v);
        ExecutionResult::More
    }

    /// VM-level property assignment
    pub(crate) async fn set_property(
        &mut self,
        state: &mut dyn WorldState,
        propname: Var,
        obj: Var,
        value: Var,
    ) -> ExecutionResult {
        let (propname, obj) = match (propname.variant(), obj.variant()) {
            (Variant::Str(propname), Variant::Obj(obj)) => (propname, obj),
            (_, _) => {
                return self.push_error(E_TYPE);
            }
        };

        let update_result = state
            .update_property(self.top().permissions, *obj, propname.as_str(), &value)
            .await;

        match update_result {
            Ok(()) => {
                self.push(&value);
            }
            Err(e) => {
                return self.push_error(e.to_error_code());
            }
        }
        ExecutionResult::More
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

    pub(crate) fn top_mut(&mut self) -> &mut Activation {
        self.stack.last_mut().expect("activation stack underflow")
    }

    pub(crate) fn top(&self) -> &Activation {
        self.stack.last().expect("activation stack underflow")
    }

    pub(crate) fn caller_perms(&self) -> Objid {
        // Filter out builtins.
        let mut stack_iter = self.stack.iter().rev().filter(|a| a.bf_index.is_none());
        // caller is the frame just before us.
        stack_iter.next();
        stack_iter.next().map(|a| a.permissions).unwrap_or(NOTHING)
    }

    pub(crate) fn task_perms(&self) -> Objid {
        let stack_top = self.stack.iter().rev().find(|a| a.bf_index.is_none());
        stack_top.map(|a| a.permissions).unwrap_or(NOTHING)
    }

    pub(crate) fn set_task_perms(&mut self, perms: Objid) {
        self.top_mut().permissions = perms;
    }

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

    pub(crate) fn parent_activation_mut(&mut self) -> &mut Activation {
        let len = self.stack.len();
        self.stack
            .get_mut(len - 2)
            .expect("activation stack underflow")
    }

    pub(crate) fn pop(&mut self) -> Var {
        self.top_mut().pop().unwrap_or_else(|| {
            panic!(
                "stack underflow, activation depth: {} PC: {}",
                self.stack.len(),
                self.top().pc
            )
        })
    }

    pub(crate) fn push(&mut self, v: &Var) {
        self.top_mut().push(v.clone())
    }

    pub(crate) fn next_op(&mut self) -> Option<Op> {
        self.top_mut().next_op()
    }

    pub(crate) fn jump(&mut self, label: Label) {
        self.top_mut().jump(label)
    }

    pub(crate) fn get_env(&self, id: Name) -> &Option<Var> {
        &self.top().environment[id.0 as usize]
    }

    pub(crate) fn set_env(&mut self, id: Name, v: &Var) {
        self.top_mut().environment[id.0 as usize] = Some(v.clone());
    }

    pub(crate) fn peek(&self, amt: usize) -> Vec<Var> {
        self.top().peek(amt)
    }

    pub(crate) fn peek_top(&self) -> Var {
        self.top().peek_top().expect("stack underflow")
    }
}
