// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use moor_common::tasks::{ConnectionDetails, NarrativeEvent, Session, SessionError};
use moor_schema::rpc as moor_rpc;
use moor_var::{Obj, Symbol, Var};

use crate::event_log::{EventLogOps, logged_narrative_event_to_flatbuffer};

/// A "session" that runs over the RPC system.
pub struct RpcSession {
    client_id: Uuid,
    player: Obj,
    /// Shared event log for persistent storage across all sessions
    event_log: Arc<dyn EventLogOps>,
    /// Transaction-local buffer for events pending commit (both logged and broadcast)
    transaction_buffer: Mutex<Vec<(Obj, Box<NarrativeEvent>)>>,
    /// Transaction-local buffer for log-only events (logged but not broadcast)
    log_only_buffer: Mutex<Vec<(Obj, Box<NarrativeEvent>)>>,
    send: Sender<SessionActions>,
}

pub enum SessionActions {
    PublishNarrativeEvents(Vec<(Obj, Box<NarrativeEvent>)>),
    RequestClientInput {
        client_id: Uuid,
        connection: Obj,
        request_id: Uuid,
        metadata: Option<Vec<(Symbol, Var)>>,
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
    RequestClientAttributes(Uuid, Obj, oneshot::Sender<Result<Var, SessionError>>),
    SetClientAttribute(Uuid, Obj, Symbol, Var),
    PublishTaskCompletion(Uuid, moor_rpc::ClientEvent),
}

impl RpcSession {
    pub fn new(
        client_id: Uuid,
        player: Obj,
        event_log: Arc<dyn EventLogOps>,
        sender: Sender<SessionActions>,
    ) -> Self {
        Self {
            client_id,
            player,
            event_log,
            transaction_buffer: Mutex::new(Vec::new()),
            log_only_buffer: Mutex::new(Vec::new()),
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
        let log_only_events: Vec<_> = {
            let mut log_only_buffer = self.log_only_buffer.lock().unwrap();
            log_only_buffer.drain(..).collect()
        };

        // Log events from both buffers to the event log
        for (player, event) in events.iter().chain(log_only_events.iter()) {
            let Some(pubkey) = self.event_log.get_pubkey(*player) else {
                continue;
            };

            // Convert to FlatBuffer LoggedNarrativeEvent (always encrypted)
            if let Ok((logged_event, presentation_action)) =
                logged_narrative_event_to_flatbuffer(*player, event.clone(), pubkey)
            {
                self.event_log.append(logged_event, presentation_action);
            }
        }

        // Only publish regular events to connected clients (not log_only_events)
        self.send
            .send(SessionActions::PublishNarrativeEvents(events))
            .map_err(|e| SessionError::CommitError(e.to_string()))?;
        Ok(())
    }

    fn rollback(&self) -> Result<(), SessionError> {
        self.transaction_buffer.lock().unwrap().clear();
        self.log_only_buffer.lock().unwrap().clear();
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

    fn request_input(
        &self,
        player: Obj,
        input_request_id: Uuid,
        metadata: Option<Vec<(Symbol, Var)>>,
    ) -> Result<(), SessionError> {
        self.send
            .send(SessionActions::RequestClientInput {
                client_id: self.client_id,
                connection: player,
                request_id: input_request_id,
                metadata,
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

    fn log_event(&self, player: Obj, event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        self.log_only_buffer.lock().unwrap().push((player, event));
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
            Some(msg) => format!("** Server is shutting down: {msg} **"),
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

    fn connection_attributes(&self, obj: Obj) -> Result<Var, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send
            .send(SessionActions::RequestClientAttributes(
                self.client_id,
                obj,
                tx,
            ))
            .map_err(|_e| SessionError::DeliveryError)?;
        rx.recv().map_err(|_e| SessionError::DeliveryError)?
    }

    fn set_connection_attribute(
        &self,
        connection_obj: Obj,
        key: Symbol,
        value: Var,
    ) -> Result<(), SessionError> {
        self.send
            .send(SessionActions::SetClientAttribute(
                self.client_id,
                connection_obj,
                key,
                value,
            ))
            .map_err(|_e| SessionError::DeliveryError)?;
        Ok(())
    }
}
