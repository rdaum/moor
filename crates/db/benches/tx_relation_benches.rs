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

use micromeasure::{
    BenchContext, BenchmarkMainOptions, BenchmarkRuntimeOptions, Throughput, benchmark_main,
    black_box,
};
use moor_db::{
    CheckRelation, Error, Relation, RelationCodomain, RelationIndex, RelationTransaction,
    Timestamp, Tx, WorkingSet,
};
use moor_var::Symbol;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

const CHECK_CHUNK_SIZE: Option<usize> = None;
const TX_OPS_CHUNK_SIZE: Option<usize> = Some(16384);
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
struct ConflictCodomain(u64);

impl RelationCodomain for ConflictCodomain {}

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

impl<C> InMemoryProvider<C>
where
    C: RelationCodomain,
{
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
}

struct CheckConflictContext {
    relation: Relation<Domain, ConflictCodomain>,
    base_index: Box<dyn RelationIndex<Domain, ConflictCodomain>>,
    check_index: Box<dyn RelationIndex<Domain, ConflictCodomain>>,
    domain: Domain,
}

impl BenchContext for CheckConflictContext {
    fn prepare(_num_chunks: usize) -> Self {
        let domain = Domain(1);
        let mut data = HashMap::new();
        data.insert(domain.clone(), ConflictCodomain(10));
        let provider = Arc::new(InMemoryProvider::new(data));
        let relation = Relation::new(Symbol::mk("tx-check-conflict"));
        let base_index = relation
            .seeded_index(provider.scan(&|_, _| true).unwrap().into_iter().map(Ok))
            .map(|(index, _)| index)
            .unwrap();
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
    sink: Arc<InMemoryProvider<PlainCodomain>>,
    relation: Relation<Domain, PlainCodomain>,
    base_index: Box<dyn RelationIndex<Domain, PlainCodomain>>,
}

impl BenchContext for ApplyContext {
    fn prepare(_num_chunks: usize) -> Self {
        let provider = Arc::new(InMemoryProvider::new(HashMap::new()));
        let relation = Relation::new(Symbol::mk("tx-apply"));
        let base_index = relation
            .seeded_index(provider.scan(&|_, _| true).unwrap().into_iter().map(Ok))
            .map(|(index, _)| index)
            .unwrap();
        Self {
            sink: provider,
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
    checker: CheckRelation<Domain, PlainCodomain>,
    ws: WorkingSet<Domain, PlainCodomain>,
}

impl BenchContext for CheckNoConflictCoreContext {
    fn prepare(_num_chunks: usize) -> Self {
        let domain = Domain(1);
        let mut data = HashMap::new();
        data.insert(domain.clone(), PlainCodomain(10));
        let provider = Arc::new(InMemoryProvider::new(data));
        let relation = Relation::new(Symbol::mk("tx-check-core-no-conflict"));
        let base_index = relation
            .seeded_index(provider.scan(&|_, _| true).unwrap().into_iter().map(Ok))
            .map(|(index, _)| index)
            .unwrap();
        let tx = Tx {
            ts: Timestamp(BASE_TS),
            visible_ts: Timestamp(BASE_TS),
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
    checker: CheckRelation<Domain, PlainCodomain>,
    ws: WorkingSet<Domain, PlainCodomain>,
}

impl BenchContext for CheckConflictIdenticalCoreContext {
    fn prepare(_num_chunks: usize) -> Self {
        let domain = Domain(1);
        let mut data = HashMap::new();
        data.insert(domain.clone(), PlainCodomain(10));
        let provider = Arc::new(InMemoryProvider::new(data));
        let relation = Relation::new(Symbol::mk("tx-check-core-identical"));
        let base_index = relation
            .seeded_index(provider.scan(&|_, _| true).unwrap().into_iter().map(Ok))
            .map(|(index, _)| index)
            .unwrap();
        let tx = Tx {
            ts: Timestamp(BASE_TS),
            visible_ts: Timestamp(BASE_TS),
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
    checker: CheckRelation<Domain, PlainCodomain>,
    ws: WorkingSet<Domain, PlainCodomain>,
}

impl BenchContext for CheckConflictUnresolvableCoreContext {
    fn prepare(_num_chunks: usize) -> Self {
        let domain = Domain(1);
        let mut data = HashMap::new();
        data.insert(domain.clone(), PlainCodomain(10));
        let provider = Arc::new(InMemoryProvider::new(data));
        let relation = Relation::new(Symbol::mk("tx-check-core-fail"));
        let base_index = relation
            .seeded_index(provider.scan(&|_, _| true).unwrap().into_iter().map(Ok))
            .map(|(index, _)| index)
            .unwrap();
        let tx = Tx {
            ts: Timestamp(BASE_TS),
            visible_ts: Timestamp(BASE_TS),
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

struct TxOpsContext {
    get_hit_master_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    get_miss_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    get_local_update_hit_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    get_local_delete_miss_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    update_hit_master_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    update_miss_none_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    update_local_insert_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    update_local_delete_none_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    upsert_hit_master_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    upsert_miss_insert_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    upsert_local_delete_to_update_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    upsert_local_delete_to_insert_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    delete_hit_master_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    delete_miss_none_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    delete_local_insert_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    delete_local_delete_none_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    insert_miss_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    insert_duplicate_err_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    insert_after_local_delete_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    bulk_get_master_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    bulk_get_local_mixed_txs: Vec<RelationTransaction<Domain, PlainCodomain>>,
    present_domain: Domain,
    missing_domain: Domain,
    resurrect_domain: Domain,
    bulk_get_domains: Vec<Domain>,
    bulk_get_mixed_domains: Vec<Domain>,
}

impl BenchContext for TxOpsContext {
    fn prepare(preferred_chunk_size: usize) -> Self {
        let present_domain = Domain(1);
        let missing_domain = Domain(2);
        let resurrect_domain = Domain(3);
        let local_mut_domain = Domain(4);

        let mut data = HashMap::new();
        data.insert(present_domain.clone(), PlainCodomain(10));
        data.insert(resurrect_domain.clone(), PlainCodomain(20));
        data.insert(local_mut_domain.clone(), PlainCodomain(30));
        for i in 0..16 {
            data.insert(Domain(100 + i), PlainCodomain(i));
        }

        let provider = Arc::new(InMemoryProvider::new(data));
        let relation = Relation::new(Symbol::mk("tx-ops"));
        let base_index = relation
            .seeded_index(provider.scan(&|_, _| true).unwrap().into_iter().map(Ok))
            .map(|(index, _)| index)
            .unwrap();
        let bulk_get_domains: Vec<_> = (0..16).map(|i| Domain(100 + i)).collect();
        let chunk_size = TX_OPS_CHUNK_SIZE.unwrap_or(preferred_chunk_size).max(1);

        let mut get_hit_master_txs = Vec::with_capacity(chunk_size);
        let mut get_miss_txs = Vec::with_capacity(chunk_size);
        let mut get_local_update_hit_txs = Vec::with_capacity(chunk_size);
        let mut get_local_delete_miss_txs = Vec::with_capacity(chunk_size);
        let mut update_hit_master_txs = Vec::with_capacity(chunk_size);
        let mut update_miss_none_txs = Vec::with_capacity(chunk_size);
        let mut update_local_insert_txs = Vec::with_capacity(chunk_size);
        let mut update_local_delete_none_txs = Vec::with_capacity(chunk_size);
        let mut upsert_hit_master_txs = Vec::with_capacity(chunk_size);
        let mut upsert_miss_insert_txs = Vec::with_capacity(chunk_size);
        let mut upsert_local_delete_to_update_txs = Vec::with_capacity(chunk_size);
        let mut upsert_local_delete_to_insert_txs = Vec::with_capacity(chunk_size);
        let mut delete_hit_master_txs = Vec::with_capacity(chunk_size);
        let mut delete_miss_none_txs = Vec::with_capacity(chunk_size);
        let mut delete_local_insert_txs = Vec::with_capacity(chunk_size);
        let mut delete_local_delete_none_txs = Vec::with_capacity(chunk_size);
        let mut insert_miss_txs = Vec::with_capacity(chunk_size);
        let mut insert_duplicate_err_txs = Vec::with_capacity(chunk_size);
        let mut insert_after_local_delete_txs = Vec::with_capacity(chunk_size);
        let mut bulk_get_master_txs = Vec::with_capacity(chunk_size);
        let mut bulk_get_local_mixed_txs = Vec::with_capacity(chunk_size);
        let mut bulk_get_mixed_domains = bulk_get_domains.clone();
        bulk_get_mixed_domains.push(missing_domain.clone());
        bulk_get_mixed_domains.push(local_mut_domain.clone());

        for i in 0..chunk_size {
            let tx = Tx {
                ts: Timestamp(BASE_TS + i as u64),
                visible_ts: Timestamp(BASE_TS + i as u64),
                snapshot_version: 0,
            };

            get_hit_master_txs.push(relation.start_from_index(&tx, base_index.as_ref()));
            get_miss_txs.push(relation.start_from_index(&tx, base_index.as_ref()));

            let mut get_local_update_hit = relation.start_from_index(&tx, base_index.as_ref());
            get_local_update_hit
                .update(&present_domain, PlainCodomain(900))
                .unwrap();
            get_local_update_hit_txs.push(get_local_update_hit);

            let mut get_local_delete_miss = relation.start_from_index(&tx, base_index.as_ref());
            get_local_delete_miss.delete(&present_domain).unwrap();
            get_local_delete_miss_txs.push(get_local_delete_miss);

            update_hit_master_txs.push(relation.start_from_index(&tx, base_index.as_ref()));
            update_miss_none_txs.push(relation.start_from_index(&tx, base_index.as_ref()));
            upsert_hit_master_txs.push(relation.start_from_index(&tx, base_index.as_ref()));
            upsert_miss_insert_txs.push(relation.start_from_index(&tx, base_index.as_ref()));

            let mut update_local_insert = relation.start_from_index(&tx, base_index.as_ref());
            update_local_insert
                .insert(missing_domain.clone(), PlainCodomain(700))
                .unwrap();
            update_local_insert_txs.push(update_local_insert);

            let mut update_local_delete_none = relation.start_from_index(&tx, base_index.as_ref());
            update_local_delete_none.delete(&present_domain).unwrap();
            update_local_delete_none_txs.push(update_local_delete_none);
            let mut upsert_local_delete_to_update =
                relation.start_from_index(&tx, base_index.as_ref());
            upsert_local_delete_to_update
                .delete(&present_domain)
                .unwrap();
            upsert_local_delete_to_update_txs.push(upsert_local_delete_to_update);
            let mut upsert_local_delete_to_insert =
                relation.start_from_index(&tx, base_index.as_ref());
            upsert_local_delete_to_insert
                .insert(missing_domain.clone(), PlainCodomain(711))
                .unwrap();
            upsert_local_delete_to_insert
                .delete(&missing_domain)
                .unwrap();
            upsert_local_delete_to_insert_txs.push(upsert_local_delete_to_insert);

            delete_hit_master_txs.push(relation.start_from_index(&tx, base_index.as_ref()));
            delete_miss_none_txs.push(relation.start_from_index(&tx, base_index.as_ref()));

            let mut delete_local_insert = relation.start_from_index(&tx, base_index.as_ref());
            delete_local_insert
                .insert(missing_domain.clone(), PlainCodomain(710))
                .unwrap();
            delete_local_insert_txs.push(delete_local_insert);

            let mut delete_local_delete_none = relation.start_from_index(&tx, base_index.as_ref());
            delete_local_delete_none.delete(&present_domain).unwrap();
            delete_local_delete_none_txs.push(delete_local_delete_none);

            insert_miss_txs.push(relation.start_from_index(&tx, base_index.as_ref()));
            insert_duplicate_err_txs.push(relation.start_from_index(&tx, base_index.as_ref()));

            let mut insert_after_local_delete = relation.start_from_index(&tx, base_index.as_ref());
            insert_after_local_delete.delete(&resurrect_domain).unwrap();
            insert_after_local_delete_txs.push(insert_after_local_delete);

            bulk_get_master_txs.push(relation.start_from_index(&tx, base_index.as_ref()));
            let mut bulk_get_local_mixed = relation.start_from_index(&tx, base_index.as_ref());
            bulk_get_local_mixed
                .update(&local_mut_domain, PlainCodomain(901))
                .unwrap();
            bulk_get_local_mixed.delete(&Domain(100)).unwrap();
            bulk_get_local_mixed
                .insert(missing_domain.clone(), PlainCodomain(902))
                .unwrap();
            bulk_get_local_mixed_txs.push(bulk_get_local_mixed);
        }

        Self {
            get_hit_master_txs,
            get_miss_txs,
            get_local_update_hit_txs,
            get_local_delete_miss_txs,
            update_hit_master_txs,
            update_miss_none_txs,
            update_local_insert_txs,
            update_local_delete_none_txs,
            upsert_hit_master_txs,
            upsert_miss_insert_txs,
            upsert_local_delete_to_update_txs,
            upsert_local_delete_to_insert_txs,
            delete_hit_master_txs,
            delete_miss_none_txs,
            delete_local_insert_txs,
            delete_local_delete_none_txs,
            insert_miss_txs,
            insert_duplicate_err_txs,
            insert_after_local_delete_txs,
            bulk_get_master_txs,
            bulk_get_local_mixed_txs,
            present_domain,
            missing_domain,
            resurrect_domain,
            bulk_get_domains,
            bulk_get_mixed_domains,
        }
    }

    fn chunk_size() -> Option<usize> {
        TX_OPS_CHUNK_SIZE
    }
}

fn check_conflict_rejection(ctx: &mut CheckConflictContext, chunk_size: usize, chunk_num: usize) {
    for i in 0..chunk_size {
        let ts = Timestamp(BASE_TS + (chunk_num as u64 * chunk_size as u64) + i as u64);
        let tx = Tx {
            ts,
            visible_ts: ts,
            snapshot_version: 0,
        };
        let mut rt = ctx.relation.start_from_index(&tx, ctx.base_index.as_ref());
        rt.update(&ctx.domain, ConflictCodomain(11)).unwrap();
        let mut ws = rt.working_set().unwrap();

        let mut checker_index = ctx.check_index.fork();
        checker_index.insert_entry(
            Timestamp(tx.ts.0 + 1),
            ctx.domain.clone(),
            ConflictCodomain(20),
        );
        let mut checker = ctx.relation.begin_check_from_index(checker_index.as_ref());
        assert!(matches!(checker.check(&mut ws), Err(Error::Conflict(_))));
        black_box(ws.len());
    }
}

fn apply_mixed_batch(ctx: &mut ApplyContext, chunk_size: usize, chunk_num: usize) {
    for i in 0..chunk_size {
        let base = chunk_num as u64 * chunk_size as u64 * 16 + i as u64 * 16;
        let tx = Tx {
            ts: Timestamp(BASE_TS + base),
            visible_ts: Timestamp(BASE_TS + base),
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
        checker.prepare_indexes(&ws);
        let provider = ctx.sink.clone();
        for (write_ts, domain, value) in ws.mutations() {
            match value {
                Some(value) => provider.put(write_ts, domain, value).unwrap(),
                None => provider.del(write_ts, domain).unwrap(),
            }
        }
        black_box(checker.dirty());
    }
}

fn check_no_conflict_core(
    ctx: &mut CheckNoConflictCoreContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for _ in 0..chunk_size {
        ctx.checker.check(&mut ctx.ws).unwrap();
        black_box(());
    }
}

fn check_conflict_identical_reject_core(
    ctx: &mut CheckConflictIdenticalCoreContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for _ in 0..chunk_size {
        assert!(matches!(
            ctx.checker.check(&mut ctx.ws),
            Err(Error::Conflict(_))
        ));
        black_box(());
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

fn tx_op_get_hit_master(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(ctx.get_hit_master_txs[i].get(&ctx.present_domain).unwrap());
    }
}

fn tx_op_get_miss(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(ctx.get_miss_txs[i].get(&ctx.missing_domain).unwrap());
    }
}

fn tx_op_get_hit_local_update(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.get_local_update_hit_txs[i]
                .get(&ctx.present_domain)
                .unwrap(),
        );
    }
}

fn tx_op_get_miss_local_delete(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.get_local_delete_miss_txs[i]
                .get(&ctx.present_domain)
                .unwrap(),
        );
    }
}

fn tx_op_update_hit_master(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.update_hit_master_txs[i]
                .update(&ctx.present_domain, PlainCodomain(100 + (i as u64)))
                .unwrap(),
        );
    }
}

fn tx_op_update_miss_none(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let tx = black_box(&mut ctx.update_miss_none_txs[i]);
        let domain = black_box(&ctx.missing_domain);
        let value = black_box(PlainCodomain(100 + (i as u64)));
        let result = tx.update(domain, value).unwrap();
        black_box(result.is_none());
    }
}

fn tx_op_update_local_insert(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.update_local_insert_txs[i]
                .update(&ctx.missing_domain, PlainCodomain(300 + (i as u64)))
                .unwrap(),
        );
    }
}

fn tx_op_update_local_delete_none(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.update_local_delete_none_txs[i]
                .update(&ctx.present_domain, PlainCodomain(400 + (i as u64)))
                .unwrap(),
        );
    }
}

fn tx_op_upsert_hit_master(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.upsert_hit_master_txs[i]
                .upsert(ctx.present_domain.clone(), PlainCodomain(450 + (i as u64)))
                .unwrap(),
        );
    }
}

fn tx_op_upsert_miss_insert(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.upsert_miss_insert_txs[i]
                .upsert(ctx.missing_domain.clone(), PlainCodomain(460 + (i as u64)))
                .unwrap(),
        );
    }
}

fn tx_op_upsert_local_delete_to_update(
    ctx: &mut TxOpsContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        black_box(
            ctx.upsert_local_delete_to_update_txs[i]
                .upsert(ctx.present_domain.clone(), PlainCodomain(470 + (i as u64)))
                .unwrap(),
        );
    }
}

fn tx_op_upsert_local_delete_to_insert(
    ctx: &mut TxOpsContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        black_box(
            ctx.upsert_local_delete_to_insert_txs[i]
                .upsert(ctx.missing_domain.clone(), PlainCodomain(480 + (i as u64)))
                .unwrap(),
        );
    }
}

fn tx_op_delete_hit_master(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.delete_hit_master_txs[i]
                .delete(&ctx.present_domain)
                .unwrap(),
        );
    }
}

fn tx_op_delete_miss_none(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.delete_miss_none_txs[i]
                .delete(&ctx.missing_domain)
                .unwrap(),
        );
    }
}

fn tx_op_delete_local_insert(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.delete_local_insert_txs[i]
                .delete(&ctx.missing_domain)
                .unwrap(),
        );
    }
}

fn tx_op_delete_local_delete_none(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.delete_local_delete_none_txs[i]
                .delete(&ctx.present_domain)
                .unwrap(),
        );
    }
}

fn tx_op_insert_miss(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        ctx.insert_miss_txs[i]
            .insert(ctx.missing_domain.clone(), PlainCodomain(500 + (i as u64)))
            .unwrap();
        black_box(());
    }
}

fn tx_op_insert_duplicate_err(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.insert_duplicate_err_txs[i]
                .insert(ctx.present_domain.clone(), PlainCodomain(600 + (i as u64)))
                .is_err(),
        );
    }
}

fn tx_op_insert_after_local_delete(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.insert_after_local_delete_txs[i]
                .insert(
                    ctx.resurrect_domain.clone(),
                    PlainCodomain(700 + (i as u64)),
                )
                .is_err(),
        );
    }
}

fn tx_op_bulk_get_master(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.bulk_get_master_txs[i]
                .bulk_get(&ctx.bulk_get_domains)
                .unwrap(),
        );
    }
}

fn tx_op_bulk_get_local_mixed(ctx: &mut TxOpsContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        black_box(
            ctx.bulk_get_local_mixed_txs[i]
                .bulk_get(&ctx.bulk_get_mixed_domains)
                .unwrap(),
        );
    }
}

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some("all, check, tx_ops, apply, or benchmark name substring".to_string()),
        runtime: BenchmarkRuntimeOptions {
            warm_up_duration: Duration::from_millis(250),
            benchmark_duration: Duration::from_secs(1),
            min_samples: 8,
            max_samples: 24,
        },
        ..BenchmarkMainOptions::default()
    },
    |runner| {
        runner.group::<CheckNoConflictCoreContext>("TX Check Benchmarks (Core No Conflict)", |g| {
            g.throughput(Throughput::per_operation(1, "rows_checked"))
                .bench("tx_check_no_conflict_core", check_no_conflict_core);
        });

        runner.group::<CheckConflictIdenticalCoreContext>(
            "TX Check Benchmarks (Core Identical Reject)",
            |g| {
                g.throughput(Throughput::per_operation(1, "rows_checked"))
                    .bench(
                        "tx_check_conflict_identical_reject_core",
                        check_conflict_identical_reject_core,
                    );
            },
        );

        runner.group::<CheckConflictContext>("TX Check Benchmarks (Conflict, End-to-End)", |g| {
            g.throughput(Throughput::per_operation(1, "rows_checked"))
                .bench("tx_check_conflict_rejection", check_conflict_rejection);
        });

        runner.group::<CheckConflictUnresolvableCoreContext>(
            "TX Check Benchmarks (Core Fail)",
            |g| {
                g.throughput(Throughput::per_operation(1, "rows_checked"))
                    .bench(
                        "tx_check_conflict_unresolvable_fail_core",
                        check_conflict_unresolvable_fail_core,
                    );
            },
        );

        runner.group::<TxOpsContext>("TX Operation Benchmarks", |g| {
            let g = g.throughput(Throughput::per_operation(1, "relation_ops"));
            g.bench("tx_op_get_hit_master", tx_op_get_hit_master);
            g.bench("tx_op_get_miss", tx_op_get_miss);
            g.bench("tx_op_get_hit_local_update", tx_op_get_hit_local_update);
            g.bench("tx_op_get_miss_local_delete", tx_op_get_miss_local_delete);
            g.bench("tx_op_update_hit_master", tx_op_update_hit_master);
            g.bench("tx_op_update_miss_none", tx_op_update_miss_none);
            g.bench("tx_op_update_local_insert", tx_op_update_local_insert);
            g.bench(
                "tx_op_update_local_delete_none",
                tx_op_update_local_delete_none,
            );
            g.bench("tx_op_upsert_hit_master", tx_op_upsert_hit_master);
            g.bench("tx_op_upsert_miss_insert", tx_op_upsert_miss_insert);
            g.bench(
                "tx_op_upsert_local_delete_to_update",
                tx_op_upsert_local_delete_to_update,
            );
            g.bench(
                "tx_op_upsert_local_delete_to_insert",
                tx_op_upsert_local_delete_to_insert,
            );
            g.bench("tx_op_delete_hit_master", tx_op_delete_hit_master);
            g.bench("tx_op_delete_miss_none", tx_op_delete_miss_none);
            g.bench("tx_op_delete_local_insert", tx_op_delete_local_insert);
            g.bench(
                "tx_op_delete_local_delete_none",
                tx_op_delete_local_delete_none,
            );
            g.bench("tx_op_insert_miss", tx_op_insert_miss);
            g.bench("tx_op_insert_duplicate_err", tx_op_insert_duplicate_err);
            g.bench(
                "tx_op_insert_after_local_delete",
                tx_op_insert_after_local_delete,
            );
            g.bench("tx_op_bulk_get_master", tx_op_bulk_get_master);
            g.bench("tx_op_bulk_get_local_mixed", tx_op_bulk_get_local_mixed);
        });

        runner.group::<ApplyContext>("TX Apply Benchmarks", |g| {
            g.throughput(Throughput::per_operation(1, "tuple_ops"))
                .bench("tx_apply_mixed_batch", apply_mixed_batch);
        });
    }
);
