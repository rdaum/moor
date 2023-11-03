use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use moor_values::model::CommandError;
use moor_values::var::objid::Objid;
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

impl WebSocketConnection {
    pub async fn handle(&mut self, connect_type: ConnectType, stream: WebSocket) {
        info!("New connection from {}, {}", self.peer_addr, self.player);
        let (mut ws_sender, mut ws_receiver) = stream.split();

        let connect_message = match connect_type {
            ConnectType::Connected => "** Connected **",
            ConnectType::Reconnected => "** Reconnected **",
            ConnectType::Created => "** Created **",
        };
        Self::write_line(&mut ws_sender, connect_message).await;

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
                        ConnectionEvent::SystemMessage(_author, msg) => {
                            Self::write_line(&mut ws_sender, &msg).await;
                        }
                        ConnectionEvent::Narrative(_author, event) => {
                            let msg = event.event();
                            Self::write_line(&mut ws_sender, &msg).await;
                        }
                        ConnectionEvent::RequestInput(request_id) => {
                            expecting_input = Some(request_id);
                        }
                        ConnectionEvent::Disconnect() => {
                            Self::write_line(&mut ws_sender, "** Disconnected **").await;
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
                Self::write_line(ws_sender, "I don't understand that.").await;
            }
            RpcResult::Failure(RpcRequestError::CommandError(CommandError::NoObjectMatch)) => {
                Self::write_line(ws_sender, "I don't see that here.").await;
            }
            RpcResult::Failure(RpcRequestError::CommandError(CommandError::NoCommandMatch)) => {
                Self::write_line(ws_sender, "I don't know how to do that.").await;
            }
            RpcResult::Failure(RpcRequestError::CommandError(CommandError::PermissionDenied)) => {
                Self::write_line(ws_sender, "You can't do that.").await;
            }
            RpcResult::Failure(e) => {
                error!("Unhandled RPC error: {:?}", e);
                return;
            }
            RpcResult::Success(s) => {
                error!("Unexpected RPC success: {:?}", s);
                return;
            }
        }
    }

    async fn write_line(ws_sender: &mut SplitSink<WebSocket, Message>, msg: &str) {
        let msg = if msg.is_empty() {
            Message::Text("\n".to_string())
        } else {
            Message::Text(msg.to_string())
        };
        ws_sender
            .send(msg)
            .await
            .expect("Unable to send message to client");
    }
}
