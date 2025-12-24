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

//! Load testing and transaction model checking utilities
//!
//! Provides shared session and system control implementations for load test binaries.

use moor_common::tasks::{
    EventLogPurgeResult, EventLogStats, NarrativeEvent, Session, SessionError, SessionFactory,
    SystemControl,
};
use moor_var::{Error, Obj, Symbol, Var};
use std::sync::Arc;

pub mod bench_common;
pub mod elle_common;

/// Simple session implementation for direct scheduler testing.
/// Provides no-op implementations of session operations for use in load tests.
pub struct DirectSession {
    player: Obj,
}

impl DirectSession {
    pub fn new(player: Obj) -> Self {
        Self { player }
    }
}

impl Session for DirectSession {
    fn commit(&self) -> Result<(), SessionError> {
        Ok(())
    }

    fn rollback(&self) -> Result<(), SessionError> {
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(DirectSession::new(self.player)))
    }

    fn request_input(
        &self,
        _player: Obj,
        _input_request_id: uuid::Uuid,
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
        Ok("test-connection".to_string())
    }

    fn disconnect(&self, _player: Obj) -> Result<(), SessionError> {
        Ok(())
    }

    fn connected_players(&self) -> Result<Vec<Obj>, SessionError> {
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
        _player: Option<Obj>,
    ) -> Result<Vec<moor_common::tasks::ConnectionDetails>, SessionError> {
        Ok(vec![])
    }

    fn connection_attributes(&self, _player: Obj) -> Result<Var, SessionError> {
        use moor_var::v_list;
        Ok(v_list(&[]))
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

/// Simple session factory for direct scheduler testing.
pub struct DirectSessionFactory {}

impl SessionFactory for DirectSessionFactory {
    fn mk_background_session(
        self: Arc<Self>,
        player: &Obj,
    ) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(DirectSession::new(*player)))
    }
}

/// No-op system control for direct scheduler testing.
pub struct NoopSystemControl {}

impl SystemControl for NoopSystemControl {
    fn shutdown(&self, _msg: Option<String>) -> Result<(), Error> {
        Ok(())
    }

    fn listeners(&self) -> Result<Vec<(Obj, String, u16, Vec<(Symbol, Var)>)>, Error> {
        Ok(vec![])
    }

    fn listen(
        &self,
        _handler_object: Obj,
        _host_type: &str,
        _port: u16,
        _options: Vec<(Symbol, Var)>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn unlisten(&self, _port: u16, _host_type: &str) -> Result<(), Error> {
        Ok(())
    }

    fn switch_player(&self, _connection_obj: Obj, _new_player: Obj) -> Result<(), Error> {
        Ok(())
    }

    fn rotate_enrollment_token(&self) -> Result<String, Error> {
        Ok(String::new())
    }

    fn player_event_log_stats(
        &self,
        _player: Obj,
        _since: Option<std::time::SystemTime>,
        _until: Option<std::time::SystemTime>,
    ) -> Result<EventLogStats, Error> {
        Ok(EventLogStats::default())
    }

    fn purge_player_event_log(
        &self,
        _player: Obj,
        _before: Option<std::time::SystemTime>,
        _drop_pubkey: bool,
    ) -> Result<EventLogPurgeResult, Error> {
        Ok(EventLogPurgeResult::default())
    }
}
