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

//! Per-client session event loop and realtime transport coordination.

pub(crate) mod webrtc;
mod websocket;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use moor_common::tasks::Event as NarrativeEventKind;
use moor_runtime_api::{
    AuthToken, ClientToken, HostType,
    api::{
        BroadcastEvent, ClientBroadcastSubscription, ClientEvent, ClientEventSubscription,
        ClientReply, ClientRequest, InvocationMode, RuntimeClient,
    },
    api_codec::encode_client_event_bytes,
};
use moor_schema::{convert::var_from_flatbuffer_ref, rpc as moor_rpc, var as moor_var_schema};
use moor_var::{Obj, Var, v_str};
use planus::ReadAsRoot;
use std::{
    collections::{HashSet, VecDeque},
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::select;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use self::webrtc::{
    SignalingMessage, WebRtcConfig, WebRtcPeer, encode_signaling_message, parse_signaling_message,
};
use self::websocket::{ReadEvent, read_websocket_event};
use crate::host::actions::{action_request, is_action_frame};

const WEBSOCKET_PING_INTERVAL: Duration = Duration::from_secs(30);

// Application-level heartbeat to detect zombie WebSocket connections.
// Unlike WebSocket ping/pong (handled by browser at protocol level), this requires
// JavaScript to process and respond, proving the client is actually alive.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(15);
const HEARTBEAT_REQUEST: u8 = 0x02;

pub struct ClientSession {
    pub(crate) player: Obj,
    pub(crate) peer_addr: SocketAddr,
    pub(crate) broadcast_sub: Box<dyn ClientBroadcastSubscription>,
    pub(crate) narrative_sub: Box<dyn ClientEventSubscription>,
    pub(crate) client_id: Uuid,
    pub(crate) client_token: ClientToken,
    pub(crate) auth_token: AuthToken,
    pub(crate) daemon_client: Arc<dyn RuntimeClient>,
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
    pub(crate) task_id: usize,
    pub(crate) start_time: Instant,
}

impl ClientSession {
    /// Build a CredentialsUpdatedEvent as serialized FlatBuffer bytes.
    fn build_credentials_event(&self) -> Result<Vec<u8>, moor_runtime_api::RpcMessageError> {
        let event = moor_runtime_api::api::ClientEventMessage {
            sequence: 0,
            event: ClientEvent::CredentialsUpdated {
                client_id: self.client_id,
                client_token: self.client_token.clone(),
            },
        };
        encode_client_event_bytes(&event)
    }

    pub async fn handle(&mut self, connect_type: moor_rpc::ConnectType, stream: WebSocket) {
        info!("New connection from {}, {}", self.peer_addr, self.player);
        let (mut ws_sender, mut ws_receiver) = stream.split();

        // Send credentials at the start of every connection.
        // This ensures the client always has the correct credentials, even after
        // reattach fails and a new connection is created.
        let credentials_bytes = match self.build_credentials_event() {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to encode credentials update: {}", e);
                return;
            }
        };
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
        let mut deferred_actions = VecDeque::new();
        let mut ping_interval = tokio::time::interval(WEBSOCKET_PING_INTERVAL);
        let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        let mut pending_heartbeat: Option<Instant> = None;
        let mut ice_candidate_rx: Option<tokio::sync::mpsc::UnboundedReceiver<SignalingMessage>> =
            None;
        loop {
            if expecting_input.is_empty()
                && self.pending_task.is_none()
                && let Some(action) = deferred_actions.pop_front()
            {
                self.process_command_line(action).await;
            }
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

            select! {
                line = read_websocket_event(
                    &mut ws_receiver,
                    !expecting_input.is_empty(),
                    &mut self.pending_task,
                ) => {
                    match line {
                        ReadEvent::Command(line) => {
                            if expecting_input.is_empty() {
                                self.process_command_line(line).await;
                            } else {
                                // Keep actions separate from read() replies, including prompt races.
                                if deferred_actions.len() >= 64 {
                                    warn!("Too many actions queued during a prompt");
                                    break;
                                }
                                deferred_actions.push_back(line);
                            }
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
                            if !self.handle_webrtc_signaling(&data, &mut ws_sender, &mut ice_candidate_rx).await {
                                break;
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
                Ok(event_msg) = self.broadcast_sub.recv_client_broadcast() => {
                    trace!("broadcast_event");
                    match event_msg.event {
                        BroadcastEvent::PingPong => {
                            let timestamp = match SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                                Ok(duration) => duration.as_nanos() as u64,
                                Err(e) => {
                                    warn!("System time before unix epoch during ping/pong handling: {}", e);
                                    0
                                }
                            };
                            let request = ClientRequest::ClientPong {
                                client_token: self.client_token.clone(),
                                client_sys_time: timestamp,
                                player: self.player,
                                host_type: HostType::WebSocket,
                                socket_addr: self.peer_addr.to_string(),
                            };
                            if let Err(e) = self.daemon_client.client_call(self.client_id, request).await {
                                warn!("Unable to send pong to RPC server: {}", e);
                                break;
                            }
                        }
                    }
                }
                Ok(event_msg) = self.narrative_sub.recv_client_event() => {
                    match &event_msg.event {
                        ClientEvent::RequestInput { request_id, .. } => {
                            expecting_input.push_back(*request_id);
                        }
                        ClientEvent::TaskSuccess { .. } |
                        ClientEvent::TaskError { .. } |
                        ClientEvent::TaskSuspended { .. } => {
                            // Clear the pending task so we can process the next command
                            self.pending_task = None;
                        }
                        ClientEvent::PlayerSwitched {
                            new_player,
                            new_auth_token,
                            ..
                        } => {
                            info!(
                                old_player = ?self.player,
                                ?new_player,
                                client_id = ?self.client_id,
                                "Updating web session player"
                            );
                            self.player = *new_player;
                            self.auth_token.clone_from(new_auth_token);
                        }
                        _ => {}
                    }

                    // Check if this is a DataEvent in a realtime-eligible domain
                    // and route over data channel if available.
                    let dc_open = self.webrtc_peer.as_ref().is_some_and(|p| p.is_open());
                    let is_realtime = !self.realtime_domains.is_empty()
                        && is_realtime_eligible(&event_msg.event, &self.realtime_domains);
                    if is_realtime {
                        debug!("Realtime event: dc_open={dc_open} peer={}", self.webrtc_peer.is_some());
                    }
                    let use_data_channel = dc_open && is_realtime;

                    let event_bytes = match encode_client_event_bytes(&event_msg) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            error!("Failed to encode client event for websocket: {}", e);
                            break;
                        }
                    };
                    if !self.forward_event_bytes(event_bytes, use_data_channel, &mut ws_sender).await {
                        break;
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
        self.daemon_client
            .client_call(
                self.client_id,
                ClientRequest::Detach {
                    client_token: self.client_token.clone(),
                    disconnected: self.is_logout,
                },
            )
            .await
            .map_err(|e| warn!("Unable to send detach event to RPC server: {}", e))
            .ok();
    }

    async fn process_command_line(&mut self, line: Message) {
        let request = if let Message::Binary(ref bytes) = line
            && is_action_frame(bytes)
        {
            match action_request(
                &bytes[1..],
                self.handler_object,
                self.auth_token.clone(),
                self.client_token.clone(),
            ) {
                Ok(request) => request,
                Err(error) => {
                    warn!(%error, "Invalid structured action");
                    return;
                }
            }
        } else {
            let line = match line.into_text() {
                Ok(line) => line,
                Err(e) => {
                    warn!("Received non-text command message: {}", e);
                    return;
                }
            };
            ClientRequest::Command {
                auth_token: self.auth_token.clone(),
                handler_object: self.handler_object,
                command: line.trim().to_string(),
                mode: InvocationMode::Connected {
                    client_token: self.client_token.clone(),
                },
            }
        };

        let reply = self
            .daemon_client
            .client_call(self.client_id, request)
            .await;
        let reply = match reply {
            Ok(reply) => reply,
            Err(e) => {
                warn!("Unable to send command to RPC server: {}", e);
                return;
            }
        };

        match reply {
            ClientReply::TaskSubmitted { task_id } => {
                self.set_pending_task(task_id as usize);
            }
            ClientReply::InputThanks => {
                warn!("Received input thanks unprovoked, out of order")
            }
            _ => {
                error!("Unexpected daemon to client reply");
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
        let Some(cmd) = var_from_input_message(message) else {
            return;
        };

        let Some(input_request_id) = expecting_input.front() else {
            warn!("Attempt to send reply to input request without an input request");
            return;
        };

        let reply = self
            .daemon_client
            .client_call(
                self.client_id,
                ClientRequest::RequestedInput {
                    client_token: self.client_token.clone(),
                    auth_token: self.auth_token.clone(),
                    request_id: *input_request_id,
                    input: cmd,
                },
            )
            .await;
        let reply = match reply {
            Ok(reply) => reply,
            Err(e) => {
                warn!("Unable to send input to RPC server: {}", e);
                return;
            }
        };

        match reply {
            ClientReply::TaskSubmitted { task_id } => {
                self.set_pending_task(task_id as usize);
                warn!("Got TaskSubmitted when expecting input-thanks")
            }
            ClientReply::InputThanks => {
                expecting_input.pop_front();
            }
            _ => {
                error!("Unexpected daemon to client reply");
            }
        }
    }

    async fn handle_webrtc_signaling(
        &mut self,
        data: &[u8],
        ws_sender: &mut SplitSink<WebSocket, Message>,
        ice_candidate_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<SignalingMessage>>,
    ) -> bool {
        if !self.webrtc_config.enabled {
            debug!("WebRTC signaling received but WebRTC is disabled");
            return true;
        }

        let Some(msg) = parse_signaling_message(data) else {
            warn!("Failed to parse WebRTC signaling message");
            return true;
        };

        match msg {
            SignalingMessage::Offer { sdp } => {
                self.handle_webrtc_offer(sdp, ws_sender, ice_candidate_rx)
                    .await
            }
            SignalingMessage::IceCandidate {
                candidate,
                sdp_mid,
                sdp_mline_index,
            } => {
                self.add_webrtc_ice_candidate(candidate, sdp_mid, sdp_mline_index)
                    .await;
                true
            }
            SignalingMessage::Answer { .. } => {
                // Server shouldn't receive answers; we send them.
                warn!("Received unexpected SDP answer from client");
                true
            }
        }
    }

    async fn handle_webrtc_offer(
        &mut self,
        sdp: String,
        ws_sender: &mut SplitSink<WebSocket, Message>,
        ice_candidate_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<SignalingMessage>>,
    ) -> bool {
        let (peer, answer_sdp) = match WebRtcPeer::new(&self.webrtc_config, &sdp).await {
            Ok(peer_and_answer) => peer_and_answer,
            Err(e) => {
                warn!("Failed to create WebRTC peer: {e}");
                return true;
            }
        };

        let (ice_tx, ice_rx) = tokio::sync::mpsc::unbounded_channel();
        peer.on_ice_candidate(ice_tx);
        *ice_candidate_rx = Some(ice_rx);

        let answer = encode_signaling_message(&SignalingMessage::Answer { sdp: answer_sdp });
        if let Err(e) = ws_sender.send(Message::Binary(answer.into())).await {
            error!("Failed to send WebRTC answer: {e}");
            return false;
        }

        self.webrtc_peer = Some(peer);
        info!("WebRTC peer connection established");
        true
    }

    async fn add_webrtc_ice_candidate(
        &mut self,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    ) {
        let Some(peer) = &self.webrtc_peer else {
            return;
        };

        let candidate_json = serde_json::json!({
            "candidate": candidate,
            "sdpMid": sdp_mid,
            "sdpMLineIndex": sdp_mline_index,
        })
        .to_string();
        if let Err(e) = peer.add_ice_candidate(&candidate_json).await {
            warn!("Failed to add ICE candidate: {e}");
        }
    }

    async fn forward_event_bytes(
        &mut self,
        bytes: Vec<u8>,
        use_data_channel: bool,
        ws_sender: &mut SplitSink<WebSocket, Message>,
    ) -> bool {
        if use_data_channel && let Some(peer) = &self.webrtc_peer {
            match peer.send(&bytes).await {
                Ok(()) => return true,
                Err(e) => {
                    debug!("Data channel send failed, falling back to WS: {e}");
                }
            }
        }

        let msg = Message::Binary(bytes.into());
        if let Err(e) = ws_sender.send(msg).await {
            error!("Failed to send message to websocket: {}", e);
            return false;
        }
        true
    }

    fn set_pending_task(&mut self, task_id: usize) -> bool {
        self.pending_task = Some(PendingTask {
            task_id,
            start_time: Instant::now(),
        });
        true
    }
}

fn var_from_input_message(message: Message) -> Option<Var> {
    match message {
        Message::Text(text) => Some(v_str(&text)),
        Message::Binary(bytes) => var_from_binary_input(&bytes),
        _ => {
            warn!("Received unsupported message type for input");
            None
        }
    }
}

fn var_from_binary_input(bytes: &[u8]) -> Option<Var> {
    let var_ref = match moor_var_schema::VarRef::read_as_root(bytes) {
        Ok(var_ref) => var_ref,
        Err(e) => {
            warn!("Invalid FlatBuffer in binary input: {}", e);
            return None;
        }
    };

    match var_from_flatbuffer_ref(var_ref) {
        Ok(var) => Some(var),
        Err(e) => {
            warn!("Failed to decode Var from FlatBuffer: {}", e);
            None
        }
    }
}

/// Check if a client event is a DataEvent whose domain is in the realtime set.
fn is_realtime_eligible(event: &ClientEvent, realtime_domains: &HashSet<String>) -> bool {
    let ClientEvent::Narrative { event, .. } = event else {
        return false;
    };
    let NarrativeEventKind::Data { namespace, .. } = &event.event else {
        return false;
    };
    realtime_domains.contains(&namespace.as_string())
}
