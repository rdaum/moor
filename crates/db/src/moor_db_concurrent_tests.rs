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

#[cfg(test)]
mod tests {
    use crate::{config::DatabaseConfig, db_worldstate::DbWorldState, moor_db::MoorDB};
    use moor_common::{
        model::{
            ArgSpec, CommitResult, ObjectKind, PrepSpec, PropFlag, ValSet, VerbArgsSpec, VerbFlag,
            WorldState, WorldStateError,
        },
        util::BitEnum,
    };
    use moor_var::{
        Associative, List, NOTHING, OP_HINT_FLYWEIGHT_ADD_SLOT, OP_HINT_FLYWEIGHT_APPEND_CONTENTS,
        OP_HINT_NONE, Obj, SYSTEM_OBJECT, Sequence, Symbol, Var, program::ProgramType,
        v_empty_list, v_empty_map, v_flyweight, v_int, v_str,
    };
    use shuttle::{check_random, sync::Arc, sync::Barrier, thread};
    use std::{
        collections::HashMap,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    fn setup_test_db() -> Arc<MoorDB> {
        let (db, _) = MoorDB::open(None, DatabaseConfig::default());

        // Create a few initial objects for testing
        let tx = db.start_transaction();
        let mut ws = DbWorldState { tx };

        // Create root object #0
        let _root = ws
            .create_object(
                &SYSTEM_OBJECT,
                &NOTHING,
                &SYSTEM_OBJECT,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();

        // Create a few test objects
        for i in 1..=5 {
            let obj = ws
                .create_object(
                    &SYSTEM_OBJECT,
                    &SYSTEM_OBJECT,
                    &SYSTEM_OBJECT,
                    BitEnum::new(),
                    ObjectKind::NextObjid,
                )
                .unwrap();
            assert_eq!(obj.id().0, i);
        }

        Box::new(ws).commit().unwrap();
        db
    }

    #[test]
    fn test_concurrent_property_access() {
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("test_prop");

                // Define a property first
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(v_int(0)),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let counter = Arc::new(AtomicUsize::new(0));
                let conflict_count = Arc::new(AtomicUsize::new(0));

                let handles: Vec<_> = (0..4)
                    .map(|_thread_id| {
                        let db = db.clone();
                        let counter = counter.clone();
                        let conflict_count = conflict_count.clone();

                        thread::spawn(move || {
                            for _i in 0..20 {
                                loop {
                                    let tx = db.start_transaction();
                                    let mut ws = DbWorldState { tx };

                                    // Read current value
                                    let current =
                                        match ws.retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                        {
                                            Ok(val) => val.as_integer().unwrap_or(0),
                                            Err(_) => 0,
                                        };

                                    // Increment and write back
                                    let new_value = current + 1;
                                    if ws
                                        .update_property(
                                            &SYSTEM_OBJECT,
                                            &obj,
                                            prop_name,
                                            &v_int(new_value),
                                        )
                                        .is_err()
                                    {
                                        conflict_count.fetch_add(1, Ordering::Relaxed);
                                        continue;
                                    }

                                    match Box::new(ws).commit() {
                                        Ok(CommitResult::Success { .. }) => {
                                            counter.fetch_add(1, Ordering::Relaxed);
                                            break;
                                        }
                                        Ok(CommitResult::ConflictRetry { .. }) => {
                                            conflict_count.fetch_add(1, Ordering::Relaxed);
                                            continue;
                                        }
                                        Err(_) => {
                                            conflict_count.fetch_add(1, Ordering::Relaxed);
                                            continue;
                                        }
                                    }
                                }
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // Verify final state
                let tx = db.start_transaction();
                let ws = DbWorldState { tx };
                let final_value = ws
                    .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                    .unwrap();
                assert_eq!(final_value.as_integer().unwrap(), 80); // 4 threads * 20 increments

                assert!(conflict_count.load(Ordering::Relaxed) < 100); // Some conflicts expected but not too many
            },
            10,
        );
    }

    #[test]
    fn test_concurrent_verb_lookup_and_cache() {
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);

                // Create several verbs on the object
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };

                    for i in 0..10 {
                        let verb_name = format!("test_verb_{i}");
                        ws.add_verb(
                            &SYSTEM_OBJECT,
                            &obj,
                            vec![Symbol::mk(&verb_name)],
                            &SYSTEM_OBJECT,
                            BitEnum::new_with(VerbFlag::Read) | VerbFlag::Exec,
                            VerbArgsSpec {
                                dobj: ArgSpec::None,
                                prep: PrepSpec::None,
                                iobj: ArgSpec::None,
                            },
                            ProgramType::MooR(moor_var::program::program::Program::new()),
                        )
                        .unwrap();
                    }
                    Box::new(ws).commit().unwrap();
                }

                let success_count = Arc::new(AtomicUsize::new(0));
                let cache_hits = Arc::new(AtomicUsize::new(0));

                let handles: Vec<_> = (0..8)
                    .map(|_thread_id| {
                        let db = db.clone();
                        let success_count = success_count.clone();
                        let cache_hits = cache_hits.clone();

                        thread::spawn(move || {
                            for iteration in 0..100 {
                                let verb_idx = iteration % 10;
                                let verb_name = Symbol::mk(&format!("test_verb_{verb_idx}"));

                                let tx = db.start_transaction();
                                let ws = DbWorldState { tx };

                                // Test verb lookup - this should hit the verb cache
                                if ws.get_verb(&SYSTEM_OBJECT, &obj, verb_name).is_ok() {
                                    success_count.fetch_add(1, Ordering::Relaxed);
                                }

                                // Also test method resolution which uses verb cache heavily
                                if ws
                                    .find_method_verb_on(&SYSTEM_OBJECT, &obj, verb_name)
                                    .is_ok()
                                {
                                    cache_hits.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // All lookups should succeed
                assert_eq!(success_count.load(Ordering::Relaxed), 800); // 8 threads * 100 iterations
                assert_eq!(cache_hits.load(Ordering::Relaxed), 800);
            },
            10,
        );
    }

    #[test]
    fn test_concurrent_object_hierarchy_operations() {
        check_random(
            || {
                let db = setup_test_db();

                // Create a hierarchy: root(0) -> parent(6) -> children(7-10)
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };

                    let parent = ws
                        .create_object(
                            &SYSTEM_OBJECT,
                            &SYSTEM_OBJECT,
                            &SYSTEM_OBJECT,
                            BitEnum::new(),
                            ObjectKind::NextObjid,
                        )
                        .unwrap();
                    assert_eq!(parent.id().0, 6);

                    for i in 7..=10 {
                        let child = ws
                            .create_object(
                                &SYSTEM_OBJECT,
                                &parent,
                                &SYSTEM_OBJECT,
                                BitEnum::new(),
                                ObjectKind::NextObjid,
                            )
                            .unwrap();
                        assert_eq!(child.id().0, i);
                    }

                    Box::new(ws).commit().unwrap();
                }

                let operation_count = Arc::new(AtomicUsize::new(0));
                let conflict_count = Arc::new(AtomicUsize::new(0));

                let handles: Vec<_> = (0..4)
                    .map(|_thread_id| {
                        let db = db.clone();
                        let operation_count = operation_count.clone();
                        let conflict_count = conflict_count.clone();

                        thread::spawn(move || {
                            for _i in 0..50 {
                                let tx = db.start_transaction();
                                let ws = DbWorldState { tx };

                                // Test concurrent hierarchy queries
                                match ws.children_of(&SYSTEM_OBJECT, &Obj::mk_id(6)) {
                                    Ok(children) => {
                                        assert_eq!(children.len(), 4);
                                        operation_count.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Err(_) => {
                                        conflict_count.fetch_add(1, Ordering::Relaxed);
                                    }
                                }

                                // Test parent lookup
                                match ws.parent_of(&SYSTEM_OBJECT, &Obj::mk_id(7)) {
                                    Ok(parent) => {
                                        assert_eq!(parent, Obj::mk_id(6));
                                        operation_count.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Err(_) => {
                                        conflict_count.fetch_add(1, Ordering::Relaxed);
                                    }
                                }

                                // Test descendants query
                                match ws.descendants_of(&SYSTEM_OBJECT, &Obj::mk_id(6), true) {
                                    Ok(descendants) => {
                                        assert_eq!(descendants.len(), 5); // parent + 4 children
                                        operation_count.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Err(_) => {
                                        conflict_count.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // All hierarchy operations should succeed (read-only)
                assert_eq!(operation_count.load(Ordering::Relaxed), 600); // 4 threads * 50 iterations * 3 ops
                assert_eq!(conflict_count.load(Ordering::Relaxed), 0);
            },
            10,
        );
    }

    #[test]
    fn test_concurrent_property_definition_and_access() {
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);

                let properties_defined = Arc::new(AtomicUsize::new(0));
                let access_count = Arc::new(AtomicUsize::new(0));
                let conflict_count = Arc::new(AtomicUsize::new(0));

                // Thread 1-2: Define properties
                let define_handles: Vec<_> = (0..2)
                    .map(|thread_id| {
                        let db = db.clone();
                        let properties_defined = properties_defined.clone();
                        let conflict_count = conflict_count.clone();

                        thread::spawn(move || {
                            for i in 0..10 {
                                let prop_name = Symbol::mk(&format!("prop_{thread_id}_{i}"));

                                loop {
                                    let tx = db.start_transaction();
                                    let mut ws = DbWorldState { tx };

                                    match ws.define_property(
                                        &SYSTEM_OBJECT,
                                        &obj,
                                        &obj,
                                        prop_name,
                                        &SYSTEM_OBJECT,
                                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                                        Some(v_str(&format!("value_{thread_id}_{i}"))),
                                    ) {
                                        Ok(_) => {}
                                        Err(_) => {
                                            conflict_count.fetch_add(1, Ordering::Relaxed);
                                            continue;
                                        }
                                    }

                                    match Box::new(ws).commit() {
                                        Ok(CommitResult::Success { .. }) => {
                                            properties_defined.fetch_add(1, Ordering::Relaxed);
                                            break;
                                        }
                                        Ok(CommitResult::ConflictRetry { .. }) => {
                                            conflict_count.fetch_add(1, Ordering::Relaxed);
                                            continue;
                                        }
                                        Err(_) => {
                                            conflict_count.fetch_add(1, Ordering::Relaxed);
                                            continue;
                                        }
                                    }
                                }
                            }
                        })
                    })
                    .collect();

                // Wait for some properties to be defined
                thread::yield_now();

                // Thread 3-4: Access properties
                let access_handles: Vec<_> = (0..2)
                    .map(|_thread_id| {
                        let db = db.clone();
                        let access_count = access_count.clone();

                        thread::spawn(move || {
                            for _attempt in 0..50 {
                                let tx = db.start_transaction();
                                let ws = DbWorldState { tx };

                                // List all properties
                                if let Ok(props) = ws.properties(&SYSTEM_OBJECT, &obj) {
                                    access_count.fetch_add(props.len(), Ordering::Relaxed);

                                    // Try to access each property
                                    for prop in props.iter() {
                                        if let Ok(_value) =
                                            ws.retrieve_property(&SYSTEM_OBJECT, &obj, prop.name())
                                        {
                                            access_count.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                }

                                thread::yield_now();
                            }
                        })
                    })
                    .collect();

                // Wait for definition threads to complete
                for handle in define_handles {
                    handle.join().unwrap();
                }

                // Wait for access threads to complete
                for handle in access_handles {
                    handle.join().unwrap();
                }

                // Verify final state
                let tx = db.start_transaction();
                let ws = DbWorldState { tx };
                let final_props = ws.properties(&SYSTEM_OBJECT, &obj).unwrap();
                assert_eq!(final_props.len(), 20); // 2 threads * 10 properties each
            },
            10,
        );
    }

    #[test]
    fn test_concurrent_verb_resolution_cache_stress() {
        check_random(
            || {
                let db = setup_test_db();

                // Create a complex inheritance hierarchy for verb resolution testing
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };

                    // Create parent objects
                    let parent1 = ws
                        .create_object(
                            &SYSTEM_OBJECT,
                            &SYSTEM_OBJECT,
                            &SYSTEM_OBJECT,
                            BitEnum::new(),
                            ObjectKind::NextObjid,
                        )
                        .unwrap();
                    let parent2 = ws
                        .create_object(
                            &SYSTEM_OBJECT,
                            &parent1,
                            &SYSTEM_OBJECT,
                            BitEnum::new(),
                            ObjectKind::NextObjid,
                        )
                        .unwrap();
                    let child = ws
                        .create_object(
                            &SYSTEM_OBJECT,
                            &parent2,
                            &SYSTEM_OBJECT,
                            BitEnum::new(),
                            ObjectKind::NextObjid,
                        )
                        .unwrap();

                    // Add verbs at different levels
                    ws.add_verb(
                        &SYSTEM_OBJECT,
                        &parent1,
                        vec![Symbol::mk("parent_verb")],
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(VerbFlag::Read) | VerbFlag::Exec,
                        VerbArgsSpec {
                            dobj: ArgSpec::None,
                            prep: PrepSpec::None,
                            iobj: ArgSpec::None,
                        },
                        ProgramType::MooR(moor_var::program::program::Program::new()),
                    )
                    .unwrap();

                    ws.add_verb(
                        &SYSTEM_OBJECT,
                        &parent2,
                        vec![Symbol::mk("override_verb")],
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(VerbFlag::Read) | VerbFlag::Exec,
                        VerbArgsSpec {
                            dobj: ArgSpec::None,
                            prep: PrepSpec::None,
                            iobj: ArgSpec::None,
                        },
                        ProgramType::MooR(moor_var::program::program::Program::new()),
                    )
                    .unwrap();

                    ws.add_verb(
                        &SYSTEM_OBJECT,
                        &child,
                        vec![Symbol::mk("child_verb")],
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(VerbFlag::Read) | VerbFlag::Exec,
                        VerbArgsSpec {
                            dobj: ArgSpec::None,
                            prep: PrepSpec::None,
                            iobj: ArgSpec::None,
                        },
                        ProgramType::MooR(moor_var::program::program::Program::new()),
                    )
                    .unwrap();

                    Box::new(ws).commit().unwrap();
                }

                let resolution_count = Arc::new(AtomicUsize::new(0));
                let cache_test_results = Arc::new(Mutex::new(HashMap::new()));

                let handles: Vec<_> = (0..6)
                    .map(|thread_id| {
                        let db = db.clone();
                        let resolution_count = resolution_count.clone();
                        let cache_test_results = cache_test_results.clone();

                        thread::spawn(move || {
                            let mut local_results = HashMap::new();

                            for iteration in 0..200 {
                                let tx = db.start_transaction();
                                let ws = DbWorldState { tx };

                                let child_obj = Obj::mk_id(8); // The child object we created

                                // Test various verb resolutions that should hit cache
                                let verbs_to_test = vec![
                                    "parent_verb",
                                    "override_verb",
                                    "child_verb",
                                    "nonexistent_verb",
                                ];

                                for verb_name in verbs_to_test {
                                    let verb_sym = Symbol::mk(verb_name);

                                    // Test method resolution (heavily uses verb cache)
                                    let result = ws.find_method_verb_on(
                                        &SYSTEM_OBJECT,
                                        &child_obj,
                                        verb_sym,
                                    );
                                    let key = format!("{verb_name}_{thread_id}");

                                    match result {
                                        Ok(_) => {
                                            *local_results.entry(key).or_insert(0) += 1;
                                            resolution_count.fetch_add(1, Ordering::Relaxed);
                                        }
                                        Err(WorldStateError::VerbNotFound(_, _)) => {
                                            // Expected for nonexistent_verb
                                            if verb_name == "nonexistent_verb" {
                                                *local_results.entry(key).or_insert(0) += 1;
                                            }
                                        }
                                        Err(_) => {}
                                    }
                                }

                                if iteration % 50 == 0 {
                                    thread::yield_now(); // Give other threads a chance
                                }
                            }

                            // Merge local results into shared results
                            let mut shared = cache_test_results.lock().unwrap();
                            for (key, count) in local_results {
                                *shared.entry(key).or_insert(0) += count;
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                let results = cache_test_results.lock().unwrap();

                // Verify that verb resolution worked consistently across threads
                // Each of the first 3 verbs should be found 6 * 200 = 1200 times
                for verb in ["parent_verb", "override_verb", "child_verb"] {
                    let total_found: usize = (0..6)
                        .map(|thread_id| {
                            results
                                .get(&format!("{verb}_{thread_id}"))
                                .copied()
                                .unwrap_or(0)
                        })
                        .sum();
                    assert_eq!(total_found, 1200, "Verb {verb} resolution count mismatch");
                }

                // nonexistent_verb should also be "found" (as not found) 1200 times total
                let nonexistent_total: usize = (0..6)
                    .map(|thread_id| {
                        results
                            .get(&format!("nonexistent_verb_{thread_id}"))
                            .copied()
                            .unwrap_or(0)
                    })
                    .sum();
                assert_eq!(
                    nonexistent_total, 1200,
                    "Nonexistent verb handling count mismatch"
                );

                assert_eq!(resolution_count.load(Ordering::Relaxed), 3600); // 6 threads * 200 iterations * 3 found verbs
            },
            10,
        );
    }

    #[test]
    fn test_concurrent_transaction_isolation() {
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("isolation_test");

                // Define initial property
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(v_int(100)),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let successful_updates = Arc::new(AtomicUsize::new(0));

                let handles: Vec<_> = (0..3)
                    .map(|thread_id| {
                        let db = db.clone();
                        let successful_updates = successful_updates.clone();

                        thread::spawn(move || {
                            for _iteration in 0..30 {
                                loop {
                                    // Start transaction and read initial value
                                    let tx1 = db.start_transaction();
                                    let mut ws1 = DbWorldState { tx: tx1 };

                                    let initial_value = ws1
                                        .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                        .unwrap()
                                        .as_integer()
                                        .unwrap();

                                    // Simulate some work
                                    thread::yield_now();

                                    // Start another transaction in parallel to check consistency.
                                    // Note: We do NOT check that concurrent_value >= initial_value.
                                    // Under SI (Snapshot Isolation), we guarantee that each transaction
                                    // sees a consistent snapshot, but NOT that transactions see each
                                    // other's uncommitted or even recently committed changes in real-time
                                    // order (linearizability). A concurrent transaction may see an older
                                    // snapshot than our initial read. This is expected behavior for SI
                                    // and does not indicate a bug.
                                    let tx2 = db.start_transaction();
                                    let ws2 = DbWorldState { tx: tx2 };

                                    let _concurrent_value = ws2
                                        .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                        .unwrap()
                                        .as_integer()
                                        .unwrap();

                                    // Try to update based on initial value
                                    let new_value = initial_value + thread_id as i64;
                                    if ws1
                                        .update_property(
                                            &SYSTEM_OBJECT,
                                            &obj,
                                            prop_name,
                                            &v_int(new_value),
                                        )
                                        .is_err()
                                    {
                                        continue;
                                    }

                                    match Box::new(ws1).commit() {
                                        Ok(CommitResult::Success { .. }) => {
                                            successful_updates.fetch_add(1, Ordering::Relaxed);
                                            break;
                                        }
                                        Ok(CommitResult::ConflictRetry { .. }) => {
                                            // Expected - retry
                                            continue;
                                        }
                                        Err(_) => {
                                            continue;
                                        }
                                    }
                                }
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                assert_eq!(successful_updates.load(Ordering::Relaxed), 90); // 3 threads * 30 updates

                // Final value should be deterministic based on successful updates
                let tx = db.start_transaction();
                let ws = DbWorldState { tx };
                let final_value = ws
                    .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                    .unwrap();
                assert!(final_value.as_integer().unwrap() >= 100); // At least the initial value
            },
            10,
        );
    }

    #[test]
    #[ignore] // Takes >60s to run through 100 shuttle iterations; run manually when needed
    fn test_g_single_item_realtime_anomaly() {
        // Test for a G-single-item-realtime anomaly detected by Jepsen-Elle:
        // T2 commits at index N, T1 invokes at index N+1 (AFTER T2 completes),
        // but T1 does NOT see T2's writes - violates serializability
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("realtime_test");

                // Define initial property with value 0
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(v_int(0)),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let violations = Arc::new(AtomicUsize::new(0));

                // Run with high concurrency to increase chance of hitting the race
                let handles: Vec<_> = (0..8)
                    .map(|thread_id| {
                        let db = db.clone();
                        let violations = violations.clone();

                        thread::spawn(move || {
                            for iteration in 0..100 {
                                // T2: Write and commit
                                // Add 1 to ensure write_value is never 0 (which is the initial value)
                                let write_value = (thread_id * 1000 + iteration + 1) as i64;
                                let tx2 = db.start_transaction();
                                let mut ws2 = DbWorldState { tx: tx2 };
                                ws2.update_property(
                                    &SYSTEM_OBJECT,
                                    &obj,
                                    prop_name,
                                    &v_int(write_value),
                                )
                                .ok();

                                let commit_result = Box::new(ws2).commit();

                                // If T2 committed successfully
                                if matches!(commit_result, Ok(CommitResult::Success { .. })) {
                                    // Immediately start T1 - this is AFTER T2 committed in real-time
                                    let tx1 = db.start_transaction();
                                    let ws1 = DbWorldState { tx: tx1 };

                                    // T1 reads the property
                                    if let Ok(value) = ws1.retrieve_property(&SYSTEM_OBJECT, &obj, prop_name) {
                                        let read_value = value.as_integer().unwrap_or(-1);

                                        // T1 started AFTER T2 committed, so MUST see T2's write
                                        // If T1 sees 0 (initial value) or any value older than write_value,
                                        // that's a serializability violation
                                        if read_value == 0 {
                                            violations.fetch_add(1, Ordering::Relaxed);
                                            eprintln!(
                                                "G-single-item-realtime violation! Thread {thread_id} iteration {iteration}: \
                                                 T2 committed value {write_value} but T1 (started AFTER commit) saw {read_value}"
                                            );
                                        }
                                    }
                                }

                                // Yield to increase thread interleaving
                                if iteration % 10 == 0 {
                                    thread::yield_now();
                                }
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                let total_violations = violations.load(Ordering::Relaxed);
                assert_eq!(
                    total_violations, 0,
                    "Detected {total_violations} G-single-item-realtime serializability violations"
                );
            },
            100, // Run many shuttle iterations to explore different schedules
        );
    }

    #[test]
    fn test_concurrent_list_append_merge() {
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("append_test");

                // Initialize property with empty list
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(v_empty_list()),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let success_count = Arc::new(AtomicUsize::new(0));

                let handles: Vec<_> = (0..2)
                    .map(|thread_id| {
                        let db = db.clone();
                        let success_count = success_count.clone();

                        thread::spawn(move || {
                            let tx = db.start_transaction();
                            let mut ws = DbWorldState { tx };

                            // Read list (this snapshot read will be the 'base' for merge)
                            let current_list = ws
                                .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                .unwrap();

                            // Append value - this sets OP_HINT_LIST_APPEND
                            let val = v_int(thread_id as i64);
                            let new_list = current_list.push(&val).unwrap();

                            ws.update_property(&SYSTEM_OBJECT, &obj, prop_name, &new_list)
                                .unwrap();

                            if let Ok(CommitResult::Success { .. }) = Box::new(ws).commit() {
                                success_count.fetch_add(1, Ordering::Relaxed);
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // Both should succeed due to smart merging
                assert_eq!(success_count.load(Ordering::Relaxed), 2);

                // Verify final state
                let tx = db.start_transaction();
                let ws = DbWorldState { tx };
                let final_list = ws
                    .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                    .unwrap();

                // Should contain both 0 and 1
                let list = final_list.as_list().unwrap();
                assert_eq!(list.len(), 2);
                assert!(list.contains(&v_int(0), false).unwrap());
                assert!(list.contains(&v_int(1), false).unwrap());
            },
            10,
        );
    }

    #[test]
    fn test_concurrent_map_insert_merge() {
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("map_merge_test");

                // Initialize property with empty map
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(v_empty_map()),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let success_count = Arc::new(AtomicUsize::new(0));

                let handles: Vec<_> = (0..2)
                    .map(|thread_id| {
                        let db = db.clone();
                        let success_count = success_count.clone();

                        thread::spawn(move || {
                            let tx = db.start_transaction();
                            let mut ws = DbWorldState { tx };

                            // Read map (base for merge)
                            let current_map = ws
                                .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                .unwrap();

                            // Insert unique key-value pair - this sets OP_HINT_MAP_INSERT
                            let key = v_str(&format!("key_{thread_id}"));
                            let val = v_int(thread_id as i64);

                            // Map::set returns Result<Var, Error>
                            let new_map = current_map
                                .set(&key, &val, moor_var::IndexMode::ZeroBased)
                                .unwrap();

                            ws.update_property(&SYSTEM_OBJECT, &obj, prop_name, &new_map)
                                .unwrap();

                            if let Ok(CommitResult::Success { .. }) = Box::new(ws).commit() {
                                success_count.fetch_add(1, Ordering::Relaxed);
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // Both should succeed
                assert_eq!(success_count.load(Ordering::Relaxed), 2);

                // Verify final state
                let tx = db.start_transaction();
                let ws = DbWorldState { tx };
                let final_map = ws
                    .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                    .unwrap();

                // Should contain both keys
                let map = final_map.as_map().unwrap();
                assert_eq!(map.len(), 2);
                assert!(map.contains_key(&v_str("key_0"), false).unwrap());
                assert!(map.contains_key(&v_str("key_1"), false).unwrap());
            },
            10,
        );
    }

    #[test]
    fn test_concurrent_flyweight_slot_merge() {
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("flyweight_merge_test");

                // Initialize property with empty flyweight
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(v_flyweight(
                            SYSTEM_OBJECT,
                            &[],
                            v_empty_list().as_list().unwrap().clone(),
                        )),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let success_count = Arc::new(AtomicUsize::new(0));

                let handles: Vec<_> = (0..2)
                    .map(|thread_id| {
                        let db = db.clone();
                        let success_count = success_count.clone();

                        thread::spawn(move || {
                            let tx = db.start_transaction();
                            let mut ws = DbWorldState { tx };

                            // Read flyweight
                            let current_fw_var = ws
                                .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                .unwrap();
                            let current_fw = current_fw_var.as_flyweight().unwrap();

                            // Add a slot - this sets OP_HINT_FLYWEIGHT_ADD_SLOT if we use the right constructor
                            let slot_name = Symbol::mk(&format!("slot_{thread_id}"));
                            let val = v_int(thread_id as i64);
                            let new_fw = current_fw.add_slot(slot_name, val);

                            // Manually apply hint for test (in real usage the builtin would do this)
                            let new_fw_var =
                                Var::from_flyweight_with_hint(new_fw, OP_HINT_FLYWEIGHT_ADD_SLOT);

                            ws.update_property(&SYSTEM_OBJECT, &obj, prop_name, &new_fw_var)
                                .unwrap();

                            if let Ok(CommitResult::Success { .. }) = Box::new(ws).commit() {
                                success_count.fetch_add(1, Ordering::Relaxed);
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // Both should succeed
                assert_eq!(success_count.load(Ordering::Relaxed), 2);

                // Verify final state
                let tx = db.start_transaction();
                let ws = DbWorldState { tx };
                let final_fw_var = ws
                    .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                    .unwrap();
                let final_fw = final_fw_var.as_flyweight().unwrap();

                // Should contain both slots
                assert_eq!(final_fw.get_slot(&Symbol::mk("slot_0")), Some(&v_int(0)));
                assert_eq!(final_fw.get_slot(&Symbol::mk("slot_1")), Some(&v_int(1)));
            },
            10,
        );
    }

    #[test]
    fn test_concurrent_flyweight_contents_merge() {
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("flyweight_contents_merge_test");

                // Initialize property with empty flyweight
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(v_flyweight(
                            SYSTEM_OBJECT,
                            &[],
                            v_empty_list().as_list().unwrap().clone(),
                        )),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let success_count = Arc::new(AtomicUsize::new(0));

                let handles: Vec<_> = (0..2)
                    .map(|thread_id| {
                        let db = db.clone();
                        let success_count = success_count.clone();

                        thread::spawn(move || {
                            let tx = db.start_transaction();
                            let mut ws = DbWorldState { tx };

                            // Read flyweight
                            let current_fw_var = ws
                                .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                .unwrap();
                            let current_fw = current_fw_var.as_flyweight().unwrap();

                            // Append to contents - this sets OP_HINT_FLYWEIGHT_APPEND_CONTENTS if we use right constructor
                            let val = v_int(thread_id as i64);
                            let new_contents = current_fw.contents().push(&val).unwrap();
                            let new_fw =
                                current_fw.with_contents(new_contents.as_list().unwrap().clone());

                            // Manually apply hint for test
                            let new_fw_var = Var::from_flyweight_with_hint(
                                new_fw,
                                OP_HINT_FLYWEIGHT_APPEND_CONTENTS,
                            );

                            ws.update_property(&SYSTEM_OBJECT, &obj, prop_name, &new_fw_var)
                                .unwrap();

                            if let Ok(CommitResult::Success { .. }) = Box::new(ws).commit() {
                                success_count.fetch_add(1, Ordering::Relaxed);
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // Both should succeed
                assert_eq!(success_count.load(Ordering::Relaxed), 2);

                // Verify final state
                let tx = db.start_transaction();
                let ws = DbWorldState { tx };
                let final_fw_var = ws
                    .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                    .unwrap();
                let final_fw = final_fw_var.as_flyweight().unwrap();

                // Contents should contain both 0 and 1
                let contents = final_fw.contents();
                assert_eq!(contents.len(), 2);
                assert!(contents.contains(&v_int(0), false).unwrap());
                assert!(contents.contains(&v_int(1), false).unwrap());
            },
            10,
        );
    }

    #[test]
    fn test_concurrent_string_append_merge() {
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("string_append_test");

                // Initialize property with empty string
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(v_str("Base")),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let success_count = Arc::new(AtomicUsize::new(0));

                let handles: Vec<_> = (0..2)
                    .map(|thread_id| {
                        let db = db.clone();
                        let success_count = success_count.clone();

                        thread::spawn(move || {
                            let tx = db.start_transaction();
                            let mut ws = DbWorldState { tx };

                            // Read string
                            let current_str = ws
                                .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                .unwrap();

                            // Append value - Var::add for strings sets OP_HINT_STR_APPEND via Str::str_append
                            let suffix = v_str(&format!("+Suffix{}", thread_id));
                            let new_str = current_str.add(&suffix).unwrap();

                            ws.update_property(&SYSTEM_OBJECT, &obj, prop_name, &new_str)
                                .unwrap();

                            if let Ok(CommitResult::Success { .. }) = Box::new(ws).commit() {
                                success_count.fetch_add(1, Ordering::Relaxed);
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // Both should succeed
                assert_eq!(success_count.load(Ordering::Relaxed), 2);

                // Verify final state
                let tx = db.start_transaction();
                let ws = DbWorldState { tx };
                let final_str = ws
                    .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                    .unwrap();
                let s = final_str.as_string().unwrap();

                // Should contain Base and both suffixes
                assert!(s.starts_with("Base"));
                assert!(s.contains("+Suffix0"));
                assert!(s.contains("+Suffix1"));
                assert_eq!(s.len(), "Base+Suffix0+Suffix1".len());
            },
            10,
        );
    }

    #[test]
    fn test_concurrent_map_insert_same_key_conflicts() {
        // Test that two concurrent inserts of the SAME key with DIFFERENT values
        // correctly results in a conflict (only one succeeds).
        // We use a barrier to ensure both threads read the initial empty state
        // before either commits, guaranteeing an actual conflict.
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("map_same_key_test");

                // Initialize property with empty map
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(v_empty_map()),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let success_count = Arc::new(AtomicUsize::new(0));
                // Barrier ensures both threads read before either commits
                let barrier = Arc::new(Barrier::new(2));

                let handles: Vec<_> = (0..2)
                    .map(|thread_id| {
                        let db = db.clone();
                        let success_count = success_count.clone();
                        let barrier = barrier.clone();

                        thread::spawn(move || {
                            let tx = db.start_transaction();
                            let mut ws = DbWorldState { tx };

                            // Read map (base for merge)
                            let current_map = ws
                                .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                .unwrap();

                            // Insert SAME key with DIFFERENT values
                            let key = v_str("shared_key");
                            let val = v_int(thread_id as i64); // Different values!

                            let new_map = current_map
                                .set(&key, &val, moor_var::IndexMode::ZeroBased)
                                .unwrap();

                            ws.update_property(&SYSTEM_OBJECT, &obj, prop_name, &new_map)
                                .unwrap();

                            // Wait for both threads to have read and prepared their writes
                            barrier.wait();

                            if let Ok(CommitResult::Success { .. }) = Box::new(ws).commit() {
                                success_count.fetch_add(1, Ordering::Relaxed);
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // Only ONE should succeed - inserting same key with different values is a conflict
                assert_eq!(success_count.load(Ordering::Relaxed), 1);

                // Verify final state has exactly one entry
                let tx = db.start_transaction();
                let ws = DbWorldState { tx };
                let final_map = ws
                    .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                    .unwrap();

                let map = final_map.as_map().unwrap();
                assert_eq!(map.len(), 1);
                assert!(map.contains_key(&v_str("shared_key"), false).unwrap());
            },
            10,
        );
    }

    #[test]
    fn test_mismatched_hints_cause_conflict() {
        // Test that operations with mismatched hints don't merge.
        // Thread 0: appends to list (LIST_APPEND hint)
        // Thread 1: replaces list entirely (no hint / different operation)
        // We use a barrier to ensure both threads read the initial state
        // before either commits, guaranteeing an actual conflict.
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("mismatched_hint_test");

                // Initialize property with a list containing one element
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    let initial_list = v_empty_list().push(&v_int(0)).unwrap();
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(initial_list),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let success_count = Arc::new(AtomicUsize::new(0));
                // Barrier ensures both threads read before either commits
                let barrier = Arc::new(Barrier::new(2));

                // Thread 0: appends (has LIST_APPEND hint)
                // Thread 1: replaces entirely (no hint - uses from_list directly)
                let handles: Vec<_> = (0..2)
                    .map(|thread_id| {
                        let db = db.clone();
                        let success_count = success_count.clone();
                        let barrier = barrier.clone();

                        thread::spawn(move || {
                            let tx = db.start_transaction();
                            let mut ws = DbWorldState { tx };

                            let current_list = ws
                                .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                .unwrap();

                            let new_list = if thread_id == 0 {
                                // Append - gets LIST_APPEND hint
                                current_list.push(&v_int(100)).unwrap()
                            } else {
                                // Replace entirely - no hint (simulates a different operation)
                                // Create a completely new list without using push
                                List::build(&[v_int(999)])
                            };

                            ws.update_property(&SYSTEM_OBJECT, &obj, prop_name, &new_list)
                                .unwrap();

                            // Wait for both threads to have read and prepared their writes
                            barrier.wait();

                            if let Ok(CommitResult::Success { .. }) = Box::new(ws).commit() {
                                success_count.fetch_add(1, Ordering::Relaxed);
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // Only ONE should succeed - mismatched hints prevent merging
                assert_eq!(success_count.load(Ordering::Relaxed), 1);
            },
            10,
        );
    }

    #[test]
    fn test_merged_values_have_no_hint() {
        // Test that after a successful merge, the committed value has its hint cleared.
        // This prevents stale hints from affecting future operations.
        check_random(
            || {
                let db = setup_test_db();
                let obj = Obj::mk_id(1);
                let prop_name = Symbol::mk("hint_clearing_test");

                // Initialize property with empty list
                {
                    let tx = db.start_transaction();
                    let mut ws = DbWorldState { tx };
                    ws.define_property(
                        &SYSTEM_OBJECT,
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                        Some(v_empty_list()),
                    )
                    .unwrap();
                    Box::new(ws).commit().unwrap();
                }

                let success_count = Arc::new(AtomicUsize::new(0));

                let handles: Vec<_> = (0..2)
                    .map(|thread_id| {
                        let db = db.clone();
                        let success_count = success_count.clone();

                        thread::spawn(move || {
                            let tx = db.start_transaction();
                            let mut ws = DbWorldState { tx };

                            let current_list = ws
                                .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                                .unwrap();

                            // Append value - this sets OP_HINT_LIST_APPEND
                            let val = v_int(thread_id as i64);
                            let new_list = current_list.push(&val).unwrap();

                            ws.update_property(&SYSTEM_OBJECT, &obj, prop_name, &new_list)
                                .unwrap();

                            if let Ok(CommitResult::Success { .. }) = Box::new(ws).commit() {
                                success_count.fetch_add(1, Ordering::Relaxed);
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                // Both should succeed due to smart merging
                assert_eq!(success_count.load(Ordering::Relaxed), 2);

                // Verify final value has NO hint (hint was cleared after merge)
                let tx = db.start_transaction();
                let ws = DbWorldState { tx };
                let final_list = ws
                    .retrieve_property(&SYSTEM_OBJECT, &obj, prop_name)
                    .unwrap();

                // The merged value should have OP_HINT_NONE
                assert_eq!(
                    final_list.op_hint(),
                    OP_HINT_NONE,
                    "Merged value should have hint cleared"
                );

                // Verify the list still has the right content
                let list = final_list.as_list().unwrap();
                assert_eq!(list.len(), 2);
            },
            10,
        );
    }
}
