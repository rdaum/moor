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

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use moor_schema::{
    convert::uuid_from_ref, convert::var_from_flatbuffer_ref, rpc as moor_rpc,
    var as moor_var_schema,
};
use moor_var::{Obj, Var, v_str};
use planus::ReadAsRoot;
use rpc_async_client::{
    pubsub_client::{broadcast_recv, events_recv},
    rpc_client::RpcClient,
};
use rpc_common::{
    AuthToken, ClientToken, mk_client_pong_msg, mk_command_msg, mk_detach_msg,
    mk_requested_input_msg, read_reply_result,
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
const WEBSOCKET_PING_INTERVAL: Duration = Duration::from_secs(30);

// Application-level heartbeat to detect zombie WebSocket connections.
// Unlike WebSocket ping/pong (handled by browser at protocol level), this requires
// JavaScript to process and respond, proving the client is actually alive.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(15);
const HEARTBEAT_REQUEST: u8 = 0x02;
const HEARTBEAT_RESPONSE: u8 = 0x01;

fn log_rpc_failure(failure: moor_rpc::FailureRef<'_>) {
    let error_ref = failure.error().expect("Missing error");
    let error_code = error_ref.error_code().expect("Missing error code");
    // Task errors are now handled client-side via FlatBuffer events
    warn!("RPC error: {:?}", error_code);
}

pub struct WebSocketConnection {
    pub(crate) player: Obj,
    pub(crate) peer_addr: SocketAddr,
    pub(crate) broadcast_sub: Subscribe,
    pub(crate) narrative_sub: Subscribe,
    pub(crate) client_id: Uuid,
    pub(crate) client_token: ClientToken,
    pub(crate) auth_token: AuthToken,
    pub(crate) rpc_client: RpcClient,
    pub(crate) handler_object: Obj,
    pub(crate) pending_task: Option<PendingTask>,
    pub(crate) close_code: Option<u16>,
    pub(crate) is_logout: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PendingTask {
    task_id: usize,
    start_time: Instant,
}

pub enum ReadEvent {
    Command(Message),
    InputReply(Message),
    ConnectionClose {
        close_code: Option<u16>,
        is_logout: bool,
    },
    PendingEvent,
    Ping(Vec<u8>),
    HeartbeatResponse,
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
        let mut ping_interval = tokio::time::interval(WEBSOCKET_PING_INTERVAL);
        let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        let mut pending_heartbeat: Option<Instant> = None;
        loop {
            // Check for heartbeat timeout - if we sent a heartbeat and haven't received
            // a response within HEARTBEAT_TIMEOUT, the connection is likely zombie.
            if let Some(sent_time) = pending_heartbeat {
                if sent_time.elapsed() > HEARTBEAT_TIMEOUT {
                    warn!(
                        "Heartbeat timeout after {:?} - client JS not responding, closing connection",
                        sent_time.elapsed()
                    );
                    break;
                }
            }

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

                loop {
                    let Some(Ok(msg)) = ws_receiver.next().await else {
                        return ReadEvent::ConnectionClose {
                            close_code: None,
                            is_logout: false,
                        };
                    };

                    // Filter out WebSocket control frames (ping, pong, close)
                    // Only process actual data messages (text/binary)
                    match msg {
                        Message::Binary(ref data) if data.len() == 1 && data[0] == 0x00 => {
                            // Application-level keepalive (single zero byte)
                            // Used to prevent proxy idle timeouts (e.g., Cloudflare)
                            trace!("Received keepalive from client");
                            continue;
                        }
                        Message::Binary(ref data)
                            if data.len() == 1 && data[0] == HEARTBEAT_RESPONSE =>
                        {
                            // Application-level heartbeat response
                            trace!("Received heartbeat response from client");
                            return ReadEvent::HeartbeatResponse;
                        }
                        Message::Text(_) | Message::Binary(_) => {
                            if !expecting_input.is_empty() {
                                return ReadEvent::InputReply(msg);
                            } else {
                                return ReadEvent::Command(msg);
                            }
                        }
                        Message::Ping(payload) => {
                            // Client sent us a ping - we must respond with a pong
                            trace!("Received ping from client");
                            return ReadEvent::Ping(payload.to_vec());
                        }
                        Message::Pong(_) => {
                            // Client responded to our ping
                            trace!("Received pong from client");
                            continue;
                        }
                        Message::Close(close_frame) => {
                            let close_code = close_frame.as_ref().map(|f| f.code);
                            let close_reason = close_frame.as_ref().map(|f| f.reason.to_string());
                            if let Some(frame) = &close_frame {
                                debug!(
                                    "WebSocket close frame received: code={}, reason={:?}",
                                    frame.code, frame.reason
                                );
                            }
                            // Check if the reason is "LOGOUT" to determine if this is an explicit logout
                            let is_logout = close_reason.as_deref() == Some("LOGOUT");
                            if is_logout {
                                debug!("Detected explicit logout from close reason");
                            }
                            return ReadEvent::ConnectionClose {
                                close_code,
                                is_logout,
                            };
                        }
                    }
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
                        ReadEvent::ConnectionClose { close_code, is_logout } => {
                            self.close_code = close_code;
                            self.is_logout = is_logout;
                            info!("Connection closed with code: {:?}, is_logout: {}", close_code, is_logout);
                            break;
                        }
                        ReadEvent::PendingEvent => {
                            continue
                        }
                        ReadEvent::Ping(payload) => {
                            trace!("Responding to client ping with pong");
                            if let Err(e) = ws_sender.send(Message::Pong(payload.into())).await {
                                error!("Failed to send pong response: {}", e);
                                break;
                            }
                        }
                        ReadEvent::HeartbeatResponse => {
                            trace!("Heartbeat response received, client JS is alive");
                            pending_heartbeat = None;
                        }
                    }
                }
                _ = heartbeat_interval.tick() => {
                    // Send application-level heartbeat request
                    // Client must respond with HEARTBEAT_RESPONSE to prove JS is processing
                    trace!("Sending heartbeat request");
                    if let Err(e) = ws_sender.send(Message::Binary(vec![HEARTBEAT_REQUEST].into())).await {
                        error!("Failed to send heartbeat request: {}", e);
                        break;
                    }
                    pending_heartbeat = Some(Instant::now());
                }
                _ = ping_interval.tick() => {
                    trace!("Sending WebSocket ping");
                    if let Err(e) = ws_sender.send(Message::Ping(vec![].into())).await {
                        error!("Failed to send WebSocket ping: {}", e);
                        break;
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
                    // Parse to check for input requests and task completions
                    let event = event_msg.event().expect("Failed to parse client event");
                    let event_ref = event.event().expect("Missing event union");

                    match event_ref {
                        moor_rpc::ClientEventUnionRef::RequestInputEvent(input_request) => {
                            let request_id_ref = input_request.request_id().expect("Missing request_id");
                            let request_id = uuid_from_ref(request_id_ref).expect("Failed to convert UUID");
                            expecting_input.push_back(request_id);
                        }
                        moor_rpc::ClientEventUnionRef::TaskSuccessEvent(_) |
                        moor_rpc::ClientEventUnionRef::TaskErrorEvent(_) |
                        moor_rpc::ClientEventUnionRef::TaskSuspendedEvent(_) => {
                            // Clear the pending task so we can process the next command
                            self.pending_task = None;
                        }
                        _ => {}
                    }

                    // Forward the raw FlatBuffer bytes to the client
                    let bytes = event_msg.consume();
                    let msg = Message::Binary(bytes.into());
                    if let Err(e) = ws_sender.send(msg).await {
                        error!("Failed to send message to websocket: {}", e);
                        break;
                    }
                }
            }
        }

        // Detach transport
        // Use the is_logout flag from the close reason to determine if session should be destroyed
        // If close reason was "LOGOUT", destroy session. Otherwise, keep alive for reconnection.
        debug!(
            "Detaching connection: close_code={:?}, is_logout={}, disconnected={}",
            self.close_code, self.is_logout, self.is_logout
        );
        let detach_msg = mk_detach_msg(&self.client_token, self.is_logout);
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

        let reply = read_reply_result(&reply_bytes).expect("Failed to parse reply");
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
                log_rpc_failure(failure);
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
        let cmd: Var = match message {
            Message::Text(text) => v_str(&text),
            Message::Binary(bytes) => {
                // Parse binary as FlatBuffer-encoded Var
                let var_ref = match moor_var_schema::VarRef::read_as_root(&bytes) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Invalid FlatBuffer in binary input: {}", e);
                        return;
                    }
                };
                match var_from_flatbuffer_ref(var_ref) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Failed to decode Var from FlatBuffer: {}", e);
                        return;
                    }
                }
            }
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

        let reply = read_reply_result(&reply_bytes).expect("Failed to parse reply");
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
                log_rpc_failure(failure);
            }
            moor_rpc::ReplyResultUnionRef::HostSuccess(_) => {
                error!("Unexpected host success");
            }
        }
    }
}
