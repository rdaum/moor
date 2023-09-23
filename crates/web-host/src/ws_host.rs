use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::headers::authorization::Basic;
use axum::headers::Authorization;
use axum::response::IntoResponse;
use axum::TypedHeader;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use metrics_macros::increment_counter;
use rpc_common::pubsub_client::broadcast_recv;
use rpc_common::pubsub_client::narrative_recv;
use rpc_common::rpc_client::RpcSendClient;
use rpc_common::BroadcastEvent;
use rpc_common::ConnectionEvent;
use rpc_common::RpcError;
use rpc_common::RpcRequest::ConnectionEstablish;
use rpc_common::{ConnectType, RpcRequest, RpcResponse, RpcResult, BROADCAST_TOPIC};
use std::net::SocketAddr;
use std::time::SystemTime;
use tmq::{request, subscribe};
use tokio::select;
use tracing::trace;
use tracing::{debug, error, info};
use uuid::Uuid;
#[derive(Clone)]
pub struct WebSocketHost {
    zmq_context: tmq::Context,
    rpc_addr: String,
    pubsub_addr: String,
}

impl WebSocketHost {
    pub fn new(rpc_addr: String, narrative_addr: String) -> Self {
        let tmq_context = tmq::Context::new();
        Self {
            zmq_context: tmq_context,
            rpc_addr,
            pubsub_addr: narrative_addr,
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LoginType {
    Connect,
    Create,
}

impl WebSocketHost {
    pub async fn handle_session(
        &self,
        login_type: LoginType,
        peer_addr: SocketAddr,
        stream: WebSocket,
        auth: Authorization<Basic>,
    ) {
        let zmq_ctx = self.zmq_context.clone();
        // Establish a connection to the RPC server
        let client_id = Uuid::new_v4();
        info!(peer_addr = ?peer_addr, client_id = ?client_id,
            "Accepted connection"
        );

        let rcp_request_sock = request(&zmq_ctx)
            .set_rcvtimeo(100)
            .set_sndtimeo(100)
            .connect(self.rpc_addr.as_str())
            .expect("Unable to bind RPC server for connection");

        // And let the RPC server know we're here, and it should start sending events on the
        // narrative subscription.
        debug!(
            self.rpc_addr,
            "Contacting RPC server to establish connection"
        );
        let mut rpc_client = RpcSendClient::new(rcp_request_sock);

        let connection_oid = match rpc_client
            .make_rpc_call(client_id, ConnectionEstablish(peer_addr.to_string()))
            .await
        {
            Ok(RpcResult::Success(RpcResponse::NewConnection(objid))) => {
                info!("Connection established, connection ID: {}", objid);
                objid
            }
            Ok(RpcResult::Failure(f)) => {
                error!("RPC failure in connection establishment: {}", f);
                return;
            }
            Ok(_) => {
                error!("Unexpected response from RPC server");
                return;
            }
            Err(e) => {
                error!("Unable to establish connection: {}", e);
                return;
            }
        };
        debug!(?client_id, connection = ?connection_oid, "Connection established");

        // Before attempting login, we subscribe to the narrative channel, using our client
        // id. The daemon should be sending events here.
        let narrative_sub = subscribe(&zmq_ctx)
            .connect(self.pubsub_addr.as_str())
            .expect("Unable to connect narrative subscriber ");
        let mut narrative_sub = narrative_sub
            .subscribe(&client_id.as_bytes()[..])
            .expect("Unable to subscribe to narrative messages for client connection");

        let broadcast_sub = subscribe(&zmq_ctx)
            .connect(self.pubsub_addr.as_str())
            .expect("Unable to connect broadcast subscriber ");
        let mut broadcast_sub = broadcast_sub
            .subscribe(BROADCAST_TOPIC)
            .expect("Unable to subscribe to broadcast messages for client connection");

        info!(
            "Subscribed on pubsub socket for {:?}, socket addr {}",
            client_id, self.pubsub_addr
        );

        let (mut ws_sender, mut ws_receiver) = stream.split();

        let connect_verb = match login_type {
            LoginType::Connect => "connect",
            LoginType::Create => "create",
        };

        let words = vec![
            connect_verb.to_string(),
            auth.username().to_string(),
            auth.password().to_string(),
        ];
        let response = rpc_client
            .make_rpc_call(client_id, RpcRequest::LoginCommand(words))
            .await
            .expect("Unable to send login request to RPC server");
        let RpcResult::Success(RpcResponse::LoginResult(Some((connect_type, player)))) = response
        else {
            error!(?response, "Login failed");

            return;
        };

        info!(?player, client_id = ?client_id, "Login successful");

        let connect_message = match connect_type {
            ConnectType::Connected => "** Connected **",
            ConnectType::Reconnected => "** Reconnected **",
            ConnectType::Created => "** Created **",
        };
        write_line(&mut ws_sender, connect_message).await;

        debug!(?player, ?client_id, "Entering command dispatch loop");

        loop {
            select! {
                line = ws_receiver.next() => {
                    let Some(Ok(line)) = line else {
                        info!("Connection closed");
                        return;
                    };
                    let line = line.into_text().unwrap();
                    let cmd = line.trim().to_string();
                    let response = rpc_client
                        .make_rpc_call(client_id, RpcRequest::Command(cmd))
                        .await.expect("Unable to send command to RPC server");

                    match response {
                        RpcResult::Success(RpcResponse::CommandComplete) => {
                            // Nothing to do
                        }
                        RpcResult::Failure(RpcError::CouldNotParseCommand) => {
                            write_line(&mut ws_sender, "I don't understand that.").await;
                        }
                        RpcResult::Failure(RpcError::NoCommandMatch(_)) => {
                            write_line(&mut ws_sender, "I don't see that here.").await;
                        }
                        RpcResult::Failure(RpcError::PermissionDenied) => {
                           write_line(&mut ws_sender, "You can't do that.").await;
                        }
                        RpcResult::Failure(e) => {
                            error!("Unhandled RPC error: {:?}", e);
                            continue;
                        }
                        RpcResult::Success(s) => {
                            error!("Unexpected RPC success: {:?}", s);
                            continue;
                        }
                    }
                }
                Ok(event) = broadcast_recv(&mut broadcast_sub) => {
                    trace!(?event, "broadcast_event");
                    match event {
                        BroadcastEvent::PingPong(_server_time) => {
                            let _ = rpc_client.make_rpc_call(client_id,
                                RpcRequest::Pong(SystemTime::now())).await.expect("Unable to send pong to RPC server");
                        }
                    }
                }
                Ok(event) = narrative_recv(client_id, &mut narrative_sub) => {
                    trace!(?event, "narrative_event");
                    match event {
                        ConnectionEvent::SystemMessage(_author, msg) => {
                            write_line(&mut ws_sender, &msg).await;
                        }
                        ConnectionEvent::Narrative(_author, event) => {
                            let msg = event.event();
                            write_line(&mut ws_sender, &msg).await;
                        }
                        ConnectionEvent::Disconnect() => {
                            write_line(&mut ws_sender, "** Disconnected **").await;
                            ws_sender.close().await.expect("Unable to close connection");
                            return ;
                        }
                    }
                }
            }
        }
    }
}

pub async fn ws_connect_handler(
    ws: WebSocketUpgrade,
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebSocketHost>,
) -> impl IntoResponse {
    increment_counter!("ws_host.new_connection");
    info!("Connection from {}", addr);
    // TODO: this should be returning 403 for authentication failures and we likely have to do the
    //   auth check before doing the socket upgrade not after. But this is a problem because the
    //   do_login_command needs a connection to write failures to. So we may have to do something
    //   wacky like provide an initial "HTTP response" type connection to the server, and then swap
    //   with the websocket connection once we've authenticated; or have the "WSConnection" handle
    //   both modes.
    ws.on_upgrade(move |socket| async move {
        ws_host
            .handle_session(LoginType::Connect, addr, socket, auth)
            .await
    })
}

pub async fn ws_create_handler(
    ws: WebSocketUpgrade,
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebSocketHost>,
) -> impl IntoResponse {
    increment_counter!("ws_host.new_connection");
    info!("Connection from {}", addr);
    ws.on_upgrade(move |socket| async move {
        ws_host
            .handle_session(LoginType::Create, addr, socket, auth)
            .await
    })
}
