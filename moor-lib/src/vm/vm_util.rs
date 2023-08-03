use crate::compiler::labels::{Label, Name};

use crate::model::world_state::WorldState;
use crate::model::ObjectError::{PropertyNotFound, PropertyPermissionDenied};

use crate::values::error::Error::{E_INVIND, E_PERM, E_PROPNF, E_TYPE};
use crate::values::var::{v_none, Var};
use crate::values::variant::Variant;
use crate::vm::activation::Activation;
use crate::vm::opcode::Op;
use crate::vm::{ExecutionResult, VM};

impl VM {
    /// VM-level property resolution.
    pub(crate) async fn resolve_property(
        &mut self,
        state: &mut dyn WorldState,
        propname: Var,
        obj: Var,
    ) -> Result<ExecutionResult, anyhow::Error> {
        let Variant::Str(propname) = propname.variant() else {
            return self.push_error(E_TYPE);
        };

        let Variant::Obj(obj) = obj.variant() else {
            return self.push_error(E_INVIND);
        };

        let result = state
            .retrieve_property(self.top().permissions.clone(), *obj, propname.as_str())
            .await;
        let v = match result {
            Ok(v) => v,
            Err(e) => match e {
                PropertyPermissionDenied => return self.push_error(E_PERM),
                PropertyNotFound(_, _) => return self.push_error(E_PROPNF),
                _ => {
                    panic!("Unexpected error in property retrieval: {:?}", e);
                }
            },
        };
        self.push(&v);
        Ok(ExecutionResult::More)
    }

    /// VM-level property assignment
    pub(crate) async fn set_property(
        &mut self,
        state: &mut dyn WorldState,
        propname: Var,
        obj: Var,
        value: Var,
    ) -> Result<ExecutionResult, anyhow::Error> {
        let (propname, obj) = match (propname.variant(), obj.variant()) {
            (Variant::Str(propname), Variant::Obj(obj)) => (propname, obj),
            (_, _) => {
                return self.push_error(E_TYPE);
            }
        };

        let update_result = state
            .update_property(
                self.top().permissions.clone(),
                *obj,
                propname.as_str(),
                &value,
            )
            .await;

        match update_result {
            Ok(()) => {
                self.push(&v_none());
            }
            Err(e) => match e {
                PropertyNotFound(_, _) => {
                    return self.push_error(E_PROPNF);
                }
                PropertyPermissionDenied => {
                    return self.push_error(E_PERM);
                }
                _ => {
                    panic!("Unexpected error in property update: {:?}", e);
                }
            },
        }
        Ok(ExecutionResult::More)
    }

    pub(crate) fn top_mut(&mut self) -> &mut Activation {
        self.stack.last_mut().expect("activation stack underflow")
    }

    pub(crate) fn top(&self) -> &Activation {
        self.stack.last().expect("activation stack underflow")
    }

    pub(crate) fn pop(&mut self) -> Var {
        self.top_mut()
            .pop()
            .unwrap_or_else(|| panic!("stack underflow, activation depth: {}", self.stack.len()))
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

    pub(crate) fn get_env(&mut self, id: Name) -> Var {
        self.top().environment[id.0 as usize].clone()
    }

    pub(crate) fn set_env(&mut self, id: Name, v: &Var) {
        self.top_mut().environment[id.0 as usize] = v.clone();
    }

    pub(crate) fn peek(&self, amt: usize) -> Vec<Var> {
        self.top().peek(amt)
    }

    pub(crate) fn peek_top(&self) -> Var {
        self.top().peek_top().expect("stack underflow")
    }
}
