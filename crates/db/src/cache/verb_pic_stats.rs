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

use crate::cache::VERB_PIC_STATS;
use moor_common::model::VerbLookupPicOutcome;
use moor_common::util::ConcurrentCounter;
use std::cell::RefCell;
use std::sync::OnceLock;

const OUTCOME_COUNT: usize = 6;
const VM_HINT_COUNT: usize = 4;
const TLS_FLUSH_BATCH_SIZE: u32 = 128;

const VM_HINT_CALL_VERB_WITH_HINT: usize = 0;
const VM_HINT_CALL_VERB_NO_HINT: usize = 1;
const VM_HINT_PASS_WITH_HINT: usize = 2;
const VM_HINT_PASS_NO_HINT: usize = 3;

#[derive(Debug, Clone, Copy)]
pub struct VerbPicSnapshot {
    pub dispatch_hits: isize,
    pub dispatch_miss_no_hint: isize,
    pub dispatch_miss_guard_mismatch: isize,
    pub dispatch_miss_version_mismatch: isize,
    pub dispatch_miss_resolve_failed: isize,
    pub dispatch_not_applicable: isize,
    pub vm_call_verb_with_hint: isize,
    pub vm_call_verb_no_hint: isize,
    pub vm_pass_with_hint: isize,
    pub vm_pass_no_hint: isize,
}

impl VerbPicSnapshot {
    #[inline]
    pub fn dispatch_total(&self) -> isize {
        self.dispatch_hits
            + self.dispatch_miss_no_hint
            + self.dispatch_miss_guard_mismatch
            + self.dispatch_miss_version_mismatch
            + self.dispatch_miss_resolve_failed
            + self.dispatch_not_applicable
    }

    #[inline]
    pub fn dispatch_hit_rate(&self) -> f64 {
        let total = self.dispatch_total() as f64;
        if total > 0.0 {
            (self.dispatch_hits as f64 / total) * 100.0
        } else {
            0.0
        }
    }
}

#[inline]
fn default_shard_count() -> usize {
    static SHARD_COUNT: OnceLock<usize> = OnceLock::new();
    *SHARD_COUNT.get_or_init(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    })
}

#[inline]
fn outcome_index(outcome: VerbLookupPicOutcome) -> usize {
    match outcome {
        VerbLookupPicOutcome::Hit => 0,
        VerbLookupPicOutcome::MissNoHint => 1,
        VerbLookupPicOutcome::MissGuardMismatch => 2,
        VerbLookupPicOutcome::MissVersionMismatch => 3,
        VerbLookupPicOutcome::MissResolveFailed => 4,
        VerbLookupPicOutcome::NotApplicable => 5,
    }
}

#[derive(Default)]
struct LocalVerbPicStats {
    dispatch: [u32; OUTCOME_COUNT],
    vm_hint: [u32; VM_HINT_COUNT],
}

impl LocalVerbPicStats {
    #[inline]
    fn should_flush(&self) -> bool {
        let dispatch_total: u32 = self.dispatch.iter().sum();
        let vm_hint_total: u32 = self.vm_hint.iter().sum();
        dispatch_total + vm_hint_total >= TLS_FLUSH_BATCH_SIZE
    }
}

struct VerbPicStatsTls(LocalVerbPicStats);

impl VerbPicStatsTls {
    #[inline]
    fn new() -> Self {
        Self(LocalVerbPicStats::default())
    }

    #[inline]
    fn flush_local(&mut self) {
        for i in 0..OUTCOME_COUNT {
            let dispatch_count = self.0.dispatch[i];
            if dispatch_count != 0 {
                VERB_PIC_STATS.dispatch[i].add(dispatch_count as isize);
            }
        }
        for i in 0..VM_HINT_COUNT {
            let vm_hint_count = self.0.vm_hint[i];
            if vm_hint_count != 0 {
                VERB_PIC_STATS.vm_hint[i].add(vm_hint_count as isize);
            }
        }
        self.0 = LocalVerbPicStats::default();
    }
}

impl Drop for VerbPicStatsTls {
    fn drop(&mut self) {
        self.flush_local();
    }
}

thread_local! {
    static VERB_PIC_STATS_TLS: RefCell<VerbPicStatsTls> = RefCell::new(VerbPicStatsTls::new());
}

#[inline]
pub fn record_verb_pic_dispatch(outcome: VerbLookupPicOutcome) {
    let index = outcome_index(outcome);
    VERB_PIC_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.dispatch[index] += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

#[inline]
fn record_vm_verb_hint(index: usize) {
    VERB_PIC_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.vm_hint[index] += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

#[inline]
pub fn record_vm_verb_hint_call_verb(hint_present: bool) {
    let index = if hint_present {
        VM_HINT_CALL_VERB_WITH_HINT
    } else {
        VM_HINT_CALL_VERB_NO_HINT
    };
    record_vm_verb_hint(index);
}

#[inline]
pub fn record_vm_verb_hint_pass(hint_present: bool) {
    let index = if hint_present {
        VM_HINT_PASS_WITH_HINT
    } else {
        VM_HINT_PASS_NO_HINT
    };
    record_vm_verb_hint(index);
}

pub struct VerbPicStats {
    dispatch: [ConcurrentCounter; OUTCOME_COUNT],
    vm_hint: [ConcurrentCounter; VM_HINT_COUNT],
}

impl VerbPicStats {
    pub fn new() -> Self {
        let shards = default_shard_count();
        Self {
            dispatch: std::array::from_fn(|_| ConcurrentCounter::new(shards)),
            vm_hint: std::array::from_fn(|_| ConcurrentCounter::new(shards)),
        }
    }

    #[inline]
    pub fn snapshot(&self) -> VerbPicSnapshot {
        VerbPicSnapshot {
            dispatch_hits: self.dispatch[0].sum(),
            dispatch_miss_no_hint: self.dispatch[1].sum(),
            dispatch_miss_guard_mismatch: self.dispatch[2].sum(),
            dispatch_miss_version_mismatch: self.dispatch[3].sum(),
            dispatch_miss_resolve_failed: self.dispatch[4].sum(),
            dispatch_not_applicable: self.dispatch[5].sum(),
            vm_call_verb_with_hint: self.vm_hint[VM_HINT_CALL_VERB_WITH_HINT].sum(),
            vm_call_verb_no_hint: self.vm_hint[VM_HINT_CALL_VERB_NO_HINT].sum(),
            vm_pass_with_hint: self.vm_hint[VM_HINT_PASS_WITH_HINT].sum(),
            vm_pass_no_hint: self.vm_hint[VM_HINT_PASS_NO_HINT].sum(),
        }
    }
}

impl Default for VerbPicStats {
    fn default() -> Self {
        Self::new()
    }
}
