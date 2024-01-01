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

use moor_values::var::objid::Objid;
use moor_values::var::Var;
use std::time::SystemTime;

pub mod command_parse;
pub mod scheduler;
pub mod sessions;

mod moo_vm_host;
mod task;
pub mod task_messages;
pub mod vm_host;

pub type TaskId = usize;

/// The minimum set of information needed to make a *resolution* call for a verb.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbCall {
    pub verb_name: String,
    pub location: Objid,
    pub this: Objid,
    pub player: Objid,
    pub args: Vec<Var>,
    pub argstr: String,
    pub caller: Objid,
}

/// External interface description of a task, for purpose of e.g. the queued_tasks() builtin.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskDescription {
    pub task_id: TaskId,
    pub start_time: Option<SystemTime>,
    pub permissions: Objid,
    pub verb_name: String,
    pub verb_definer: Objid,
    pub line_number: usize,
    pub this: Objid,
}

pub mod vm_test_utils {
    use crate::tasks::moo_vm_host::MooVmHost;
    use crate::tasks::sessions::Session;
    use crate::tasks::vm_host::{VMHost, VMHostResponse};
    use crate::tasks::VerbCall;
    use crate::vm::vm_execute::VmExecParams;
    use moor_values::model::world_state::WorldState;
    use moor_values::var::Var;
    use moor_values::SYSTEM_OBJECT;
    use std::sync::Arc;
    use std::time::Duration;

    pub async fn call_verb(
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
        verb_name: &str,
        args: Vec<Var>,
    ) -> Var {
        let (scs_tx, _scs_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut vm_host =
            MooVmHost::new(20, 90_000, Duration::from_secs(5), session.clone(), scs_tx);

        let (sched_send, _) = tokio::sync::mpsc::unbounded_channel();
        let _vm_exec_params = VmExecParams {
            world_state,
            session: session.clone(),
            scheduler_sender: sched_send.clone(),
            max_stack_depth: 50,
            ticks_left: 90_000,
            time_left: None,
        };

        let vi = world_state
            .find_method_verb_on(SYSTEM_OBJECT, SYSTEM_OBJECT, verb_name)
            .await
            .unwrap();
        vm_host
            .start_call_method_verb(
                0,
                SYSTEM_OBJECT,
                vi,
                VerbCall {
                    verb_name: verb_name.to_string(),
                    location: SYSTEM_OBJECT,
                    this: SYSTEM_OBJECT,
                    player: SYSTEM_OBJECT,
                    args,
                    argstr: "".to_string(),
                    caller: SYSTEM_OBJECT,
                },
            )
            .await;

        // Call repeatedly into exec until we ge either an error or Complete.
        loop {
            match vm_host.exec_interpreter(0, world_state).await {
                VMHostResponse::ContinueOk => {
                    continue;
                }
                VMHostResponse::DispatchFork(f) => {
                    panic!("Unexpected fork: {:?}", f);
                }
                VMHostResponse::AbortLimit(a) => {
                    panic!("Unexpected abort: {:?}", a);
                }
                VMHostResponse::CompleteException(e) => {
                    panic!("Unexpected exception: {:?}", e)
                }
                VMHostResponse::CompleteSuccess(v) => {
                    return v;
                }
                VMHostResponse::CompleteAbort => {
                    panic!("Unexpected abort");
                }
                VMHostResponse::Suspend(_) => {
                    panic!("Unexpected suspend");
                }
                VMHostResponse::SuspendNeedInput => {
                    panic!("Unexpected suspend need input");
                }
            }
        }
    }
}
