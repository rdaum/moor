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

//! End-to-end task execution through the real scheduler and transactional database.
//! Shared builders keep task submission and result observation explicit in each domain test.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use moor_common::{
    model::{
        ArgSpec, CommitResult, ObjFlag, ObjectKind, ObjectRef, PrepSpec, PropFlag, VerbArgsSpec,
        VerbFlag, WorldState, WorldStateError, WorldStateSource,
        loader::{LoaderInterface, SnapshotInterface},
    },
    tasks::{
        CommandError, MockClientSession, NoopClientSession, NoopSystemControl, SchedulerError,
        SessionError, SessionFactory,
    },
    util::BitEnum,
};
use moor_compiler::{CompileOptions, Program, compile};
use moor_db::{Database, DatabaseConfig, GCInterface, SnapshotCallback, TxDB};
use moor_var::{
    E_DIV, List, NOTHING, Obj, SYSTEM_OBJECT, Symbol, Var, program::ProgramType, v_empty_str,
    v_int, v_obj, v_str,
};

use crate::{
    config::{Config, FeaturesConfig},
    tasks::{
        NoopTasksDb, TaskHandle, TaskNotification,
        scheduler::{Scheduler, SchedulerThreads},
        scheduler_client::SchedulerClient,
    },
};

mod batch;
mod commands;
mod execution;
mod input;
mod limits;
mod programs;
mod transactions;

struct TestVerb {
    name: Symbol,
    program: Program,
    argspec: VerbArgsSpec,
}

struct NoopSessionFactory;

impl SessionFactory for NoopSessionFactory {
    fn mk_background_session(
        self: Arc<Self>,
        _player: &Obj,
    ) -> Result<Arc<dyn moor_common::tasks::Session>, SessionError> {
        Ok(Arc::new(NoopClientSession::new()))
    }
}

struct RunningScheduler {
    client: SchedulerClient,
    threads: Option<SchedulerThreads>,
}

impl Drop for RunningScheduler {
    fn drop(&mut self) {
        let _ = self.client.submit_shutdown("Task test complete");
        if let Some(threads) = self.threads.take() {
            let _ = threads.join();
        }
    }
}

fn system_permissions() -> moor_common::model::TaskPermissions {
    moor_common::model::TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new())
}

/// Create a TxDB, populate it with a system object (wizard/programmer),
/// optionally add verbs, and commit.
fn setup_database(verbs: &[TestVerb]) -> TxDB {
    let (db, _) = TxDB::try_open(None, DatabaseConfig::default()).unwrap();
    let mut tx = db.new_world_state().unwrap();

    let sysobj = tx
        .create_object(
            &system_permissions(),
            &NOTHING,
            &SYSTEM_OBJECT,
            ObjFlag::all_flags(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    tx.update_property(
        &system_permissions(),
        &sysobj,
        Symbol::mk("name"),
        &v_str("system"),
    )
    .unwrap();
    tx.update_property(
        &system_permissions(),
        &sysobj,
        Symbol::mk("programmer"),
        &v_int(1),
    )
    .unwrap();
    tx.update_property(
        &system_permissions(),
        &sysobj,
        Symbol::mk("wizard"),
        &v_int(1),
    )
    .unwrap();

    for TestVerb {
        name,
        program,
        argspec,
    } in verbs
    {
        tx.add_verb(
            &system_permissions(),
            &SYSTEM_OBJECT,
            vec![*name],
            &SYSTEM_OBJECT,
            BitEnum::new_with(VerbFlag::Exec),
            *argspec,
            ProgramType::MooR(program.clone()),
        )
        .unwrap();
    }
    tx.commit().unwrap();

    db
}

fn start_scheduler(database: Box<dyn Database>) -> (SchedulerClient, RunningScheduler) {
    let scheduler = Scheduler::new(
        semver::Version::new(0, 0, 0),
        database,
        Box::new(NoopTasksDb {}),
        Arc::new(Config::default()),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    );
    let threads = scheduler
        .start(Arc::new(NoopSessionFactory))
        .expect("Failed to start scheduler");
    let client = scheduler.client().unwrap();
    let running_scheduler = RunningScheduler {
        client: client.clone(),
        threads: Some(threads),
    };
    (client, running_scheduler)
}

fn setup_scheduler(verbs: &[TestVerb]) -> (SchedulerClient, RunningScheduler) {
    start_scheduler(Box::new(setup_database(verbs)))
}

/// Wait for a task result, handling suspended notifications.
fn wait_result(handle: &TaskHandle) -> Result<moor_var::Var, SchedulerError> {
    loop {
        match handle
            .receiver()
            .recv_timeout(Duration::from_secs(5))
            .expect("Task result timed out")
        {
            (_, Ok(TaskNotification::Result(v))) => return Ok(v),
            (_, Ok(TaskNotification::Suspended)) => continue,
            (_, Err(e)) => return Err(e),
        }
    }
}
