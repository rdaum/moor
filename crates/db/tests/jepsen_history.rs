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

//! Loads an EDN file containing a history from the jepsen project `history.sim` tool, and run the
//! history against our database implementation, and verify the right results come out.

use edn_format::{Keyword, ParserOptions, Value};
use std::path::Path;

#[derive(Debug)]
pub enum Type {
    /// Perform the operation in value (append, read, etc)
    Invoke,
    /// The operation is expected to be successful
    Ok,
    /// The operation should have yielded a conflict
    Fail,
}

#[derive(Debug)]
pub enum Operation {
    Append(usize, i32),
    Read(usize, Option<Vec<i32>>),
}

#[derive(Debug)]
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
            panic!("expected value: {:?}", x);
        };

        let index = m.get(&Value::Keyword(Keyword::from_name("index"))).unwrap();
        let Value::Integer(index) = index else {
            panic!("expected integer: {:?}", index);
        };
        let index = *index as usize;

        let time = m.get(&Value::Keyword(Keyword::from_name("time"))).unwrap();
        let Value::Integer(time) = time else {
            panic!("expected integer: {:?}", time);
        };
        let time = *time as i32;

        let r#type = m.get(&Value::Keyword(Keyword::from_name("type"))).unwrap();
        let r#type = match r#type {
            Value::Keyword(k) => match k.name() {
                "invoke" => Type::Invoke,
                "ok" => Type::Ok,
                "fail" => Type::Fail,
                _ => panic!("unexpected type: {:?}", k),
            },
            _ => panic!("expected keyword: {:?}", r#type),
        };

        let process = m
            .get(&Value::Keyword(Keyword::from_name("process")))
            .unwrap();
        let Value::Integer(process) = process else {
            panic!("expected integer: {:?}", process);
        };
        let process = *process as usize;

        let value = m.get(&Value::Keyword(Keyword::from_name("value"))).unwrap();
        let Value::Vector(value) = value else {
            panic!("expected vector: {:?}", value);
        };
        let mut operations = Vec::with_capacity(value.len());
        for op in value {
            let op = match op {
                Value::Vector(v) => v,
                _ => panic!("expected vector: {:?}", op),
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
                            _ => panic!("expected integer: {:?}", x),
                        })
                        .collect();
                    Operation::Read(*i as usize, Some(v))
                }
                _ => panic!("unexpected operation: {:?}", op),
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
    use crate::{Operation, Type};
    use eyre::bail;
    use moor_db::{Error, Provider, Timestamp, TransactionalCache, TransactionalTable, Tx};
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_parse_edn() {
        let ops = super::parse_edn(Path::new("tests/si-list-append-dataset.edn"));
        assert!(!ops.is_empty());
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestDomain(usize);

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestCodomain(Vec<i32>);

    struct TestProvider {
        data: Arc<Mutex<HashMap<TestDomain, TestCodomain>>>,
    }

    impl Provider<TestDomain, TestCodomain> for TestProvider {
        fn get(
            &self,
            domain: &TestDomain,
        ) -> Result<Option<(Timestamp, TestCodomain, usize)>, Error> {
            let data = self.data.lock().unwrap();
            if let Some(codomain) = data.get(domain) {
                Ok(Some((Timestamp(0), codomain.clone(), 8)))
            } else {
                Ok(None)
            }
        }

        fn put(
            &self,
            _timestamp: Timestamp,
            domain: TestDomain,
            codomain: TestCodomain,
        ) -> Result<(), Error> {
            let mut data = self.data.lock().unwrap();
            data.insert(domain, codomain);
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
        ) -> Result<Vec<(Timestamp, TestDomain, TestCodomain, usize)>, Error>
        where
            F: Fn(&TestDomain, &TestCodomain) -> bool,
        {
            let data = self.data.lock().unwrap();
            Ok(data
                .iter()
                .filter(|(k, v)| predicate(k, v))
                .map(|(k, v)| (Timestamp(0), k.clone(), v.clone(), 16))
                .collect())
        }
    }

    /// Given a workload, run it against our transaction implementation and verify the results.
    fn run_workload_check(path: &Path) -> Result<(), eyre::Error> {
        let mut workload = super::parse_edn(path);
        let backing = HashMap::new();
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let backing_store = Arc::new(TransactionalCache::new(provider.clone(), 1 << 16));

        let mut transactions = HashMap::new();

        // workload *must* be sorted by index
        workload.sort_by(|a, b| a.index.cmp(&b.index));

        let mut tx_counter = 0;
        for entry in &workload {
            match entry.r#type {
                Type::Invoke => {
                    tx_counter += 1;
                    let tx = Tx {
                        ts: Timestamp(tx_counter),
                    };
                    let cache = TransactionalTable::new(tx, backing_store.clone());
                    backing_store.clone().start(&tx);

                    if transactions.insert(entry.process, (tx, cache)).is_some() {
                        bail!("transaction already exists");
                    }
                }
                Type::Ok => {
                    // Get the working set for the transaction
                    let (_tx, mut cache) = transactions.remove(&entry.process).unwrap();

                    // Perform the operations.
                    for ops in &entry.operations {
                        match ops {
                            Operation::Append(key, value) => {
                                // Read, append, set
                                let key = TestDomain(*key);
                                let mut codomain =
                                    cache.get(&key).unwrap().unwrap_or(TestCodomain(vec![]));
                                codomain.0.push(*value);
                                cache
                                    .upsert(key, codomain)
                                    .map_err(|_| eyre::eyre!("append failed"))?;
                            }
                            Operation::Read(key, _) => {
                                // Reads happen but we don't check them until the transaction is
                                // committed. This is just to prime the cache and get the timestamps
                                // doing the timestamping.
                                let key = TestDomain(*key);
                                cache.get(&key).map_err(|_| eyre::eyre!("read failed"))?;
                            }
                        }
                    }

                    let ws = cache.working_set();

                    {
                        let lock = backing_store.lock();
                        let lock = backing_store
                            .check(lock, &ws)
                            .map_err(|_| eyre::eyre!("check failed in commit"))?;
                        let _l = backing_store
                            .apply(lock, ws)
                            .map_err(|_| eyre::eyre!("apply failed in commit"))?;
                    }
                }
                Type::Fail => {
                    let (_tx, cache) = transactions.remove(&entry.process).unwrap();

                    // Returns "false" if our _expected_ failure did not happen
                    let fail_check_fn = || {
                        for ops in &entry.operations {
                            match ops {
                                Operation::Read(key, expected) => {
                                    let key = TestDomain(*key);
                                    let codomain = cache.get(&key).unwrap().map(|x| x.0);
                                    if *expected != codomain {
                                        return Ok(());
                                    }
                                }
                                Operation::Append(key, value) => {
                                    let key = TestDomain(*key);
                                    let codomain =
                                        cache.get(&key).unwrap().unwrap_or(TestCodomain(vec![]));
                                    // The appended value should *not* be in there
                                    if !codomain.0.contains(value) {
                                        return Ok(());
                                    }
                                }
                            }
                        }
                        let ws = cache.working_set();
                        let lock = backing_store.lock();
                        let lock = match backing_store.check(lock, &ws) {
                            Err(Error::Conflict) => {
                                return Ok(());
                            }
                            Err(e) => panic!("unexpected error: {:?}", e),
                            Ok(lock) => lock,
                        };
                        match backing_store.apply(lock, ws) {
                            Ok(_) => bail!("Expected conflict, got none in {entry:?}"),
                            Err(Error::Conflict) => Ok(()),
                            Err(e) => panic!("unexpected error: {:?}", e),
                        }
                    };
                    return fail_check_fn();
                }
            }
        }
        Ok(())
    }
    #[test]
    fn test_run_serializable_workload() {
        // This is our "serializable" list append workload, generated by `jepsen` `history.sim`
        // Note that we also have a ssi- strict-serializable workload file that currently fails.
        run_workload_check(Path::new("tests/si-list-append-dataset.edn")).unwrap();
    }

    // This test is expected to fail, as we don't support strict-serializable transactions yet.
    #[test]
    #[ignore]
    fn test_run_strict_serializable_workload() {
        run_workload_check(Path::new("tests/ssi-list-append-dataset.edn")).unwrap();
    }
}
