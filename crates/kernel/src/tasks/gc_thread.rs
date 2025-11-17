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

//! Concurrent GC mark phase thread spawned by scheduler

use std::{collections::HashSet, sync::Arc, thread, time::Instant};

use flume::{self, Sender};
use moor_var::Obj;
use tracing::{debug, error, info};

use crate::{
    config::Config,
    tasks::{sched_counters, scheduler_client::SchedulerClientMsg},
};
use moor_common::{
    tasks::SchedulerError, tasks::SchedulerError::GarbageCollectionFailed, util::PerfTimerGuard,
};

/// Spawn a thread to perform concurrent GC mark phase
pub fn spawn_gc_mark_phase(
    gc_tx: Box<dyn moor_db::GCInterface>,
    _config: Arc<Config>,
    scheduler_sender: Sender<SchedulerClientMsg>,
    vm_refs: HashSet<Obj>,
    mutation_timestamp_before_mark: Option<u64>,
    gc_cycle_count: u64,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let result = run_gc_mark_phase(gc_tx, vm_refs, gc_cycle_count);

        match result {
            Ok(unreachable_objects) => {
                if scheduler_sender
                    .send(SchedulerClientMsg::GCMarkPhaseComplete {
                        unreachable_objects,
                        mutation_timestamp_before_mark,
                    })
                    .is_err()
                {
                    error!("Failed to send GC mark phase results to scheduler");
                }
            }
            Err(e) => {
                error!("GC mark phase failed: {e}");
                // Send empty results to indicate failure
                if scheduler_sender
                    .send(SchedulerClientMsg::GCMarkPhaseComplete {
                        unreachable_objects: HashSet::new(),
                        mutation_timestamp_before_mark,
                    })
                    .is_err()
                {
                    error!("Failed to send GC mark phase failure to scheduler");
                }
            }
        }
    })
}

/// Run the mark phase in a background thread
fn run_gc_mark_phase(
    mut gc: Box<dyn moor_db::GCInterface>,
    vm_refs: HashSet<Obj>,
    _gc_cycle_count: u64,
) -> Result<HashSet<Obj>, SchedulerError> {
    let start_time = Instant::now();
    let perfc = sched_counters();
    let _t = PerfTimerGuard::new(&perfc.gc_mark_phase);

    // Get all anonymous objects
    let all_anon_objects = gc
        .get_anonymous_objects()
        .map_err(|e| GarbageCollectionFailed(format!("Failed to get anonymous objects: {e}")))?;

    // Get all DB references to anonymous objects
    let db_refs = gc
        .scan_anonymous_object_references()
        .map_err(|e| GarbageCollectionFailed(format!("Failed to scan DB references: {e}")))?;

    // Mark reachable objects
    let mut reachable_objects = HashSet::new();

    // Mark objects referenced from VM (filter to anonymous only)
    reachable_objects.extend(vm_refs.into_iter().filter(|obj| obj.is_anonymous()));

    // Mark objects referenced from DB
    reachable_objects.extend(db_refs.iter().flat_map(|(_, refs)| refs.clone()));

    // Find unreachable objects
    let unreachable_objects: HashSet<Obj> = all_anon_objects
        .difference(&reachable_objects)
        .copied()
        .collect();

    let mark_duration = start_time.elapsed();
    if unreachable_objects.is_empty() {
        debug!(
            "GC mark: no unreachable objects identified in {:.2}ms",
            mark_duration.as_millis()
        );
    } else {
        info!(
            "GC mark: {} unreachable objects identified in {:.2}ms",
            unreachable_objects.len(),
            mark_duration.as_millis()
        );
    }
    Ok(unreachable_objects)
}
