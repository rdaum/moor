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

//! WebRTC data channel support for realtime event delivery.
//!
//! Provides an optional unreliable/unordered data channel alongside the
//! WebSocket connection. Events whose namespace is in the configured
//! "realtime domains" set are routed over the data channel when available,
//! falling back to WebSocket delivery.
//!
//! Signaling (SDP offer/answer, ICE candidates) is multiplexed on the
//! existing WebSocket using a `0x03` prefix byte followed by JSON.

use std::sync::Arc;

use serde_derive::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

/// WebSocket message prefix for WebRTC signaling messages.
pub const SIGNALING_PREFIX: u8 = 0x03;

/// Configuration for WebRTC data channel support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRtcConfig {
    #[serde(default)]
    pub enabled: bool,

    /// STUN/TURN server URLs for ICE connectivity.
    #[serde(default = "WebRtcConfig::default_ice_servers")]
    pub ice_servers: Vec<String>,

    /// Namespaces eligible for data channel delivery.
    /// Events with these namespaces are sent over the data channel when available.
    #[serde(default)]
    pub realtime_domains: Vec<String>,

    /// Whether the data channel should be ordered.
    /// `false` (default) gives lowest latency — packets can arrive out of order.
    #[serde(default)]
    pub ordered: bool,

    /// Maximum retransmits. `None` = unreliable (fire and forget).
    /// `Some(0)` = also unreliable but explicit. `Some(n)` = partially reliable.
    #[serde(default)]
    pub max_retransmits: Option<u16>,
}

impl WebRtcConfig {
    fn default_ice_servers() -> Vec<String> {
        vec!["stun:stun.l.google.com:19302".to_string()]
    }
}

impl Default for WebRtcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ice_servers: Self::default_ice_servers(),
            realtime_domains: vec![],
            ordered: false,
            max_retransmits: None,
        }
    }
}

/// JSON envelope for signaling messages exchanged over WebSocket.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SignalingMessage {
    Offer {
        sdp: String,
    },
    Answer {
        sdp: String,
    },
    #[serde(rename = "ice")]
    IceCandidate {
        candidate: String,
        #[serde(rename = "sdpMid")]
        sdp_mid: Option<String>,
        #[serde(rename = "sdpMLineIndex")]
        sdp_mline_index: Option<u16>,
    },
}

/// Wraps a WebRTC peer connection and the client-created data channel.
///
/// Created lazily when the client sends an SDP offer over the WebSocket.
/// The client creates the data channel; the server receives it via
/// `on_data_channel` and uses it for sending realtime events.
pub struct WebRtcPeer {
    peer_connection: Arc<RTCPeerConnection>,
    /// The data channel, set asynchronously when the client's channel arrives.
    data_channel: Arc<tokio::sync::Mutex<Option<Arc<RTCDataChannel>>>>,
    /// Set to `true` once the data channel's `on_open` fires.
    dc_open: Arc<std::sync::atomic::AtomicBool>,
}

impl WebRtcPeer {
    /// Create a new peer connection that receives the client's data channel.
    ///
    /// Returns the peer and the SDP answer to send back to the client.
    pub async fn new(
        config: &WebRtcConfig,
        offer_sdp: &str,
    ) -> Result<(Self, String), eyre::Error> {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs()?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine)?;

        let setting_engine = SettingEngine::default();

        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .with_setting_engine(setting_engine)
            .build();

        let ice_servers: Vec<RTCIceServer> = config
            .ice_servers
            .iter()
            .map(|url| RTCIceServer {
                urls: vec![url.clone()],
                ..Default::default()
            })
            .collect();

        let rtc_config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        let peer_connection = Arc::new(api.new_peer_connection(rtc_config).await?);

        let dc_open = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let data_channel: Arc<tokio::sync::Mutex<Option<Arc<RTCDataChannel>>>> =
            Arc::new(tokio::sync::Mutex::new(None));

        // Receive the client-created data channel.
        {
            let dc_open_clone = dc_open.clone();
            let dc_slot = data_channel.clone();
            peer_connection.on_data_channel(Box::new(move |channel: Arc<RTCDataChannel>| {
                info!("WebRTC data channel received: {}", channel.label());
                let dc_open_clone2 = dc_open_clone.clone();
                let dc_slot2 = dc_slot.clone();
                let channel2 = channel.clone();
                Box::pin(async move {
                    // Store the channel reference.
                    *dc_slot2.lock().await = Some(channel2.clone());

                    // Channel may already be open by the time we receive it.
                    if channel2.ready_state() == RTCDataChannelState::Open {
                        info!("WebRTC data channel already open");
                        dc_open_clone2.store(true, std::sync::atomic::Ordering::SeqCst);
                    }

                    let dc_open_for_open = dc_open_clone2.clone();
                    channel2.on_open(Box::new(move || {
                        info!("WebRTC data channel opened");
                        dc_open_for_open.store(true, std::sync::atomic::Ordering::SeqCst);
                        Box::pin(async {})
                    }));

                    let dc_open_for_close = dc_open_clone2;
                    channel2.on_close(Box::new(move || {
                        info!("WebRTC data channel closed");
                        dc_open_for_close.store(false, std::sync::atomic::Ordering::SeqCst);
                        Box::pin(async {})
                    }));
                })
            }));
        }

        // Track peer connection state.
        {
            peer_connection.on_peer_connection_state_change(Box::new(move |state| {
                debug!("WebRTC peer connection state: {state}");
                if state == RTCPeerConnectionState::Failed
                    || state == RTCPeerConnectionState::Disconnected
                {
                    warn!("WebRTC peer connection lost: {state}");
                }
                Box::pin(async {})
            }));
        }

        // Set remote description (the client's offer).
        let offer = RTCSessionDescription::offer(offer_sdp.to_string())?;
        peer_connection.set_remote_description(offer).await?;

        // Create answer.
        let answer = peer_connection.create_answer(None).await?;
        let answer_sdp = answer.sdp.clone();

        // Set local description (starts ICE gathering).
        peer_connection.set_local_description(answer).await?;

        Ok((
            Self {
                peer_connection,
                data_channel,
                dc_open,
            },
            answer_sdp,
        ))
    }

    /// Add a remote ICE candidate received from the client.
    pub async fn add_ice_candidate(&self, candidate_json: &str) -> Result<(), eyre::Error> {
        let candidate =
            serde_json::from_str::<webrtc::ice_transport::ice_candidate::RTCIceCandidateInit>(
                candidate_json,
            )?;
        self.peer_connection
            .add_ice_candidate(candidate)
            .await
            .map_err(Into::into)
    }

    /// Whether the data channel is currently open and ready for sending.
    pub fn is_open(&self) -> bool {
        self.dc_open
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Send binary data over the data channel.
    /// Returns an error if the channel is not open or the send fails.
    pub async fn send(&self, data: &[u8]) -> Result<(), eyre::Error> {
        if !self.is_open() {
            return Err(eyre::eyre!("Data channel not open"));
        }
        let dc = self.data_channel.lock().await;
        let Some(channel) = dc.as_ref() else {
            return Err(eyre::eyre!("Data channel not available"));
        };
        let b = bytes::Bytes::copy_from_slice(data);
        channel.send(&b).await.map(|_| ()).map_err(Into::into)
    }

    /// Close the peer connection.
    pub async fn close(&self) {
        if let Err(e) = self.peer_connection.close().await {
            warn!("Error closing WebRTC peer connection: {e}");
        }
    }

    /// Set up a callback to collect ICE candidates to send to the client.
    /// The provided sender receives serialized signaling messages.
    pub fn on_ice_candidate(
        &self,
        ice_tx: tokio::sync::mpsc::UnboundedSender<SignalingMessage>,
    ) {
        self.peer_connection.on_ice_candidate(Box::new(
            move |candidate| {
                if let Some(candidate) = candidate {
                    let candidate_str = candidate.to_json().map(|init| SignalingMessage::IceCandidate {
                        candidate: init.candidate,
                        sdp_mid: init.sdp_mid,
                        sdp_mline_index: init.sdp_mline_index,
                    });
                    match candidate_str {
                        Ok(msg) => {
                            let _ = ice_tx.send(msg);
                        }
                        Err(e) => {
                            warn!("Failed to serialize ICE candidate: {e}");
                        }
                    }
                }
                Box::pin(async {})
            },
        ));
    }
}

impl Drop for WebRtcPeer {
    fn drop(&mut self) {
        let pc = self.peer_connection.clone();
        tokio::spawn(async move {
            if let Err(e) = pc.close().await {
                warn!("Error closing WebRTC peer connection on drop: {e}");
            }
        });
    }
}

/// Parse a WebRTC signaling message from a WebSocket binary frame.
/// The frame must start with `SIGNALING_PREFIX` (0x03) followed by JSON.
pub fn parse_signaling_message(data: &[u8]) -> Option<SignalingMessage> {
    if data.is_empty() || data[0] != SIGNALING_PREFIX {
        return None;
    }
    serde_json::from_slice(&data[1..]).ok()
}

/// Encode a signaling message as a WebSocket binary frame (0x03 + JSON).
pub fn encode_signaling_message(msg: &SignalingMessage) -> Vec<u8> {
    let json = serde_json::to_vec(msg).expect("signaling message serialization");
    let mut frame = Vec::with_capacity(1 + json.len());
    frame.push(SIGNALING_PREFIX);
    frame.extend_from_slice(&json);
    frame
}
