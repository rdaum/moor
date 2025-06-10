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

use flume::Sender;
use std::sync::Mutex;
use uuid::Uuid;

use moor_common::tasks::NarrativeEvent;
use moor_common::tasks::{ConnectionDetails, Session, SessionError, SessionFactory};
use moor_var::Obj;

use crate::event_log::EventLog;
use crate::rpc_server::RpcServer;

/// A "session" that runs over the RPC system.
pub struct RpcSession {
    client_id: Uuid,
    player: Obj,
    /// Shared event log for persistent storage across all sessions
    event_log: Arc<EventLog>,
    /// Transaction-local buffer for events pending commit
    transaction_buffer: Mutex<Vec<(Obj, Box<NarrativeEvent>)>>,
    send: Sender<SessionActions>,
}

pub(crate) enum SessionActions {
    PublishNarrativeEvents(Vec<(Obj, Box<NarrativeEvent>)>),
    RequestClientInput {
        client_id: Uuid,
        connection: Obj,
        request_id: Uuid,
    },
    SendSystemMessage {
        client_id: Uuid,
        connection: Obj,
        system_message: String,
    },
    RequestConnectionName(Uuid, Obj, oneshot::Sender<Result<String, SessionError>>),
    Disconnect(Uuid, Obj),
    RequestConnectedPlayers(Uuid, oneshot::Sender<Result<Vec<Obj>, SessionError>>),
    RequestConnectedSeconds(Uuid, Obj, oneshot::Sender<Result<f64, SessionError>>),
    RequestIdleSeconds(Uuid, Obj, oneshot::Sender<Result<f64, SessionError>>),
    RequestConnections(
        Uuid,
        Option<Obj>,
        oneshot::Sender<Result<Vec<Obj>, SessionError>>,
    ),
    RequestConnectionDetails(
        Uuid,
        Option<Obj>,
        oneshot::Sender<Result<Vec<ConnectionDetails>, SessionError>>,
    ),
}

impl RpcSession {
    pub fn new(
        client_id: Uuid,
        player: Obj,
        event_log: Arc<EventLog>,
        sender: Sender<SessionActions>,
    ) -> Self {
        Self {
            client_id,
            player,
            event_log,
            transaction_buffer: Mutex::new(Vec::new()),
            send: sender,
        }
    }
}

impl Session for RpcSession {
    fn commit(&self) -> Result<(), SessionError> {
        let events: Vec<_> = {
            let mut transaction_buffer = self.transaction_buffer.lock().unwrap();
            transaction_buffer.drain(..).collect()
        };

        // First, persist all events to the shared EventLog
        for (player, event) in &events {
            self.event_log.append(*player, event.clone());
        }

        // Then, publish them to connected clients
        self.send
            .send(SessionActions::PublishNarrativeEvents(events))
            .map_err(|e| SessionError::CommitError(e.to_string()))?;
        Ok(())
    }

    fn rollback(&self) -> Result<(), SessionError> {
        let mut transaction_buffer = self.transaction_buffer.lock().unwrap();
        transaction_buffer.clear();
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(Self::new(
            self.client_id,
            self.player,
            self.event_log.clone(),
            self.send.clone(),
        )))
    }

    fn request_input(&self, player: Obj, input_request_id: Uuid) -> Result<(), SessionError> {
        self.send
            .send(SessionActions::RequestClientInput {
                client_id: self.client_id,
                connection: player,
                request_id: input_request_id,
            })
            .map_err(|e| SessionError::CommitError(e.to_string()))?;
        Ok(())
    }

    fn send_event(&self, player: Obj, event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        self.transaction_buffer
            .lock()
            .unwrap()
            .push((player, event));
        Ok(())
    }

    fn send_system_msg(&self, player: Obj, msg: &str) -> Result<(), SessionError> {
        self.send
            .send(SessionActions::SendSystemMessage {
                client_id: self.client_id,
                connection: player,
                system_message: msg.to_string(),
            })
            .map_err(|e| SessionError::CommitError(e.to_string()))?;
        Ok(())
    }

    fn notify_shutdown(&self, msg: Option<String>) -> Result<(), SessionError> {
        let shutdown_msg = match msg {
            Some(msg) => format!("** Server is shutting down: {} **", msg),
            None => "** Server is shutting down ** ".to_string(),
        };
        self.send
            .send(SessionActions::SendSystemMessage {
                client_id: self.client_id,
                connection: self.player,
                system_message: shutdown_msg,
            })
            .map_err(|e| SessionError::CommitError(e.to_string()))
    }

    fn connection_name(&self, player: Obj) -> Result<String, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send
            .send(SessionActions::RequestConnectionName(
                self.client_id,
                player,
                tx,
            ))
            .map_err(|_e| SessionError::DeliveryError)?;
        rx.recv().map_err(|_e| SessionError::DeliveryError)?
    }

    fn disconnect(&self, player: Obj) -> Result<(), SessionError> {
        self.send
            .send(SessionActions::Disconnect(self.client_id, player))
            .map_err(|_e| SessionError::DeliveryError)?;
        Ok(())
    }

    fn connected_players(&self) -> Result<Vec<Obj>, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send
            .send(SessionActions::RequestConnectedPlayers(self.client_id, tx))
            .map_err(|_e| SessionError::DeliveryError)?;
        rx.recv().map_err(|_e| SessionError::DeliveryError)?
    }

    fn connected_seconds(&self, player: Obj) -> Result<f64, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send
            .send(SessionActions::RequestConnectedSeconds(
                self.client_id,
                player,
                tx,
            ))
            .map_err(|_e| SessionError::DeliveryError)?;
        rx.recv().map_err(|_e| SessionError::DeliveryError)?
    }

    fn idle_seconds(&self, player: Obj) -> Result<f64, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send
            .send(SessionActions::RequestIdleSeconds(
                self.client_id,
                player,
                tx,
            ))
            .map_err(|_e| SessionError::DeliveryError)?;
        rx.recv().map_err(|_e| SessionError::DeliveryError)?
    }

    fn connections(&self, player: Option<Obj>) -> Result<Vec<Obj>, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send
            .send(SessionActions::RequestConnections(
                self.client_id,
                player,
                tx,
            ))
            .map_err(|_e| SessionError::DeliveryError)?;
        rx.recv().map_err(|_e| SessionError::DeliveryError)?
    }

    fn connection_details(
        &self,
        player: Option<Obj>,
    ) -> Result<Vec<ConnectionDetails>, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send
            .send(SessionActions::RequestConnectionDetails(
                self.client_id,
                player,
                tx,
            ))
            .map_err(|_e| SessionError::DeliveryError)?;
        rx.recv().map_err(|_e| SessionError::DeliveryError)?
    }
}

impl SessionFactory for RpcServer {
    fn mk_background_session(
        self: Arc<Self>,
        player: &Obj,
    ) -> Result<Arc<dyn Session>, SessionError> {
        let client_id = Uuid::new_v4();
        let session = RpcSession::new(
            client_id,
            *player,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        );
        let session = Arc::new(session);
        Ok(session)
    }
}
