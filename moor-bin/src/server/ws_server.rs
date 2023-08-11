use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{anyhow, Error};
use async_trait::async_trait;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use axum::extract::{ConnectInfo, WebSocketUpgrade};
use axum::headers::authorization::Basic;
use axum::headers::Authorization;
use axum::response::IntoResponse;
use axum::{Extension, TypedHeader};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use metrics_macros::{counter, increment_counter};
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, trace, warn};

use moor_lib::tasks::scheduler::{Scheduler, TaskWaiterResult};
use moor_lib::tasks::Sessions;
use moor_value::model::objects::ObjFlag;
use moor_value::model::permissions::PermissionsContext;
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::{Objid, SYSTEM_OBJECT};
use moor_value::var::{v_objid, v_str};
use moor_value::var::variant::Variant;

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

pub struct WebSocketServer {
    sessions: Arc<RwLock<WebSocketSessions>>,
    scheduler: Scheduler,
    // Downward counter for connection ids, starting at -1.
    next_connection_number: i64,
}

impl WebSocketServer {
    pub fn new(scheduler: Scheduler, shutdown_sender: Sender<Option<String>>) -> Self {
        let inner = WebSocketSessions {
            connections: Default::default(),
            shutdown_sender,
        };
        Self {
            scheduler,
            sessions: Arc::new(RwLock::new(inner)),
            // Start at #-4, since #-3 and above are reserved.
            next_connection_number: -4,
        }
    }
}

/// Handles connection to an existing player, via websocket connection & basic-auth.
pub async fn ws_connect_handler(
    ws: WebSocketUpgrade,
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(ws_server): Extension<Arc<RwLock<WebSocketServer>>>,
) -> impl IntoResponse {
    info!("New websocket connection from {}", addr);
    // TODO validate player id
    // TODO password in headers? auth phase?
    ws.on_upgrade(move |socket| ws_handle_connection(ws_server, addr, socket, auth))
}

async fn ws_send_error(
    server: Arc<RwLock<WebSocketServer>>,
    player: Objid,
    msg: String,
) -> Result<(), anyhow::Error> {
    let server = server.write().await;
    server
        .sessions
        .clone()
        .write()
        .await
        .send_text(player, msg.as_str())
        .await
}

#[instrument(skip(server, stream))]
pub async fn ws_handle_connection(
    server: Arc<RwLock<WebSocketServer>>,
    peer: SocketAddr,
    stream: WebSocket,
    auth: Authorization<Basic>,
) {
    // TODO big need of cleanup here factor chunks here into separate functions, as this has grown
    // too large.

    increment_counter!("ws_server.new_connection");
    info!(?peer, "New websocket connection");
    let (ws_sender, mut ws_receiver) = stream.split();

    // Get a connection number.
    let (connection, event_receiver) = {
        let mut server = server.write().await;
        let connection_oid = {
            let sessions = &mut server.sessions.write().await;
            let connections = &mut sessions.connections;

            let connection_oid = Objid(server.next_connection_number);
            let client_connection = WsConnection {
                player: connection_oid,
                peer_addr: peer,
                ws_sender,
                connected_time: std::time::Instant::now(),
                last_activity: std::time::Instant::now(),
            };

            connections.insert(connection_oid, client_connection);
            connection_oid
        };
        server.next_connection_number -= 1;
        debug!(?connection_oid, "$do_login_command");
        // Call the scheduler to initiate $do_login_command
        let sessions = server.sessions.clone();
        // TODO: Clarify permissions here.  Do we need to be wizard? JHC requires only that callers()
        // returns empty?
        let permissions = PermissionsContext::root_for(Objid(0), BitEnum::new_with(ObjFlag::Wizard));
        let task_id = server
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

        let receiver = server.scheduler.subscribe_to_task(task_id).await.unwrap();

        (connection_oid, receiver)
    };

    // Now we spin waiting for the task to complete.  The server will output to the connection obj
    // we created.
    // And we will parse its result. If it's an object that's our new player object to sign in as.
    let connect_result = event_receiver.await.unwrap();
    let (player, ws_sender) = match connect_result {
        TaskWaiterResult::Success(v) => {
            increment_counter!("ws_server.login_success");
            let server = server.write().await;
            let sessions = &mut server.sessions.write().await;
            let connections = &mut sessions.connections;
            let Some(connection_record) = connections.remove(&connection) else {
                error!("Missing connection record for auth'd player");
                return;
            };
            let Variant::Obj(player) = v.variant() else {
                error!("invalid result from connect");
                // Kill the connection.
                return;
            }                                ;

            info!(player = ?*player, "connected");
            (*player, connection_record.ws_sender)
        }
        _ => {
            increment_counter!("ws_server.login_failure");
            error!("login failure");
            return;
        }
    };

    // Now, re-register connection with player.
    let is_reconnected = {
        let server = server.write().await;
        let sessions = &mut server.sessions.write().await;
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
                increment_counter!("ws_server.reconnect");
                SplitSink::send(&mut old.ws_sender, "** Redirecting connection to new port **".into())
                    .await
                    .unwrap();

                // TODO: the problem here is that the other loop will go ahead and enter exit phase
                // and remove the connection (via player) from the connections table at the bottom,
                // which will in turn kill us off.
                // Probably we'll need to think about how to have that not happen. But not tonight.
                let result = old.ws_sender.close().await;
                if let Err(e) = result {
                    error!("{:?}", e);
                }
                true
            }
            None => {
                false
            }
        };
        let connect_msg = if is_reconnected {
            "** Redirecting old connection to this port **"
        } else {
            "** Connected **"
        };
        let new = connections.get_mut(&player).unwrap();
        SplitSink::send(&mut new.ws_sender, connect_msg.into())
            .await
            .unwrap();
        is_reconnected
    };

    // And submit $user_connected(player)/$user_reconnected(player.
    // And thus the user shall be logged-in.
    {
        let mut server = server.write().await;
        let sessions = server.sessions.clone();
        let connected_verb = if is_reconnected {
            "user_reconnected".to_string()
        } else {
            "user_connected".to_string()
        };
        match server
            .scheduler
            .submit_verb_task(
                player,
                SYSTEM_OBJECT,
                connected_verb,
                vec![v_objid(player)],
                PermissionsContext::root_for(player, BitEnum::new_with(ObjFlag::Read)),
                sessions,
            )
            .await {
            Ok(_) => {
                trace!(player = ?player, "user_connected task submitted");
            }
            Err(e) => {
                warn!(player = ?player, "Could not issue user_connected task for connected player: {:?}", e);
            }
        }
    };

    // Task submission loop.
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
            let server = server.write().await;
            let mut sessions = server.sessions.write().await;
            let Some(connection) = sessions.connections.get_mut(&player) else {
                error!("No connection for player: #{}", player.0);
                break;
            };
            connection.last_activity = std::time::Instant::now();
        }
        let task_id = {
            let mut server = server.write().await;
            let sessions = server.sessions.clone();
            server
                .scheduler
                .submit_command_task(player, cmd, sessions)
                .await
        };
        if let Err(e) = task_id {
            increment_counter!("ws_server.submit_error");

            error!("Error submitting command ({}): {:?}", cmd, e);
            ws_send_error(server.clone(), player, format!("{:?}", e))
                .await
                .unwrap();
            continue;
        }
    }

    // Now drop the connection from sessions.
    // And any tasks that are associated with us should be aborted.
    {
        let server = server.write().await;
        {
            let connections = &mut server.sessions.write().await.connections;
            if connections.remove(&player).is_none() {
                trace!(?player, "connection already removed / no connection");
            }
        }
        counter!("ws_server.connection_finished", 1, "peer" => peer.to_string());
        info!("WebSocket session finished: {}", peer);
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

    async fn connection_name(&self, player: Objid) -> Result<String, anyhow::Error> {
        increment_counter!("ws_server.sessions.request_connection_name");
        let Some(conn) = self.connections.get(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        Ok(conn.peer_addr.to_string())
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
