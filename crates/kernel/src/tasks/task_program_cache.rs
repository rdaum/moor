// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use std::collections::{HashMap, HashSet};

use ahash::AHasher;
use lazy_static::lazy_static;
use moor_common::model::{PropertyLookupHint, VerbLookupHint};
use moor_common::model::{VerbProgramKey, WorldState, WorldStateError};
use moor_common::util::ConcurrentCounter;
use moor_compiler::Program;
use moor_var::{
    Obj,
    program::{ProgramType, labels::Offset},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramSlot {
    pub program_ptr: usize,
    pub program_ic_ptr: Option<usize>,
    pub global_width: usize,
}

#[derive(Debug)]
pub struct ProgramInlineCaches {
    main_property: Vec<Option<PropertyLookupHint>>,
    fork_property: Vec<Vec<Option<PropertyLookupHint>>>,
    lambda_property: Vec<Vec<Option<PropertyLookupHint>>>,
    main_verb: Vec<Option<VerbLookupHint>>,
    fork_verb: Vec<Vec<Option<VerbLookupHint>>>,
    lambda_verb: Vec<Vec<Option<VerbLookupHint>>>,
}

impl ProgramInlineCaches {
    fn new(program: &Program) -> Self {
        let main_property = vec![None; program.main_vector().len()];
        let main_verb = vec![None; program.main_vector().len()];
        let fork_property = program
            .0
            .fork_vectors
            .iter()
            .map(|fv| vec![None; fv.1.len()])
            .collect();
        let fork_verb = program
            .0
            .fork_vectors
            .iter()
            .map(|fv| vec![None; fv.1.len()])
            .collect();
        let lambda_property = program
            .0
            .lambda_programs
            .iter()
            .map(|lp| vec![None; lp.main_vector().len()])
            .collect();
        let lambda_verb = program
            .0
            .lambda_programs
            .iter()
            .map(|lp| vec![None; lp.main_vector().len()])
            .collect();
        Self {
            main_property,
            fork_property,
            lambda_property,
            main_verb,
            fork_verb,
            lambda_verb,
        }
    }

    pub fn main_property_mut(&mut self) -> &mut [Option<PropertyLookupHint>] {
        &mut self.main_property
    }

    pub fn fork_property_mut(&mut self, fork_offset: Offset) -> &mut [Option<PropertyLookupHint>] {
        &mut self.fork_property[fork_offset.0 as usize]
    }

    pub fn lambda_property_mut(
        &mut self,
        lambda_offset: Offset,
    ) -> &mut [Option<PropertyLookupHint>] {
        &mut self.lambda_property[lambda_offset.0 as usize]
    }

    pub fn main_verb_mut(&mut self) -> &mut [Option<VerbLookupHint>] {
        &mut self.main_verb
    }

    pub fn fork_verb_mut(&mut self, fork_offset: Offset) -> &mut [Option<VerbLookupHint>] {
        &mut self.fork_verb[fork_offset.0 as usize]
    }

    pub fn lambda_verb_mut(&mut self, lambda_offset: Offset) -> &mut [Option<VerbLookupHint>] {
        &mut self.lambda_verb[lambda_offset.0 as usize]
    }
}

#[derive(Debug)]
struct CachedProgramSlot {
    program: Program,
    inline_caches: ProgramInlineCaches,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProgramCacheLocalSnapshot {
    pub hits: i64,
    pub misses: i64,
    pub inserts: i64,
    pub reclaimed: i64,
}

#[derive(Debug)]
pub struct ProgramCacheGlobalStats {
    hits: ConcurrentCounter,
    misses: ConcurrentCounter,
    inserts: ConcurrentCounter,
    reclaimed: ConcurrentCounter,
    live_slots: ConcurrentCounter,
}

impl Default for ProgramCacheGlobalStats {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgramCacheGlobalStats {
    pub fn new() -> Self {
        Self {
            hits: ConcurrentCounter::new(32),
            misses: ConcurrentCounter::new(32),
            inserts: ConcurrentCounter::new(32),
            reclaimed: ConcurrentCounter::new(32),
            live_slots: ConcurrentCounter::new(32),
        }
    }

    fn snapshot(&self) -> ProgramCacheGlobalSnapshot {
        ProgramCacheGlobalSnapshot {
            hits: self.hits.sum() as i64,
            misses: self.misses.sum() as i64,
            inserts: self.inserts.sum() as i64,
            reclaimed: self.reclaimed.sum() as i64,
            live_slots: self.live_slots.sum() as i64,
        }
    }
}

lazy_static! {
    pub static ref PROGRAM_CACHE_GLOBAL_STATS: ProgramCacheGlobalStats =
        ProgramCacheGlobalStats::new();
}

pub fn program_cache_global_stats() -> ProgramCacheGlobalSnapshot {
    PROGRAM_CACHE_GLOBAL_STATS.snapshot()
}

#[derive(Debug, Default)]
pub struct TaskProgramCache {
    slots: Vec<Option<Box<CachedProgramSlot>>>,
    cache: HashMap<VerbProgramKey, usize, std::hash::BuildHasherDefault<AHasher>>,
    local_hits: i64,
    local_misses: i64,
    local_inserts: i64,
    local_reclaimed: i64,
}

impl TaskProgramCache {
    pub fn resolve_verb_slot(
        &mut self,
        world_state: &dyn WorldState,
        perms: &Obj,
        verb_definer: &Obj,
        verb_uuid: uuid::Uuid,
    ) -> Result<ResolveVerbSlotResult, WorldStateError> {
        let key = VerbProgramKey {
            verb_definer: *verb_definer,
            verb_uuid,
        };
        if let Some(slot) = self.cache.get(&key).copied()
            && self.program_for_slot(slot).is_some()
        {
            self.local_hits += 1;
            PROGRAM_CACHE_GLOBAL_STATS.hits.add(1);
            return Ok(ResolveVerbSlotResult {
                slot: self.slot_info(slot),
                cache_hit: true,
                inserted: false,
            });
        }
        self.cache.remove(&key);
        self.local_misses += 1;
        PROGRAM_CACHE_GLOBAL_STATS.misses.add(1);

        let (program, _) = world_state.retrieve_verb(perms, verb_definer, verb_uuid)?;
        let ProgramType::MooR(program) = program;
        let inline_caches = ProgramInlineCaches::new(&program);
        let entry = CachedProgramSlot {
            program,
            inline_caches,
        };

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
        PROGRAM_CACHE_GLOBAL_STATS.inserts.add(1);
        PROGRAM_CACHE_GLOBAL_STATS.live_slots.add(1);
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
            PROGRAM_CACHE_GLOBAL_STATS
                .reclaimed
                .add(reclaimed_i as isize);
            PROGRAM_CACHE_GLOBAL_STATS
                .live_slots
                .add(-(reclaimed_i as isize));
        }

        reclaimed
    }

    pub fn local_stats_snapshot(&self) -> ProgramCacheLocalSnapshot {
        ProgramCacheLocalSnapshot {
            hits: self.local_hits,
            misses: self.local_misses,
            inserts: self.local_inserts,
            reclaimed: self.local_reclaimed,
        }
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
            program_ptr: program as *const Program as usize,
            program_ic_ptr: self
                .slots
                .get(slot)
                .and_then(Option::as_deref)
                .map(|entry| &entry.inline_caches as *const ProgramInlineCaches as usize),
            global_width: program.var_names().global_width(),
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
        let p1_slot = Box::new(CachedProgramSlot {
            inline_caches: ProgramInlineCaches::new(&p1),
            program: p1,
        });
        let p2_slot = Box::new(CachedProgramSlot {
            inline_caches: ProgramInlineCaches::new(&p2),
            program: p2,
        });
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
        assert!(cache.cache.get(&key2).is_none());
        assert_ne!(p1_ptr, p2_ptr);
    }
}
