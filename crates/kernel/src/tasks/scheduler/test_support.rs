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

//! Shared scheduler builders and a session with explicit commit barriers.
//! Domain tests control the barriers and perform their own transitions and assertions.

use crate::tasks::scheduler::*;
use crate::tasks::{
    TaskStart,
    registry::{RunningTask, RunningTaskPhase, SuspendedTask, WakeCondition},
    task::Task,
    task_control::TaskControl,
};
use moor_common::{
    tasks::{
        ConnectionDetails, NarrativeEvent, NoopClientSession, NoopSystemControl, Session,
        SessionError, SessionFactory, TaskId,
    },
    util::{Instant, Timestamp},
};
use moor_db::{DatabaseConfig, TxDB};
use moor_var::{Obj, SYSTEM_OBJECT, Symbol};
use std::sync::{Barrier, OnceLock};
use uuid::Uuid;

pub(super) struct NoopSessionFactory;

impl SessionFactory for NoopSessionFactory {
    fn mk_background_session(
        self: Arc<Self>,
        _player: &Obj,
    ) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(NoopClientSession::new()))
    }
}

pub(super) struct BlockingCommitSession {
    pub(super) commit_entered: Arc<Barrier>,
    pub(super) release_commit: Arc<Barrier>,
    pub(super) connection_obj: Option<Obj>,
    pub(super) source_connections: Option<Vec<Obj>>,
    pub(super) fail_commit: bool,
}

impl Session for BlockingCommitSession {
    fn commit(&self) -> Result<(), SessionError> {
        self.commit_entered.wait();
        self.release_commit.wait();
        if self.fail_commit {
            return Err(SessionError::CommitError("test session failure".into()));
        }
        Ok(())
    }

    fn rollback(&self) -> Result<(), SessionError> {
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Ok(self)
    }

    fn request_input(
        &self,
        _player: Obj,
        _input_request_id: Uuid,
        _metadata: Option<Vec<(Symbol, Var)>>,
    ) -> Result<(), SessionError> {
        Ok(())
    }

    fn send_event(&self, _player: Obj, _event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        Ok(())
    }

    fn log_event(&self, _player: Obj, _event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        Ok(())
    }

    fn send_system_msg(&self, _player: Obj, _msg: &str) -> Result<(), SessionError> {
        Ok(())
    }

    fn notify_shutdown(&self, _msg: Option<String>) -> Result<(), SessionError> {
        Ok(())
    }

    fn connection_name(&self, _player: Obj) -> Result<String, SessionError> {
        Ok(String::new())
    }

    fn disconnect(&self, _player: Obj) -> Result<(), SessionError> {
        Ok(())
    }

    fn connected_players(&self, _include_all: bool) -> Result<Vec<Obj>, SessionError> {
        Ok(vec![])
    }

    fn connected_seconds(&self, _player: Obj) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn idle_seconds(&self, _player: Obj) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn connections(&self, _player: Option<Obj>) -> Result<Vec<Obj>, SessionError> {
        Ok(vec![])
    }

    fn connection_details(
        &self,
        player: Option<Obj>,
    ) -> Result<Vec<ConnectionDetails>, SessionError> {
        let connection_objs = match (player, &self.source_connections) {
            (Some(_), Some(source_connections)) => source_connections.clone(),
            _ => self.connection_obj.into_iter().collect(),
        };
        Ok(connection_objs
            .into_iter()
            .map(|connection_obj| ConnectionDetails {
                connection_obj,
                peer_addr: String::new(),
                idle_seconds: 0.0,
                acceptable_content_types: vec![],
            })
            .collect())
    }

    fn connection_attributes(&self, _obj: Obj) -> Result<Var, SessionError> {
        Ok(moor_var::v_list(&[]))
    }

    fn set_connection_attribute(
        &self,
        _connection_obj: Obj,
        _key: Symbol,
        _value: Var,
    ) -> Result<(), SessionError> {
        Ok(())
    }
}

pub(super) fn scheduler_with_system_control(system_control: Arc<dyn SystemControl>) -> Scheduler {
    let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    Scheduler::new(
        semver::Version::new(0, 0, 0),
        Box::new(database),
        Box::new(crate::tasks::NoopTasksDb {}),
        Arc::new(Config::default()),
        system_control,
        None,
        None,
    )
}

pub(super) fn scheduler() -> Scheduler {
    scheduler_with_system_control(Arc::new(NoopSystemControl::default()))
}

pub(super) fn suspended_task(task_id: TaskId) -> SuspendedTask {
    let server_options = ServerOptions {
        bg_seconds: 0.0,
        bg_ticks: 0,
        fg_seconds: 0.0,
        fg_ticks: 0,
        max_stack_depth: 0,
        dump_interval: None,
        gc_interval: None,
        max_task_retries: DEFAULT_MAX_TASK_RETRIES,
        max_task_mailbox: DEFAULT_MAX_TASK_MAILBOX,
        db_commit_queue_warn: DEFAULT_DB_COMMIT_QUEUE_WARN,
        db_commit_queue_timeout: DEFAULT_DB_COMMIT_QUEUE_TIMEOUT,
        rollback_on_task_limit: false,
    };
    SuspendedTask {
        enqueued_at: Timestamp::now(),
        wake_condition: WakeCondition::Never,
        task: Task::new(
            task_id,
            SYSTEM_OBJECT,
            SYSTEM_OBJECT,
            TaskStart::StartEval {
                player: SYSTEM_OBJECT,
                program: Default::default(),
                initial_env: None,
            },
            &server_options,
            Arc::new(TaskControl::new()),
        ),
        session: Arc::new(NoopClientSession::new()),
        result_sender: None,
        timer_generation: 0,
    }
}

pub(super) fn insert_active_task(
    scheduler: &Scheduler,
    task_id: TaskId,
    session: Arc<dyn Session>,
) -> Box<Task> {
    let task_start = TaskStart::StartEval {
        player: SYSTEM_OBJECT,
        program: Default::default(),
        initial_env: None,
    };
    let control = Arc::new(TaskControl::new());
    let task = Task::new(
        task_id,
        SYSTEM_OBJECT,
        SYSTEM_OBJECT,
        task_start.clone(),
        scheduler.server_options.load().as_ref(),
        control.clone(),
    );

    let mut lifecycle = scheduler.lifecycle.lock();
    let registration = lifecycle.task_q.register_task(task_id);
    lifecycle.task_q.insert_active(
        task_id,
        RunningTask {
            registration,
            effects: Default::default(),
            phase: RunningTaskPhase::Running,
            player: SYSTEM_OBJECT,
            task_start,
            dispatched_at: Instant::now(),
            run_baseline: Arc::new(OnceLock::new()),
            abort_error: None,
            control,
            session,
            result_sender: None,
        },
    );
    drop(lifecycle);
    task
}
