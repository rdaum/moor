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

use moor_bench_utils::{BenchContext, black_box};
use moor_db::{
    CheckRelation, Error, Provider, Relation, RelationCodomain, RelationIndex, Timestamp, Tx,
    WorkingSet,
};
use moor_var::Symbol;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const CHECK_CHUNK_SIZE: Option<usize> = None;
const APPLY_TX_PER_CHUNK: usize = 16_384;
const APPLY_TUPLE_OPS_PER_TX: u64 = 12;
const BASE_TS: u64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Domain(u64);

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Domain({})", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PlainCodomain(u64);

impl RelationCodomain for PlainCodomain {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MergeCodomain(u64);

impl RelationCodomain for MergeCodomain {
    fn try_merge(&self, base: &Self, theirs: &Self) -> Option<Self> {
        Some(MergeCodomain(
            self.0.wrapping_add(theirs.0).wrapping_sub(base.0),
        ))
    }
}

#[derive(Clone)]
struct InMemoryProvider<C> {
    data: Arc<Mutex<HashMap<Domain, C>>>,
}

impl<C> InMemoryProvider<C> {
    fn new(data: HashMap<Domain, C>) -> Self {
        Self {
            data: Arc::new(Mutex::new(data)),
        }
    }
}

impl<C> Provider<Domain, C> for InMemoryProvider<C>
where
    C: RelationCodomain,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, C)>, Error> {
        let data = self.data.lock().unwrap();
        Ok(data.get(domain).cloned().map(|v| (Timestamp(1), v)))
    }

    fn put(&self, _timestamp: Timestamp, domain: &Domain, codomain: &C) -> Result<(), Error> {
        let mut data = self.data.lock().unwrap();
        data.insert(domain.clone(), codomain.clone());
        Ok(())
    }

    fn del(&self, _timestamp: Timestamp, domain: &Domain) -> Result<(), Error> {
        let mut data = self.data.lock().unwrap();
        data.remove(domain);
        Ok(())
    }

    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, C)>, Error>
    where
        F: Fn(&Domain, &C) -> bool,
    {
        let data = self.data.lock().unwrap();
        Ok(data
            .iter()
            .filter(|(k, v)| predicate(k, v))
            .map(|(k, v)| (Timestamp(1), k.clone(), v.clone()))
            .collect())
    }

    fn stop(&self) -> Result<(), Error> {
        Ok(())
    }
}

struct CheckMergeContext {
    relation: Relation<Domain, MergeCodomain, InMemoryProvider<MergeCodomain>>,
    base_index: Box<dyn RelationIndex<Domain, MergeCodomain>>,
    check_index: Box<dyn RelationIndex<Domain, MergeCodomain>>,
    domain: Domain,
}

impl BenchContext for CheckMergeContext {
    fn prepare(_num_chunks: usize) -> Self {
        let domain = Domain(1);
        let mut data = HashMap::new();
        data.insert(domain.clone(), MergeCodomain(10));
        let provider = Arc::new(InMemoryProvider::new(data));
        let relation = Relation::new(Symbol::mk("tx-check-merge"), provider);
        let base_index = relation.seeded_index().unwrap();
        let check_index = base_index.fork();
        Self {
            relation,
            base_index,
            check_index,
            domain,
        }
    }

    fn chunk_size() -> Option<usize> {
        CHECK_CHUNK_SIZE
    }
}

struct ApplyContext {
    relation: Relation<Domain, PlainCodomain, InMemoryProvider<PlainCodomain>>,
    base_index: Box<dyn RelationIndex<Domain, PlainCodomain>>,
}

impl BenchContext for ApplyContext {
    fn prepare(_num_chunks: usize) -> Self {
        let provider = Arc::new(InMemoryProvider::new(HashMap::new()));
        let relation = Relation::new(Symbol::mk("tx-apply"), provider);
        let base_index = relation.seeded_index().unwrap();
        Self {
            relation,
            base_index,
        }
    }

    fn chunk_size() -> Option<usize> {
        Some(APPLY_TX_PER_CHUNK)
    }

    fn operations_per_chunk() -> Option<u64> {
        Some((APPLY_TX_PER_CHUNK as u64) * APPLY_TUPLE_OPS_PER_TX)
    }
}

struct CheckNoConflictCoreContext {
    checker: CheckRelation<Domain, PlainCodomain, InMemoryProvider<PlainCodomain>>,
    ws: WorkingSet<Domain, PlainCodomain>,
}

impl BenchContext for CheckNoConflictCoreContext {
    fn prepare(_num_chunks: usize) -> Self {
        let domain = Domain(1);
        let mut data = HashMap::new();
        data.insert(domain.clone(), PlainCodomain(10));
        let provider = Arc::new(InMemoryProvider::new(data));
        let relation = Relation::new(Symbol::mk("tx-check-core-no-conflict"), provider);
        let base_index = relation.seeded_index().unwrap();
        let tx = Tx {
            ts: Timestamp(BASE_TS),
            snapshot_version: 0,
        };
        let mut rt = relation.start_from_index(&tx, base_index.as_ref());
        rt.update(&domain, PlainCodomain(100)).unwrap();
        let ws = rt.working_set().unwrap();
        let checker = relation.begin_check_from_index(base_index.as_ref());
        Self { checker, ws }
    }
}

struct CheckConflictIdenticalCoreContext {
    checker: CheckRelation<Domain, PlainCodomain, InMemoryProvider<PlainCodomain>>,
    ws: WorkingSet<Domain, PlainCodomain>,
}

impl BenchContext for CheckConflictIdenticalCoreContext {
    fn prepare(_num_chunks: usize) -> Self {
        let domain = Domain(1);
        let mut data = HashMap::new();
        data.insert(domain.clone(), PlainCodomain(10));
        let provider = Arc::new(InMemoryProvider::new(data));
        let relation = Relation::new(Symbol::mk("tx-check-core-identical"), provider);
        let base_index = relation.seeded_index().unwrap();
        let tx = Tx {
            ts: Timestamp(BASE_TS),
            snapshot_version: 0,
        };
        let mut rt = relation.start_from_index(&tx, base_index.as_ref());
        rt.update(&domain, PlainCodomain(123)).unwrap();
        let ws = rt.working_set().unwrap();

        let mut checker_index = base_index.fork();
        checker_index.insert_entry(Timestamp(BASE_TS + 1), domain, PlainCodomain(123));
        let checker = relation.begin_check_from_index(checker_index.as_ref());
        Self { checker, ws }
    }
}

struct CheckConflictUnresolvableCoreContext {
    checker: CheckRelation<Domain, PlainCodomain, InMemoryProvider<PlainCodomain>>,
    ws: WorkingSet<Domain, PlainCodomain>,
}

impl BenchContext for CheckConflictUnresolvableCoreContext {
    fn prepare(_num_chunks: usize) -> Self {
        let domain = Domain(1);
        let mut data = HashMap::new();
        data.insert(domain.clone(), PlainCodomain(10));
        let provider = Arc::new(InMemoryProvider::new(data));
        let relation = Relation::new(Symbol::mk("tx-check-core-fail"), provider);
        let base_index = relation.seeded_index().unwrap();
        let tx = Tx {
            ts: Timestamp(BASE_TS),
            snapshot_version: 0,
        };
        let mut rt = relation.start_from_index(&tx, base_index.as_ref());
        rt.update(&domain, PlainCodomain(11)).unwrap();
        let ws = rt.working_set().unwrap();

        let mut checker_index = base_index.fork();
        checker_index.insert_entry(Timestamp(BASE_TS + 1), domain, PlainCodomain(20));
        let checker = relation.begin_check_from_index(checker_index.as_ref());
        Self { checker, ws }
    }
}

fn check_conflict_merge_rewrite(ctx: &mut CheckMergeContext, chunk_size: usize, chunk_num: usize) {
    for i in 0..chunk_size {
        let tx = Tx {
            ts: Timestamp(BASE_TS + (chunk_num as u64 * chunk_size as u64) + i as u64),
            snapshot_version: 0,
        };
        let mut rt = ctx.relation.start_from_index(&tx, ctx.base_index.as_ref());
        rt.update(&ctx.domain, MergeCodomain(11)).unwrap();
        let mut ws = rt.working_set().unwrap();

        let mut checker_index = ctx.check_index.fork();
        checker_index.insert_entry(Timestamp(tx.ts.0 + 1), ctx.domain.clone(), MergeCodomain(20));
        let mut checker = ctx.relation.begin_check_from_index(checker_index.as_ref());
        checker.check(&mut ws).unwrap();
        black_box(ws.len());
    }
}

fn apply_mixed_batch(ctx: &mut ApplyContext, chunk_size: usize, chunk_num: usize) {
    for i in 0..chunk_size {
        let base = chunk_num as u64 * chunk_size as u64 * 16 + i as u64 * 16;
        let tx = Tx {
            ts: Timestamp(BASE_TS + base),
            snapshot_version: 0,
        };
        let mut rt = ctx.relation.start_from_index(&tx, ctx.base_index.as_ref());

        for j in 0..8 {
            let domain = Domain(base + j);
            rt.insert(domain, PlainCodomain(j)).unwrap();
        }
        for j in 0..4 {
            let domain = Domain(base + j);
            rt.delete(&domain).unwrap();
        }

        let ws = rt.working_set().unwrap();
        let mut checker = ctx.relation.begin_check_from_index(ctx.base_index.as_ref());
        checker.apply(ws).unwrap();
        black_box(checker.dirty());
    }
}

fn check_no_conflict_core(ctx: &mut CheckNoConflictCoreContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        black_box(ctx.checker.check(&mut ctx.ws).unwrap());
    }
}

fn check_conflict_identical_accept_core(
    ctx: &mut CheckConflictIdenticalCoreContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for _ in 0..chunk_size {
        black_box(ctx.checker.check(&mut ctx.ws).unwrap());
    }
}

fn check_conflict_unresolvable_fail_core(
    ctx: &mut CheckConflictUnresolvableCoreContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for _ in 0..chunk_size {
        black_box(ctx.checker.check(&mut ctx.ws).is_err());
    }
}

pub fn main() {
    use moor_bench_utils::{BenchmarkDef, generate_session_summary, run_benchmark_group};
    use std::env;

    #[cfg(target_os = "linux")]
    {
        use moor_bench_utils::perf_event::{Builder, events::Hardware};
        if Builder::new(Hardware::INSTRUCTIONS).build().is_err() {
            eprintln!(
                "⚠️  Perf events are not available on this system (insufficient permissions or kernel support)."
            );
            eprintln!("   Continuing with timing-only benchmarks (performance counters disabled).");
            eprintln!();
        }
    }

    let args: Vec<String> = env::args().collect();
    let filter = if let Some(separator_pos) = args.iter().position(|arg| arg == "--") {
        args.get(separator_pos + 1).map(|s| s.as_str())
    } else {
        args.iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--") && !args[0].contains(arg.as_str()))
            .map(|s| s.as_str())
    };

    if let Some(f) = filter {
        eprintln!("Running tx micro benchmarks matching filter: '{f}'");
        eprintln!("Available filters: all, check, apply, or benchmark name substring");
        eprintln!();
    }

    let check_no_conflict_core_benchmarks = [BenchmarkDef {
        name: "tx_check_no_conflict_core",
        group: "check",
        func: check_no_conflict_core,
    }];
    let check_identical_core_benchmarks = [BenchmarkDef {
        name: "tx_check_conflict_identical_accept_core",
        group: "check",
        func: check_conflict_identical_accept_core,
    }];
    let check_fail_core_benchmarks = [BenchmarkDef {
        name: "tx_check_conflict_unresolvable_fail_core",
        group: "check",
        func: check_conflict_unresolvable_fail_core,
    }];

    let check_merge_benchmarks = [BenchmarkDef {
        name: "tx_check_conflict_merge_rewrite",
        group: "check",
        func: check_conflict_merge_rewrite,
    }];

    let apply_benchmarks = [BenchmarkDef {
        name: "tx_apply_mixed_batch",
        group: "apply",
        func: apply_mixed_batch,
    }];

    run_benchmark_group::<CheckNoConflictCoreContext>(
        &check_no_conflict_core_benchmarks,
        "TX Check Benchmarks (Core No Conflict)",
        filter,
    );
    run_benchmark_group::<CheckConflictIdenticalCoreContext>(
        &check_identical_core_benchmarks,
        "TX Check Benchmarks (Core Identical Accept)",
        filter,
    );
    run_benchmark_group::<CheckMergeContext>(
        &check_merge_benchmarks,
        "TX Check Benchmarks (Merge, End-to-End)",
        filter,
    );
    run_benchmark_group::<CheckConflictUnresolvableCoreContext>(
        &check_fail_core_benchmarks,
        "TX Check Benchmarks (Core Fail)",
        filter,
    );
    run_benchmark_group::<ApplyContext>(&apply_benchmarks, "TX Apply Benchmarks", filter);

    if filter.is_some() {
        eprintln!("\nTX micro benchmark filtering complete.");
    }

    generate_session_summary();
}
