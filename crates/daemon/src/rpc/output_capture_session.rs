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

use std::sync::{Arc, Mutex};
use uuid::Uuid;

use moor_common::tasks::{ConnectionDetails, NarrativeEvent, Session, SessionError};
use moor_var::{List, Obj, Symbol, Var};
use tracing::info;

/// A session that captures narrative events in memory for later retrieval
/// Used for unauthenticated system verb calls that don't have active connections
pub struct OutputCaptureSession {
    client_id: Uuid,
    player: Obj,
    /// Captured narrative events
    captured_events: Mutex<Vec<(Obj, Box<NarrativeEvent>)>>,
}

impl OutputCaptureSession {
    pub fn new(client_id: Uuid, player: Obj) -> Self {
        Self {
            client_id,
            player,
            captured_events: Mutex::new(Vec::new()),
        }
    }

    /// Get the captured narrative events
    pub fn take_captured_events(&self) -> Vec<(Obj, Box<NarrativeEvent>)> {
        let mut events = self.captured_events.lock().unwrap();
        events.drain(..).collect()
    }
}

impl Session for OutputCaptureSession {
    fn commit(&self) -> Result<(), SessionError> {
        // For output capture sessions, we don't publish events to clients
        // They are stored in memory and will be returned with the task result
        Ok(())
    }

    fn rollback(&self) -> Result<(), SessionError> {
        let mut events = self.captured_events.lock().unwrap();
        events.clear();
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(Self::new(self.client_id, self.player)))
    }

    fn request_input(&self, _player: Obj, _input_request_id: Uuid) -> Result<(), SessionError> {
        // Input requests are not supported for output capture sessions
        Err(SessionError::CommitError(
            "Input requests not supported for output capture sessions".to_string(),
        ))
    }

    fn send_event(&self, player: Obj, event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        let mut events = self.captured_events.lock().unwrap();
        events.push((player, event));
        info!(
            "OutputCaptureSession: captured narrative event for player {:?}, total events: {}",
            player,
            events.len()
        );
        Ok(())
    }

    fn send_system_msg(&self, _player: Obj, _msg: &str) -> Result<(), SessionError> {
        // System messages are not captured for output capture sessions
        Ok(())
    }

    fn notify_shutdown(&self, _msg: Option<String>) -> Result<(), SessionError> {
        Ok(())
    }

    fn connection_name(&self, _player: Obj) -> Result<String, SessionError> {
        Ok("output-capture-session".to_string())
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
    ) -> Result<Vec<ConnectionDetails>, SessionError> {
        Ok(vec![])
    }

    fn connection_attributes(&self, _obj: Obj) -> Result<Var, SessionError> {
        Ok(Var::from(List::mk_list(&[])))
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
