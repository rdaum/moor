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

//! Benchmarks comparing regular objects, anonymous objects (nursery), and promoted anonymous objects.
//!
//! Nursery objects are task-local anonymous objects that haven't been promoted to the database yet.
//! This benchmark suite measures:
//! - Object creation overhead (regular vs nursery)
//! - Property access patterns (get/set on different object types)
//! - Promotion cost (swizzling nursery to anonymous)

use std::{hint::black_box, sync::Arc, time::Duration};

use criterion::{Criterion, criterion_group, criterion_main};

use moor_common::{
    model::{CommitResult, ObjFlag, ObjectKind, VerbArgsSpec, VerbFlag, WorldState, WorldStateSource},
    tasks::NoopClientSession,
    util::BitEnum,
};
use moor_compiler::{CompileOptions, compile};
use moor_db::{DatabaseConfig, TxDB};
use moor_kernel::{
    config::FeaturesConfig,
    testing::vm_test_utils::setup_task_context,
    vm::{VMHostResponse, builtins::BuiltinRegistry, vm_host::VmHost},
};
use moor_var::{List, NOTHING, SYSTEM_OBJECT, Symbol, program::ProgramType, v_empty_str, v_obj};

fn create_db_with_verb(verb_code: &str) -> TxDB {
    let (ws_source, _) = TxDB::open(None, DatabaseConfig::default());
    let mut tx = ws_source.new_world_state().unwrap();

    let _sysobj = tx
        .create_object(
            &SYSTEM_OBJECT,
            &NOTHING,
            &SYSTEM_OBJECT,
            BitEnum::all(),
            ObjectKind::NextObjid,
        )
        .unwrap();

    let program = compile(verb_code, CompileOptions::default()).unwrap();
    tx.add_verb(
        &SYSTEM_OBJECT,
        &SYSTEM_OBJECT,
        vec![Symbol::mk("test")],
        &SYSTEM_OBJECT,
        VerbFlag::rxd(),
        VerbArgsSpec::this_none_this(),
        ProgramType::MooR(program),
    )
    .unwrap();

    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    ws_source
}

fn prepare_call_verb(
    world_state: &mut dyn WorldState,
    verb_name: &str,
    max_ticks: usize,
) -> VmHost {
    let mut vm_host = VmHost::new(0, 20, max_ticks, Duration::from_secs(1000));

    let verb_name = Symbol::mk(verb_name);
    let (program, verbdef) = world_state
        .find_method_verb_on(&SYSTEM_OBJECT, &SYSTEM_OBJECT, verb_name)
        .unwrap();
    let permissions_flags = BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer;
    vm_host.start_call_method_verb(
        0,
        verbdef,
        verb_name,
        v_obj(SYSTEM_OBJECT),
        SYSTEM_OBJECT,
        List::mk_list(&[]),
        v_obj(SYSTEM_OBJECT),
        v_empty_str(),
        permissions_flags,
        program,
    );
    vm_host
}

/// Create FeaturesConfig with anonymous_objects enabled
fn features_config() -> FeaturesConfig {
    FeaturesConfig {
        anonymous_objects: true,
        ..FeaturesConfig::default()
    }
}

/// Run the VM until completion
fn execute_to_completion(
    session: Arc<dyn moor_common::tasks::Session>,
    vm_host: &mut VmHost,
) {
    vm_host.reset_ticks();
    vm_host.reset_time();

    let config = features_config();
    let builtins = BuiltinRegistry::new();

    loop {
        match vm_host.exec_interpreter(0, session.as_ref(), &builtins, &config) {
            VMHostResponse::ContinueOk => continue,
            VMHostResponse::CompleteSuccess(_) => return,
            VMHostResponse::AbortLimit(reason) => panic!("Abort: {:?}", reason),
            VMHostResponse::CompleteException(e) => panic!("Exception: {:?}", e),
            VMHostResponse::DispatchFork(_) => panic!("Unexpected fork"),
            VMHostResponse::Suspend(_) => panic!("Unexpected suspend"),
            VMHostResponse::SuspendNeedInput(_) => panic!("Unexpected suspend need input"),
            VMHostResponse::CompleteAbort => panic!("Unexpected abort"),
            VMHostResponse::RollbackRetry => panic!("Unexpected rollback retry"),
            VMHostResponse::CompleteRollback(_) => panic!("Unexpected complete rollback"),
        }
    }
}

/// Benchmark object creation: regular vs nursery (anonymous)
fn object_creation_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("object_creation");
    group.sample_size(20);

    let num_creates: u64 = 1_000;
    let max_ticks = (num_creates * 100) as usize;

    group.throughput(criterion::Throughput::Elements(num_creates));

    // Create regular objects (NextObjid - persisted to DB)
    group.bench_function("regular_object", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            for i in [1..{num_creates}]
                create(#-1, #0, 0);
            endfor
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Create nursery objects (Anonymous - task-local, no DB write)
    group.bench_function("nursery_object", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            for i in [1..{num_creates}]
                create(#-1, #0, 1);
            endfor
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    group.finish();
}

/// Benchmark property/slot access on different object types
fn property_access_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("property_access");
    group.sample_size(20);

    let num_accesses: u64 = 10_000;
    let max_ticks = (num_accesses * 20) as usize;

    group.throughput(criterion::Throughput::Elements(num_accesses));

    // Property set on regular object (requires add_property first)
    group.bench_function("regular_object_set", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            o = create(#-1, #0, 0);
            add_property(o, "value", 0, {{#0, "wrc"}});
            for i in [1..{num_accesses}]
                o.value = i;
            endfor
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Slot set on nursery object (dynamic slots, no add_property needed)
    group.bench_function("nursery_object_set", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            o = create(#-1, #0, 1);
            for i in [1..{num_accesses}]
                o.value = i;
            endfor
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Property get on regular object
    group.bench_function("regular_object_get", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            o = create(#-1, #0, 0);
            add_property(o, "value", 42, {{#0, "wrc"}});
            total = 0;
            for i in [1..{num_accesses}]
                total = total + o.value;
            endfor
            return total;
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Slot get on nursery object
    group.bench_function("nursery_object_get", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            o = create(#-1, #0, 1);
            o.value = 42;
            total = 0;
            for i in [1..{num_accesses}]
                total = total + o.value;
            endfor
            return total;
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    group.finish();
}

/// Benchmark nursery-to-anonymous promotion (swizzling)
fn promotion_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("nursery_promotion");
    group.sample_size(20);

    let num_promotions: u64 = 100;
    let max_ticks = (num_promotions * 200) as usize;

    group.throughput(criterion::Throughput::Elements(num_promotions));

    // Promote nursery objects by storing in regular object property
    group.bench_function("promote_via_property_store", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            box = create(#-1, #0, 0);
            add_property(box, "child", 0, {{#0, "wrc"}});
            for i in [1..{num_promotions}]
                e = create(#-1, #0, 1);
                e.data = i;
                box.child = e;
            endfor
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Promote nursery objects with nested structure
    group.bench_function("promote_nested_structure", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            box = create(#-1, #0, 0);
            add_property(box, "tree", 0, {{#0, "wrc"}});
            for i in [1..{num_promotions}]
                inner = create(#-1, #0, 1);
                inner.value = i;
                outer = create(#-1, #0, 1);
                outer.child = inner;
                box.tree = outer;
            endfor
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Promote nursery objects in a list
    // Note: This benchmark tests nursery-to-anonymous promotion when a list of
    // nursery objects is stored. Each nursery object is created and has a slot set
    // via dynamic slots (no add_property needed for nursery). When the list is
    // assigned to box.items, swizzling promotes all nursery objects to real
    // anonymous objects in the DB.
    // Note: "children" is a reserved property name (builtin), so we use "items"
    group.bench_function("promote_list_of_nursery", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            box = create(#-1, #0, 0);
            add_property(box, "items", {{}}, {{#0, "wrc"}});
            for i in [1..{num_promotions}]
                things = {{}};
                for j in [1..5]
                    e = create(#-1, #0, 1);
                    e.val = j;
                    things = {{@things, e}};
                endfor
                box.items = things;
            endfor
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    group.finish();
}

/// Benchmark valid() and parent() introspection
fn introspection_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("object_introspection");
    group.sample_size(20);

    let num_checks: u64 = 10_000;
    let max_ticks = (num_checks * 20) as usize;

    group.throughput(criterion::Throughput::Elements(num_checks));

    // valid() on regular object
    group.bench_function("valid_regular", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            o = create(#-1, #0, 0);
            n = 0;
            for i in [1..{num_checks}]
                if (valid(o))
                    n = n + 1;
                endif
            endfor
            return n;
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // valid() on nursery object
    group.bench_function("valid_nursery", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            o = create(#-1, #0, 1);
            n = 0;
            for i in [1..{num_checks}]
                if (valid(o))
                    n = n + 1;
                endif
            endfor
            return n;
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // parent() on regular object
    group.bench_function("parent_regular", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            o = create(#-1, #0, 0);
            n = 0;
            for i in [1..{num_checks}]
                if (parent(o) == #-1)
                    n = n + 1;
                endif
            endfor
            return n;
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // parent() on nursery object
    group.bench_function("parent_nursery", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            o = create(#-1, #0, 1);
            n = 0;
            for i in [1..{num_checks}]
                if (parent(o) == #-1)
                    n = n + 1;
                endif
            endfor
            return n;
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    group.finish();
}

/// Combined workload: create, use, optionally promote
fn realistic_workload_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_workload");
    group.sample_size(20);

    let num_iterations: u64 = 500;
    let max_ticks = (num_iterations * 200) as usize;

    group.throughput(criterion::Throughput::Elements(num_iterations));

    // Workload using regular objects (everything persisted)
    // Note: Regular objects require add_property for each property
    group.bench_function("all_regular", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            for i in [1..{num_iterations}]
                o = create(#-1, #0, 0);
                add_property(o, "x", i, {{#0, "wrc"}});
                add_property(o, "y", i * 2, {{#0, "wrc"}});
                s = o.x + o.y;
            endfor
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Workload using nursery objects (all task-local, discarded at end)
    // Nursery objects have dynamic slots - no add_property needed
    group.bench_function("all_nursery_no_promotion", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            for i in [1..{num_iterations}]
                o = create(#-1, #0, 1);
                o.x = i;
                o.y = i * 2;
                s = o.x + o.y;
            endfor
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Mixed workload: nursery objects, some promoted
    group.bench_function("nursery_with_promotion", |b| {
        let db = create_db_with_verb(&format!(
            r#"
            box = create(#-1, #0, 0);
            add_property(box, "saved", 0, {{#0, "wrc"}});
            for i in [1..{num_iterations}]
                o = create(#-1, #0, 1);
                o.x = i;
                o.y = i * 2;
                s = o.x + o.y;
                if (i % 10 == 0)
                    box.saved = o;
                endif
            endfor
            "#
        ));

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "test", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    object_creation_benchmarks,
    property_access_benchmarks,
    promotion_benchmarks,
    introspection_benchmarks,
    realistic_workload_benchmarks
);
criterion_main!(benches);
