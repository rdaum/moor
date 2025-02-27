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

use eyre::Context;
use moor_compiler::to_literal;
use moor_kernel::SchedulerClient;
use moor_kernel::tasks::scheduler_test_utils;
use moor_kernel::tasks::sessions::{NoopClientSession, Session, SessionError, SessionFactory};
use moor_moot::{MootOptions, MootRunner, execute_moot_test};
use moor_values::{Obj, Var, v_none};
use std::path::Path;
use std::sync::Arc;
// TODO: consolidate with what's in kernel/testsuite/moo_suite.rs?

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

    fn eval<S: Into<String>>(&mut self, player: &Obj, command: S) -> eyre::Result<()> {
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

    fn command<S: AsRef<str>>(&mut self, player: &Obj, command: S) -> eyre::Result<()> {
        let command: &str = command.as_ref();
        eprintln!("{player} >> {}", command);
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

    fn read_line(&mut self, _player: &Obj) -> eyre::Result<Option<String>> {
        unimplemented!("Not supported on SchedulerMootRunner");
    }

    fn read_eval_result(&mut self, player: &Obj) -> eyre::Result<Option<Var>> {
        Ok(self
            .eval_result
            .take()
            .inspect(|var| eprintln!("{player} << {}", to_literal(var))))
    }

    fn read_command_result(&mut self, player: &Obj) -> eyre::Result<Option<Var>> {
        self.read_eval_result(player)
    }

    fn none(&self) -> Var {
        v_none()
    }
}

pub(crate) struct NoopSessionFactory {}
impl SessionFactory for NoopSessionFactory {
    fn mk_background_session(
        self: Arc<Self>,
        _player: &Obj,
    ) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(NoopClientSession::new()))
    }
}

pub(crate) fn run_test(options: &MootOptions, scheduler_client: SchedulerClient, path: &Path) {
    execute_moot_test(
        SchedulerMootRunner::new(scheduler_client.clone(), Arc::new(NoopClientSession::new())),
        options,
        path,
        || Ok(()),
    );
}
