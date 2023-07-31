use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_tungstenite::{accept_async, WebSocketStream};
use tracing::{error, info, instrument};
use tungstenite::{Error, Message};

use moor_lib::tasks::scheduler::Scheduler;
use moor_lib::tasks::Sessions;
use moor_lib::values::objid::Objid;

struct WebSocketSessions {
    connections: HashMap<Objid, WsConnection>,
}

struct WsConnection {
    player: Objid,
    sink: SplitSink<WebSocketStream<TcpStream>, Message>,
}

pub struct WebSocketServer {
    sessions: Arc<RwLock<WebSocketSessions>>,
    scheduler: Arc<RwLock<Scheduler>>,
}

impl WebSocketServer {
    pub fn new(scheduler: Arc<RwLock<Scheduler>>) -> Self {
        let inner = WebSocketSessions {
            connections: Default::default(),
        };
        Self {
            scheduler,
            sessions: Arc::new(RwLock::new(inner)),
        }
    }
}

async fn ws_accept_connection(
    server: Arc<RwLock<WebSocketServer>>,
    peer: SocketAddr,
    stream: TcpStream,
) {
    if let Err(e) = ws_handle_connection(server.clone(), peer, stream).await {
        if let Some(e) = e.downcast_ref::<tungstenite::Error>() {
            match e {
                Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
                err => info!("Error processing connection: {}", err),
            }
        }
    }
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
async fn ws_handle_connection(
    server: Arc<RwLock<WebSocketServer>>,
    peer: SocketAddr,
    stream: TcpStream,
) -> Result<(), anyhow::Error> {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");

    let player = Objid(2);

    info!("New inbound websocket connection: {}", peer);
    let (ws_sender, mut ws_receiver) = WebSocketStream::split(ws_stream);

    // Register connection with player.
    {
        let server = server.write().await;
        let client_connection = WsConnection {
            player,
            sink: ws_sender,
        };
        let connections = &mut server.sessions.write().await.connections;

        let mut old = connections.insert(player, client_connection);
        if let Some(ref mut old) = old {
            SplitSink::send(&mut old.sink, "Reconnecting".into())
                .await
                .unwrap();
            let result = old.sink.close().await;
            if let Err(e) = result {
                error!("{:?}", e);
            }
        }
    }

    // Task submission loop.
    while let Some(msg) = ws_receiver.next().await {
        let Ok(msg) = msg else {
            error!("Error receiving a message: {}", msg.unwrap_err());
            break;
        };
        if msg.is_text() || msg.is_binary() {
            let cmd = msg.into_text().unwrap();
            let cmd = cmd.as_str().trim();

            let task_id = {
                let server = server.read().await;
                let mut scheduler = server.scheduler.write().await;
                scheduler
                    .setup_command_task(player, cmd, server.sessions.clone())
                    .await
            };
            let task_id = match task_id {
                Ok(task_id) => task_id,
                Err(e) => {
                    error!("Unable to parse command ({}): {:?}", cmd, e);
                    ws_send_error(
                        server.clone(),
                        player,
                        format!("Unable to parse command ({}): {}", cmd, e),
                    )
                    .await?;

                    continue;
                }
            };
            {
                let server = server.write().await;
                let mut scheduler = server.scheduler.write().await;
                if let Err(e) = scheduler.start_task(task_id).await {
                    error!("Unable to execute: {}", e);
                    continue;
                };
            }
        }
    }

    // Now drop the connection from sessions
    {
        let server = server.write().await;
        let connections = &mut server.sessions.write().await.connections;
        connections.remove(&player).unwrap();
        info!("WebSocket session finished: {}", peer);
    }

    Ok(())
}

pub async fn ws_server_start(
    server: Arc<RwLock<WebSocketServer>>,
    addr: String,
) -> Result<(), anyhow::Error> {
    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: {}", addr);

    tokio::select! {
        stream = listener.accept() => {
            let stream = stream.unwrap().0;
            let peer = stream
                .peer_addr()
                .expect("connected streams should have a peer address");

            tokio::spawn(ws_accept_connection(server.clone(), peer, stream));
        }
    }

    Ok(())
}

#[async_trait]
impl Sessions for WebSocketSessions {
    async fn send_text(&mut self, player: Objid, msg: &str) -> Result<(), anyhow::Error> {
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
        SplitSink::send(&mut conn.sink, msg.into()).await?;

        Ok(())
    }

    async fn connected_players(&self) -> Result<Vec<Objid>, anyhow::Error> {
        Ok(self.connections.keys().cloned().collect())
    }
}
