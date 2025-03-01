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

use std::sync::Arc;

use std::sync::Mutex;
use tracing::trace;
use uuid::Uuid;

use moor_common::tasks::NarrativeEvent;
use moor_kernel::tasks::sessions::{Session, SessionError, SessionFactory};
use moor_var::Obj;

use crate::rpc_server::RpcServer;

/// A "session" that runs over the RPC system.
pub struct RpcSession {
    client_id: Uuid,
    rpc_server: Arc<RpcServer>,
    player: Obj,
    // TODO: manage this buffer better -- e.g. if it grows too big, for long-running tasks, etc. it
    //  should be mmap'd to disk or something.
    // TODO: We could also use Boxcar or other append-only lockless container for this, since we only
    //  ever append.
    session_buffer: Mutex<Vec<(Obj, NarrativeEvent)>>,
}

impl RpcSession {
    pub fn new(client_id: Uuid, rpc_server: Arc<RpcServer>, player: Obj) -> Self {
        Self {
            client_id,
            rpc_server,
            player,
            session_buffer: Default::default(),
        }
    }
}

impl Session for RpcSession {
    fn commit(&self) -> Result<(), SessionError> {
        trace!(player = ?self.player, client_id = ?self.client_id, "Committing session");
        let events: Vec<_> = {
            let mut session_buffer = self.session_buffer.lock().unwrap();
            session_buffer.drain(..).collect()
        };

        let rpc_server = self.rpc_server.clone();
        rpc_server
            .publish_narrative_events(&events[..])
            .map_err(|e| SessionError::CommitError(e.to_string()))?;

        Ok(())
    }

    fn rollback(&self) -> Result<(), SessionError> {
        let mut session_buffer = self.session_buffer.lock().unwrap();
        session_buffer.clear();
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        // We ask the rpc server to create a new session, otherwise we'd need to have a copy of all
        // the info to create a Publish. The rpc server has that, though.
        let new_session = self
            .rpc_server
            .clone()
            .new_session(self.client_id, self.player.clone())?;
        Ok(new_session)
    }

    fn request_input(&self, player: Obj, input_request_id: Uuid) -> Result<(), SessionError> {
        self.rpc_server
            .clone()
            .request_client_input(self.client_id, player, input_request_id)?;
        Ok(())
    }

    fn send_event(&self, player: Obj, event: NarrativeEvent) -> Result<(), SessionError> {
        self.session_buffer.lock().unwrap().push((player, event));
        Ok(())
    }

    fn send_system_msg(&self, player: Obj, msg: &str) -> Result<(), SessionError> {
        self.rpc_server
            .send_system_message(self.client_id, player, msg.to_string())?;
        Ok(())
    }

    fn notify_shutdown(&self, msg: Option<String>) -> Result<(), SessionError> {
        let shutdown_msg = match msg {
            Some(msg) => format!("** Server is shutting down: {} **", msg),
            None => "** Server is shutting down ** ".to_string(),
        };
        self.rpc_server.send_system_message(
            self.client_id,
            self.player.clone(),
            shutdown_msg.clone(),
        )?;
        Ok(())
    }

    fn connection_name(&self, player: Obj) -> Result<String, SessionError> {
        self.rpc_server.connection_name_for(player)
    }

    fn disconnect(&self, player: Obj) -> Result<(), SessionError> {
        self.rpc_server.disconnect(player)
    }

    fn connected_players(&self) -> Result<Vec<Obj>, SessionError> {
        self.rpc_server.connected_players()
    }

    fn connected_seconds(&self, player: Obj) -> Result<f64, SessionError> {
        self.rpc_server.connected_seconds_for(player)
    }

    fn idle_seconds(&self, player: Obj) -> Result<f64, SessionError> {
        self.rpc_server.idle_seconds_for(player)
    }
}

impl SessionFactory for RpcServer {
    fn mk_background_session(
        self: Arc<Self>,
        player: &Obj,
    ) -> Result<Arc<dyn Session>, SessionError> {
        self.clone().new_session(Uuid::new_v4(), player.clone())
    }
}
