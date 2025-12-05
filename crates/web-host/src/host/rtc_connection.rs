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
//! WebRTC Data Channel transport - experimental alternative to WebSocket.
//! Provides the same event forwarding semantics but over WebRTC data channels.

use crate::host::{auth, web_host::WsHostError, WebHost};
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use moor_schema::rpc as moor_rpc;
use moor_var::Obj;
use rpc_async_client::{
    pubsub_client::{broadcast_recv, events_recv},
    rpc_client::RpcClient,
};
use rpc_common::{
    AuthToken, CLIENT_BROADCAST_TOPIC, ClientToken, mk_client_pong_msg, mk_command_msg,
    mk_detach_msg,
};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tmq::subscribe;
use tokio::select;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use webrtc::{
    api::APIBuilder,
    data_channel::{RTCDataChannel, data_channel_message::DataChannelMessage},
    peer_connection::{
        RTCPeerConnection,
        configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
};

#[derive(Debug, Deserialize)]
pub struct RtcOfferRequest {
    pub sdp: String,
}

#[derive(Debug, Serialize)]
pub struct RtcAnswerResponse {
    pub sdp: String,
    pub client_id: String,
    pub client_token: String,
}

/// Accept an SDP offer from a client and return an SDP answer.
/// This establishes the WebRTC connection and starts the event forwarding loop.
pub async fn rtc_offer_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(host): State<WebHost>,
    header_map: HeaderMap,
    Json(offer_req): Json<RtcOfferRequest>,
) -> impl IntoResponse {
    // Extract auth token from header
    let auth_token = match auth::extract_auth_token_header(&header_map) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };

    // Check for existing client credentials (for reconnection)
    let client_hint = auth::extract_client_credentials(&header_map);

    // Try to reattach if we have client credentials, otherwise do fresh attach
    // Fresh attach with ConnectType::Connected triggers welcome events
    let (player, client_id, client_token, rpc_client) = if let Some((hint_id, hint_token)) =
        client_hint
    {
        // Try reattach first
        match host
            .reattach_authenticated(auth_token.clone(), hint_id, hint_token.clone(), addr)
            .await
        {
            Ok(details) => details,
            Err(WsHostError::AuthenticationFailed) => {
                warn!("RTC reattach failed, falling back to fresh attach");
                // Fall through to fresh attach below
                match host
                    .attach_authenticated(
                        auth_token.clone(),
                        Some(moor_rpc::ConnectType::Connected),
                        addr,
                    )
                    .await
                {
                    Ok(details) => details,
                    Err(WsHostError::AuthenticationFailed) => {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                    Err(e) => {
                        error!("RTC attach error: {}", e);
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    }
                }
            }
            Err(e) => {
                error!("RTC reattach error: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    } else {
        // No client credentials - fresh connection, triggers welcome events
        match host
            .attach_authenticated(
                auth_token.clone(),
                Some(moor_rpc::ConnectType::Connected),
                addr,
            )
            .await
        {
            Ok(details) => details,
            Err(WsHostError::AuthenticationFailed) => {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            Err(e) => {
                error!("RTC attach error: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    // Subscribe to ZMQ events AFTER we have client_id but events will be buffered
    let zmq_ctx = host.zmq_context().clone();

    let mut narrative_socket_builder = subscribe(&zmq_ctx);
    let mut broadcast_socket_builder = subscribe(&zmq_ctx);

    // Configure CURVE encryption if keys provided
    if let Some((client_secret, client_public, server_public)) = host.curve_keys() {
        use rpc_async_client::zmq;
        let client_secret_bytes = match zmq::z85_decode(client_secret) {
            Ok(b) => b,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
        let client_public_bytes = match zmq::z85_decode(client_public) {
            Ok(b) => b,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
        let server_public_bytes = match zmq::z85_decode(server_public) {
            Ok(b) => b,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };

        narrative_socket_builder = narrative_socket_builder
            .set_curve_secretkey(&client_secret_bytes)
            .set_curve_publickey(&client_public_bytes)
            .set_curve_serverkey(&server_public_bytes);

        broadcast_socket_builder = broadcast_socket_builder
            .set_curve_secretkey(&client_secret_bytes)
            .set_curve_publickey(&client_public_bytes)
            .set_curve_serverkey(&server_public_bytes);
    }

    let narrative_sub = narrative_socket_builder
        .connect(host.pubsub_addr())
        .expect("Unable to connect narrative subscriber")
        .subscribe(&client_id.as_bytes()[..])
        .expect("Unable to subscribe to narrative messages");

    let broadcast_sub = broadcast_socket_builder
        .connect(host.pubsub_addr())
        .expect("Unable to connect broadcast subscriber")
        .subscribe(CLIENT_BROADCAST_TOPIC)
        .expect("Unable to subscribe to broadcast messages");

    info!(
        "RTC: Attached player {} with client {:?}",
        player, client_id
    );

    // Create WebRTC peer connection
    let peer_connection = match create_peer_connection().await {
        Ok(pc) => Arc::new(pc),
        Err(e) => {
            error!("Failed to create peer connection: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Parse the client's offer
    let offer = match RTCSessionDescription::offer(offer_req.sdp.clone()) {
        Ok(o) => o,
        Err(e) => {
            error!("Invalid SDP offer: {}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Log candidate count from offer for debugging
    let offer_candidate_count = offer_req.sdp.matches("a=candidate:").count();
    info!("Offer SDP has {} ICE candidates", offer_candidate_count);
    if offer_candidate_count == 0 {
        warn!("No ICE candidates in offer SDP! Full SDP:\n{}", offer_req.sdp);
    }

    // Log full offer for debugging component IDs
    debug!("Full offer SDP:\n{}", offer_req.sdp);

    // Set remote description
    if let Err(e) = peer_connection.set_remote_description(offer).await {
        error!("Failed to set remote description: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Create answer
    let answer = match peer_connection.create_answer(None).await {
        Ok(a) => a,
        Err(e) => {
            error!("Failed to create answer: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Set local description
    if let Err(e) = peer_connection.set_local_description(answer.clone()).await {
        error!("Failed to set local description: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Wait for ICE gathering to complete (simplified - in production you might trickle)
    let _ = gather_ice_candidates(&peer_connection).await;

    // Get the final local description with ICE candidates
    let local_desc = match peer_connection.local_description().await {
        Some(desc) => {
            // Filter out component 2 (RTCP) candidates - data channels only use component 1
            let filtered_sdp: String = desc
                .sdp
                .lines()
                .filter(|line| {
                    // Keep all non-candidate lines
                    if !line.starts_with("a=candidate:") {
                        return true;
                    }
                    // For candidate lines, only keep component 1
                    // Format: a=candidate:foundation component ...
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        // parts[0] is "a=candidate:foundation", parts[1] is component
                        parts[1] == "1"
                    } else {
                        true
                    }
                })
                .collect::<Vec<_>>()
                .join("\r\n");

            let candidate_count = filtered_sdp.matches("a=candidate:").count();
            debug!("Answer SDP has {} ICE candidates (after filtering component 2)", candidate_count);
            if candidate_count == 0 {
                warn!("No ICE candidates in answer SDP!");
            }

            RTCSessionDescription::answer(filtered_sdp).unwrap_or(desc)
        }
        None => {
            error!("No local description after ICE gathering");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Start the RTC session handler in a background task
    let pc_clone = peer_connection.clone();
    let player_clone = player;
    let auth_token_clone = auth_token.clone();
    let client_token_clone = client_token.clone();

    tokio::spawn(async move {
        if let Err(e) = run_rtc_session(
            pc_clone,
            client_id,
            client_token_clone,
            auth_token_clone,
            rpc_client,
            player_clone,
            addr,
            narrative_sub,
            broadcast_sub,
            host.handler_object.clone(),
        )
        .await
        {
            error!("RTC session error: {}", e);
        }
    });

    // Return the answer
    let response = RtcAnswerResponse {
        sdp: local_desc.sdp,
        client_id: client_id.to_string(),
        client_token: client_token.0.clone(),
    };

    Json(response).into_response()
}

async fn create_peer_connection() -> Result<RTCPeerConnection, webrtc::Error> {
    // For data-channel-only, we don't need media codecs or interceptors
    // This should prevent webrtc-rs from generating RTCP (component 2) candidates
    let api = APIBuilder::new().build();

    // For localhost/LAN, we don't need STUN - host candidates are sufficient
    // Add STUN servers for production deployments behind NAT
    let config = RTCConfiguration {
        ice_servers: vec![],
        ..Default::default()
    };

    api.new_peer_connection(config).await
}

async fn gather_ice_candidates(pc: &RTCPeerConnection) {
    use webrtc::ice_transport::ice_gatherer_state::RTCIceGathererState;
    use webrtc::ice_transport::ice_gathering_state::RTCIceGatheringState;

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    pc.on_ice_gathering_state_change(Box::new(move |state| {
        debug!("ICE gathering state changed: {:?}", state);
        if state == RTCIceGathererState::Complete {
            let _ = done_tx.try_send(());
        }
        Box::pin(async {})
    }));

    // Check if already complete (race condition fix)
    if pc.ice_gathering_state() == RTCIceGatheringState::Complete {
        debug!("ICE gathering already complete");
        return;
    }

    // Wait for gathering with short timeout - host candidates come fast
    match tokio::time::timeout(Duration::from_secs(2), done_rx.recv()).await {
        Ok(_) => debug!("ICE gathering completed"),
        Err(_) => debug!("ICE gathering timeout (2s), proceeding with available candidates"),
    }
}

async fn run_rtc_session(
    peer_connection: Arc<RTCPeerConnection>,
    client_id: Uuid,
    client_token: ClientToken,
    auth_token: AuthToken,
    rpc_client: RpcClient,
    player: Obj,
    peer_addr: SocketAddr,
    mut narrative_sub: tmq::subscribe::Subscribe,
    mut broadcast_sub: tmq::subscribe::Subscribe,
    handler_object: Obj,
) -> Result<(), eyre::Error> {
    info!(
        "Starting RTC session for player {} from {}",
        player, peer_addr
    );

    // Set up channels for data channel and messages
    // cmd_tx must be created BEFORE on_data_channel so we can set up the message handler
    // immediately when the data channel is received (avoiding race with client's READY)
    let (dc_tx, mut dc_rx) = tokio::sync::mpsc::channel::<Arc<RTCDataChannel>>(1);
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(100);

    let cmd_tx_for_handler = cmd_tx.clone();
    peer_connection.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
        let dc_tx = dc_tx.clone();
        let cmd_tx = cmd_tx_for_handler.clone();
        Box::pin(async move {
            info!("Data channel opened: {}", dc.label());

            // Set up message handler IMMEDIATELY to not miss any messages (including READY)
            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let cmd_tx = cmd_tx.clone();
                Box::pin(async move {
                    let _ = cmd_tx.send(msg.data.to_vec()).await;
                })
            }));

            let _ = dc_tx.send(dc).await;
        })
    }));

    // Wait for connection or data channel
    let (state_tx, mut state_rx) = tokio::sync::mpsc::channel::<RTCPeerConnectionState>(1);
    peer_connection.on_peer_connection_state_change(Box::new(move |state| {
        let state_tx = state_tx.clone();
        Box::pin(async move {
            let _ = state_tx.send(state).await;
        })
    }));

    // Wait for data channel with timeout
    let data_channel = tokio::time::timeout(Duration::from_secs(30), dc_rx.recv())
        .await
        .map_err(|_| eyre::eyre!("Timeout waiting for data channel"))?
        .ok_or_else(|| eyre::eyre!("Data channel sender dropped"))?;

    info!("Data channel established: {}", data_channel.label());

    // Wait for client READY signal before starting event forwarding
    // This prevents race condition where we send events before client's onopen fires
    info!("Waiting for client READY signal...");
    let ready_timeout = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(msg) = cmd_rx.recv().await {
                if msg == b"READY" {
                    return Ok(());
                }
                // If it's not READY, it might be a command sent early - log and continue waiting
                debug!("Received non-READY message while waiting: {:?}", String::from_utf8_lossy(&msg));
            } else {
                return Err(eyre::eyre!("Command channel closed while waiting for READY"));
            }
        }
    })
    .await;

    match ready_timeout {
        Ok(Ok(())) => info!("Client READY signal received, starting event forwarding"),
        Ok(Err(e)) => return Err(e),
        Err(_) => {
            warn!("Timeout waiting for client READY signal, proceeding anyway");
        }
    }

    // ZMQ subscriptions are passed in from the handler (subscribed before reattach)
    // Main event loop
    loop {
        select! {
            // Connection state changed
            Some(state) = state_rx.recv() => {
                match state {
                    RTCPeerConnectionState::Disconnected |
                    RTCPeerConnectionState::Failed |
                    RTCPeerConnectionState::Closed => {
                        info!("RTC connection closed for player {}", player);
                        break;
                    }
                    _ => {}
                }
            }

            // Command from client via data channel
            Some(cmd_bytes) = cmd_rx.recv() => {
                let cmd = String::from_utf8_lossy(&cmd_bytes).to_string();
                debug!("RTC command: {}", cmd);

                let command_msg = mk_command_msg(
                    &client_token,
                    &auth_token,
                    &handler_object,
                    cmd,
                );

                if let Err(e) = rpc_client.make_client_rpc_call(client_id, command_msg).await {
                    error!("RPC error: {}", e);
                }
            }

            // Broadcast event from daemon
            Ok(event_msg) = broadcast_recv(&mut broadcast_sub) => {
                let event = event_msg.event().expect("Failed to parse broadcast event");
                match event.event().expect("Missing event union") {
                    moor_rpc::ClientsBroadcastEventUnionRef::ClientsBroadcastPingPong(_) => {
                        let timestamp = SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_nanos() as u64;
                        let pong_msg = mk_client_pong_msg(
                            &client_token,
                            timestamp,
                            &player,
                            moor_rpc::HostType::WebSocket, // Reuse for now
                            peer_addr.to_string(),
                        );
                        let _ = rpc_client.make_client_rpc_call(client_id, pong_msg).await;
                    }
                }
            }

            // Narrative event from daemon - forward to data channel
            Ok(event_msg) = events_recv(client_id, &mut narrative_sub) => {
                let bytes = event_msg.consume();
                debug!("RTC forwarding event ({} bytes) to data channel", bytes.len());
                if let Err(e) = data_channel.send(&bytes.into()).await {
                    warn!("Failed to send event via data channel: {}", e);
                    break;
                }
            }
        }
    }

    // Cleanup
    let detach_msg = mk_detach_msg(&client_token, false);
    let _ = rpc_client.make_client_rpc_call(client_id, detach_msg).await;
    let _ = peer_connection.close().await;

    info!("RTC session ended for player {}", player);
    Ok(())
}

