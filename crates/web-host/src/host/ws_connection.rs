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

use crate::host::{serialize_var, var_as_json};
use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use moor_values::tasks::{AbortLimitReason, CommandError, Event, SchedulerError, VerbProgramError};
use moor_values::{Objid, Var};
use rpc_async_client::pubsub_client::broadcast_recv;
use rpc_async_client::pubsub_client::events_recv;
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_common::ClientsBroadcastEvent;
use rpc_common::{
    AuthToken, ClientToken, ConnectType, DaemonToClientReply, HostClientToDaemonMessage,
    ReplyResult, RpcMessageError,
};
use rpc_common::{ClientEvent, HostType};
use serde_json::Value;
use std::net::SocketAddr;
use std::time::SystemTime;
use tmq::subscribe::Subscribe;
use tokio::select;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

pub struct WebSocketConnection {
    pub(crate) player: Objid,
    pub(crate) peer_addr: SocketAddr,
    pub(crate) broadcast_sub: Subscribe,
    pub(crate) narrative_sub: Subscribe,
    pub(crate) client_id: Uuid,
    pub(crate) client_token: ClientToken,
    pub(crate) auth_token: AuthToken,
    pub(crate) rpc_client: RpcSendClient,
    pub(crate) handler_object: Objid,
}

/// The JSON output of a narrative event.
#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NarrativeOutput {
    author: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<String>,
    server_time: SystemTime,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ErrorOutput {
    message: String,
    description: Option<Vec<String>>,
    server_time: SystemTime,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize)]
pub struct ValueResult(#[serde(serialize_with = "serialize_var")] Var);

impl WebSocketConnection {
    pub async fn handle(&mut self, connect_type: ConnectType, stream: WebSocket) {
        info!("New connection from {}, {}", self.peer_addr, self.player);
        let (mut ws_sender, mut ws_receiver) = stream.split();

        let connect_message = match connect_type {
            ConnectType::Connected => "*** Connected ***",
            ConnectType::Reconnected => "*** Reconnected ***",
            ConnectType::Created => "*** Created ***",
        };
        Self::emit_narrative(
            &mut ws_sender,
            NarrativeOutput {
                author: self.player.id(),
                system_message: Some(connect_message.to_string()),
                message: None,
                content_type: Some("text/plain".to_string()),
                server_time: SystemTime::now(),
            },
        )
        .await;

        debug!(client_id = ?self.client_id, "Entering command dispatch loop");

        let mut expecting_input = None;
        loop {
            select! {
                line = ws_receiver.next() => {
                    let Some(Ok(line)) = line else {
                        info!("Connection closed");
                        return;
                    };
                    self.process_line(line, &mut expecting_input, &mut ws_sender).await;
                }
                Ok(event) = broadcast_recv(&mut self.broadcast_sub) => {
                    trace!(?event, "broadcast_event");
                    match event {
                        ClientsBroadcastEvent::PingPong(_server_time) => {
                            let _ = self.rpc_client.make_client_rpc_call(self.client_id,
                                HostClientToDaemonMessage::ClientPong(self.client_token.clone(), SystemTime::now(),
                                    self.handler_object.clone(), HostType::WebSocket, self.peer_addr)).await.expect("Unable to send pong to RPC server");

                        }
                    }
                }
                Ok(event) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    trace!(?event, "narrative_event");
                    match event {
                        ClientEvent::SystemMessage(author, msg) => {
                            Self::emit_narrative(&mut ws_sender, NarrativeOutput {
                                author: author.id(),
                                system_message: Some(msg),
                                message: None,
                                content_type: Some("text/plain".to_string()),
                                server_time: SystemTime::now(),
                            }).await;
                        }
                        ClientEvent::Narrative(_author, event) => {
                            let msg = event.event();
                            let Event::Notify(msg, content_type) = msg;
                            let content_type = content_type.map(|s| s.to_string());
                            Self::emit_narrative(&mut ws_sender, NarrativeOutput {
                                author: event.author.id(),
                                system_message: None,
                                message: Some(var_as_json(&msg)),
                                content_type,
                                server_time: event.timestamp(),
                            }).await;
                        }
                        ClientEvent::RequestInput(request_id) => {
                            expecting_input = Some(request_id);
                        }
                        ClientEvent::Disconnect() => {
                            Self::emit_narrative(&mut ws_sender, NarrativeOutput {
                                author: self.player.id(),
                                system_message: Some("** Disconnected **".to_string()),
                                message: None,
                                content_type: Some("text/plain".to_string()),
                                server_time: SystemTime::now(),
                            }).await;
                            ws_sender.close().await.expect("Unable to close connection");
                            return ;
                        }
                        ClientEvent::TaskError(te) => {
                            self.handle_task_error(&mut ws_sender, te).await.expect("Unable to handle task error");
                        }
                        ClientEvent::TaskSuccess(s) => {
                            Self::emit_value(&mut ws_sender, ValueResult(s)).await;
                        }
                    }
                }
            }
        }
    }

    async fn process_line(
        &mut self,
        line: Message,
        expecting_input: &mut Option<u128>,
        ws_sender: &mut SplitSink<WebSocket, Message>,
    ) {
        let line = line.into_text().unwrap();
        let cmd = line.trim().to_string();

        let response = match expecting_input.take() {
            Some(input_request_id) => self
                .rpc_client
                .make_client_rpc_call(
                    self.client_id,
                    HostClientToDaemonMessage::RequestedInput(
                        self.client_token.clone(),
                        self.auth_token.clone(),
                        input_request_id,
                        cmd,
                    ),
                )
                .await
                .expect("Unable to send input to RPC server"),
            None => self
                .rpc_client
                .make_client_rpc_call(
                    self.client_id,
                    HostClientToDaemonMessage::Command(
                        self.client_token.clone(),
                        self.auth_token.clone(),
                        self.handler_object.clone(),
                        cmd,
                    ),
                )
                .await
                .expect("Unable to send command to RPC server"),
        };

        match response {
            ReplyResult::ClientSuccess(DaemonToClientReply::CommandSubmitted(_))
            | ReplyResult::ClientSuccess(DaemonToClientReply::InputThanks) => {
                // Nothing to do
            }
            ReplyResult::Failure(RpcMessageError::TaskError(e)) => {
                self.handle_task_error(ws_sender, e)
                    .await
                    .expect("Unable to handle task error");
            }
            ReplyResult::Failure(e) => {
                error!("Unhandled RPC error: {:?}", e);
            }
            ReplyResult::ClientSuccess(s) => {
                error!("Unexpected RPC success: {:?}", s);
            }
            ReplyResult::HostSuccess(hs) => {
                error!("Unexpected host success: {:?}", hs);
            }
        }
    }

    async fn handle_task_error(
        &mut self,
        ws_sender: &mut SplitSink<WebSocket, Message>,
        task_error: SchedulerError,
    ) -> Result<(), eyre::Error> {
        match task_error {
            SchedulerError::CommandExecutionError(CommandError::CouldNotParseCommand) => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "I don't understand that.".to_string(),
                        description: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await;
            }
            SchedulerError::CommandExecutionError(CommandError::NoObjectMatch) => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "I don't see that here.".to_string(),
                        description: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await
            }
            SchedulerError::CommandExecutionError(CommandError::NoCommandMatch) => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "I don't know how to do that.".to_string(),
                        description: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await
            }
            SchedulerError::CommandExecutionError(CommandError::PermissionDenied) => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "You can't do that.".to_string(),
                        description: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await
            }
            SchedulerError::VerbProgramFailed(VerbProgramError::CompilationError(lines)) => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "Verb not programmed.".to_string(),
                        description: Some(lines),
                        server_time: SystemTime::now(),
                    },
                )
                .await
            }
            SchedulerError::VerbProgramFailed(VerbProgramError::NoVerbToProgram) => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "Verb not programmed.".to_string(),
                        description: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await
            }
            SchedulerError::TaskAbortedLimit(AbortLimitReason::Ticks(_)) => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "Task ran out of ticks".to_string(),
                        description: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await
            }
            SchedulerError::TaskAbortedLimit(AbortLimitReason::Time(_)) => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "Task ran out of seconds".to_string(),
                        description: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await
            }
            SchedulerError::TaskAbortedError => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "Task aborted".to_string(),
                        description: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await
            }
            SchedulerError::TaskAbortedException(e) => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "Task exception".to_string(),
                        description: Some(vec![format!("{}", e)]),
                        server_time: SystemTime::now(),
                    },
                )
                .await
            }
            SchedulerError::TaskAbortedCancelled => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "Task cancelled".to_string(),
                        description: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await
            }
            _ => {
                warn!(?task_error, "Unhandled unexpected task error");
            }
        }
        Ok(())
    }

    async fn emit_narrative(ws_sender: &mut SplitSink<WebSocket, Message>, msg: NarrativeOutput) {
        // Serialize to JSON.
        let msg = serde_json::to_string(&msg).unwrap();
        let msg = Message::Text(msg);
        ws_sender
            .send(msg)
            .await
            .expect("Unable to send message to client");
    }

    async fn emit_error(ws_sender: &mut SplitSink<WebSocket, Message>, msg: ErrorOutput) {
        // Serialize to JSON.
        let msg = serde_json::to_string(&msg).unwrap();
        let msg = Message::Text(msg);
        ws_sender
            .send(msg)
            .await
            .expect("Unable to send message to client");
    }

    async fn emit_value(ws_sender: &mut SplitSink<WebSocket, Message>, msg: ValueResult) {
        // Serialize to JSON.
        let msg = serde_json::to_string(&msg).unwrap();
        let msg = Message::Text(msg);
        ws_sender
            .send(msg)
            .await
            .expect("Unable to send message to client");
    }
}
