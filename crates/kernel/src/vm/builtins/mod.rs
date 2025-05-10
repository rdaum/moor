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

use fast_counter::ConcurrentCounter;
use lazy_static::lazy_static;
use std::sync::Arc;
use thiserror::Error;

use crate::config::FeaturesConfig;
use crate::tasks::task_scheduler_client::TaskSchedulerClient;
use crate::vm::activation::{BfFrame, Frame};
use crate::vm::builtins::bf_age_crypto::register_bf_age_crypto;
use crate::vm::builtins::bf_flyweights::register_bf_flyweights;
use crate::vm::builtins::bf_list_sets::register_bf_list_sets;
use crate::vm::builtins::bf_maps::register_bf_maps;
use crate::vm::builtins::bf_num::register_bf_num;
use crate::vm::builtins::bf_objects::register_bf_objects;
use crate::vm::builtins::bf_properties::register_bf_properties;
use crate::vm::builtins::bf_server::{bf_noop, register_bf_server};
use crate::vm::builtins::bf_strings::register_bf_strings;
use crate::vm::builtins::bf_values::register_bf_values;
use crate::vm::builtins::bf_verbs::register_bf_verbs;
use crate::vm::exec_state::VMExecState;
use crate::vm::vm_host::ExecutionResult;
use moor_common::model::Perms;
use moor_common::model::WorldState;
use moor_common::model::WorldStateError;
use moor_common::tasks::Session;
use moor_common::util::PerfCounter;
use moor_compiler::{BUILTINS, BuiltinId};
use moor_var::Var;
use moor_var::{Error, List};
use moor_var::{ErrorCode, Symbol};
use moor_var::{Obj, v_bool_int};

mod bf_age_crypto;
mod bf_flyweights;
mod bf_list_sets;
mod bf_maps;
mod bf_num;
mod bf_objects;
mod bf_properties;
pub mod bf_server;
mod bf_strings;
mod bf_values;
mod bf_verbs;

lazy_static! {
    static ref BF_COUNTERS: Arc<BfCounters> = Arc::new(BfCounters::new());
}

pub struct BfCounters(Vec<PerfCounter>);

impl Default for BfCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl BfCounters {
    pub fn new() -> Self {
        let mut counters = Vec::with_capacity(BUILTINS.len());
        for i in 0..BUILTINS.len() {
            counters.push(PerfCounter {
                operation: BUILTINS.names[&BuiltinId(i as u16)],
                invocations: ConcurrentCounter::new(0),
                cumulative_duration_nanos: ConcurrentCounter::new(0),
            });
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

pub fn bf_perf_counters() -> Arc<BfCounters> {
    BF_COUNTERS.clone()
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
        let mut builtins: Vec<Box<BuiltinFunction>> = Vec::with_capacity(BUILTINS.len());
        for _ in 0..BUILTINS.len() {
            builtins.push(Box::new(bf_noop))
        }
        register_bf_server(&mut builtins);
        register_bf_num(&mut builtins);
        register_bf_values(&mut builtins);
        register_bf_strings(&mut builtins);
        register_bf_list_sets(&mut builtins);
        register_bf_maps(&mut builtins);
        register_bf_objects(&mut builtins);
        register_bf_verbs(&mut builtins);
        register_bf_properties(&mut builtins);
        register_bf_flyweights(&mut builtins);
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
pub(crate) struct BfCallState<'a> {
    /// The name of the invoked function.
    pub(crate) name: Symbol,
    /// Arguments passed to the function.
    pub(crate) args: List,
    /// The current execution state of this task in this VM, including the stack
    /// so that BFs can inspect and manipulate it.
    pub(crate) exec_state: &'a mut VMExecState,
    /// Handle to the current database transaction.
    pub(crate) world_state: &'a mut dyn WorldState,
    /// For connection / message management.
    pub(crate) session: Arc<dyn Session>,
    /// For sending messages up to the scheduler
    pub(crate) task_scheduler_client: TaskSchedulerClient,
    /// Config
    pub(crate) config: FeaturesConfig,
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
        let flags = self.world_state.flags_of(&who)?;
        Ok(Perms {
            who: who.clone(),
            flags,
        })
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
}

pub(crate) type BuiltinFunction = fn(&mut BfCallState<'_>) -> Result<BfRet, BfErr>;

/// Return possibilities from a built-in function.
pub(crate) enum BfRet {
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
