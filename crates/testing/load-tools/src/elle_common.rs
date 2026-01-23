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

//! Shared utilities for Elle consistency model checkers

use edn_format::{Keyword, Value};
use moor_common::model::{CommitResult, ObjAttrs, ObjectKind};
use moor_db::{Database, TxDB};
use moor_var::{Obj, Symbol};
use std::{
    collections::BTreeMap, fs::File, io::Write as _, path::PathBuf, sync::Arc, thread,
    time::Instant,
};

/// Setup a test database with an object and properties
pub fn setup_test_database<F>(
    db: &TxDB,
    num_props: usize,
    prop_prefix: &str,
    init_value_fn: F,
) -> Result<(Obj, Vec<Symbol>), eyre::Error>
where
    F: Fn(usize) -> moor_var::Var,
{
    let mut loader = db.loader_client()?;

    // Create test object
    let obj_attrs = ObjAttrs::default();
    let obj = loader.create_object(ObjectKind::NextObjid, &obj_attrs)?;

    // Create properties
    let mut prop_symbols = vec![];
    for i in 0..num_props {
        let prop_name = format!("{prop_prefix}_{i}");
        let prop_sym = Symbol::mk(&prop_name);
        let init_value = init_value_fn(i);
        loader.define_property(
            &obj,
            &obj,
            prop_sym,
            &obj,
            Default::default(),
            Some(init_value),
        )?;
        prop_symbols.push(prop_sym);
    }

    match loader.commit()? {
        CommitResult::Success { .. } => Ok((obj, prop_symbols)),
        CommitResult::ConflictRetry { .. } => Err(eyre::eyre!("Conflict during setup")),
    }
}

/// An EDN event with timestamp and process ID
#[derive(Debug, Clone)]
pub struct EdnEvent {
    pub timestamp: Instant,
    pub process_id: usize,
    pub event_type: EventType,
    pub f: String,    // function name like "read", "write", "append"
    pub value: Value, // EDN value for the operation
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Invoke,
    Ok,
    Fail,
}

impl EdnEvent {
    pub fn invoke(timestamp: Instant, process_id: usize, f: String, value: Value) -> Self {
        Self {
            timestamp,
            process_id,
            event_type: EventType::Invoke,
            f,
            value,
        }
    }

    pub fn ok(timestamp: Instant, process_id: usize, f: String, value: Value) -> Self {
        Self {
            timestamp,
            process_id,
            event_type: EventType::Ok,
            f,
            value,
        }
    }

    pub fn fail(timestamp: Instant, process_id: usize, f: String, value: Value) -> Self {
        Self {
            timestamp,
            process_id,
            event_type: EventType::Fail,
            f,
            value,
        }
    }

    /// Convert to EDN map for output
    pub fn to_edn_map(&self, index: usize) -> BTreeMap<Value, Value> {
        let mut map = BTreeMap::new();
        let type_str = match self.event_type {
            EventType::Invoke => "invoke",
            EventType::Ok => "ok",
            EventType::Fail => "fail",
        };
        map.insert(
            Value::Keyword(Keyword::from_name("type")),
            Value::Keyword(Keyword::from_name(type_str)),
        );
        map.insert(
            Value::Keyword(Keyword::from_name("process")),
            Value::Integer(self.process_id as i64),
        );
        map.insert(
            Value::Keyword(Keyword::from_name("f")),
            Value::Keyword(Keyword::from_name(&self.f)),
        );
        map.insert(
            Value::Keyword(Keyword::from_name("value")),
            self.value.clone(),
        );
        map.insert(
            Value::Keyword(Keyword::from_name("index")),
            Value::Integer(index as i64),
        );
        map
    }
}

/// Write EDN history to file
pub fn write_edn_history(events: &[EdnEvent], output_path: &PathBuf) -> Result<(), eyre::Error> {
    let mut output_document = String::new();

    for (i, event) in events.iter().enumerate() {
        let map = event.to_edn_map(i);
        let edn_value = Value::Map(map);
        output_document.push_str(&format!("{}\n", edn_format::emit_str(&edn_value)));
    }

    let mut file = File::create(output_path)?;
    file.write_all(output_document.as_bytes())?;
    Ok(())
}

/// Run concurrent workloads and collect timestamped results
pub fn run_concurrent_workloads<F, R>(
    db: Arc<TxDB>,
    obj: Obj,
    props: Vec<Symbol>,
    num_workloads: usize,
    num_iterations: usize,
    workload_fn: F,
) -> Result<Vec<R>, eyre::Error>
where
    F: Fn(Arc<TxDB>, Obj, Vec<Symbol>, usize, usize) -> Result<Vec<R>, eyre::Error>
        + Send
        + Sync
        + Clone
        + 'static,
    R: Send + 'static,
{
    println!("Starting {num_workloads} concurrent workloads");

    let mut handles = vec![];
    for process_id in 0..num_workloads {
        let db = db.clone();
        let props = props.clone();
        let workload_fn = workload_fn.clone();

        let handle = thread::spawn(move || workload_fn(db, obj, props, process_id, num_iterations));
        handles.push(handle);
    }

    println!("\nWaiting for threads to complete...");
    let mut all_results = vec![];
    let mut completed = 0;
    for handle in handles {
        let results = handle.join().expect("Thread panicked")?;
        all_results.extend(results);
        completed += 1;
        if completed % 5 == 0 {
            println!("Collected results from {completed}/{num_workloads} threads");
        }
    }

    Ok(all_results)
}

/// Retry helper - executes a closure with automatic retry on ConflictRetry
pub fn with_retry<F, T>(max_retries: usize, mut f: F) -> Result<Option<T>, eyre::Error>
where
    F: FnMut() -> Result<T, eyre::Error>,
{
    for _ in 0..max_retries {
        match f() {
            Ok(result) => return Ok(Some(result)),
            Err(e) => {
                if e.to_string().contains("ConflictRetry") {
                    continue;
                }
                return Err(e);
            }
        }
    }
    Ok(None) // Max retries exceeded
}
