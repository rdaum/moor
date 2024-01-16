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

use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use moor_values::model::CommandError;
use moor_values::var::Objid;
use rpc_common::pubsub_client::broadcast_recv;
use rpc_common::pubsub_client::narrative_recv;
use rpc_common::rpc_client::RpcSendClient;
use rpc_common::BroadcastEvent;
use rpc_common::ConnectionEvent;
use rpc_common::{
    AuthToken, ClientToken, ConnectType, RpcRequest, RpcRequestError, RpcResponse, RpcResult,
};
use std::net::SocketAddr;
use std::time::SystemTime;
use tmq::subscribe::Subscribe;
use tokio::select;
use tracing::{debug, error, info, trace};
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
}

/// The JSON output of a narrative event.
#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NarrativeOutput {
    origin_player: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    server_time: SystemTime,
}

impl WebSocketConnection {
    pub async fn handle(&mut self, connect_type: ConnectType, stream: WebSocket) {
        info!("New connection from {}, {}", self.peer_addr, self.player);
        let (mut ws_sender, mut ws_receiver) = stream.split();

        let connect_message = match connect_type {
            ConnectType::Connected => "** Connected **",
            ConnectType::Reconnected => "** Reconnected **",
            ConnectType::Created => "** Created **",
        };
        Self::emit_event(
            &mut ws_sender,
            NarrativeOutput {
                origin_player: self.player.0,
                system_message: Some(connect_message.to_string()),
                message: None,
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
                        BroadcastEvent::PingPong(_server_time) => {
                            let _ = self.rpc_client.make_rpc_call(self.client_id,
                                RpcRequest::Pong(self.client_token.clone(), SystemTime::now())).await.expect("Unable to send pong to RPC server");
                        }
                    }
                }
                Ok(event) = narrative_recv(self.client_id, &mut self.narrative_sub) => {
                    trace!(?event, "narrative_event");
                    match event {
                        ConnectionEvent::SystemMessage(author, msg) => {
                            Self::emit_event(&mut ws_sender, NarrativeOutput {
                                origin_player: author.0,
                                system_message: Some(msg),
                                message: None,
                                server_time: SystemTime::now(),
                            }).await;
                        }
                        ConnectionEvent::Narrative(author, event) => {
                            let msg = event.event();
                            Self::emit_event(&mut ws_sender, NarrativeOutput {
                                origin_player: author.0,
                                system_message: None,
                                message: Some(match msg {
                                    moor_values::model::Event::TextNotify(msg) => msg,
                                }),
                                server_time: event.timestamp(),
                            }).await;
                        }
                        ConnectionEvent::RequestInput(request_id) => {
                            expecting_input = Some(request_id);
                        }
                        ConnectionEvent::Disconnect() => {
                            Self::emit_event(&mut ws_sender, NarrativeOutput {
                                origin_player: self.player.0,
                                system_message: Some("** Disconnected **".to_string()),
                                message: None,
                                server_time: SystemTime::now(),
                            }).await;
                            ws_sender.close().await.expect("Unable to close connection");
                            return ;
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
                .make_rpc_call(
                    self.client_id,
                    RpcRequest::RequestedInput(
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
                .make_rpc_call(
                    self.client_id,
                    RpcRequest::Command(self.client_token.clone(), self.auth_token.clone(), cmd),
                )
                .await
                .expect("Unable to send command to RPC server"),
        };

        match response {
            RpcResult::Success(RpcResponse::CommandSubmitted(_))
            | RpcResult::Success(RpcResponse::InputThanks) => {
                // Nothing to do
            }
            RpcResult::Failure(RpcRequestError::CommandError(
                CommandError::CouldNotParseCommand,
            )) => {
                Self::emit_event(
                    ws_sender,
                    NarrativeOutput {
                        origin_player: self.player.0,
                        system_message: Some("I don't understand that.".to_string()),
                        message: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await;
            }
            RpcResult::Failure(RpcRequestError::CommandError(CommandError::NoObjectMatch)) => {
                Self::emit_event(
                    ws_sender,
                    NarrativeOutput {
                        origin_player: self.player.0,
                        system_message: Some("I don't know what you're talking about.".to_string()),
                        message: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await;
            }
            RpcResult::Failure(RpcRequestError::CommandError(CommandError::NoCommandMatch)) => {
                Self::emit_event(
                    ws_sender,
                    NarrativeOutput {
                        origin_player: self.player.0,
                        system_message: Some("I don't know how to do that.".to_string()),
                        message: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await;
            }
            RpcResult::Failure(RpcRequestError::CommandError(CommandError::PermissionDenied)) => {
                Self::emit_event(
                    ws_sender,
                    NarrativeOutput {
                        origin_player: self.player.0,
                        system_message: Some("You can't do that.".to_string()),
                        message: None,
                        server_time: SystemTime::now(),
                    },
                )
                .await;
            }
            RpcResult::Failure(e) => {
                error!("Unhandled RPC error: {:?}", e);
            }
            RpcResult::Success(s) => {
                error!("Unexpected RPC success: {:?}", s);
            }
        }
    }

    async fn emit_event(ws_sender: &mut SplitSink<WebSocket, Message>, msg: NarrativeOutput) {
        // Serialize to JSON.
        let msg = serde_json::to_string(&msg).unwrap();
        let msg = Message::Text(msg);
        ws_sender
            .send(msg)
            .await
            .expect("Unable to send message to client");
    }
}
