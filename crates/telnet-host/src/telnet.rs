use std::net::SocketAddr;
use std::time::SystemTime;

use anyhow::bail;
use anyhow::Context;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;
use futures_util::StreamExt;
use tmq::subscribe::Subscribe;
use tmq::{request, subscribe};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{debug, error, info, trace};
use uuid::Uuid;

use moor_kernel::tasks::command_parse::parse_into_words;
use moor_values::model::CommandError;
use moor_values::var::objid::Objid;
use rpc_common::pubsub_client::{broadcast_recv, narrative_recv};
use rpc_common::rpc_client::RpcSendClient;
use rpc_common::RpcRequest::ConnectionEstablish;
use rpc_common::{
    BroadcastEvent, ConnectType, ConnectionEvent, RpcRequestError, RpcResult, BROADCAST_TOPIC,
};
use rpc_common::{RpcRequest, RpcResponse};

/// Out of band messages are prefixed with this string, e.g. for MCP clients.
const OUT_OF_BAND_PREFIX: &str = "#$#";

pub(crate) struct TelnetConnection {
    client_id: Uuid,
    write: SplitSink<Framed<TcpStream, LinesCodec>, String>,
    read: SplitStream<Framed<TcpStream, LinesCodec>>,
}

impl TelnetConnection {
    async fn run(
        &mut self,
        narrative_sub: &mut Subscribe,
        broadcast_sub: &mut Subscribe,
        rpc_client: &mut RpcSendClient,
    ) -> Result<(), anyhow::Error> {
        // Provoke welcome message, which is a login command with no arguments, and we
        // don't care about the reply at this point.
        rpc_client
            .make_rpc_call(self.client_id, RpcRequest::LoginCommand(vec![]))
            .await
            .expect("Unable to send login request to RPC server");

        let Ok((player, connect_type)) = self
            .authorization_phase(narrative_sub, broadcast_sub, rpc_client)
            .await
        else {
            bail!("Unable to authorize connection");
        };

        let connect_message = match connect_type {
            ConnectType::Connected => "** Connected **",
            ConnectType::Reconnected => "** Reconnected **",
            ConnectType::Created => "** Created **",
        };
        self.write.send(connect_message.to_string()).await?;

        debug!(?player, client_id = ?self.client_id, "Entering command dispatch loop");
        if self
            .command_loop(narrative_sub, broadcast_sub, rpc_client)
            .await
            .is_err()
        {
            info!("Connection closed");
        };

        // Let the server know this client is gone.
        rpc_client
            .make_rpc_call(self.client_id, RpcRequest::Detach)
            .await?;

        Ok(())
    }

    async fn authorization_phase(
        &mut self,
        narrative_sub: &mut Subscribe,
        broadcast_sub: &mut Subscribe,
        rpc_client: &mut RpcSendClient,
    ) -> Result<(Objid, ConnectType), anyhow::Error> {
        debug!(client_id = ?self.client_id, "Entering auth loop");
        loop {
            select! {
                Ok(event) = broadcast_recv(broadcast_sub) => {
                    trace!(?event, "broadcast_event");
                    match event {
                        BroadcastEvent::PingPong(_server_time) => {
                            let _ = rpc_client.make_rpc_call(self.client_id,
                                RpcRequest::Pong(SystemTime::now())).await?;
                        }
                    }
                }
                Ok(event) = narrative_recv(self.client_id, narrative_sub) => {
                    trace!(?event, "narrative_event");
                    match event {
                        ConnectionEvent::SystemMessage(_author, msg) => {
                            self.write.send(msg).await.with_context(|| "Unable to send message to client")?;
                        }
                        ConnectionEvent::Narrative(_author, event) => {
                            let msg = event.event();
                            self.write.send(msg).await.with_context(|| "Unable to send message to client")?;
                        }
                        ConnectionEvent::Disconnect() => {
                            self.write.close().await?;
                            bail!("Disconnect before login");
                        }
                    }
                }
                // Auto loop
                line = self.read.next() => {
                    let Some(line) = line else {
                        bail!("Connection closed before login");
                    };
                    let line = line.unwrap();
                    let words = parse_into_words(&line);
                    let response = rpc_client.make_rpc_call(self.client_id,
                        RpcRequest::LoginCommand(words)).await.expect("Unable to send login request to RPC server");
                    if let RpcResult::Success(RpcResponse::LoginResult(Some((connect_type, player)))) = response {
                        info!(?player, client_id = ?self.client_id, "Login successful");
                        return Ok((player, connect_type))
                    }
                }
            }
        }
    }

    async fn command_loop(
        &mut self,
        narrative_sub: &mut Subscribe,
        broadcast_sub: &mut Subscribe,
        rpc_client: &mut RpcSendClient,
    ) -> Result<(), anyhow::Error> {
        loop {
            select! {
                line = self.read.next() => {
                    let Some(line) = line else {
                        info!("Connection closed");
                        return Ok(());
                    };
                    let line = line.unwrap();

                    // If the line begins with the out of band prefix, then send it that way,
                    // instead. And really just fire and forget.
                    let response = if line.starts_with(OUT_OF_BAND_PREFIX) {
                        rpc_client.make_rpc_call(self.client_id, RpcRequest::OutOfBand(line)).await?
                    } else {
                        rpc_client.make_rpc_call(self.client_id, RpcRequest::Command(line)).await?
                    };

                    match response {
                        RpcResult::Success(RpcResponse::CommandComplete) => {
                            // Nothing to do
                        }
                        RpcResult::Failure(RpcRequestError::CommandError(CommandError::CouldNotParseCommand)) => {
                            self.write.send("I don't understand that.".to_string()).await?;
                        }
                        RpcResult::Failure(RpcRequestError::CommandError(CommandError::NoObjectMatch)) => {
                            self.write.send("I don't see that here.".to_string()).await?;
                        }
                        RpcResult::Failure(RpcRequestError::CommandError(CommandError::NoCommandMatch)) => {
                            self.write.send("I don't understand that.".to_string()).await?;
                        }
                        RpcResult::Failure(RpcRequestError::CommandError(CommandError::PermissionDenied)) => {
                            self.write.send("You can't do that.".to_string()).await?;
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
                Ok(event) = broadcast_recv(broadcast_sub) => {
                    trace!(?event, "broadcast_event");
                    match event {
                        BroadcastEvent::PingPong(_server_time) => {
                            let _ = rpc_client.make_rpc_call(self.client_id,
                                RpcRequest::Pong(SystemTime::now())).await?;
                        }
                    }
                }
                Ok(event) = narrative_recv(self.client_id, narrative_sub) => {
                    trace!(?event, "narrative_event");
                    match event {
                        ConnectionEvent::SystemMessage(_author, msg) => {
                            self.write.send(msg).await.with_context(|| "Unable to send message to client")?;
                        }
                        ConnectionEvent::Narrative(_author, event) => {
                            let msg = event.event();
                            self.write.send(msg).await.with_context(|| "Unable to send message to client")?;
                        }
                        ConnectionEvent::Disconnect() => {
                            self.write.send("** Disconnected **".to_string()).await.expect("Unable to send disconnect message to client");
                            self.write.close().await.expect("Unable to close connection");
                            return Ok(())
                        }
                    }
                }
            }
        }
    }
}

pub async fn telnet_listen_loop(
    telnet_sockaddr: SocketAddr,
    rpc_address: &str,
    narrative_address: &str,
) -> Result<(), anyhow::Error> {
    let listener = TcpListener::bind(telnet_sockaddr).await?;
    let zmq_ctx = tmq::Context::new();

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let zmq_ctx = zmq_ctx.clone();
        let pubsub_address = narrative_address.to_string();
        let rpc_address = rpc_address.to_string();
        tokio::spawn(async move {
            let client_id = Uuid::new_v4();
            info!(peer_addr = ?peer_addr, client_id = ?client_id,
                "Accepted connection"
            );

            let rcp_request_sock = request(&zmq_ctx)
                .set_rcvtimeo(100)
                .set_sndtimeo(100)
                .connect(rpc_address.as_str())
                .expect("Unable to bind RPC server for connection");

            // And let the RPC server know we're here, and it should start sending events on the
            // narrative subscription.
            debug!(rpc_address, "Contacting RPC server to establish connection");
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
                    bail!("RPC failure in connection establishment: {}", f);
                }
                Ok(_) => {
                    bail!("Unexpected response from RPC server");
                }
                Err(e) => {
                    bail!("Unable to establish connection: {}", e);
                }
            };
            debug!(client_id = ?client_id, connection = ?connection_oid, "Connection established");

            // Before attempting login, we subscribe to the narrative channel, using our client
            // id. The daemon should be sending events here.
            let narrative_sub = subscribe(&zmq_ctx)
                .connect(pubsub_address.as_str())
                .expect("Unable to connect narrative subscriber ");
            let mut narrative_sub = narrative_sub
                .subscribe(&client_id.as_bytes()[..])
                .expect("Unable to subscribe to narrative messages for client connection");

            let broadcast_sub = subscribe(&zmq_ctx)
                .connect(pubsub_address.as_str())
                .expect("Unable to connect broadcast subscriber ");
            let mut broadcast_sub = broadcast_sub
                .subscribe(BROADCAST_TOPIC)
                .expect("Unable to subscribe to broadcast messages for client connection");

            info!(
                "Subscribed on pubsub socket for {:?}, socket addr {}",
                client_id, pubsub_address
            );

            // Re-ify the connection.
            let framed_stream = Framed::new(stream, LinesCodec::new());
            let (write, read): (SplitSink<Framed<TcpStream, LinesCodec>, String>, _) =
                framed_stream.split();
            let mut tcp_connection = TelnetConnection {
                client_id,
                write,
                read,
            };

            tcp_connection
                .run(&mut narrative_sub, &mut broadcast_sub, &mut rpc_client)
                .await?;
            Ok(())
        });
    }
}
