use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{accept_async, WebSocketStream};
use tracing::{error, info};
use tungstenite::{Error, Message};

use crate::model::var::Objid;
use crate::server::scheduler::Scheduler;
use crate::server::Sessions;

struct WebSocketSessions {
    connections: HashMap<Objid, WsConnection>,
}

struct WsConnection {
    player: Objid,
    sink: SplitSink<WebSocketStream<TcpStream>, Message>,
}

pub struct WebSocketServer {
    sessions: Arc<Mutex<WebSocketSessions>>,
    scheduler: Arc<Mutex<Scheduler>>,
}

impl WebSocketServer {
    pub fn new(scheduler: Arc<Mutex<Scheduler>>) -> Self {
        let inner = WebSocketSessions {
            connections: Default::default(),
        };
        Self {
            scheduler,
            sessions: Arc::new(Mutex::new(inner)),
        }
    }
}

async fn ws_accept_connection(
    server: Arc<Mutex<WebSocketServer>>,
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
    server: Arc<Mutex<WebSocketServer>>,
    player: Objid,
    msg: String,
) -> Result<(), anyhow::Error> {
    let server = server.lock().await;
    server
        .sessions
        .clone()
        .lock()
        .await
        .send_text(player, msg)
        .await
}

async fn ws_handle_connection(
    server: Arc<Mutex<WebSocketServer>>,
    peer: SocketAddr,
    stream: TcpStream,
) -> Result<(), anyhow::Error> {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");

    let player = Objid(2);

    info!("New WebSocket connection: {}", peer);
    let (ws_sender, mut ws_receiver) = WebSocketStream::split(ws_stream);

    // Register connection with player.
    {
        let server = server.lock().await;
        let client_connection = WsConnection {
            player,
            sink: ws_sender,
        };
        let connections = &mut server.sessions.lock().await.connections;

        let mut old = connections.insert(player, client_connection);
        if let Some(ref mut old) = old {
            SplitSink::send(&mut old.sink, "Reconnecting".into())
                .await
                .unwrap();
            old.sink
                .close()
                .await
                .expect("Could not close old connection");
        }
    }

    // Task submission loop.
    while let Some(msg) = ws_receiver.next().await {
        let msg = msg?;
        if msg.is_text() || msg.is_binary() {
            let cmd = msg.into_text().unwrap();
            let cmd = cmd.as_str().trim();

            let task_id = {
                let server = server.lock().await;
                let mut scheduler = server.scheduler.lock().await;
                scheduler
                    .setup_parse_command_task(player, cmd, server.sessions.clone())
                    .await
            };
            let task_id = match task_id {
                Ok(task_id) => task_id,
                Err(e) => {
                    error!("Unable to parse command ({}): {:?}", cmd, e);
                    ws_send_error(
                        server.clone(),
                        player,
                        format!("Unable to parse command ({}): {:?}", cmd, e),
                    )
                    .await?;

                    continue;
                }
            };
            info!("Task: {:?}", task_id);
            {
                let server = server.lock().await;
                let mut scheduler = server.scheduler.lock().await;
                if let Err(e) = scheduler.start_task(task_id).await {
                    error!("Unable to execute: {}", e);
                    continue;
                };
            }
        }
    }

    // Now drop the connection from sessions
    {
        let server = server.lock().await;
        let connections = &mut server.sessions.lock().await.connections;
        connections.remove(&player).unwrap();
    }

    Ok(())
}

pub async fn ws_server_start(
    server: Arc<Mutex<WebSocketServer>>,

    addr: String,
) -> Result<(), anyhow::Error> {
    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");

        tokio::spawn(ws_accept_connection(server.clone(), peer, stream));
    }

    Ok(())
}

#[async_trait]
impl Sessions for WebSocketSessions {
    async fn send_text(&mut self, player: Objid, msg: String) -> Result<(), anyhow::Error> {
        let Some(conn) = self.connections.get_mut(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        SplitSink::send(&mut conn.sink, msg.into()).await?;

        Ok(())
    }

    async fn connected_players(&mut self) -> Result<Vec<Objid>, anyhow::Error> {
        Ok(self.connections.keys().cloned().collect())
    }
}
