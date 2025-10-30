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
use std::sync::Arc;
use thiserror::Error;

use crate::vm::builtins::bf_obj_load::register_bf_obj_load;
use crate::{
    config::FeaturesConfig,
    task_context::with_current_transaction,
    vm::{
        activation::{BfFrame, Frame},
        builtins::{
            bf_age_crypto::register_bf_age_crypto,
            bf_documents::register_bf_documents,
            bf_flyweights::register_bf_flyweights,
            bf_list_sets::register_bf_list_sets,
            bf_maps::register_bf_maps,
            bf_num::register_bf_num,
            bf_objects::register_bf_objects,
            bf_properties::register_bf_properties,
            bf_server::{bf_noop, register_bf_server},
            bf_strings::register_bf_strings,
            bf_values::register_bf_values,
            bf_verbs::register_bf_verbs,
        },
        exec_state::VMExecState,
        vm_host::ExecutionResult,
    },
};
use moor_common::{
    model::{Perms, WorldStateError},
    util::PerfCounter,
};
use moor_compiler::{BUILTINS, BuiltinId, DiagnosticRenderOptions, DiagnosticVerbosity};
use moor_var::{
    E_INVARG, E_TYPE, Error, ErrorCode, List, Map, Obj, Sequence, Symbol, Var, Variant, v_bool_int,
    v_map,
};

mod bf_age_crypto;
mod bf_documents;
mod bf_flyweights;
mod bf_list_sets;
mod bf_maps;
mod bf_num;
mod bf_obj_load;
mod bf_objects;
mod bf_properties;
pub mod bf_server;
mod bf_strings;
mod bf_values;
mod bf_verbs;
mod docs;

#[cfg(test)]
#[path = "test_function_help.rs"]
mod test_function_help;

lazy_static! {
    static ref BF_COUNTERS: BfCounters = BfCounters::new();
}

thread_local! {
    static BF_COUNTERS_TLS: &'static BfCounters = &BF_COUNTERS;
}

pub struct BfCounters(Vec<PerfCounter>);

impl Default for BfCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl BfCounters {
    pub fn new() -> Self {
        let mut counters = Vec::with_capacity(BUILTINS.number_of());
        for b in BUILTINS.names.values() {
            counters.push(PerfCounter::new(*b));
        }
        Self(counters)
    }

    pub fn counter_for(&self, id: BuiltinId) -> &PerfCounter {
        &self.0[id.0 as usize]
    }

    pub fn all_counters(&self) -> Vec<&PerfCounter> {
        self.0.iter().collect()
    }
}

pub fn bf_perf_counters() -> &'static BfCounters {
    BF_COUNTERS_TLS.with(|c| *c)
}

/// The bundle of builtins are stored here, and passed around globally.
#[derive(Clone)]
pub struct BuiltinRegistry {
    // The set of built-in functions, indexed by their Name offset in the variable stack.
    pub(crate) builtins: Arc<Vec<Box<BuiltinFunction>>>,
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinRegistry {
    pub fn new() -> Self {
        let mut builtins: Vec<Box<BuiltinFunction>> = Vec::with_capacity(BUILTINS.number_of());
        for _ in 0..BUILTINS.number_of() {
            builtins.push(Box::new(bf_noop))
        }
        register_bf_server(&mut builtins);
        register_bf_num(&mut builtins);
        register_bf_values(&mut builtins);
        register_bf_strings(&mut builtins);
        register_bf_list_sets(&mut builtins);
        register_bf_maps(&mut builtins);
        register_bf_objects(&mut builtins);
        register_bf_obj_load(&mut builtins);
        register_bf_verbs(&mut builtins);
        register_bf_properties(&mut builtins);
        register_bf_flyweights(&mut builtins);
        register_bf_documents(&mut builtins);
        register_bf_age_crypto(builtins.as_mut());

        BuiltinRegistry {
            builtins: Arc::new(builtins),
        }
    }

    pub(crate) fn builtin_for(&self, id: &BuiltinId) -> &BuiltinFunction {
        &self.builtins[id.0 as usize]
    }
}

/// The arguments and other state passed to a built-in function.
/// WorldState, TaskSchedulerClient, and Session are now accessed via the global task context.
pub(crate) struct BfCallState<'a> {
    /// The name of the invoked function.
    pub(crate) name: Symbol,
    /// Arguments passed to the function.
    pub(crate) args: &'a List,
    /// The current execution state of this task in this VM, including the stack
    /// so that BFs can inspect and manipulate it.
    pub(crate) exec_state: &'a mut VMExecState,
    /// Config
    pub(crate) config: &'a FeaturesConfig,
}

impl BfCallState<'_> {
    pub fn caller_perms(&self) -> Obj {
        self.exec_state.caller_perms()
    }
    pub fn task_perms_who(&self) -> Obj {
        self.exec_state.task_perms()
    }
    pub fn task_perms(&self) -> Result<Perms, WorldStateError> {
        let who = self.task_perms_who();
        let flags = with_current_transaction(|world_state| world_state.flags_of(&who))?;
        Ok(Perms { who, flags })
    }

    pub fn bf_frame(&self) -> &BfFrame {
        let Frame::Bf(frame) = &self.exec_state.top().frame else {
            panic!("Expected a BF frame at the top of the stack");
        };

        frame
    }

    pub fn bf_frame_mut(&mut self) -> &mut BfFrame {
        let Frame::Bf(frame) = &mut self.exec_state.top_mut().frame else {
            panic!("Expected a BF frame at the top of the stack");
        };

        frame
    }

    /// Construct a boolean value from a truthy value but convert to mooR boolean only if that
    /// feature is enabled.
    pub fn v_bool(&self, truthy: bool) -> Var {
        if !self.config.use_boolean_returns {
            v_bool_int(truthy)
        } else {
            Var::mk_bool(truthy)
        }
    }

    /// Convert a map or alist (list of {key, value} pairs) to a Map.
    /// Returns an error if the value is neither a map nor a valid alist.
    pub fn map_or_alist_to_map(&self, value: &Var) -> Result<Map, BfErr> {
        match value.variant() {
            Variant::Map(m) => Ok(m.clone()),
            Variant::List(l) => {
                let mut pairs = Vec::new();
                for item in l.iter() {
                    let Some(pair_list) = item.as_list() else {
                        return Err(BfErr::ErrValue(
                            E_TYPE.msg("Alist must be a list of {key, value} pairs"),
                        ));
                    };
                    if pair_list.len() != 2 {
                        return Err(BfErr::ErrValue(
                            E_TYPE.msg("Alist pairs must have exactly 2 elements"),
                        ));
                    }
                    let key = pair_list.index(0).map_err(BfErr::ErrValue)?;
                    let val = pair_list.index(1).map_err(BfErr::ErrValue)?;
                    pairs.push((key, val));
                }
                Ok(v_map(&pairs).as_map().unwrap().clone())
            }
            _ => Err(BfErr::ErrValue(
                E_TYPE.msg("Expected map or alist (list of {key, value} pairs)"),
            )),
        }
    }
}

pub(crate) type BuiltinFunction = fn(&mut BfCallState<'_>) -> Result<BfRet, BfErr>;

/// Return possibilities from a built-in function.
pub(crate) enum BfRet {
    /// Successful return with no relevant value.
    /// This will just get turned into v_int(0), but I want to call it out as a distinct path.
    /// We used to return v_none here, until TYPE_NONE became E_VARNF.
    RetNil,
    /// Successful return, with a value to be pushed to the value stack.
    Ret(Var),
    /// BF wants to return control back to the VM, with specific instructions to things like
    /// `suspend` or dispatch to a verb call or execute eval.
    VmInstr(ExecutionResult),
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum BfErr {
    #[error("Error in built-in function: {0}")]
    ErrValue(Error),
    #[error("Error in built-in function: {0}")]
    Code(ErrorCode),
    #[error("Raised error: {0:?}")]
    Raise(Error),
    #[error("Transaction rollback-retry")]
    Rollback,
}

pub(crate) fn world_state_bf_err(err: WorldStateError) -> BfErr {
    match err {
        WorldStateError::RollbackRetry => BfErr::Rollback,
        _ => BfErr::ErrValue(err.into()),
    }
}
pub(crate) fn parse_diagnostic_options(
    verbosity: Option<i64>,
    output_mode: Option<i64>,
) -> Result<DiagnosticRenderOptions, BfErr> {
    let verbosity = match verbosity {
        Some(0) => DiagnosticVerbosity::Summary,
        Some(1) => DiagnosticVerbosity::Notes,
        Some(2) => DiagnosticVerbosity::Detailed,
        Some(_) => {
            return Err(BfErr::Code(E_INVARG));
        }
        None => DiagnosticVerbosity::Summary,
    };

    let (use_graphics, use_color) = match output_mode {
        Some(0) => (false, false),
        Some(1) => (true, false),
        Some(2) => (true, true),
        Some(_) => {
            return Err(BfErr::Code(E_INVARG));
        }
        None => (false, false),
    };

    Ok(DiagnosticRenderOptions {
        verbosity,
        use_graphics,
        use_color,
    })
}
