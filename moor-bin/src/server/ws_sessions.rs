use anyhow::{anyhow, Error};
use async_trait::async_trait;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use metrics_macros::increment_counter;
use moor_lib::tasks::sessions::Session;
use moor_value::var::objid::Objid;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, warn};

pub(crate) struct WebSocketSessions {
    connections: HashMap<Objid, WsConnection>,
    shutdown_sender: Sender<Option<String>>,
}

// The persistent websocket `connection` for the user, which exists across multiple sessions.
pub(crate) struct WsConnection {
    player: Objid,
    peer_addr: SocketAddr,
    ws_sender: SplitSink<WebSocket, Message>,
    connected_time: std::time::Instant,
    last_activity: std::time::Instant,
}

impl WebSocketSessions {
    pub fn new(shutdown_sender: Sender<Option<String>>) -> Self {
        Self {
            connections: HashMap::new(),
            shutdown_sender,
        }
    }

    pub(crate) async fn new_session(
        sessions: Arc<RwLock<Self>>,
        player: Objid,
    ) -> anyhow::Result<Arc<WebSocketSession>> {
        let session = WebSocketSession {
            player,
            ws_sessions: sessions,
            session_buffer: Mutex::new(vec![]),
        };
        Ok(Arc::new(session))
    }

    /// Register the given connection object for the given web socket sender. If there was an
    /// existing connection for the given object, it will be replaced, a reconnect message sent to
    /// the connection, the old connection closed, and a "true" value returned to indicate that
    /// this was a reconnect.  
    pub(crate) async fn register_connection(
        &mut self,
        connection_oid: Objid,
        peer_addr: SocketAddr,
        ws_sender: SplitSink<WebSocket, Message>,
    ) -> anyhow::Result<bool> {
        let connection = WsConnection {
            player: connection_oid,
            peer_addr,
            ws_sender,
            connected_time: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
        };
        let mut old = self.connections.insert(connection_oid, connection);
        match old {
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
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Unregister for given session, returning the websocket sender and peer address for the
    /// connection, for the server to do with as it pleases.
    pub(crate) async fn unregister_connection(
        &mut self,
        connection_oid: Objid,
    ) -> anyhow::Result<Option<(SplitSink<WebSocket, Message>, SocketAddr)>> {
        let connections = &mut self.connections;

        let Some(connection) = connections.remove(&connection_oid) else {
            return Ok(None);
        };

        Ok(Some((connection.ws_sender, connection.peer_addr)))
    }

    /// Marks that activity occurred on the given player's connection.
    /// Used for managing `idle_seconds`
    pub(crate) fn record_activity(&mut self, player: Objid) -> anyhow::Result<()> {
        let Some(connection) = self.connections.get_mut(&player) else {
            warn!(
                "No connection for player: #{} during attempt at recording activity",
                player.0
            );

            // TODO: Not really an 'error' I suppose... ? Think about this.
            return Ok(());
        };
        connection.last_activity = std::time::Instant::now();
        Ok(())
    }

    /// Send text to the given connection without going through the transactional buffering.
    /// Used by the server and by the internals of the connection itself.
    pub(crate) async fn write_msg(
        &mut self,
        connection_oid: Objid,
        msg: &str,
    ) -> anyhow::Result<()> {
        let Some(conn) = self.connections.get_mut(&connection_oid) else {
            // TODO This can be totally harmless, if a user disconnected while a transaction was in
            //  progress. But it can also be a sign of a bug, so we'll log it for now but remove the
            //  warning later.
            warn!(
                "No connection for player: #{} during attempt at sending message",
                connection_oid.0
            );
            return Ok(());
        };
        if conn.player != connection_oid {
            return Err(anyhow!(
                "integrity error; connection for objid: {} is for player: {}",
                connection_oid,
                conn.player
            ));
        }
        SplitSink::send(&mut conn.ws_sender, msg.into()).await?;

        Ok(())
    }
}

// A per-transaction `session`. The handle through which the scheduler and VM interact with the
// connection. Exists for the lifetime of a single transaction and talks back to WebSocketSessions
// to get it to do the dirty I/O work with the connection.
pub(crate) struct WebSocketSession {
    player: Objid,
    ws_sessions: Arc<RwLock<WebSocketSessions>>,
    // TODO: manage this buffer better -- e.g. if it grows too big, for long-running tasks, etc. it
    //  should be mmap'd to disk or something.
    session_buffer: Mutex<Vec<(Objid, String)>>,
}

#[async_trait]
impl Session for WebSocketSession {
    async fn commit(&self) -> Result<(), Error> {
        increment_counter!("wYou're rights_server.sessions.commit");
        let mut sessions = self.ws_sessions.write().await;
        let mut buffer = self.session_buffer.lock().await;
        for (player, msg) in buffer.drain(..) {
            sessions.write_msg(player, &msg).await?;
        }
        Ok(())
    }

    async fn rollback(&self) -> Result<(), Error> {
        increment_counter!("ws_server.sessions.rollback");
        let mut buffer = self.session_buffer.lock().await;
        buffer.clear();
        Ok(())
    }

    async fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, Error> {
        Ok(Arc::new(WebSocketSession {
            player: self.player,
            ws_sessions: self.ws_sessions.clone(),
            session_buffer: Default::default(),
        }))
    }

    async fn send_text(&self, player: Objid, msg: &str) -> Result<(), anyhow::Error> {
        increment_counter!("ws_server.sessions.send_text");
        let mut buffer = self.session_buffer.lock().await;
        buffer.push((player, msg.to_string()));
        Ok(())
    }

    async fn send_system_msg(&self, player: Objid, msg: &str) -> Result<(), Error> {
        increment_counter!("ws_server.sessions.send_text");
        let mut sessions = self.ws_sessions.write().await;
        sessions.write_msg(player, msg).await
    }

    async fn shutdown(&self, msg: Option<String>) -> Result<(), anyhow::Error> {
        increment_counter!("ws_server.sessions.shutdown");
        let mut sessions = self.ws_sessions.write().await;
        if let Some(msg) = msg.clone() {
            sessions.write_msg(self.player, &msg).await?;
        }
        sessions.shutdown_sender.send(msg).await.unwrap();
        Ok(())
    }

    async fn connection_name(&self, player: Objid) -> Result<String, anyhow::Error> {
        let sessions = self.ws_sessions.read().await;

        increment_counter!("ws_server.sessions.request_connection_name");

        let Some(conn) = sessions.connections.get(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        // should be of form "port <lport> from <host>, port <port>" to match LambdaMOO

        // TODO moo does a hostname lookup at connect time, which is kind of awful, but required for
        // $login etc. to be able to do their blacklisting and stuff.
        // for now i'll just return IP, but in the future we'll need to resolve the DNS at connect
        // time. But the async DNS resolvers for Rust don't seem to reverse DNS... So there's that.
        // Potentially there's something in the axum headers?
        // We also don't know our listen-port here, so I'll just fake it for now.
        let conn_string = format!(
            "port 7777 from {}, port {}",
            conn.peer_addr.ip(),
            conn.peer_addr.port()
        );

        Ok(conn_string)
    }

    async fn disconnect(&self, player: Objid) -> Result<(), Error> {
        let mut sessions = self.ws_sessions.write().await;

        increment_counter!("ws_server.sessions.disconnect");
        let Some(mut conn) = sessions.connections.remove(&player) else {
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

    async fn connected_players(&self) -> Result<Vec<Objid>, anyhow::Error> {
        let sessions = self.ws_sessions.read().await;

        increment_counter!("ws_server.sessions.request_connected_player");

        Ok(sessions
            .connections
            .keys()
            .copied()
            .filter(|c| c.0 >= 0)
            .collect())
    }

    async fn connected_seconds(&self, player: Objid) -> Result<f64, anyhow::Error> {
        let sessions = self.ws_sessions.read().await;

        increment_counter!("ws_server.sessions.request_connected_seconds");
        let Some(conn) = sessions.connections.get(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        let now = std::time::Instant::now();
        let duration = now - conn.connected_time;
        Ok(duration.as_secs_f64())
    }

    async fn idle_seconds(&self, player: Objid) -> Result<f64, anyhow::Error> {
        let sessions = self.ws_sessions.read().await;

        increment_counter!("ws_server.sessions.request.idle_seconds");
        let Some(conn) = sessions.connections.get(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        let now = std::time::Instant::now();
        let duration = now - conn.last_activity;
        Ok(duration.as_secs_f64())
    }
}
