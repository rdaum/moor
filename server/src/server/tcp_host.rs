use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Error;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use metrics_macros::increment_counter;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{error, info, warn};

use moor_core::tasks::command_parse::parse_into_words;
use moor_core::tasks::scheduler::SchedulerError;
use moor_values::model::NarrativeEvent;
use moor_values::var::objid::Objid;
use moor_values::SYSTEM_OBJECT;

use crate::server::connection::Connection;
use crate::server::server::Server;
use crate::server::{ConnectType, DisconnectReason};

pub struct TcpHost {
    listener: TcpListener,
    server: Arc<Server>,
}

impl TcpHost {
    pub async fn new(
        socket_address: SocketAddr,
        server: Arc<Server>,
    ) -> Result<Self, anyhow::Error> {
        let listener = TcpListener::bind(socket_address).await?;
        Ok(Self { listener, server })
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        info!("Listening on {:?}", self.listener.local_addr()?);
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            info!("Accepted connection from {:?}", peer_addr);

            let connection_server = self.server.clone();
            tokio::spawn(async move {
                process_connection_loop(peer_addr, stream, connection_server).await;
            });
        }
    }
}

pub struct TcpConnection {
    player: Objid,
    peer_addr: SocketAddr,
    stream: SplitSink<Framed<TcpStream, LinesCodec>, String>,
    last_activity: Instant,
    connect_time: Instant,
}

impl TcpConnection {}
#[async_trait::async_trait]
impl Connection for TcpConnection {
    async fn write_message(&mut self, msg: NarrativeEvent) -> Result<(), Error> {
        self.stream.send(msg.event().to_string()).await?;
        Ok(())
    }

    async fn notify_connected(
        &mut self,
        player: Objid,
        connect_type: ConnectType,
    ) -> Result<(), Error> {
        match connect_type {
            ConnectType::Connected => {
                self.stream.send("** Connected **".to_string()).await?;
            }
            ConnectType::Reconnected => {
                self.stream.send("** Reconnected **".to_string()).await?;
            }
            ConnectType::Created => {
                self.stream.send("** Created **".to_string()).await?;
            }
        }
        self.player = player;
        Ok(())
    }

    async fn disconnect(&mut self, reason: DisconnectReason) -> Result<(), Error> {
        match reason {
            DisconnectReason::None => {
                self.stream.send("** Disconnected **".to_string()).await?;
            }
            DisconnectReason::Reconnected => {
                self.stream.send("** Reconnected **".to_string()).await?;
            }
            DisconnectReason::Booted(msg) => {
                self.stream
                    .send(format!("** Disconnected: {:?} **", msg))
                    .await?;
            }
        }
        if let Err(e) = self.stream.close().await {
            error!(error=?e, "Error closing connection");
        }
        Ok(())
    }

    async fn connection_name(&self, _player: Objid) -> Result<String, Error> {
        // See TODO on WSConnection::connection_name
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
        self.connect_time
    }
}

async fn send_error(server: Arc<Server>, player: Objid, message: &str) -> Result<(), Error> {
    server
        .write_messages(
            SYSTEM_OBJECT,
            &[(
                player,
                NarrativeEvent::new_ephemeral(SYSTEM_OBJECT, message.to_string()),
            )],
        )
        .await
}
async fn process_connection_loop(peer_addr: SocketAddr, stream: TcpStream, server: Arc<Server>) {
    info!("Accepted connection from {:?}", peer_addr);
    let framed_stream = Framed::new(stream, LinesCodec::new());
    let (write, mut read) = framed_stream.split();
    let connection_oid = server
        .clone()
        .new_connection(|oid| {
            Ok(Box::new(TcpConnection {
                player: oid,
                peer_addr,
                stream: write,
                last_activity: Instant::now(),
                connect_time: Instant::now(),
            }))
        })
        .await
        .expect("Failed to create connection");

    // First thing is to ask the server to send the welcome message.
    if let Err(e) = server.clone().send_welcome_message(connection_oid).await {
        error!(error=?e, "Error sending welcome message");
        server
            .disconnected(connection_oid)
            .await
            .expect("Failed to record disconnect");
        return;
    }

    // Read lines from the connection and feed them to authenticate until we get a successful login.
    let (_, player) = loop {
        let line = match read.next().await {
            None => {
                info!("Connection closed");
                server
                    .disconnected(connection_oid)
                    .await
                    .expect("Failed to record disconnect");
                return;
            }
            Some(Ok(line)) => line,
            Some(Err(e)) => {
                error!(error=?e, "Error reading from connection");
                server
                    .disconnected(connection_oid)
                    .await
                    .expect("Failed to record disconnect");
                return;
            }
        };
        increment_counter!("tcp_host.login_command_received");
        let words = parse_into_words(&line);
        match server
            .clone()
            .login_command_line(connection_oid, &words)
            .await
        {
            Ok(Some(auth_result)) => break auth_result,
            Ok(None) => {
                info!("Login failure");
                increment_counter!("tcp_host.login_failure");
            }
            Err(e) => {
                error!(error=?e, "Error authenticating");
                server
                    .disconnected(connection_oid)
                    .await
                    .expect("Failed to record disconnect");
                return;
            }
        }
    };

    info!("Login successful");

    // Now we're logged in, so we can start the main submission loop, until disconnect.
    loop {
        let cmd = match read.next().await {
            None => {
                info!("Connection closed");
                break;
            }
            Some(Ok(cmd)) => cmd,
            Some(Err(e)) => {
                error!(error=?e, "Error reading from connection");
                break;
            }
        };
        increment_counter!("tcp_host.command_received");
        let cmd = cmd.as_str().trim();

        // Record activity on the connection, so we can compute idle_seconds.
        if let Err(e) = server.record_activity(player).await {
            warn!(player = ?player, "Error recording activity on connection: {:?}", e)
        }

        if let Err(e) = server.clone().handle_inbound_command(player, cmd).await {
            error!(player=?player, command=cmd, error=?e, "Error submitting command task");

            match e {
                SchedulerError::CouldNotParseCommand(_) | SchedulerError::NoCommandMatch(_, _) => {
                    increment_counter!("tcp_host.command_parse_error");
                    if let Err(e) =
                        send_error(server.clone(), player, "I don't understand that.").await
                    {
                        error!(player=?player, error=?e, "Error sending parse error message");
                        break;
                    }
                }
                SchedulerError::PermissionDenied => {
                    increment_counter!("tcp_host.command_permission_error");
                    if let Err(e) = send_error(server.clone(), player, "You can't do that.").await {
                        error!(player=?player, error=?e, "Error sending permission denied message");
                        break;
                    }
                }
                _ => {
                    increment_counter!("tcp_host.command_internal_error");
                    if let Err(e) = send_error(server.clone(), player, "Internal error.").await {
                        error!(player=?player, error=?e, "Error sending internal error message");
                        break;
                    }
                }
            }
        }
    }
    if let Err(e) = server.disconnected(player).await {
        error!(player=?player, error=?e, "Error recording player disconnect");
    }
    info!("Connection closed");
}
