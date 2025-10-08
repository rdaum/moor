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

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use moor_schema::{convert::uuid_from_ref, rpc as moor_rpc};
use moor_var::{Obj, v_binary, v_str};
use planus::ReadAsRoot;
use rpc_async_client::{
    pubsub_client::{broadcast_recv, events_recv},
    rpc_client::RpcSendClient,
};
use rpc_common::{
    AuthToken, ClientToken, mk_client_pong_msg, mk_command_msg, mk_detach_msg,
    mk_requested_input_msg,
};
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
    pub async fn handle(&mut self, connect_type: moor_rpc::ConnectType, stream: WebSocket) {
        info!("New connection from {}, {}", self.peer_addr, self.player);
        let (mut ws_sender, mut ws_receiver) = stream.split();

        // Connection message is now sent via SystemMessageEvent from the daemon
        match connect_type {
            moor_rpc::ConnectType::Connected => {
                debug!("Player {} connected", self.player);
            }
            moor_rpc::ConnectType::Reconnected => {
                debug!("Player {} reconnected", self.player);
            }
            moor_rpc::ConnectType::Created => {
                debug!("Player {} created", self.player);
            }
            moor_rpc::ConnectType::NoConnect => {
                unreachable!("NoConnect should not reach WebSocket handler")
            }
        };

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
                            self.process_command_line(line).await;
                        }
                        ReadEvent::InputReply(line) =>{
                            self.process_requested_input_line(line, &mut expecting_input).await;
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
                    // Still need to parse to check for input requests and task completions
                    let event = event_msg.event().expect("Failed to parse client event");
                    let event_ref = event.event().expect("Missing event union");

                    match event_ref {
                        moor_rpc::ClientEventUnionRef::RequestInputEvent(input_request) => {
                            let request_id_ref = input_request.request_id().expect("Missing request_id");
                            let request_id = uuid_from_ref(request_id_ref).expect("Failed to convert UUID");
                            expecting_input.push_back(request_id);
                        }
                        moor_rpc::ClientEventUnionRef::TaskSuccessEvent(_) |
                        moor_rpc::ClientEventUnionRef::TaskErrorEvent(_) => {
                            // Clear the pending task so we can process the next command
                            self.pending_task = None;
                        }
                        _ => {}
                    }
                    // Forward the raw FlatBuffer bytes to the client
                    let bytes = event_msg.consume();
                    let msg = Message::Binary(bytes.into());
                    ws_sender.send(msg).await.ok();
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

    async fn process_command_line(&mut self, line: Message) {
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
                // Task errors are now handled client-side via FlatBuffer events
                warn!("RPC error: {:?}", error_code);
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
    ) {
        let cmd = match message {
            Message::Text(text) => v_str(&text), // Convert text to Var::Str
            Message::Binary(bytes) => v_binary(bytes.to_vec()), // Convert binary to Var::Binary
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
                // Task errors are now handled client-side via FlatBuffer events
                warn!("RPC error: {:?}", error_code);
            }
            moor_rpc::ReplyResultUnionRef::HostSuccess(_) => {
                error!("Unexpected host success");
            }
        }
    }
}
