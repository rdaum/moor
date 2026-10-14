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

use crate::tasks::scheduler::*;
use moor_common::tasks::NoopSystemControl;
use moor_db::{DatabaseConfig, TxDB};

use crate::tasks::scheduler::test_support::*;

#[test]
fn shutdown_joins_worker_response_thread_with_live_sender() {
    let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    let (_worker_send, worker_recv) = flume::unbounded();
    let scheduler = Scheduler::new(
        semver::Version::new(0, 0, 0),
        Box::new(database),
        Box::new(crate::tasks::NoopTasksDb {}),
        Arc::new(Config::default()),
        Arc::new(NoopSystemControl::default()),
        None,
        Some(worker_recv),
    );
    let threads = scheduler
        .start(Arc::new(NoopSessionFactory))
        .expect("scheduler should start");

    scheduler.stop(None).expect("scheduler should stop");
    threads
        .join()
        .expect("all scheduler-owned threads should stop");
}

#[test]
fn shutdown_collects_gc_worker_handle() {
    let scheduler = scheduler();
    let threads = scheduler
        .start(Arc::new(NoopSessionFactory))
        .expect("scheduler should start");

    scheduler.run_gc_cycle();
    assert!(scheduler.gc_thread.lock().is_some());

    scheduler.stop(None).expect("scheduler should stop");
    assert!(scheduler.gc_thread.lock().is_none());
    let lc = scheduler.lifecycle.lock();
    assert_eq!(lc.gc_phase, gc::GcPhase::Idle);
    drop(lc);

    threads
        .join()
        .expect("all scheduler-owned threads should stop");
}
