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

use crate::cache::PROPERTY_PIC_STATS;
use moor_common::model::PropertyLookupPicOutcome;
use moor_common::util::ConcurrentCounter;
use std::cell::RefCell;
use std::sync::OnceLock;

const OUTCOME_COUNT: usize = 6;
const VM_HINT_COUNT: usize = 8;
const TLS_FLUSH_BATCH_SIZE: u32 = 128;
const VM_HINT_GET_PROP_WITH_HINT: usize = 0;
const VM_HINT_GET_PROP_NO_HINT: usize = 1;
const VM_HINT_PUSH_GET_PROP_WITH_HINT: usize = 2;
const VM_HINT_PUSH_GET_PROP_NO_HINT: usize = 3;
const VM_HINT_PUT_PROP_WITH_HINT: usize = 4;
const VM_HINT_PUT_PROP_NO_HINT: usize = 5;
const VM_HINT_PUT_PROP_AT_WITH_HINT: usize = 6;
const VM_HINT_PUT_PROP_AT_NO_HINT: usize = 7;

#[derive(Debug, Clone, Copy)]
pub struct PropertyPicSnapshot {
    pub read_hits: isize,
    pub read_miss_no_hint: isize,
    pub read_miss_guard_mismatch: isize,
    pub read_miss_version_mismatch: isize,
    pub read_miss_resolve_failed: isize,
    pub read_not_applicable: isize,
    pub write_hits: isize,
    pub write_miss_no_hint: isize,
    pub write_miss_guard_mismatch: isize,
    pub write_miss_version_mismatch: isize,
    pub write_miss_resolve_failed: isize,
    pub write_not_applicable: isize,
    pub vm_get_prop_with_hint: isize,
    pub vm_get_prop_no_hint: isize,
    pub vm_push_get_prop_with_hint: isize,
    pub vm_push_get_prop_no_hint: isize,
    pub vm_put_prop_with_hint: isize,
    pub vm_put_prop_no_hint: isize,
    pub vm_put_prop_at_with_hint: isize,
    pub vm_put_prop_at_no_hint: isize,
}

impl PropertyPicSnapshot {
    #[inline]
    pub fn read_total(&self) -> isize {
        self.read_hits
            + self.read_miss_no_hint
            + self.read_miss_guard_mismatch
            + self.read_miss_version_mismatch
            + self.read_miss_resolve_failed
            + self.read_not_applicable
    }

    #[inline]
    pub fn write_total(&self) -> isize {
        self.write_hits
            + self.write_miss_no_hint
            + self.write_miss_guard_mismatch
            + self.write_miss_version_mismatch
            + self.write_miss_resolve_failed
            + self.write_not_applicable
    }

    #[inline]
    pub fn read_hit_rate(&self) -> f64 {
        let total = self.read_total() as f64;
        if total > 0.0 {
            (self.read_hits as f64 / total) * 100.0
        } else {
            0.0
        }
    }

    #[inline]
    pub fn write_hit_rate(&self) -> f64 {
        let total = self.write_total() as f64;
        if total > 0.0 {
            (self.write_hits as f64 / total) * 100.0
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
fn outcome_index(outcome: PropertyLookupPicOutcome) -> usize {
    match outcome {
        PropertyLookupPicOutcome::Hit => 0,
        PropertyLookupPicOutcome::MissNoHint => 1,
        PropertyLookupPicOutcome::MissGuardMismatch => 2,
        PropertyLookupPicOutcome::MissVersionMismatch => 3,
        PropertyLookupPicOutcome::MissResolveFailed => 4,
        PropertyLookupPicOutcome::NotApplicable => 5,
    }
}

#[derive(Default)]
struct LocalPropertyPicStats {
    read: [u32; OUTCOME_COUNT],
    write: [u32; OUTCOME_COUNT],
    vm_hint: [u32; VM_HINT_COUNT],
}

impl LocalPropertyPicStats {
    #[inline]
    fn should_flush(&self) -> bool {
        let read_total: u32 = self.read.iter().sum();
        let write_total: u32 = self.write.iter().sum();
        let vm_hint_total: u32 = self.vm_hint.iter().sum();
        read_total + write_total + vm_hint_total >= TLS_FLUSH_BATCH_SIZE
    }
}

struct PropertyPicStatsTls(LocalPropertyPicStats);

impl PropertyPicStatsTls {
    #[inline]
    fn new() -> Self {
        Self(LocalPropertyPicStats::default())
    }

    #[inline]
    fn flush_local(&mut self) {
        for i in 0..OUTCOME_COUNT {
            let read_count = self.0.read[i];
            if read_count != 0 {
                PROPERTY_PIC_STATS.read[i].add(read_count as isize);
            }

            let write_count = self.0.write[i];
            if write_count != 0 {
                PROPERTY_PIC_STATS.write[i].add(write_count as isize);
            }
        }
        for i in 0..VM_HINT_COUNT {
            let vm_hint_count = self.0.vm_hint[i];
            if vm_hint_count != 0 {
                PROPERTY_PIC_STATS.vm_hint[i].add(vm_hint_count as isize);
            }
        }
        self.0 = LocalPropertyPicStats::default();
    }
}

impl Drop for PropertyPicStatsTls {
    fn drop(&mut self) {
        self.flush_local();
    }
}

thread_local! {
    static PROPERTY_PIC_STATS_TLS: RefCell<PropertyPicStatsTls> = RefCell::new(PropertyPicStatsTls::new());
}

#[inline]
pub fn record_property_pic_read(outcome: PropertyLookupPicOutcome) {
    let index = outcome_index(outcome);
    PROPERTY_PIC_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.read[index] += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

#[inline]
pub fn record_property_pic_write(outcome: PropertyLookupPicOutcome) {
    let index = outcome_index(outcome);
    PROPERTY_PIC_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.write[index] += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

#[inline]
fn record_vm_property_hint(index: usize) {
    PROPERTY_PIC_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.vm_hint[index] += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

#[inline]
pub fn record_vm_property_hint_get_prop(hint_present: bool) {
    let index = if hint_present {
        VM_HINT_GET_PROP_WITH_HINT
    } else {
        VM_HINT_GET_PROP_NO_HINT
    };
    record_vm_property_hint(index);
}

#[inline]
pub fn record_vm_property_hint_push_get_prop(hint_present: bool) {
    let index = if hint_present {
        VM_HINT_PUSH_GET_PROP_WITH_HINT
    } else {
        VM_HINT_PUSH_GET_PROP_NO_HINT
    };
    record_vm_property_hint(index);
}

#[inline]
pub fn record_vm_property_hint_put_prop(hint_present: bool) {
    let index = if hint_present {
        VM_HINT_PUT_PROP_WITH_HINT
    } else {
        VM_HINT_PUT_PROP_NO_HINT
    };
    record_vm_property_hint(index);
}

#[inline]
pub fn record_vm_property_hint_put_prop_at(hint_present: bool) {
    let index = if hint_present {
        VM_HINT_PUT_PROP_AT_WITH_HINT
    } else {
        VM_HINT_PUT_PROP_AT_NO_HINT
    };
    record_vm_property_hint(index);
}

pub struct PropertyPicStats {
    read: [ConcurrentCounter; OUTCOME_COUNT],
    write: [ConcurrentCounter; OUTCOME_COUNT],
    vm_hint: [ConcurrentCounter; VM_HINT_COUNT],
}

impl PropertyPicStats {
    pub fn new() -> Self {
        let shards = default_shard_count();
        Self {
            read: std::array::from_fn(|_| ConcurrentCounter::new(shards)),
            write: std::array::from_fn(|_| ConcurrentCounter::new(shards)),
            vm_hint: std::array::from_fn(|_| ConcurrentCounter::new(shards)),
        }
    }

    #[inline]
    pub fn snapshot(&self) -> PropertyPicSnapshot {
        PropertyPicSnapshot {
            read_hits: self.read[0].sum(),
            read_miss_no_hint: self.read[1].sum(),
            read_miss_guard_mismatch: self.read[2].sum(),
            read_miss_version_mismatch: self.read[3].sum(),
            read_miss_resolve_failed: self.read[4].sum(),
            read_not_applicable: self.read[5].sum(),
            write_hits: self.write[0].sum(),
            write_miss_no_hint: self.write[1].sum(),
            write_miss_guard_mismatch: self.write[2].sum(),
            write_miss_version_mismatch: self.write[3].sum(),
            write_miss_resolve_failed: self.write[4].sum(),
            write_not_applicable: self.write[5].sum(),
            vm_get_prop_with_hint: self.vm_hint[VM_HINT_GET_PROP_WITH_HINT].sum(),
            vm_get_prop_no_hint: self.vm_hint[VM_HINT_GET_PROP_NO_HINT].sum(),
            vm_push_get_prop_with_hint: self.vm_hint[VM_HINT_PUSH_GET_PROP_WITH_HINT].sum(),
            vm_push_get_prop_no_hint: self.vm_hint[VM_HINT_PUSH_GET_PROP_NO_HINT].sum(),
            vm_put_prop_with_hint: self.vm_hint[VM_HINT_PUT_PROP_WITH_HINT].sum(),
            vm_put_prop_no_hint: self.vm_hint[VM_HINT_PUT_PROP_NO_HINT].sum(),
            vm_put_prop_at_with_hint: self.vm_hint[VM_HINT_PUT_PROP_AT_WITH_HINT].sum(),
            vm_put_prop_at_no_hint: self.vm_hint[VM_HINT_PUT_PROP_AT_NO_HINT].sum(),
        }
    }
}

impl Default for PropertyPicStats {
    fn default() -> Self {
        Self::new()
    }
}
