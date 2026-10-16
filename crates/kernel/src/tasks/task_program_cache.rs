// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use ahash::AHasher;
use fast_telemetry::{DeriveLabel, LabeledCounter};
use moor_common::model::{
    TaskPermissions, VerbProgramCacheToken, VerbProgramKey, WorldState, WorldStateError,
};
use moor_compiler::Program;
use moor_var::{Obj, program::ProgramType};
pub use moor_vm::{CachedProgramPtr, ProgramSlot};

#[derive(Debug)]
struct CachedProgramSlot {
    program: Program,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolveVerbSlotResult {
    pub slot: ProgramSlot,
    pub cache_hit: bool,
    pub inserted: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProgramCacheGlobalSnapshot {
    pub hits: i64,
    pub misses: i64,
    pub inserts: i64,
    pub reclaimed: i64,
    pub live_slots: i64,
}

#[derive(Copy, Clone, Debug, DeriveLabel)]
#[label_name = "op"]
pub enum ProgramCacheOp {
    Hits,
    Misses,
    Inserts,
    Reclaimed,
    LiveSlots,
}

pub struct ProgramCacheGlobalStats {
    counters: LabeledCounter<ProgramCacheOp>,
}

impl Default for ProgramCacheGlobalStats {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgramCacheGlobalStats {
    pub fn new() -> Self {
        Self {
            counters: LabeledCounter::new(16),
        }
    }

    pub fn inc(&self, op: ProgramCacheOp) {
        self.counters.inc(op);
    }

    pub fn add(&self, op: ProgramCacheOp, value: isize) {
        self.counters.add(op, value);
    }

    fn snapshot(&self) -> ProgramCacheGlobalSnapshot {
        ProgramCacheGlobalSnapshot {
            hits: self.counters.get(ProgramCacheOp::Hits) as i64,
            misses: self.counters.get(ProgramCacheOp::Misses) as i64,
            inserts: self.counters.get(ProgramCacheOp::Inserts) as i64,
            reclaimed: self.counters.get(ProgramCacheOp::Reclaimed) as i64,
            live_slots: self.counters.get(ProgramCacheOp::LiveSlots) as i64,
        }
    }
}

pub static PROGRAM_CACHE_GLOBAL_STATS: LazyLock<ProgramCacheGlobalStats> =
    LazyLock::new(ProgramCacheGlobalStats::new);

pub fn program_cache_global_stats() -> ProgramCacheGlobalSnapshot {
    PROGRAM_CACHE_GLOBAL_STATS.snapshot()
}

#[derive(Debug, Default)]
pub struct TaskProgramCache {
    slots: Vec<Option<Box<CachedProgramSlot>>>,
    cache: HashMap<VerbProgramKey, usize, std::hash::BuildHasherDefault<AHasher>>,
    token: Option<VerbProgramCacheToken>,
    local_hits: i64,
    local_misses: i64,
    local_inserts: i64,
    local_reclaimed: i64,
}

impl TaskProgramCache {
    pub fn resolve_verb_slot(
        &mut self,
        world_state: &dyn WorldState,
        permissions: &TaskPermissions,
        authorization_obj: &Obj,
        verb_definer: &Obj,
        verb_uuid: uuid::Uuid,
    ) -> Result<ResolveVerbSlotResult, WorldStateError> {
        let token = world_state.verb_program_cache_token();
        if self.token != Some(token) {
            // New calls must see the current transaction's programs. Active and retry frames
            // still hold pointers into old slots, which remain allocated until reclamation.
            self.cache.clear();
            self.token = Some(token);
        }
        let key = VerbProgramKey {
            verb_definer: *verb_definer,
            verb_uuid,
        };
        if let Some(slot) = self.cache.get(&key).copied()
            && self.program_for_slot(slot).is_some()
        {
            self.local_hits += 1;
            PROGRAM_CACHE_GLOBAL_STATS.inc(ProgramCacheOp::Hits);
            return Ok(ResolveVerbSlotResult {
                slot: self.slot_info(slot),
                cache_hit: true,
                inserted: false,
            });
        }
        self.cache.remove(&key);
        self.local_misses += 1;
        PROGRAM_CACHE_GLOBAL_STATS.inc(ProgramCacheOp::Misses);

        let (program, _) = world_state.retrieve_verb_for_execution(
            permissions,
            authorization_obj,
            verb_definer,
            verb_uuid,
        )?;
        let ProgramType::MooR(program) = program;
        let entry = CachedProgramSlot { program };

        let slot = if let Some(reuse_slot) = self.slots.iter().position(|entry| entry.is_none()) {
            self.slots[reuse_slot] = Some(Box::new(entry));
            reuse_slot
        } else {
            let new_slot = self.slots.len();
            self.slots.push(Some(Box::new(entry)));
            new_slot
        };
        self.cache.insert(key, slot);
        self.local_inserts += 1;
        PROGRAM_CACHE_GLOBAL_STATS.inc(ProgramCacheOp::Inserts);
        PROGRAM_CACHE_GLOBAL_STATS.inc(ProgramCacheOp::LiveSlots);
        Ok(ResolveVerbSlotResult {
            slot: self.slot_info(slot),
            cache_hit: false,
            inserted: true,
        })
    }

    pub fn program_for_slot(&self, slot: usize) -> Option<&Program> {
        self.slots
            .get(slot)
            .and_then(Option::as_deref)
            .map(|entry| &entry.program)
    }

    pub fn reclaim_unreferenced(
        &mut self,
        live_program_ptrs: &HashSet<usize, std::hash::BuildHasherDefault<AHasher>>,
    ) -> usize {
        let mut reclaimed = 0usize;
        for slot in &mut self.slots {
            let Some(program) = slot.as_ref() else {
                continue;
            };
            let ptr = &program.as_ref().program as *const Program as usize;
            if !live_program_ptrs.contains(&ptr) {
                *slot = None;
                reclaimed += 1;
            }
        }
        let mut new_cache = HashMap::default();
        for (key, slot_idx) in self.cache.drain() {
            if self.slots.get(slot_idx).and_then(Option::as_ref).is_some() {
                new_cache.insert(key, slot_idx);
            }
        }
        self.cache = new_cache;

        if reclaimed > 0 {
            let reclaimed_i = reclaimed as i64;
            self.local_reclaimed += reclaimed_i;
            PROGRAM_CACHE_GLOBAL_STATS.add(ProgramCacheOp::Reclaimed, reclaimed_i as isize);
            PROGRAM_CACHE_GLOBAL_STATS.add(ProgramCacheOp::LiveSlots, -(reclaimed_i as isize));
        }

        reclaimed
    }

    pub fn total_slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn live_slot_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    pub fn key_count(&self) -> usize {
        self.cache.len()
    }

    fn slot_info(&self, slot: usize) -> ProgramSlot {
        let program = self
            .program_for_slot(slot)
            .expect("Invalid program slot in task program cache");
        ProgramSlot {
            // SAFETY: the program lives in a boxed cache slot and the cache keeps that allocation
            // stable while frames can hold the pointer.
            program_ptr: unsafe { CachedProgramPtr::from_program(program) },
            global_width: program.var_names().global_width(),
            main_max_stack: program.main_max_stack(),
            main_max_scope_depth: program.main_max_scope_depth(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_common::model::VerbProgramKey;
    use moor_compiler::{CompileOptions, compile};
    use moor_var::SYSTEM_OBJECT;
    use uuid::Uuid;

    #[test]
    fn reclaim_unreferenced_drops_dead_slots_and_cache_entries() {
        let p1 = compile("return 1;", CompileOptions::default()).unwrap();
        let p2 = compile("return 2;", CompileOptions::default()).unwrap();
        let p1_slot = Box::new(CachedProgramSlot { program: p1 });
        let p2_slot = Box::new(CachedProgramSlot { program: p2 });
        let p1_ptr = &p1_slot.as_ref().program as *const Program as usize;
        let p2_ptr = &p2_slot.as_ref().program as *const Program as usize;

        let key1 = VerbProgramKey {
            verb_definer: SYSTEM_OBJECT,
            verb_uuid: Uuid::new_v4(),
        };
        let key2 = VerbProgramKey {
            verb_definer: SYSTEM_OBJECT,
            verb_uuid: Uuid::new_v4(),
        };

        let mut cache = TaskProgramCache {
            slots: vec![Some(p1_slot), Some(p2_slot)],
            cache: HashMap::default(),
            token: None,
            local_hits: 0,
            local_misses: 0,
            local_inserts: 0,
            local_reclaimed: 0,
        };
        cache.cache.insert(key1, 0);
        cache.cache.insert(key2, 1);

        let mut live = HashSet::with_hasher(std::hash::BuildHasherDefault::<AHasher>::default());
        live.insert(p1_ptr);

        cache.reclaim_unreferenced(&live);

        assert!(cache.program_for_slot(0).is_some());
        assert!(cache.program_for_slot(1).is_none());
        assert_eq!(cache.cache.get(&key1).copied(), Some(0));
        assert!(!cache.cache.contains_key(&key2));
        assert_ne!(p1_ptr, p2_ptr);
    }
}
