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

use super::*;
use crate::tasks::TaskProgramCache;
use moor_common::model::HasUuid;
use moor_var::v_list;

fn eval(client: &SchedulerClient, source: &str) -> TaskHandle {
    client
        .submit_eval_task(
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            source.to_string(),
            None,
            Arc::new(NoopClientSession::new()),
            Arc::new(FeaturesConfig::default()),
        )
        .unwrap()
}

fn setup_probe(source: &str) -> (SchedulerClient, RunningScheduler) {
    setup_scheduler(&[TestVerb {
        name: Symbol::mk("probe"),
        program: compile(source, CompileOptions::default()).unwrap(),
        argspec: VerbArgsSpec::this_none_this(),
    }])
}

#[test]
fn reprogrammed_verb_is_visible_in_the_same_task() {
    let (client, _scheduler) = setup_probe("return 1;");
    let handle = eval(
        &client,
        r#"
        first = #0:probe();
        set_verb_code(#0, "probe", {"return 2;"});
        second = #0:probe();
        set_verb_code(#0, "probe", {"return 3;"});
        suspend(0);
        return {first, second, #0:probe()};
        "#,
    );
    assert_eq!(
        wait_result(&handle).unwrap(),
        v_list(&[1.into(), 2.into(), 3.into()])
    );
}

#[test]
fn active_program_survives_its_own_replacement() {
    let (client, _scheduler) = setup_probe(
        r#"
        if (args) return 1; endif
        set_verb_code(this, "probe", {"return 2;"});
        immediate = this:probe(1);
        suspend(0);
        return {immediate, this:probe(1), 1};
        "#,
    );
    let handle = eval(&client, "return #0:probe();");
    assert_eq!(
        wait_result(&handle).unwrap(),
        v_list(&[2.into(), 2.into(), 1.into()])
    );
}

#[test]
fn suspended_program_sees_another_tasks_replacement() {
    let (client, _scheduler) =
        setup_probe("if (args) return 1; endif suspend(); return {this:probe(1), 1};");
    let handle = eval(&client, "return #0:probe();");
    assert!(matches!(
        handle
            .receiver()
            .recv_timeout(Duration::from_secs(5))
            .unwrap(),
        (_, Ok(TaskNotification::Suspended))
    ));
    let update = eval(
        &client,
        "set_verb_code(#0, \"probe\", {\"return 2;\"}); return 1;",
    );
    assert_eq!(wait_result(&update).unwrap(), v_int(1));
    let resume = eval(&client, &format!("resume({}); return 1;", handle.task_id()));
    assert_eq!(wait_result(&resume).unwrap(), v_int(1));
    assert_eq!(wait_result(&handle).unwrap(), v_list(&[2.into(), 1.into()]));
}

#[test]
fn program_cache_observes_loader_edits_and_rollback() {
    let original = compile("return 1;", CompileOptions::default()).unwrap();
    let replacement = compile("return 2;", CompileOptions::default()).unwrap();
    let name = Symbol::mk("probe");
    let db = setup_database(&[TestVerb {
        name,
        program: original.clone(),
        argspec: VerbArgsSpec::this_none_this(),
    }]);
    let permissions = system_permissions();
    let mut tx = db.new_world_state().unwrap();
    let definition = tx.get_verb(&permissions, &SYSTEM_OBJECT, name).unwrap();
    let mut cache = TaskProgramCache::default();
    let resolve = |cache: &mut TaskProgramCache, tx: &dyn WorldState| {
        cache
            .resolve_verb_slot(
                tx,
                &permissions,
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                definition.uuid(),
            )
            .unwrap()
    };
    let first = resolve(&mut cache, tx.as_ref());
    assert!(!first.cache_hit);
    assert!(resolve(&mut cache, tx.as_ref()).cache_hit);

    // Ordinary property writes do not invalidate program lookups.
    tx.update_property(
        &permissions,
        &SYSTEM_OBJECT,
        Symbol::mk("name"),
        &v_str("renamed"),
    )
    .unwrap();
    assert!(resolve(&mut cache, tx.as_ref()).cache_hit);

    // Loader operations retain the transaction, but must invalidate program lookups.
    let token = tx.verb_program_cache_token();
    let mut loader = tx.as_loader_interface().unwrap();
    loader
        .update_verb(
            &SYSTEM_OBJECT,
            definition.uuid(),
            &[name],
            &SYSTEM_OBJECT,
            definition.flags(),
            definition.args(),
            ProgramType::MooR(replacement.clone()),
        )
        .unwrap();
    let tx = loader.as_world_state().unwrap();
    assert_eq!(tx.verb_program_cache_token().transaction, token.transaction);
    assert_ne!(tx.verb_program_cache_token(), token);
    let second = resolve(&mut cache, tx.as_ref());
    assert!(!second.cache_hit);
    assert_ne!(first.slot.program_ptr, second.slot.program_ptr);
    assert_eq!(cache.program_for_slot(0), Some(&original));
    assert_eq!(cache.program_for_slot(1), Some(&replacement));

    // Retrying after rollback must not execute the abandoned replacement.
    tx.rollback().unwrap();
    let tx = db.new_world_state().unwrap();
    let retried = resolve(&mut cache, tx.as_ref());
    assert!(!retried.cache_hit);
    assert_eq!(cache.program_for_slot(2), Some(&original));
    assert!(resolve(&mut cache, tx.as_ref()).cache_hit);

    // Old frames retain their programs until explicit reclamation proves them unused.
    let live = std::collections::HashSet::from_iter([
        first.slot.program_ptr.addr(),
        retried.slot.program_ptr.addr(),
    ]);
    assert_eq!(cache.reclaim_unreferenced(&live), 1);
    assert_eq!(cache.program_for_slot(0), Some(&original));
    assert_eq!(cache.program_for_slot(1), None);
    assert_eq!(cache.program_for_slot(2), Some(&original));
    assert!(resolve(&mut cache, tx.as_ref()).cache_hit);
}
