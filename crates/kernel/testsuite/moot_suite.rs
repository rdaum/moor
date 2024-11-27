// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Moot is a simple text-based test format for testing the kernel.
//!
//! See example.moot for a full-fledged example

use std::{path::Path, sync::Arc};

use eyre::Context;

use crate::common::create_fjall_db;
#[cfg(feature = "relbox")]
use common::create_relbox_db;
use common::{create_wiredtiger_db, testsuite_dir};
use moor_compiler::to_literal;
use moor_db::Database;
use moor_kernel::tasks::sessions::{NoopSystemControl, SessionError, SessionFactory};
use moor_kernel::tasks::NoopTasksDb;
use moor_kernel::{
    config::Config,
    tasks::{
        scheduler::Scheduler,
        scheduler_test_utils,
        sessions::{NoopClientSession, Session},
    },
    SchedulerClient,
};
use moor_moot::{execute_moot_test, MootRunner};
use moor_values::{v_none, Objid, Var};

mod common;

#[derive(Clone)]
struct SchedulerMootRunner {
    scheduler: SchedulerClient,
    session: Arc<dyn Session>,
    eval_result: Option<Var>,
}
impl SchedulerMootRunner {
    fn new(scheduler: SchedulerClient, session: Arc<dyn Session>) -> Self {
        Self {
            scheduler,
            session,
            eval_result: None,
        }
    }
}
impl MootRunner for SchedulerMootRunner {
    type Value = Var;

    fn eval<S: Into<String>>(&mut self, player: &Objid, command: S) -> eyre::Result<()> {
        let command = command.into();
        eprintln!("{player} >> ; {command}");
        self.eval_result = Some(
            scheduler_test_utils::call_eval(
                self.scheduler.clone(),
                self.session.clone(),
                player,
                command.clone(),
            )
            .wrap_err(format!(
                "SchedulerMootRunner::eval({player}, {:?})",
                command
            ))?,
        );
        Ok(())
    }

    fn command<S: AsRef<str>>(&mut self, player: &Objid, command: S) -> eyre::Result<()> {
        let command: &str = command.as_ref();
        eprintln!("{player} >> ; {}", command);
        self.eval_result = Some(
            scheduler_test_utils::call_command(
                self.scheduler.clone(),
                self.session.clone(),
                player,
                command,
            )
            .wrap_err(format!(
                "SchedulerMootRunner::command({player}, {:?})",
                command
            ))?,
        );
        Ok(())
    }

    fn none(&self) -> Var {
        v_none()
    }

    fn read_line(&mut self, _player: &Objid) -> eyre::Result<Option<String>> {
        unimplemented!("Not supported on SchedulerMootRunner");
    }

    fn read_eval_result(&mut self, player: &Objid) -> eyre::Result<Option<Var>> {
        Ok(self
            .eval_result
            .take()
            .inspect(|var| eprintln!("{player} << {}", to_literal(var))))
    }
}

#[cfg(feature = "relbox")]
fn test_relbox(path: &Path) {
    test(create_relbox_db(), path);
}
#[cfg(feature = "relbox")]
test_each_file::test_each_path! { in "./crates/kernel/testsuite/moot" as relbox => test_relbox }

fn test_wiredtiger(path: &Path) {
    test(create_wiredtiger_db(), path);
}
test_each_file::test_each_path! { in "./crates/kernel/testsuite/moot" as wiredtiger => test_wiredtiger }

fn test_fjall(path: &Path) {
    test(create_fjall_db(), path);
}
test_each_file::test_each_path! { in "./crates/kernel/testsuite/moot" as fjall => test_fjall }

struct NoopSessionFactory {}
impl SessionFactory for NoopSessionFactory {
    fn mk_background_session(
        self: Arc<Self>,
        _player: &Objid,
    ) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(NoopClientSession::new()))
    }
}

fn test(db: Box<dyn Database>, path: &Path) {
    if path.is_dir() {
        return;
    }
    let tasks_db = Box::new(NoopTasksDb {});
    let scheduler = Scheduler::new(
        db,
        tasks_db,
        Arc::new(Config::default()),
        Arc::new(NoopSystemControl::default()),
    );
    let scheduler_client = scheduler.client().unwrap();
    let session_factory = Arc::new(NoopSessionFactory {});
    let scheduler_loop_jh = std::thread::Builder::new()
        .name("moor-scheduler".to_string())
        .spawn(move || scheduler.run(session_factory.clone()))
        .expect("Failed to spawn scheduler");

    execute_moot_test(
        SchedulerMootRunner::new(scheduler_client.clone(), Arc::new(NoopClientSession::new())),
        path,
        || Ok(()),
    );

    scheduler_client
        .submit_shutdown("Test is done")
        .expect("Failed to shut down scheduler");
    scheduler_loop_jh
        .join()
        .expect("Failed to join() scheduler");
}

#[test]
#[ignore = "Useful for debugging; just run a single test"]
fn test_single() {
    // cargo test -p moor-kernel --test moot-suite test_single -- --ignored
    // CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --test moot-suite -- test_single --ignored
    test_fjall(&testsuite_dir().join("moot/truthiness.moot"));
}
