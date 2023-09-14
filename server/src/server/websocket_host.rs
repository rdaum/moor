use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Error;
use async_trait::async_trait;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::headers::authorization::Basic;
use axum::headers::Authorization;
use axum::response::IntoResponse;
use axum::TypedHeader;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use metrics_macros::increment_counter;
use tracing::{error, info, instrument, trace, warn};

use moor_core::tasks::scheduler::SchedulerError;
use moor_values::model::NarrativeEvent;
use moor_values::var::objid::Objid;
use moor_values::SYSTEM_OBJECT;

use crate::server::server::Server;

use super::connection::Connection;
use super::{ConnectType, DisconnectReason, LoginType};

#[derive(Clone)]
pub struct WebSocketHost {
    server: Arc<Server>,
}

impl WebSocketHost {
    pub fn new(server: Arc<Server>) -> Self {
        Self { server }
    }
}

/// Handles connection to an existing player, via websocket connection & basic-auth.
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
    // TODO: only async Rust could produce an entity as demonic as this. Let's go on and pretend the
    //   pain is all worth it.
    ws.on_upgrade(
        move |socket| async move { ws_host.handle_player_connect(addr, socket, auth).await },
    )
}

/// Handles the attempt to create a new player, via websocket connection & basic-auth.
pub async fn ws_create_handler(
    ws: WebSocketUpgrade,
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebSocketHost>,
) -> impl IntoResponse {
    increment_counter!("ws_host.new_connection");

    info!("Connection from {}", addr);
    // TODO: this should be returning 403 for authentication failures.
    ws.on_upgrade(
        move |socket| async move { ws_host.handle_player_create(addr, socket, auth).await },
    )
}

impl WebSocketHost {
    /// Websocket session handling for `connect` to an existing player with basic-auth credentials.
    #[instrument(skip(self, stream, auth))]
    pub async fn handle_player_connect(
        &self,
        peer: SocketAddr,
        stream: WebSocket,
        auth: Authorization<Basic>,
    ) {
        let (ws_sender, ws_receiver) = stream.split();

        // Get a connection number, registered in the server.
        let connection_oid = self.create_connection(peer, ws_sender).await;

        let server = self.server.clone();
        let (_, player) = match server
            .authenticate(
                connection_oid,
                LoginType::Connect,
                auth.username(),
                auth.password(),
            )
            .await
        {
            Ok(Some(r)) => r,
            Ok(None) => {
                warn!("Authentication failure");
                self.player_disconnected(connection_oid).await;
                return;
            }
            Err(e) => {
                warn!("Login failure: {}", e.to_string());
                self.player_disconnected(connection_oid).await;
                return;
            }
        };

        // And thus the user is logged in.
        self.player_connection(player, peer, ws_receiver).await;
    }

    /// Websocket session handling for `create` to for a new player with basic-auth credentials to
    /// establish the user's new player.
    #[instrument(skip(self, stream, auth))]
    pub async fn handle_player_create(
        &self,
        peer: SocketAddr,
        stream: WebSocket,
        auth: Authorization<Basic>,
    ) {
        let (ws_sender, ws_receiver) = stream.split();

        // Get a connection number, registered in the server.
        let connection_oid = self.create_connection(peer, ws_sender).await;

        let server = self.server.clone();

        let (_, player) = match server
            .authenticate(
                connection_oid,
                LoginType::Create,
                auth.username(),
                auth.password(),
            )
            .await
        {
            Ok(Some(r)) => r,
            Ok(None) => {
                warn!("Could not create player");
                self.player_disconnected(connection_oid).await;
                return;
            }
            Err(e) => {
                warn!("Create failure: {}", e.to_string());
                self.player_disconnected(connection_oid).await;
                return;
            }
        };

        // And thus the user is logged in as a new player.
        self.player_connection(player, peer, ws_receiver).await;
    }

    /// The actual core websocket handling loop for an authenticated (connected/created) player.
    async fn player_connection(
        &self,
        player: Objid,
        peer: SocketAddr,
        ws_receiver: SplitStream<WebSocket>,
    ) {
        // Core entry/task submission loop, runs as long as the connection 'tis open.
        self.submission_loop(player, ws_receiver).await;

        // Now drop the connection from sessions.
        self.player_disconnected(player).await;
        info!("WebSocket session finished: {}", peer);
    }

    async fn submission_loop(&self, player: Objid, mut ws_receiver: SplitStream<WebSocket>) {
        while let Some(msg) = ws_receiver.next().await {
            let msg = match msg {
                Ok(msg) => msg,
                Err(e) => {
                    increment_counter!("ws_host.command_receive_error");
                    error!("Error receiving a message: {:?}", e);
                    continue;
                }
            };
            let cmd = match msg.into_text() {
                Ok(cmd) => cmd,
                Err(e) => {
                    increment_counter!("ws_host.command_decode_error");
                    error!("Error decoding a message: {:?}", e);
                    continue;
                }
            };
            increment_counter!("ws_host.command_received");
            let cmd = cmd.as_str().trim();

            // Record activity on the connection, so we can compute idle_seconds.
            if let Err(e) = self.server.record_activity(player).await {
                warn!(player = ?player, "Error recording activity on connection: {:?}", e)
            }

            if let Err(e) = self
                .server
                .clone()
                .handle_inbound_command(player, cmd)
                .await
            {
                error!(player=?player, command=cmd, error=?e, "Error submitting command task");

                match e {
                    SchedulerError::CouldNotParseCommand(_)
                    | SchedulerError::NoCommandMatch(_, _) => {
                        increment_counter!("ws_host.command_parse_error");
                        self.send_error(player, "I don't understand that.".to_string())
                            .await
                            .unwrap();
                    }
                    SchedulerError::PermissionDenied => {
                        increment_counter!("ws_host.command_permission_error");
                        self.send_error(player, "You can't do that.".to_string())
                            .await
                            .unwrap();
                    }
                    _ => {
                        increment_counter!("ws_host.command_internal_error");
                        self.send_error(
                            player,
                            "Internal error. Let your nearest wizard know".to_string(),
                        )
                        .await
                        .unwrap();
                        error!(player=?player, command=cmd, error=?e, "Internal error in command submission");
                    }
                }
            }
        }
    }

    async fn send_error(&self, player: Objid, msg: String) -> Result<(), anyhow::Error> {
        self.server
            .write_messages(
                SYSTEM_OBJECT,
                &[(player, NarrativeEvent::new_ephemeral(SYSTEM_OBJECT, msg))],
            )
            .await
    }

    async fn create_connection(
        &self,
        peer_addr: SocketAddr,
        ws_sender: SplitSink<WebSocket, Message>,
    ) -> Objid {
        increment_counter!("ws_host.new_connection");
        self.server
            .new_connection(move |connection_oid| {
                Ok(Box::new(WsConnection {
                    player: connection_oid,
                    peer_addr,
                    ws_sender,
                    connected_time: Instant::now(),
                    last_activity: Instant::now(),
                }))
            })
            .await
            .expect("new connection")
    }

    async fn player_disconnected(&self, connection_object: Objid) {
        increment_counter!("ws_host.player_disconnected");
        match self.server.disconnected(connection_object).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                trace!(
                    ?connection_object,
                    "connection already removed / no connection for object"
                );
            }
            Err(e) => {
                error!(
                    ?connection_object,
                    "error deregistering connection: {:?}", e
                );
            }
        }
    }
}

// The persistent websocket `connection` for the user, which exists across multiple sessions.
pub struct WsConnection {
    player: Objid,
    peer_addr: SocketAddr,
    ws_sender: SplitSink<WebSocket, Message>,
    connected_time: Instant,
    last_activity: Instant,
}

#[async_trait]
impl Connection for WsConnection {
    async fn write_message(&mut self, msg: NarrativeEvent) -> Result<(), Error> {
        let msg = if msg.event().is_empty() {
            Message::Text("\n".to_string())
        } else {
            Message::Text(msg.event().to_string())
        };

        SplitSink::send(&mut self.ws_sender, msg).await?;
        Ok(())
    }

    async fn notify_connected(
        &mut self,
        _player: Objid,
        connect_type: ConnectType,
    ) -> Result<(), Error> {
        match connect_type {
            ConnectType::Connected => {
                let connect_msg = "** Connected **";
                SplitSink::send(&mut self.ws_sender, Message::Text(connect_msg.to_string()))
                    .await?;
            }
            ConnectType::Reconnected => {
                increment_counter!("ws_host.player_reconnected");
                let reconnect_msg = "** Redirecting old connection to this port **";
                SplitSink::send(
                    &mut self.ws_sender,
                    Message::Text(reconnect_msg.to_string()),
                )
                .await?;
            }
            ConnectType::Created => {
                let connect_msg = "** Created **";
                SplitSink::send(&mut self.ws_sender, Message::Text(connect_msg.to_string()))
                    .await?;
            }
        }
        Ok(())
    }

    async fn disconnect(&mut self, reason: DisconnectReason) -> Result<(), Error> {
        match reason {
            DisconnectReason::Reconnected => {
                let reconnect_msg = "** Redirecting connection to new port **";
                SplitSink::send(
                    &mut self.ws_sender,
                    Message::Text(reconnect_msg.to_string()),
                )
                .await?;
            }
            DisconnectReason::Booted(Some(msg)) => {
                increment_counter!("ws_host.player_booted");
                SplitSink::send(
                    &mut self.ws_sender,
                    Message::Text(format!("** You have been booted off the server: {msg} **")),
                )
                .await?;
            }
            _ => {}
        }
        self.ws_sender
            .send(Message::Close(Some(CloseFrame {
                code: axum::extract::ws::close_code::NORMAL,
                reason: Default::default(),
            })))
            .await?;
        Ok(())
    }

    async fn connection_name(&self, _player: Objid) -> Result<String, Error> {
        // should be of form "port <lport> from <host>, port <port>" to match LambdaMOO

        // TODO moo does a hostname lookup at connect time, which is kind of awful, but required for
        // $login etc. to be able to do their blacklisting and stuff.
        // for now i'll just return IP, but in the future we'll need to resolve the DNS at connect
        // time. But the async DNS resolvers for Rust don't seem to reverse DNS... So there's that.
        // Potentially there's something in the axum headers?
        // We also don't know our listen-port here, so I'll just fake it for now.
        let conn_string = format!(
            "port 7777 from {}, port {}",
            self.peer_addr.ip(),
            self.peer_addr.port()
        );
        Ok(conn_string)
    }

    async fn player(&self) -> Objid {
        self.player
    }

    async fn update_player(&mut self, player: Objid) -> Result<(), Error> {
        self.player = player;
        Ok(())
    }

    async fn last_activity(&self) -> Instant {
        self.last_activity
    }

    async fn record_activity(&mut self, when: Instant) {
        self.last_activity = when;
    }

    async fn connected_time(&self) -> Instant {
        self.connected_time
    }
}
