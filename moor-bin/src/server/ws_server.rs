use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, bail, Error};
use async_trait::async_trait;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::headers::authorization::Basic;
use axum::headers::Authorization;
use axum::response::IntoResponse;
use axum::TypedHeader;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use metrics_macros::increment_counter;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, trace, warn};

use moor_lib::tasks::scheduler::{Scheduler, TaskWaiterResult};
use moor_lib::tasks::Sessions;
use moor_value::model::objects::ObjFlag;
use moor_value::model::permissions::PermissionsContext;
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::{Objid, SYSTEM_OBJECT};
use moor_value::var::variant::Variant;
use moor_value::var::{v_objid, v_str};

struct WebSocketSessions {
    connections: HashMap<Objid, WsConnection>,
    shutdown_sender: Sender<Option<String>>,
}

struct WsConnection {
    player: Objid,
    peer_addr: SocketAddr,
    ws_sender: SplitSink<WebSocket, Message>,
    connected_time: std::time::Instant,
    last_activity: std::time::Instant,
}

#[derive(Clone)]
pub struct WebSocketServer {
    inner: Arc<RwLock<Inner>>,
}

struct Inner {
    sessions: Arc<RwLock<WebSocketSessions>>,
    scheduler: Scheduler,
    // Downward counter for connection ids, starting at -1.
    next_connection_number: AtomicI64,
}

impl WebSocketServer {
    pub fn new(scheduler: Scheduler, shutdown_sender: Sender<Option<String>>) -> Self {
        let inner = WebSocketSessions {
            connections: Default::default(),
            shutdown_sender,
        };
        Self {
            inner: Arc::new(RwLock::new(Inner {
                scheduler,
                sessions: Arc::new(RwLock::new(inner)),
                // Start at #-4, since #-3 and above are reserved.
                next_connection_number: AtomicI64::new(-4),
            })),
        }
    }
}

/// Handles connection to an existing player, via websocket connection & basic-auth.
pub async fn ws_connect_handler(
    ws: WebSocketUpgrade,
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_server): State<WebSocketServer>,
) -> impl IntoResponse {
    info!("New websocket connection from {}", addr);
    // TODO: only async Rust could produce an entity as demonic as this. Let's go on and pretend the
    // pain is all worth it.
    ws.on_upgrade(
        move |socket| async move { ws_server.ws_handle_connection(addr, socket, auth).await },
    )
}

impl WebSocketServer {
    #[instrument(skip(self, stream))]
    pub async fn ws_handle_connection(
        &self,
        peer: SocketAddr,
        stream: WebSocket,
        auth: Authorization<Basic>,
    ) {
        increment_counter!("ws_server.new_connection");
        info!(?peer, "New websocket connection");
        let (ws_sender, ws_receiver) = stream.split();

        // Get a connection number, registered in the server.
        let connection_oid = self.create_connection(peer, ws_sender).await;

        let (player, ws_sender) = match self.call_do_login_command(connection_oid, auth).await {
            Ok(r) => r,
            Err(e) => {
                increment_counter!("ws_server.login_failure");
                error!(?e, "login failure");
                return;
            }
        };
        increment_counter!("ws_server.login_success");

        // Now, re-register connection with player.
        let Ok(is_reconnected) = self.register_connection(ws_sender, peer, player).await else {
            increment_counter!("ws_server.connection_registration_failure");
            error!("Failed to register connection");
            return;
        };

        if is_reconnected {
            increment_counter!("ws_server.reconnection")
        } else {
            increment_counter!("ws_server.new_connection")
        }

        // And thus the user shall be logged-in.

        // Now submit $user_connected(player)/$user_reconnected(player) to the scheduler.
        // Which allows the core to send welcome messages, etc. to the user.
        self.submit_connected_task(player, is_reconnected).await;

        // Core entry/task submission loop, runs as long as the connection 'tis open.
        self.submission_loop(player, ws_receiver).await;

        // Now drop the connection from sessions.
        self.deregister_connection(player).await;
        info!("WebSocket session finished: {}", peer);
    }

    async fn submission_loop(&self, player: Objid, mut ws_receiver: SplitStream<WebSocket>) {
        while let Ok(Some(msg)) = ws_receiver.try_next().await {
            let cmd = match msg.into_text() {
                Ok(cmd) => cmd,
                Err(e) => {
                    increment_counter!("ws_server.error_decoding_message");
                    error!("Error decoding a message: {:?}", e);
                    continue;
                }
            };
            increment_counter!("ws_server.message_received");
            let cmd = cmd.as_str().trim();

            // Record activity on the connection, to compute idle_seconds.
            {
                let inner = self.inner.read().await;
                let mut sessions = inner.sessions.write().await;
                let Some(connection) = sessions.connections.get_mut(&player) else {
                    error!("No connection for player: #{}", player.0);
                    break;
                };
                connection.last_activity = std::time::Instant::now();
            }
            let task_id = {
                let inner = self.inner.read().await;
                let sessions = inner.sessions.clone();
                inner
                    .scheduler
                    .submit_command_task(player, cmd, sessions)
                    .await
            };
            if let Err(e) = task_id {
                increment_counter!("ws_server.submit_error");

                error!("Error submitting command ({}): {:?}", cmd, e);
                self.send_error(player, format!("{:?}", e))
                    .await
                    .unwrap();
                continue;
            }
        }
    }


    async fn send_error(&self, player: Objid, msg: String) -> Result<(), anyhow::Error> {
        let inner = self.inner.read().await;
        inner
            .sessions
            .clone()
            .write()
            .await
            .send_text(player, msg.as_str())
            .await
    }

    async fn create_connection(
        &self,
        peer: SocketAddr,
        ws_sender: SplitSink<WebSocket, Message>,
    ) -> Objid {
        
        {
            let inner = self.inner.read().await;
            let mut sessions = inner.sessions.write().await;
            let connections = &mut sessions.connections;

            let connection_oid = Objid(inner.next_connection_number.fetch_sub(1, Ordering::SeqCst));
            let client_connection = WsConnection {
                player: connection_oid,
                peer_addr: peer,
                ws_sender,
                connected_time: std::time::Instant::now(),
                last_activity: std::time::Instant::now(),
            };

            connections.insert(connection_oid, client_connection);
            connection_oid
        }
    }

    async fn call_do_login_command(
        &self,
        connection_oid: Objid,
        auth: Authorization<Basic>,
    ) -> Result<(Objid, SplitSink<WebSocket, Message>), anyhow::Error> {
        let event_receiver = {
            trace!(?connection_oid, "$do_login_command");
            // Call the scheduler to initiate $do_login_command
            let inner = self.inner.read().await;
            let sessions = inner.sessions.clone();
            let permissions =
                PermissionsContext::root_for(Objid(0), BitEnum::new_with(ObjFlag::Wizard));
            let task_id = inner
                .scheduler
                .submit_verb_task(
                    connection_oid,
                    SYSTEM_OBJECT,
                    "do_login_command".to_string(),
                    vec![
                        v_str("connect"),
                        v_str(auth.username()),
                        v_str(auth.password()),
                    ],
                    permissions,
                    sessions,
                )
                .await
                .unwrap();

            inner.scheduler.subscribe_to_task(task_id).await?
        };

        // Now we spin waiting for the task to complete.  The server will output to the connection obj
        // we created while that's happening
        // We will wait on the subscription channel for this task,
        // And if it's successful and if it's an object that's our new player object to sign in as.
        // Otherwise, The Fail.
        let connect_result = event_receiver.await?;
        let (player, ws_sender) = match connect_result {
            TaskWaiterResult::Success(v) => {
                let inner = self.inner.read().await;
                let sessions = &mut inner.sessions.write().await;
                let connections = &mut sessions.connections;
                let Some(connection_record) = connections.remove(&connection_oid) else {
                    bail!("Missing connection record for auth'd player");
                };
                let Variant::Obj(player) = v.variant() else {
                    bail!("invalid result from connect");
                };

                info!(player = ?*player, "connected");
                (*player, connection_record.ws_sender)
            }
            _ => {
                bail!("login failure");
            }
        };

        Ok((player, ws_sender))
    }

    async fn register_connection(
        &self,
        ws_sender: SplitSink<WebSocket, Message>,
        peer: SocketAddr,
        player: Objid,
    ) -> Result<bool, anyhow::Error> {
        let inner = self.inner.read().await;
        let mut sessions = inner.sessions.write().await;
        let connections = &mut sessions.connections;
        let client_connection = WsConnection {
            player,
            peer_addr: peer,
            ws_sender,
            connected_time: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
        };
        let mut old = connections.insert(player, client_connection);
        let is_reconnected = match old {
            Some(ref mut old) => {
                SplitSink::send(
                    &mut old.ws_sender,
                    "** Redirecting connection to new port **".into(),
                )
                .await?;

                // TODO: the problem here is that the other loop will go ahead and enter exit phase
                // and remove the connection (via player) from the connections table at the bottom,
                // which will in turn kill us off.
                // Probably we'll need to think about how to have that not happen. But not tonight.
                let result = old.ws_sender.close().await;
                if let Err(e) = result {
                    error!("Failure to close old connection {:?}", e);
                }
                true
            }
            None => false,
        };
        let connect_msg = if is_reconnected {
            "** Redirecting old connection to this port **"
        } else {
            "** Connected **"
        };
        let new = connections.get_mut(&player).unwrap();
        SplitSink::send(&mut new.ws_sender, connect_msg.into()).await?;
        Ok(is_reconnected)
    }


    async fn deregister_connection(&self, player: Objid) {
        let inner = self.inner.read().await;
        let connections = &mut inner.sessions.write().await.connections;
        // TODO: properly handle reconnects.
        let Some(connection) = connections.remove(&player) else {
            trace!(?player, "connection already removed / no connection for player");
            return;
        };
        info!(player = ?player, peer = ?connection.peer_addr, "disconnected");
        increment_counter!("ws_server.connection_finished");
    }

    async fn submit_connected_task(&self, player: Objid, is_reconnected: bool) {
        let sessions = self.inner.read().await.sessions.clone();
        let connected_verb = if is_reconnected {
            "user_reconnected".to_string()
        } else {
            "user_connected".to_string()
        };
        match self
            .inner
            .write()
            .await
            .scheduler
            .submit_verb_task(
                player,
                SYSTEM_OBJECT,
                connected_verb,
                vec![v_objid(player)],
                PermissionsContext::root_for(player, BitEnum::new_with(ObjFlag::Read)),
                sessions,
            )
            .await
        {
            Ok(_) => {
                trace!(player = ?player, "user_connected task submitted");
            }
            Err(e) => {
                warn!(player = ?player, "Could not issue user_connected task for connected player: {:?}", e);
            }
        }
    }
}

#[async_trait]
impl Sessions for WebSocketSessions {
    async fn send_text(&mut self, player: Objid, msg: &str) -> Result<(), anyhow::Error> {
        increment_counter!("ws_server.sessions.send_text");

        let Some(conn) = self.connections.get_mut(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        if conn.player != player {
            return Err(anyhow!(
                "integrity error; connection for objid: #{} is for player: #{}",
                player.0,
                conn.player.0
            ));
        }
        SplitSink::send(&mut conn.ws_sender, msg.into()).await?;

        Ok(())
    }

    async fn shutdown(&mut self, msg: Option<String>) -> Result<(), anyhow::Error> {
        increment_counter!("ws_server.sessions.shutdown");
        self.shutdown_sender.send(msg).await.unwrap();
        Ok(())
    }

    async fn connection_name(&self, player: Objid) -> Result<String, anyhow::Error> {
        increment_counter!("ws_server.sessions.request_connection_name");
        let Some(conn) = self.connections.get(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        Ok(conn.peer_addr.to_string())
    }

    async fn disconnect(&mut self, player: Objid) -> Result<(), Error> {
        increment_counter!("ws_server.sessions.disconnect");
        let Some(mut conn) = self.connections.remove(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        if conn.player != player {
            return Err(anyhow!(
                "integrity error; connection for objid: #{} is for player: #{}",
                player.0,
                conn.player.0
            ));
        }
        conn.ws_sender
            .send(Message::Close(Some(CloseFrame {
                code: axum::extract::ws::close_code::NORMAL,
                reason: Default::default(),
            })))
            .await?;
        Ok(())
    }

    fn connected_players(&self) -> Result<Vec<Objid>, anyhow::Error> {
        increment_counter!("ws_server.sessions.request_connected_player");

        Ok(self
            .connections
            .keys()
            .cloned()
            .filter(|c| c.0 >= 0)
            .collect())
    }

    fn connected_seconds(&self, player: Objid) -> Result<f64, anyhow::Error> {
        increment_counter!("ws_server.sessions.request_connected_seconds");
        let Some(conn) = self.connections.get(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        let now = std::time::Instant::now();
        let duration = now - conn.connected_time;
        Ok(duration.as_secs_f64())
    }

    fn idle_seconds(&self, player: Objid) -> Result<f64, anyhow::Error> {
        increment_counter!("ws_server.sessions.request.idle_seconds");
        let Some(conn) = self.connections.get(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        let now = std::time::Instant::now();
        let duration = now - conn.last_activity;
        Ok(duration.as_secs_f64())
    }
}
