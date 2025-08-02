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

use crate::host::{serialize_var, var_as_json};
use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use moor_common::tasks::{
    AbortLimitReason, CommandError, Event, Exception, Presentation, SchedulerError,
    VerbProgramError,
};
use moor_var::{Obj, Var, v_obj};
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
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime};
use tmq::subscribe::Subscribe;
use tokio::select;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

const TASK_TIMEOUT: Duration = Duration::from_secs(10);

pub struct WebSocketConnection {
    pub(crate) player: Obj,
    pub(crate) peer_addr: SocketAddr,
    pub(crate) broadcast_sub: Subscribe,
    pub(crate) narrative_sub: Subscribe,
    pub(crate) client_id: Uuid,
    pub(crate) client_token: ClientToken,
    pub(crate) auth_token: AuthToken,
    pub(crate) rpc_client: RpcSendClient,
    pub(crate) handler_object: Obj,
    pub(crate) pending_task: Option<PendingTask>,
}

/// The JSON output of a narrative event.
#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NarrativeOutput {
    /// The unique event ID for this narrative event
    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<String>,
    /// The object that authored or caused the event.
    author: Value,
    /// If this is a system message, this is the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    system_message: Option<String>,
    /// If this is a user message, this is the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<Value>,
    /// If this is a message, the content type of the message, e.g. text/plain, text/html, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<String>,
    /// When the event happened, in the server's system time.
    server_time: SystemTime,
    /// If this is a presentation, the presentation.
    #[serde(skip_serializing_if = "Option::is_none")]
    present: Option<Presentation>,
    /// If this is an unpresent, the id to unpresent.
    #[serde(skip_serializing_if = "Option::is_none")]
    unpresent: Option<String>,
    /// If this is a traceback 'splosion, it's here.
    #[serde(skip_serializing_if = "Option::is_none")]
    traceback: Option<Traceback>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Traceback {
    error: String,
    traceback: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ErrorOutput {
    message: String,
    description: Option<Vec<String>>,
    server_time: SystemTime,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize)]
pub struct ValueResult(#[serde(serialize_with = "serialize_var")] Var);

#[derive(Debug, PartialEq, Eq)]
pub struct PendingTask {
    task_id: usize,
    start_time: Instant,
}

pub enum ReadEvent {
    Command(Message),
    InputReply(Message),
    ConnectionClose,
    PendingEvent,
}

impl WebSocketConnection {
    pub async fn handle(&mut self, connect_type: ConnectType, stream: WebSocket) {
        info!("New connection from {}, {}", self.peer_addr, self.player);
        let (mut ws_sender, mut ws_receiver) = stream.split();

        let connect_message = match connect_type {
            ConnectType::Connected => "*** Connected ***",
            ConnectType::Reconnected => "*** Reconnected ***",
            ConnectType::Created => "*** Created ***",
        };
        Self::emit_narrative_sys_msg(
            &mut ws_sender,
            &self.player,
            Some("text/plain".to_string()),
            connect_message.to_string(),
        )
        .await;

        debug!(client_id = ?self.client_id, "Entering command dispatch loop");

        let mut expecting_input = VecDeque::new();
        loop {
            // We should not send the next line until we've received a narrative event for the
            // previous.
            //
            let input_future = async {
                if let Some(pt) = &self.pending_task
                    && expecting_input.is_empty()
                    && pt.start_time.elapsed() > TASK_TIMEOUT
                {
                    error!(
                        "Task {} stuck without response for more than {TASK_TIMEOUT:?}",
                        pt.task_id
                    );
                    self.pending_task = None;
                } else if self.pending_task.is_some() && expecting_input.is_empty() {
                    return ReadEvent::PendingEvent;
                }

                let Some(Ok(line)) = ws_receiver.next().await else {
                    return ReadEvent::ConnectionClose;
                };

                if !expecting_input.is_empty() {
                    ReadEvent::InputReply(line)
                } else {
                    ReadEvent::Command(line)
                }
            };

            select! {
                line = input_future => {
                    match line {
                        ReadEvent::Command(line) => {
                            self.process_command_line(line, &mut ws_sender).await;
                        }
                        ReadEvent::InputReply(line) =>{
                            self.process_requested_input_line(line, &mut expecting_input, &mut ws_sender).await;
                        }
                        ReadEvent::ConnectionClose => {
                            info!("Connection closed");
                            break;
                        }
                        ReadEvent::PendingEvent => {
                            continue
                        }
                    }
                }
                Ok(event) = broadcast_recv(&mut self.broadcast_sub) => {
                    trace!(?event, "broadcast_event");
                    match event {
                        ClientsBroadcastEvent::PingPong(_server_time) => {
                            let _ = self.rpc_client.make_client_rpc_call(self.client_id,
                                HostClientToDaemonMessage::ClientPong(self.client_token.clone(), SystemTime::now(),
                                    self.handler_object, HostType::WebSocket, self.peer_addr)).await.expect("Unable to send pong to RPC server");

                        }
                    }
                }
                Ok(event) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    if let Some(input_request) = self.handle_narrative_event(&mut ws_sender, event).await {
                        expecting_input.push_back(input_request);
                    }
                }
            }
        }

        // We're done now send detach.
        self.rpc_client
            .make_client_rpc_call(
                self.client_id,
                HostClientToDaemonMessage::Detach(self.client_token.clone()),
            )
            .await
            .expect("Unable to send detach event to RPC server");
    }

    async fn handle_narrative_event(
        &mut self,
        ws_sender: &mut SplitSink<WebSocket, Message>,
        event: ClientEvent,
    ) -> Option<Uuid> {
        trace!(?event, "narrative_event");
        match event {
            ClientEvent::SystemMessage(author, msg) => {
                Self::emit_narrative_sys_msg(
                    ws_sender,
                    &author,
                    Some("text/plain".to_string()),
                    msg,
                )
                .await;
            }
            ClientEvent::Narrative(_author, event) => {
                let msg = event.event();
                let event_id = event.event_id().to_string();
                match &msg {
                    Event::Notify(msg, content_type) => {
                        let content_type = content_type.map(|s| s.to_string());
                        Self::emit_narrative_msg(
                            ws_sender,
                            Some(event_id),
                            event.author(),
                            content_type,
                            msg.clone(),
                        )
                        .await;
                    }
                    Event::Traceback(exception) => {
                        Self::emit_traceback(ws_sender, Some(event_id), event.author(), exception)
                            .await;
                    }
                    Event::Present(p) => {
                        Self::emit_present(ws_sender, Some(event_id), event.author(), p.clone())
                            .await;
                    }
                    Event::Unpresent(id) => {
                        Self::emit_unpresent(ws_sender, Some(event_id), event.author(), id.clone())
                            .await;
                    }
                }
            }
            ClientEvent::RequestInput(request_id) => {
                return Some(request_id);
            }
            ClientEvent::Disconnect() => {
                self.pending_task = None;
                Self::emit_narrative_sys_msg(
                    ws_sender,
                    &self.player,
                    Some("text/plain".to_string()),
                    "** Disconnected **".to_string(),
                )
                .await;
                ws_sender.close().await.expect("Unable to close connection");
            }
            ClientEvent::TaskError(ti, te) => {
                if let Some(pending_event) = self.pending_task.take()
                    && pending_event.task_id != ti
                {
                    error!(
                        "Inbound task response {ti} does not belong to the event we submitted and are expecting {pending_event:?}"
                    );
                }
                self.handle_task_error(ws_sender, te)
                    .await
                    .expect("Unable to handle task error");
            }
            ClientEvent::TaskSuccess(ti, s) => {
                if let Some(pending_event) = self.pending_task.take()
                    && pending_event.task_id != ti
                {
                    error!(
                        "Inbound task response {ti} does not belong to the event we submitted and are expecting {pending_event:?}"
                    );
                }
                Self::emit_value(ws_sender, ValueResult(s)).await;
            }
            ClientEvent::PlayerSwitched {
                new_player,
                new_auth_token,
            } => {
                info!(
                    "Switching player from {} to {} for client {}",
                    self.player, new_player, self.client_id
                );
                self.player = new_player;
                self.auth_token = new_auth_token;
                info!(
                    "Player switched successfully to {} for client {}",
                    new_player, self.client_id
                );
            }
        }

        None
    }

    async fn process_command_line(
        &mut self,
        line: Message,
        ws_sender: &mut SplitSink<WebSocket, Message>,
    ) {
        let line = line.into_text().unwrap();
        let cmd = line.trim().to_string();

        match self
            .rpc_client
            .make_client_rpc_call(
                self.client_id,
                HostClientToDaemonMessage::Command(
                    self.client_token.clone(),
                    self.auth_token.clone(),
                    self.handler_object,
                    cmd,
                ),
            )
            .await
            .expect("Unable to send command to RPC server")
        {
            ReplyResult::ClientSuccess(DaemonToClientReply::TaskSubmitted(ti)) => {
                self.pending_task = Some(PendingTask {
                    task_id: ti,
                    start_time: Instant::now(),
                });
            }
            ReplyResult::ClientSuccess(DaemonToClientReply::InputThanks) => {
                warn!("Received input thanks unprovoked, out of order")
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

    async fn process_requested_input_line(
        &mut self,
        line: Message,
        expecting_input: &mut VecDeque<Uuid>,
        ws_sender: &mut SplitSink<WebSocket, Message>,
    ) {
        let line = line.into_text().unwrap();
        let cmd = line.trim().to_string();

        let Some(input_request_id) = expecting_input.front() else {
            warn!("Attempt to send reply to input request without an input request");
            return;
        };

        match self
            .rpc_client
            .make_client_rpc_call(
                self.client_id,
                HostClientToDaemonMessage::RequestedInput(
                    self.client_token.clone(),
                    self.auth_token.clone(),
                    *input_request_id,
                    cmd,
                ),
            )
            .await
            .expect("Unable to send input to RPC server")
        {
            ReplyResult::ClientSuccess(DaemonToClientReply::TaskSubmitted(task_id)) => {
                self.pending_task = Some(PendingTask {
                    task_id,
                    start_time: Instant::now(),
                });
                warn!("Got TaskSubmitted when expecting input-thanks")
            }
            ReplyResult::ClientSuccess(DaemonToClientReply::InputThanks) => {
                expecting_input.pop_front();
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
            SchedulerError::VerbProgramFailed(VerbProgramError::CompilationError(ce)) => {
                Self::emit_error(
                    ws_sender,
                    ErrorOutput {
                        message: "Verb not programmed.".to_string(),
                        description: Some(vec![ce.to_string()]),
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
            SchedulerError::TaskAbortedException(_e) => {
                // No need to emit anything here, the standard exception handler should show.
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

    async fn emit_present(
        ws_sender: &mut SplitSink<WebSocket, Message>,
        event_id: Option<String>,
        author: &Var,
        present: Presentation,
    ) {
        Self::emit_narrative(
            ws_sender,
            NarrativeOutput {
                event_id,
                author: var_as_json(&author.clone()),
                system_message: None,
                message: None,
                content_type: Some(present.content_type.clone()),
                server_time: SystemTime::now(),
                present: Some(present),
                unpresent: None,
                traceback: None,
            },
        )
        .await;
    }

    async fn emit_unpresent(
        ws_sender: &mut SplitSink<WebSocket, Message>,
        event_id: Option<String>,
        author: &Var,
        id: String,
    ) {
        Self::emit_narrative(
            ws_sender,
            NarrativeOutput {
                event_id,
                author: var_as_json(&author.clone()),
                system_message: None,
                message: None,
                content_type: None,
                server_time: SystemTime::now(),
                present: None,
                unpresent: Some(id),
                traceback: None,
            },
        )
        .await
    }

    async fn emit_narrative_msg(
        ws_sender: &mut SplitSink<WebSocket, Message>,
        event_id: Option<String>,
        author: &Var,
        content_type: Option<String>,
        msg: Var,
    ) {
        Self::emit_narrative(
            ws_sender,
            NarrativeOutput {
                event_id,
                author: var_as_json(&author.clone()),
                system_message: None,
                message: Some(var_as_json(&msg)),
                content_type,
                server_time: SystemTime::now(),
                present: None,
                unpresent: None,
                traceback: None,
            },
        )
        .await;
    }

    async fn emit_narrative_sys_msg(
        ws_sender: &mut SplitSink<WebSocket, Message>,
        author: &Obj,
        content_type: Option<String>,
        msg: String,
    ) {
        Self::emit_narrative(
            ws_sender,
            NarrativeOutput {
                event_id: None, // System messages don't have event IDs
                author: var_as_json(&v_obj(*author)),
                system_message: Some(msg),
                message: None,
                content_type,
                server_time: SystemTime::now(),
                present: None,
                unpresent: None,
                traceback: None,
            },
        )
        .await;
    }

    async fn emit_traceback(
        ws_sender: &mut SplitSink<WebSocket, Message>,
        event_id: Option<String>,
        author: &Var,
        exception: &Exception,
    ) {
        let mut traceback = vec![];
        for frame in &exception.backtrace {
            let Some(s) = frame.as_string() else {
                continue;
            };
            traceback.push(s.to_string());
        }
        Self::emit_narrative(
            ws_sender,
            NarrativeOutput {
                event_id,
                author: var_as_json(author),
                system_message: None,
                message: None,
                content_type: None,
                server_time: SystemTime::now(),
                present: None,
                unpresent: None,
                traceback: Some(Traceback {
                    error: format!("{exception}"),
                    traceback,
                }),
            },
        )
        .await;
    }

    async fn emit_narrative(ws_sender: &mut SplitSink<WebSocket, Message>, msg: NarrativeOutput) {
        // Serialize to JSON.
        let msg = serde_json::to_string(&msg).unwrap();
        let msg = Message::Text(msg.into());
        ws_sender.send(msg).await.ok();
    }

    async fn emit_error(ws_sender: &mut SplitSink<WebSocket, Message>, msg: ErrorOutput) {
        // Serialize to JSON.
        let msg = serde_json::to_string(&msg).unwrap();
        let msg = Message::Text(msg.into());
        ws_sender.send(msg).await.ok();
    }

    async fn emit_value(ws_sender: &mut SplitSink<WebSocket, Message>, msg: ValueResult) {
        // Serialize to JSON.
        let msg = serde_json::to_string(&msg).unwrap();
        let msg = Message::Text(msg.into());
        ws_sender.send(msg).await.ok();
    }
}
