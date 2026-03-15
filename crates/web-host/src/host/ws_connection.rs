// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use moor_schema::{
    common as moor_common, convert::uuid_from_ref, convert::var_from_flatbuffer_ref,
    rpc as moor_rpc, var as moor_var_schema,
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
    collections::{HashSet, VecDeque},
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tmq::subscribe::Subscribe;
use tokio::select;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use super::webrtc::{
    self, SignalingMessage, WebRtcConfig, WebRtcPeer, encode_signaling_message,
    parse_signaling_message,
};

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
    match failure.error() {
        Ok(error_ref) => match error_ref.error_code() {
            Ok(error_code) => warn!("RPC error: {:?}", error_code),
            Err(e) => warn!("RPC failure missing error code: {}", e),
        },
        Err(e) => warn!("RPC failure missing error payload: {}", e),
    }
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
    pub(crate) webrtc_config: Arc<WebRtcConfig>,
    pub(crate) realtime_domains: HashSet<String>,
    pub(crate) webrtc_peer: Option<WebRtcPeer>,
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
    WebRtcSignaling(Vec<u8>),
}

impl WebSocketConnection {
    /// Build a CredentialsUpdatedEvent as serialized FlatBuffer bytes
    fn build_credentials_event(&self) -> Vec<u8> {
        let event = moor_rpc::ClientEvent {
            event: moor_rpc::ClientEventUnion::CredentialsUpdatedEvent(Box::new(
                moor_rpc::CredentialsUpdatedEvent {
                    client_id: Box::new(moor_common::Uuid {
                        data: self.client_id.as_bytes().to_vec(),
                    }),
                    client_token: Box::new(moor_rpc::ClientToken {
                        token: self.client_token.0.clone(),
                    }),
                },
            )),
        };
        let mut builder = planus::Builder::new();
        builder.finish(event, None).to_vec()
    }

    pub async fn handle(&mut self, connect_type: moor_rpc::ConnectType, stream: WebSocket) {
        info!("New connection from {}, {}", self.peer_addr, self.player);
        let (mut ws_sender, mut ws_receiver) = stream.split();

        // Send credentials at the start of every connection.
        // This ensures the client always has the correct credentials, even after
        // reattach fails and a new connection is created.
        let credentials_bytes = self.build_credentials_event();
        if let Err(e) = ws_sender
            .send(Message::Binary(credentials_bytes.into()))
            .await
        {
            error!("Failed to send credentials update: {}", e);
            return;
        }
        debug!(client_id = ?self.client_id, "Sent credentials update to client");

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
                error!("NoConnect reached WebSocket handler unexpectedly");
                return;
            }
        };

        debug!(client_id = ?self.client_id, "Entering command dispatch loop");

        let mut expecting_input = VecDeque::new();
        let mut ping_interval = tokio::time::interval(WEBSOCKET_PING_INTERVAL);
        let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        let mut pending_heartbeat: Option<Instant> = None;
        let mut ice_candidate_rx: Option<tokio::sync::mpsc::UnboundedReceiver<SignalingMessage>> =
            None;
        loop {
            // Check for heartbeat timeout - if we sent a heartbeat and haven't received
            // a response within HEARTBEAT_TIMEOUT, the websocket connection is likely zombie.
            if let Some(sent_time) = pending_heartbeat
                && sent_time.elapsed() > HEARTBEAT_TIMEOUT
            {
                warn!(
                    "Heartbeat timeout after {:?} - websocket not responding, closing connection",
                    sent_time.elapsed()
                );
                break;
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
                        Message::Binary(ref data)
                            if !data.is_empty() && data[0] == webrtc::SIGNALING_PREFIX =>
                        {
                            // WebRTC signaling message
                            return ReadEvent::WebRtcSignaling(data.to_vec());
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
                        ReadEvent::WebRtcSignaling(data) => {
                            if !self.webrtc_config.enabled {
                                debug!("WebRTC signaling received but WebRTC is disabled");
                                continue;
                            }
                            let Some(msg) = parse_signaling_message(&data) else {
                                warn!("Failed to parse WebRTC signaling message");
                                continue;
                            };
                            match msg {
                                SignalingMessage::Offer { sdp } => {
                                    match WebRtcPeer::new(&self.webrtc_config, &sdp).await {
                                        Ok((peer, answer_sdp)) => {
                                            // Set up ICE candidate forwarding over WebSocket.
                                            let (ice_tx, ice_rx) = tokio::sync::mpsc::unbounded_channel();
                                            peer.on_ice_candidate(ice_tx);
                                            ice_candidate_rx = Some(ice_rx);

                                            // Send SDP answer back.
                                            let answer = encode_signaling_message(&SignalingMessage::Answer { sdp: answer_sdp });
                                            if let Err(e) = ws_sender.send(Message::Binary(answer.into())).await {
                                                error!("Failed to send WebRTC answer: {e}");
                                                break;
                                            }
                                            self.webrtc_peer = Some(peer);
                                            info!("WebRTC peer connection established");
                                        }
                                        Err(e) => {
                                            warn!("Failed to create WebRTC peer: {e}");
                                        }
                                    }
                                }
                                SignalingMessage::IceCandidate { candidate, sdp_mid, sdp_mline_index } => {
                                    if let Some(peer) = &self.webrtc_peer {
                                        let candidate_json = serde_json::json!({
                                            "candidate": candidate,
                                            "sdpMid": sdp_mid,
                                            "sdpMLineIndex": sdp_mline_index,
                                        }).to_string();
                                        if let Err(e) = peer.add_ice_candidate(&candidate_json).await {
                                            warn!("Failed to add ICE candidate: {e}");
                                        }
                                    }
                                }
                                SignalingMessage::Answer { .. } => {
                                    // Server shouldn't receive answers — we send them.
                                    warn!("Received unexpected SDP answer from client");
                                }
                            }
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
                Some(ice_msg) = async {
                    match ice_candidate_rx.as_mut() {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    let frame = encode_signaling_message(&ice_msg);
                    if let Err(e) = ws_sender.send(Message::Binary(frame.into())).await {
                        error!("Failed to send ICE candidate: {e}");
                        break;
                    }
                }
                Ok(event_msg) = broadcast_recv(&mut self.broadcast_sub) => {
                    let event = match event_msg.event() {
                        Ok(event) => event,
                        Err(e) => {
                            warn!("Failed to parse broadcast event: {}", e);
                            continue;
                        }
                    };
                    trace!("broadcast_event");
                    match event.event() {
                        Ok(moor_rpc::ClientsBroadcastEventUnionRef::ClientsBroadcastPingPong(_server_time)) => {
                            let timestamp = match SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                                Ok(duration) => duration.as_nanos() as u64,
                                Err(e) => {
                                    warn!("System time before unix epoch during ping/pong handling: {}", e);
                                    0
                                }
                            };
                            let pong_msg = mk_client_pong_msg(
                                &self.client_token,
                                timestamp,
                                &self.player,
                                moor_rpc::HostType::WebSocket,
                                self.peer_addr.to_string(),
                            );
                            if let Err(e) = self.rpc_client.make_client_rpc_call(self.client_id, pong_msg).await {
                                warn!("Unable to send pong to RPC server: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Broadcast event missing event union: {}", e);
                            continue;
                        }
                    }
                }
                Ok(event_msg) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    // Parse to check for input requests and task completions
                    let event = match event_msg.event() {
                        Ok(event) => event,
                        Err(e) => {
                            warn!("Failed to parse client event: {}", e);
                            continue;
                        }
                    };
                    let event_ref = match event.event() {
                        Ok(event_ref) => event_ref,
                        Err(e) => {
                            warn!("Client event missing event union: {}", e);
                            continue;
                        }
                    };

                    match event_ref {
                        moor_rpc::ClientEventUnionRef::RequestInputEvent(input_request) => {
                            let request_id_ref = match input_request.request_id() {
                                Ok(request_id_ref) => request_id_ref,
                                Err(e) => {
                                    warn!("RequestInputEvent missing request_id: {}", e);
                                    continue;
                                }
                            };
                            let request_id = match uuid_from_ref(request_id_ref) {
                                Ok(request_id) => request_id,
                                Err(e) => {
                                    warn!("Failed to convert request_id UUID: {}", e);
                                    continue;
                                }
                            };
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

                    // Check if this is a DataEvent in a realtime-eligible domain
                    // and route over data channel if available.
                    let dc_open = self.webrtc_peer.as_ref().is_some_and(|p| p.is_open());
                    let is_realtime = !self.realtime_domains.is_empty()
                        && is_realtime_eligible(&event_ref, &self.realtime_domains);
                    if is_realtime {
                        debug!("Realtime event: dc_open={dc_open} peer={}", self.webrtc_peer.is_some());
                    }
                    let use_data_channel = dc_open && is_realtime;

                    let bytes = event_msg.consume();
                    if use_data_channel {
                        if let Some(peer) = &self.webrtc_peer
                            && let Err(e) = peer.send(&bytes).await {
                                // Fall back to WebSocket on send failure.
                                debug!("Data channel send failed, falling back to WS: {e}");
                                let msg = Message::Binary(bytes.into());
                                if let Err(e) = ws_sender.send(msg).await {
                                    error!("Failed to send message to websocket: {}", e);
                                    break;
                                }
                            }
                    } else {
                        let msg = Message::Binary(bytes.into());
                        if let Err(e) = ws_sender.send(msg).await {
                            error!("Failed to send message to websocket: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        // Close WebRTC peer if one was established.
        self.close_webrtc().await;

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
            .map_err(|e| warn!("Unable to send detach event to RPC server: {}", e))
            .ok();
    }

    async fn process_command_line(&mut self, line: Message) {
        let line = match line.into_text() {
            Ok(line) => line,
            Err(e) => {
                warn!("Received non-text command message: {}", e);
                return;
            }
        };
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
            .await;
        let reply_bytes = match reply_bytes {
            Ok(reply_bytes) => reply_bytes,
            Err(e) => {
                warn!("Unable to send command to RPC server: {}", e);
                return;
            }
        };

        let reply = match read_reply_result(&reply_bytes) {
            Ok(reply) => reply,
            Err(e) => {
                warn!("Failed to parse command reply: {}", e);
                return;
            }
        };
        match reply.result() {
            Ok(moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success)) => {
                let daemon_reply = match client_success.reply().ok() {
                    Some(daemon_reply) => daemon_reply,
                    None => {
                        warn!("Command reply missing daemon reply");
                        return;
                    }
                };
                match daemon_reply.reply() {
                    Ok(moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted)) => {
                        let ti = match task_submitted.task_id() {
                            Ok(task_id) => task_id as usize,
                            Err(e) => {
                                warn!("TaskSubmitted missing task_id: {}", e);
                                return;
                            }
                        };
                        self.pending_task = Some(PendingTask {
                            task_id: ti,
                            start_time: Instant::now(),
                        });
                    }
                    Ok(moor_rpc::DaemonToClientReplyUnionRef::InputThanks(_)) => {
                        warn!("Received input thanks unprovoked, out of order")
                    }
                    Ok(_) => {
                        error!("Unexpected daemon to client reply");
                    }
                    Err(e) => {
                        warn!("Command reply missing daemon reply union: {}", e);
                    }
                }
            }
            Ok(moor_rpc::ReplyResultUnionRef::Failure(failure)) => {
                log_rpc_failure(failure);
            }
            Ok(moor_rpc::ReplyResultUnionRef::HostSuccess(_)) => {
                error!("Unexpected host success");
            }
            Err(e) => {
                warn!("Command reply missing top-level result: {}", e);
            }
        }
    }

    /// Close the WebRTC peer connection if one exists.
    async fn close_webrtc(&mut self) {
        if let Some(peer) = self.webrtc_peer.take() {
            peer.close().await;
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
            .await;
        let reply_bytes = match reply_bytes {
            Ok(reply_bytes) => reply_bytes,
            Err(e) => {
                warn!("Unable to send input to RPC server: {}", e);
                return;
            }
        };

        let reply = match read_reply_result(&reply_bytes) {
            Ok(reply) => reply,
            Err(e) => {
                warn!("Failed to parse input reply: {}", e);
                return;
            }
        };
        match reply.result() {
            Ok(moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success)) => {
                let daemon_reply = match client_success.reply().ok() {
                    Some(daemon_reply) => daemon_reply,
                    None => {
                        warn!("Input reply missing daemon reply");
                        return;
                    }
                };
                match daemon_reply.reply() {
                    Ok(moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted)) => {
                        let task_id = match task_submitted.task_id() {
                            Ok(task_id) => task_id as usize,
                            Err(e) => {
                                warn!("TaskSubmitted missing task_id: {}", e);
                                return;
                            }
                        };
                        self.pending_task = Some(PendingTask {
                            task_id,
                            start_time: Instant::now(),
                        });
                        warn!("Got TaskSubmitted when expecting input-thanks")
                    }
                    Ok(moor_rpc::DaemonToClientReplyUnionRef::InputThanks(_)) => {
                        expecting_input.pop_front();
                    }
                    Ok(_) => {
                        error!("Unexpected daemon to client reply");
                    }
                    Err(e) => {
                        warn!("Input reply missing daemon reply union: {}", e);
                    }
                }
            }
            Ok(moor_rpc::ReplyResultUnionRef::Failure(failure)) => {
                log_rpc_failure(failure);
            }
            Ok(moor_rpc::ReplyResultUnionRef::HostSuccess(_)) => {
                error!("Unexpected host success");
            }
            Err(e) => {
                warn!("Input reply missing top-level result: {}", e);
            }
        }
    }
}

/// Check if a client event is a DataEvent whose domain is in the realtime set.
fn is_realtime_eligible(
    event_ref: &moor_rpc::ClientEventUnionRef<'_>,
    realtime_domains: &HashSet<String>,
) -> bool {
    let moor_rpc::ClientEventUnionRef::NarrativeEventMessage(narrative) = event_ref else {
        return false;
    };
    let Ok(narrative_event) = narrative.event() else {
        return false;
    };
    let Ok(event) = narrative_event.event() else {
        return false;
    };
    let Ok(event_union) = event.event() else {
        return false;
    };
    let moor_common::EventUnionRef::DataEvent(data_event) = event_union else {
        return false;
    };
    let Ok(domain_sym) = data_event.domain() else {
        return false;
    };
    let Ok(domain) = domain_sym.value() else {
        return false;
    };
    realtime_domains.contains(domain)
}
