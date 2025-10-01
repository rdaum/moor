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
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use moor_common::{
    schema::rpc as moor_rpc,
    tasks::{
        AbortLimitReason, CommandError, Event, Exception, Presentation, SchedulerError,
        VerbProgramError,
    },
};
use moor_var::{Obj, Var, v_obj};
use planus::ReadAsRoot;
use rpc_async_client::{
    pubsub_client::{broadcast_recv, events_recv},
    rpc_client::RpcSendClient,
};
use rpc_common::{
    AuthToken, ClientToken, extract_obj, mk_client_pong_msg, mk_command_msg, mk_detach_msg,
    mk_requested_input_msg,
};
use serde_json::Value;
use std::{
    collections::VecDeque,
    net::SocketAddr,
    time::{Duration, Instant, SystemTime},
};
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
    /// Whether to suppress the automatic newline for this message
    #[serde(skip_serializing_if = "Option::is_none")]
    no_newline: Option<bool>,
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
    /// Normalize MOO content types to standard MIME types
    fn normalize_content_type(content_type: Option<String>) -> Option<String> {
        content_type.map(|ct| {
            match ct.as_str() {
                "text_djot" => "text/djot".to_string(),
                "text_html" => "text/html".to_string(),
                "text_plain" => "text/plain".to_string(),
                _ => ct, // Pass through unknown types unchanged
            }
        })
    }

    pub async fn handle(&mut self, connect_type: moor_rpc::ConnectType, stream: WebSocket) {
        info!("New connection from {}, {}", self.peer_addr, self.player);
        let (mut ws_sender, mut ws_receiver) = stream.split();

        let connect_message = match connect_type {
            moor_rpc::ConnectType::Connected => "*** Connected ***",
            moor_rpc::ConnectType::Reconnected => "*** Reconnected ***",
            moor_rpc::ConnectType::Created => "*** Created ***",
            moor_rpc::ConnectType::NoConnect => {
                unreachable!("NoConnect should not reach WebSocket handler")
            }
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
                Ok(event_msg) = broadcast_recv(&mut self.broadcast_sub) => {
                    let event = event_msg.event().expect("Failed to parse broadcast event");
                    trace!("broadcast_event");
                    match event.event().expect("Missing event union") {
                        moor_rpc::ClientsBroadcastEventUnionRef::ClientsBroadcastPingPong(_server_time) => {
                            let timestamp = SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_nanos() as u64;
                            let pong_msg = mk_client_pong_msg(
                                &self.client_token,
                                timestamp,
                                &self.player,
                                moor_rpc::HostType::WebSocket,
                                self.peer_addr.to_string(),
                            );
                            let _ = self.rpc_client.make_client_rpc_call(self.client_id, pong_msg).await.expect("Unable to send pong to RPC server");
                        }
                    }
                }
                Ok(event_msg) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    let event = event_msg.event().expect("Failed to parse client event");
                    let event_ref = event.event().expect("Missing event union");
                    if let Some(input_request) = self.handle_narrative_event(&mut ws_sender, event_ref).await {
                        expecting_input.push_back(input_request);
                    }
                }
            }
        }

        // We're done now send detach.
        let detach_msg = mk_detach_msg(&self.client_token, true);
        self.rpc_client
            .make_client_rpc_call(self.client_id, detach_msg)
            .await
            .expect("Unable to send detach event to RPC server");
    }

    async fn handle_narrative_event(
        &mut self,
        ws_sender: &mut SplitSink<WebSocket, Message>,
        event_ref: moor_rpc::ClientEventUnionRef<'_>,
    ) -> Option<Uuid> {
        trace!("narrative_event");
        match event_ref {
            moor_rpc::ClientEventUnionRef::SystemMessageEvent(sys_msg) => {
                let author = extract_obj(&sys_msg, "player", |s| s.player())
                    .expect("Failed to extract player");
                let msg = sys_msg.message().expect("Missing message").to_string();
                Self::emit_narrative_sys_msg(
                    ws_sender,
                    &author,
                    Some("text/plain".to_string()),
                    msg,
                )
                .await;
            }
            moor_rpc::ClientEventUnionRef::NarrativeEventMessage(narrative) => {
                let event_ref = narrative.event().expect("Missing narrative event");
                let narrative_event = rpc_common::narrative_event_from_ref(event_ref)
                    .expect("Failed to convert narrative event");
                let msg = narrative_event.event();
                let event_id = narrative_event.event_id().to_string();
                let author = narrative_event.author();
                match &msg {
                    Event::Notify {
                        value: msg,
                        content_type,
                        no_flush,
                        no_newline,
                    } => {
                        // In web context, no_flush isn't directly applicable (websockets handle buffering)
                        // but no_newline can be passed to the client for proper message concatenation.
                        let _ = no_flush; // Acknowledge parameter - not used in websocket context
                        let content_type = Self::normalize_content_type(
                            content_type.as_ref().map(|s| s.to_string()),
                        );
                        Self::emit_narrative_msg(
                            ws_sender,
                            Some(event_id),
                            author,
                            content_type,
                            msg.clone(),
                            if *no_newline { Some(true) } else { None },
                        )
                        .await;
                    }
                    Event::Traceback(exception) => {
                        Self::emit_traceback(ws_sender, Some(event_id), author, exception).await;
                    }
                    Event::Present(p) => {
                        Self::emit_present(ws_sender, Some(event_id), author, p.clone()).await;
                    }
                    Event::Unpresent(id) => {
                        Self::emit_unpresent(ws_sender, Some(event_id), author, id.clone()).await;
                    }
                }
            }
            moor_rpc::ClientEventUnionRef::RequestInputEvent(request_input) => {
                let request_id_ref = request_input.request_id().expect("Missing request_id");
                let request_id_data = request_id_ref.data().expect("Missing request_id data");
                let request_id = Uuid::from_slice(request_id_data).expect("Invalid request UUID");
                return Some(request_id);
            }
            moor_rpc::ClientEventUnionRef::DisconnectEvent(_) => {
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
            moor_rpc::ClientEventUnionRef::TaskErrorEvent(task_err) => {
                let ti = task_err.task_id().expect("Missing task_id") as usize;
                if let Some(pending_event) = self.pending_task.take()
                    && pending_event.task_id != ti
                {
                    error!(
                        "Inbound task response {ti} does not belong to the event we submitted and are expecting {pending_event:?}"
                    );
                }
                let err_ref = task_err.error().expect("Missing error");
                let te = rpc_common::scheduler_error_from_ref(err_ref)
                    .expect("Failed to convert scheduler error");
                self.handle_task_error(ws_sender, te)
                    .await
                    .expect("Unable to handle task error");
            }
            moor_rpc::ClientEventUnionRef::TaskSuccessEvent(task_success) => {
                let ti = task_success.task_id().expect("Missing task_id") as usize;
                if let Some(pending_event) = self.pending_task.take()
                    && pending_event.task_id != ti
                {
                    error!(
                        "Inbound task response {ti} does not belong to the event we submitted and are expecting {pending_event:?}"
                    );
                }
                let value_ref = task_success.result().expect("Missing value");
                let value_bytes = value_ref.data().expect("Missing value data");
                let s = rpc_common::var_from_flatbuffer_bytes(value_bytes)
                    .expect("Failed to decode value");
                Self::emit_value(ws_sender, ValueResult(s)).await;
            }
            moor_rpc::ClientEventUnionRef::PlayerSwitchedEvent(switch) => {
                let new_player = extract_obj(&switch, "new_player", |s| s.new_player())
                    .expect("Failed to extract new_player");
                let new_auth_token_ref = switch.new_auth_token().expect("Missing new_auth_token");
                let new_auth_token = AuthToken(
                    new_auth_token_ref
                        .token()
                        .expect("Missing token")
                        .to_string(),
                );
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
            moor_rpc::ClientEventUnionRef::SetConnectionOptionEvent(_) => {
                // WebSocket connections don't currently support connection options
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

        let command_msg = mk_command_msg(
            &self.client_token,
            &self.auth_token,
            &self.handler_object,
            cmd,
        );

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, command_msg)
            .await
            .expect("Unable to send command to RPC server");

        let reply =
            moor_rpc::ReplyResultRef::read_as_root(&reply_bytes).expect("Failed to parse reply");
        match reply.result().expect("Missing result") {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success.reply().expect("Missing reply");
                match daemon_reply.reply().expect("Missing reply union") {
                    moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted) => {
                        let ti = task_submitted.task_id().expect("Missing task_id") as usize;
                        self.pending_task = Some(PendingTask {
                            task_id: ti,
                            start_time: Instant::now(),
                        });
                    }
                    moor_rpc::DaemonToClientReplyUnionRef::InputThanks(_) => {
                        warn!("Received input thanks unprovoked, out of order")
                    }
                    _ => {
                        error!("Unexpected daemon to client reply");
                    }
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error_ref = failure.error().expect("Missing error");
                let error_code = error_ref.error_code().expect("Missing error code");
                if error_code == moor_rpc::RpcMessageErrorCode::TaskError {
                    if let Ok(Some(scheduler_error_ref)) = error_ref.scheduler_error() {
                        let e = rpc_common::scheduler_error_from_ref(scheduler_error_ref)
                            .expect("Failed to convert scheduler error");
                        self.handle_task_error(ws_sender, e)
                            .await
                            .expect("Unable to handle task error");
                    }
                } else {
                    error!("Unhandled RPC error: {:?}", error_code);
                }
            }
            moor_rpc::ReplyResultUnionRef::HostSuccess(_) => {
                error!("Unexpected host success");
            }
        }
    }

    async fn process_requested_input_line(
        &mut self,
        message: Message,
        expecting_input: &mut VecDeque<Uuid>,
        ws_sender: &mut SplitSink<WebSocket, Message>,
    ) {
        let cmd = match message {
            Message::Text(text) => Var::mk_str(&text), // Convert text to Var::Str
            Message::Binary(bytes) => Var::mk_binary(bytes.to_vec()), // Convert binary to Var::Binary
            _ => {
                warn!("Received unsupported message type for input");
                return;
            }
        };

        let Some(input_request_id) = expecting_input.front() else {
            warn!("Attempt to send reply to input request without an input request");
            return;
        };

        let Some(input_msg) = mk_requested_input_msg(
            &self.client_token,
            &self.auth_token,
            *input_request_id,
            &cmd,
        ) else {
            warn!("Failed to create requested input message");
            return;
        };

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, input_msg)
            .await
            .expect("Unable to send input to RPC server");

        let reply =
            moor_rpc::ReplyResultRef::read_as_root(&reply_bytes).expect("Failed to parse reply");
        match reply.result().expect("Missing result") {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success.reply().expect("Missing reply");
                match daemon_reply.reply().expect("Missing reply union") {
                    moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted) => {
                        let task_id = task_submitted.task_id().expect("Missing task_id") as usize;
                        self.pending_task = Some(PendingTask {
                            task_id,
                            start_time: Instant::now(),
                        });
                        warn!("Got TaskSubmitted when expecting input-thanks")
                    }
                    moor_rpc::DaemonToClientReplyUnionRef::InputThanks(_) => {
                        expecting_input.pop_front();
                    }
                    _ => {
                        error!("Unexpected daemon to client reply");
                    }
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error_ref = failure.error().expect("Missing error");
                let error_code = error_ref.error_code().expect("Missing error code");
                if error_code == moor_rpc::RpcMessageErrorCode::TaskError {
                    if let Ok(Some(scheduler_error_ref)) = error_ref.scheduler_error() {
                        let e = rpc_common::scheduler_error_from_ref(scheduler_error_ref)
                            .expect("Failed to convert scheduler error");
                        self.handle_task_error(ws_sender, e)
                            .await
                            .expect("Unable to handle task error");
                    }
                } else {
                    error!("Unhandled RPC error: {:?}", error_code);
                }
            }
            moor_rpc::ReplyResultUnionRef::HostSuccess(_) => {
                error!("Unexpected host success");
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
                content_type: Self::normalize_content_type(Some(present.content_type.clone())),
                no_newline: None,
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
                no_newline: None,
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
        no_newline: Option<bool>,
    ) {
        Self::emit_narrative(
            ws_sender,
            NarrativeOutput {
                event_id,
                author: var_as_json(&author.clone()),
                system_message: None,
                message: Some(var_as_json(&msg)),
                content_type,
                no_newline,
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
                no_newline: None,
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
                no_newline: None,
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
