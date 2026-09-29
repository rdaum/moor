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

//! Replays an EDN list-append history against a list-valued relation entry per key.

use edn_format::{Keyword, ParserOptions, Value};
use std::path::Path;

#[derive(Debug, Clone)]
pub enum Type {
    /// Perform the operation in value (append, read, etc)
    Invoke,
    /// The operation is expected to be successful
    Ok,
    /// The operation should have yielded a conflict
    Fail,
}

#[derive(Debug, Clone)]
pub enum Operation {
    Append(usize, i32),
    Read(usize, Option<Vec<i32>>),
}

#[derive(Debug, Clone)]
pub struct Entry {
    index: usize,
    _time: i32,
    r#type: Type,
    process: usize,
    _f: String,
    operations: Vec<Operation>,
}

/// Given a `history.sim` generated EDN file, parse it into a list of `Entry` structs which
/// represent the operations to be performed.
pub fn parse_edn(path: &Path) -> Vec<Entry> {
    let mut ops = vec![];
    let file = std::fs::read_to_string(path).unwrap();
    let parser = edn_format::Parser::from_str(&file, ParserOptions::default());
    for x in parser {
        let x = x.unwrap();
        let Value::Map(m) = x else {
            panic!("expected value: {x:?}");
        };

        let index = m.get(&Value::Keyword(Keyword::from_name("index"))).unwrap();
        let Value::Integer(index) = index else {
            panic!("expected integer: {index:?}");
        };
        let index = *index as usize;

        let time = m.get(&Value::Keyword(Keyword::from_name("time"))).unwrap();
        let Value::Integer(time) = time else {
            panic!("expected integer: {time:?}");
        };
        let time = *time as i32;

        let r#type = m.get(&Value::Keyword(Keyword::from_name("type"))).unwrap();
        let r#type = match r#type {
            Value::Keyword(k) => match k.name() {
                "invoke" => Type::Invoke,
                "ok" => Type::Ok,
                "fail" => Type::Fail,
                _ => panic!("unexpected type: {k:?}"),
            },
            _ => panic!("expected keyword: {type:?}"),
        };

        let process = m
            .get(&Value::Keyword(Keyword::from_name("process")))
            .unwrap();
        let Value::Integer(process) = process else {
            panic!("expected integer: {process:?}");
        };
        let process = *process as usize;

        let value = m.get(&Value::Keyword(Keyword::from_name("value"))).unwrap();
        let Value::Vector(value) = value else {
            panic!("expected vector: {value:?}");
        };
        let mut operations = Vec::with_capacity(value.len());
        for op in value {
            let op = match op {
                Value::Vector(v) => v,
                _ => panic!("expected vector: {op:?}"),
            };
            let op = match &op[..] {
                [Value::Keyword(k), Value::Integer(i), Value::Integer(j)]
                    if k.name() == "append" =>
                {
                    Operation::Append(*i as usize, *j as i32)
                }
                [Value::Keyword(k), Value::Integer(i), Value::Nil] if k.name() == "r" => {
                    Operation::Read(*i as usize, None)
                }
                [Value::Keyword(k), Value::Integer(i), Value::Vector(v)] if k.name() == "r" => {
                    let v = v
                        .iter()
                        .map(|x| match x {
                            Value::Integer(i) => *i as i32,
                            _ => panic!("expected integer: {x:?}"),
                        })
                        .collect();
                    Operation::Read(*i as usize, Some(v))
                }
                _ => panic!("unexpected operation: {op:?}"),
            };
            operations.push(op);
        }
        let entry = Entry {
            index,
            _time: time,
            r#type,
            process,
            _f: "txn".to_string(),
            operations,
        };
        ops.push(entry);
    }

    ops
}

#[cfg(test)]
mod tests {
    use crate::{Entry, Operation, Type};
    use arc_swap::ArcSwap;
    use eyre::{bail, ensure};
    use moor_common::model::WorldStateError;
    use moor_db::{
        Error, Provider, Relation, RelationCodomain, RelationIndex, Timestamp, Tx, WorkingSet,
    };
    use moor_var::Symbol;
    use std::{
        collections::{HashMap, HashSet},
        path::Path,
        sync::{Arc, Mutex},
    };

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestDomain(usize);

    impl std::fmt::Display for TestDomain {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TestDomain({})", self.0)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestCodomain(Vec<i32>);
    impl RelationCodomain for TestCodomain {}

    #[derive(Clone)]
    struct TestProvider {
        data: Arc<Mutex<HashMap<TestDomain, TestCodomain>>>,
    }

    impl Provider<TestDomain, TestCodomain> for TestProvider {
        fn get(&self, domain: &TestDomain) -> Result<Option<(Timestamp, TestCodomain)>, Error> {
            let data = self.data.lock().unwrap();
            if let Some(codomain) = data.get(domain) {
                Ok(Some((Timestamp(0), codomain.clone())))
            } else {
                Ok(None)
            }
        }

        fn put(
            &self,
            _timestamp: Timestamp,
            domain: &TestDomain,
            codomain: &TestCodomain,
        ) -> Result<(), Error> {
            let mut data = self.data.lock().unwrap();
            data.insert(domain.clone(), codomain.clone());
            Ok(())
        }

        fn del(&self, _timestamp: Timestamp, domain: &TestDomain) -> Result<(), Error> {
            let mut data = self.data.lock().unwrap();
            data.remove(domain);
            Ok(())
        }

        fn scan<F>(
            &self,
            predicate: &F,
        ) -> Result<Vec<(Timestamp, TestDomain, TestCodomain)>, Error>
        where
            F: Fn(&TestDomain, &TestCodomain) -> bool,
        {
            let data = self.data.lock().unwrap();
            Ok(data
                .iter()
                .filter(|(k, v)| predicate(k, v))
                .map(|(k, v)| (Timestamp(0), k.clone(), v.clone()))
                .collect())
        }

        fn stop(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    /// Persist a working set into an in-memory test provider, surfacing errors.
    fn persist_working_set(
        provider: &TestProvider,
        working_set: &WorkingSet<TestDomain, TestCodomain>,
    ) -> Result<(), Error> {
        for (write_ts, domain, value) in working_set.mutations() {
            match value {
                Some(value) => provider.put(write_ts, domain, value)?,
                None => provider.del(write_ts, domain)?,
            }
        }
        Ok(())
    }

    /// Replay a history against one list-valued relation entry per key. The fixture's
    /// expected reads and commit outcomes are specified independently of the relation.
    fn run_workload_check(workload: &[Entry]) -> Result<usize, eyre::Error> {
        let backing = HashMap::new();
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let backing_store = Arc::new(Relation::new(Symbol::mk("test"), provider));
        let root_index: Arc<ArcSwap<Box<dyn RelationIndex<TestDomain, TestCodomain>>>> =
            Arc::new(ArcSwap::new(Arc::new(
                backing_store
                    .seeded_index()
                    .map_err(|e| eyre::eyre!("seeded_index failed: {e:?}"))?,
            )));
        let mut transactions = HashMap::new();
        let mut tx_counter = 0;

        for (processed, entry) in workload.iter().enumerate() {
            ensure!(
                entry.index == processed,
                "history index mismatch at row {processed}"
            );
            match entry.r#type {
                Type::Invoke => {
                    tx_counter += 1;
                    let tx = Tx {
                        ts: Timestamp(tx_counter),
                        visible_ts: Timestamp(tx_counter),
                        snapshot_version: 0,
                    };
                    let snapshot = root_index.load();
                    let transaction = backing_store
                        .clone()
                        .start_from_index(&tx, snapshot.as_ref().as_ref());
                    ensure!(
                        transactions.insert(entry.process, transaction).is_none(),
                        "process {} already has an open transaction at index {}",
                        entry.process,
                        entry.index
                    );
                }
                Type::Ok | Type::Fail => {
                    let mut cache = transactions.remove(&entry.process).ok_or_else(|| {
                        eyre::eyre!("completion without invocation at index {}", entry.index)
                    })?;
                    for op in &entry.operations {
                        match op {
                            Operation::Append(key, value) => {
                                let key = TestDomain(*key);
                                let mut current = cache
                                    .get(&key)
                                    .map_err(|e| {
                                        eyre::eyre!("read at index {}: {e:?}", entry.index)
                                    })?
                                    .unwrap_or(TestCodomain(vec![]));
                                current.0.push(*value);
                                cache.upsert(key, current).map_err(|e| {
                                    eyre::eyre!("append at index {}: {e:?}", entry.index)
                                })?;
                            }
                            Operation::Read(key, expected) => {
                                let actual = cache
                                    .get(&TestDomain(*key))
                                    .map_err(|e| {
                                        eyre::eyre!("read at index {}: {e:?}", entry.index)
                                    })?
                                    .map(|value| value.0);
                                ensure!(
                                    actual == *expected,
                                    "read mismatch at index {} for key {}: expected {:?}, got {:?}",
                                    entry.index,
                                    key,
                                    expected,
                                    actual
                                );
                            }
                        }
                    }
                    let mut ws = match cache.working_set() {
                        Ok(ws) => ws,
                        Err(WorldStateError::RollbackRetry)
                            if matches!(entry.r#type, Type::Fail) =>
                        {
                            continue;
                        }
                        Err(e) => bail!("working set at index {}: {e:?}", entry.index),
                    };
                    let snapshot = root_index.load();
                    let mut cr = backing_store.begin_check_from_index(snapshot.as_ref().as_ref());
                    match (entry.r#type.clone(), cr.check(&mut ws)) {
                        (Type::Ok, Ok(())) => {
                            cr.prepare_indexes(&ws);
                            persist_working_set(backing_store.provider(), &ws).map_err(|e| {
                                eyre::eyre!("persist at index {}: {e:?}", entry.index)
                            })?;
                            cr.commit(&root_index);
                        }
                        (Type::Fail, Err(Error::Conflict(_))) => {}
                        (Type::Fail, Ok(())) => {
                            bail!(
                                "expected conflict at index {}, but check succeeded",
                                entry.index
                            )
                        }
                        (_, Err(e)) => {
                            bail!("unexpected check result at index {}: {e:?}", entry.index)
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        ensure!(
            transactions.is_empty(),
            "history ended with open transactions"
        );
        Ok(workload.len())
    }

    /// The external SI fixture supplies requests and client-call timing. Its recorded
    /// responses came from a different snapshot schedule, so this replay uses an
    /// independent model for a fixed snapshot-at-invocation schedule.
    fn replay_full_request_workload(workload: &[Entry]) -> Result<usize, eyre::Error> {
        struct ModelTransaction {
            values: HashMap<usize, Vec<i32>>,
            base_versions: HashMap<usize, usize>,
            writes: HashSet<usize>,
            request: Vec<Operation>,
        }

        let provider = Arc::new(TestProvider {
            data: Arc::new(Mutex::new(HashMap::new())),
        });
        let relation = Arc::new(Relation::new(Symbol::mk("full_history"), provider));
        let root_index: Arc<ArcSwap<Box<dyn RelationIndex<TestDomain, TestCodomain>>>> =
            Arc::new(ArcSwap::new(Arc::new(relation.seeded_index()?)));
        let mut active = HashMap::new();
        let mut values: HashMap<usize, Vec<i32>> = HashMap::new();
        let mut versions: HashMap<usize, usize> = HashMap::new();
        let mut timestamp = 0;
        let mut commits = 0;
        let mut conflicts = 0;
        let mut reads = 0;

        for (row, entry) in workload.iter().enumerate() {
            ensure!(entry.index == row, "history index mismatch at row {row}");
            match entry.r#type {
                Type::Invoke => {
                    timestamp += 1;
                    let tx = Tx {
                        ts: Timestamp(timestamp),
                        visible_ts: Timestamp(timestamp),
                        snapshot_version: 0,
                    };
                    let root = root_index.load();
                    let mut cache = relation
                        .clone()
                        .start_from_index(&tx, root.as_ref().as_ref());
                    let mut model = ModelTransaction {
                        values: HashMap::new(),
                        base_versions: HashMap::new(),
                        writes: HashSet::new(),
                        request: entry.operations.clone(),
                    };
                    for operation in &entry.operations {
                        let key = match operation {
                            Operation::Append(key, _) | Operation::Read(key, _) => *key,
                        };
                        if let std::collections::hash_map::Entry::Vacant(version) =
                            model.base_versions.entry(key)
                        {
                            version.insert(*versions.get(&key).unwrap_or(&0));
                            if let Some(value) = values.get(&key) {
                                model.values.insert(key, value.clone());
                            }
                        }
                        match operation {
                            Operation::Read(_, _) => {
                                reads += 1;
                                let expected = model.values.get(&key).cloned();
                                let actual = cache.get(&TestDomain(key))?.map(|v| v.0);
                                ensure!(
                                    actual == expected,
                                    "modeled read mismatch at row {row}, key {key}: expected {expected:?}, got {actual:?}"
                                );
                            }
                            Operation::Append(_, value) => {
                                let mut actual = cache
                                    .get(&TestDomain(key))?
                                    .unwrap_or(TestCodomain(Vec::new()));
                                actual.0.push(*value);
                                cache.upsert(TestDomain(key), actual)?;
                                model.values.entry(key).or_default().push(*value);
                                model.writes.insert(key);
                            }
                        }
                    }
                    ensure!(
                        active.insert(entry.process, (cache, model)).is_none(),
                        "process {} already active at row {row}",
                        entry.process
                    );
                }
                Type::Ok | Type::Fail => {
                    let (cache, model) = active
                        .remove(&entry.process)
                        .ok_or_else(|| eyre::eyre!("completion without invocation at row {row}"))?;
                    ensure!(
                        model.request.len() == entry.operations.len()
                            && model.request.iter().zip(&entry.operations).all(
                                |(invoked, completed)| match (invoked, completed) {
                                    (Operation::Append(a, x), Operation::Append(b, y)) => {
                                        a == b && x == y
                                    }
                                    (Operation::Read(a, _), Operation::Read(b, _)) => a == b,
                                    _ => false,
                                }
                            ),
                        "completion request differs from invocation at row {row}"
                    );
                    let should_conflict = model.writes.iter().any(|key| {
                        versions.get(key).copied().unwrap_or(0)
                            != model.base_versions.get(key).copied().unwrap_or(0)
                    });
                    let mut working_set = cache.working_set()?;
                    let root = root_index.load();
                    let mut checker = relation.begin_check_from_index(root.as_ref().as_ref());
                    let check = checker.check(&mut working_set);
                    if should_conflict {
                        ensure!(
                            matches!(check, Err(Error::Conflict(_))),
                            "expected modeled write conflict at row {row}, got {check:?}"
                        );
                        conflicts += 1;
                        continue;
                    }
                    check.map_err(|error| eyre::eyre!("check at row {row}: {error:?}"))?;
                    checker.prepare_indexes(&working_set);
                    persist_working_set(relation.provider(), &working_set)?;
                    checker.commit(&root_index);
                    commits += 1;
                    for key in model.writes {
                        values.insert(key, model.values[&key].clone());
                        versions.insert(key, commits);
                    }
                }
            }
        }

        ensure!(active.is_empty(), "history ended with open transactions");
        let root = root_index.load();
        let tx = Tx {
            ts: Timestamp(timestamp + 1),
            visible_ts: Timestamp(timestamp + 1),
            snapshot_version: 0,
        };
        let final_cache = relation.start_from_index(&tx, root.as_ref().as_ref());
        let final_values: HashMap<_, _> = final_cache
            .scan(&|_, _| true)?
            .into_iter()
            .map(|(key, value)| (key.0, value.0))
            .collect();
        ensure!(
            final_values == values,
            "final relation differs from modeled state"
        );
        ensure!(commits > 0 && conflicts > 0 && reads > 0);
        Ok(workload.len())
    }

    #[test]
    fn test_replay_relation_history_checks_every_completion() {
        let history = super::parse_edn(Path::new("tests/relation-list-history.edn"));
        assert_eq!(run_workload_check(&history).unwrap(), history.len());

        let mut corrupted = history;
        corrupted[7].operations[1] = Operation::Read(2, Some(vec![99]));
        let error = run_workload_check(&corrupted).unwrap_err();
        assert!(
            error.to_string().contains("read mismatch at index 7"),
            "{error:?}"
        );
    }

    #[test]
    fn test_full_simulated_request_workloads_against_relation_model() {
        for fixture in [
            "tests/si-list-append-dataset.edn",
            "tests/ssi-list-append-dataset.edn",
        ] {
            let history = super::parse_edn(Path::new(fixture));
            assert_eq!(history.len(), 20_000, "{fixture}");
            assert_eq!(
                replay_full_request_workload(&history).unwrap(),
                history.len(),
                "{fixture}"
            );
        }
    }
}
