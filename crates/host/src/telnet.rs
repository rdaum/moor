use std::net::SocketAddr;

use anyhow::bail;
use anyhow::Context;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;
use futures_util::StreamExt;
use tmq::request_reply::RequestSender;
use tmq::subscribe::Subscribe;
use tmq::{request, subscribe};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::time::Instant;
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{debug, error, info, trace};
use uuid::Uuid;

use moor_kernel::tasks::command_parse::parse_into_words;
use moor_values::var::objid::Objid;
use rpc_common::RpcRequest::ConnectionEstablish;
use rpc_common::{ConnectType, ConnectionEvent, RpcError, RpcResult};
use rpc_common::{RpcRequest, RpcResponse};

use crate::rpc;
use crate::rpc::make_rpc_call;
use crate::rpc::narrative_recv;

pub(crate) struct TelnetConnection {
    client_id: Uuid,
    player: Objid,
    peer_addr: SocketAddr,
    write: SplitSink<Framed<TcpStream, LinesCodec>, String>,
    read: SplitStream<Framed<TcpStream, LinesCodec>>,
    last_activity: Instant,
    connect_time: Instant,
}

impl TelnetConnection {
    async fn authorization_phase(
        &mut self,
        narrative_sub: &mut Subscribe,
        mut rcp_request_sock: RequestSender,
    ) -> Result<(Objid, ConnectType, RequestSender), anyhow::Error> {
        debug!(client_id = ?self.client_id, "Entering auth loop");
        loop {
            select! {
                Ok(msg) = narrative_recv(self.client_id, narrative_sub) => {
                    trace!(msg = ?msg, "narrative_message");
                    match msg {
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
                    let (response, rcp_request_sock_rep) = make_rpc_call(self.client_id, rcp_request_sock,
                        RpcRequest::LoginCommand(words)).await.expect("Unable to send login request to RPC server");
                    rcp_request_sock = rcp_request_sock_rep;
                    if let RpcResult::Success(RpcResponse::LoginResult(Some((connect_type, player)))) = response {
                        info!(?player, client_id = ?self.client_id, "Login successful");
                        return Ok((player, connect_type, rcp_request_sock))
                    }
                }
            }
        }
    }

    async fn command_loop(
        &mut self,
        narrative_sub: &mut Subscribe,
        mut rcp_request_sock: RequestSender,
    ) -> Result<(), anyhow::Error> {
        loop {
            select! {
                line = self.read.next() => {
                    let Some(line) = line else {
                        info!("Connection closed");
                        return Ok(());
                    };
                    let line = line.unwrap();
                    let (response, rcp_request_sock_rep) = make_rpc_call(self.client_id, rcp_request_sock,
                        RpcRequest::Command(line)).await?;
                    rcp_request_sock = rcp_request_sock_rep;
                    match response {
                        RpcResult::Success(RpcResponse::CommandComplete) => {
                            // Nothing to do
                        }
                        RpcResult::Failure(RpcError::CouldNotParseCommand) => {
                            self.write.send("I don't understand that.".to_string()).await?;
                        }
                        RpcResult::Failure(RpcError::NoCommandMatch(_)) => {
                            self.write.send("I don't see that here.".to_string()).await?;
                        }
                        RpcResult::Failure(RpcError::PermissionDenied) => {
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
                Ok(msg) = narrative_recv(self.client_id, narrative_sub) => {
                    trace!(msg = ?msg, "narrative_message");
                    match msg {
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
        let narrative_address = narrative_address.to_string();
        let rpc_address = rpc_address.to_string();
        tokio::spawn(async move {
            let client_id = Uuid::new_v4();
            info!(peer_addr = ?peer_addr, client_id = ?client_id,
                "Accepted connection"
            );

            let mut rcp_request_sock = request(&zmq_ctx)
                .set_rcvtimeo(100)
                .set_sndtimeo(100)
                .connect(rpc_address.as_str())
                .expect("Unable to bind RPC server for connection");

            // And let the RPC server know we're here, and it should start sending events on the
            // narrative subscription.
            debug!(rpc_address, "Contacting RPC server to establish connection");
            let connection_oid = match rpc::make_rpc_call(
                client_id,
                rcp_request_sock,
                ConnectionEstablish(peer_addr.to_string()),
            )
            .await
            {
                Ok((RpcResult::Success(RpcResponse::NewConnection(objid)), recv_sock)) => {
                    info!("Connection established, connection ID: {}", objid);
                    rcp_request_sock = recv_sock;
                    objid
                }
                Ok((RpcResult::Failure(f), _)) => {
                    bail!("RPC failure in connection establishment: {}", f);
                }
                Ok((_, _)) => {
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
                .connect(narrative_address.as_str())
                .expect("Unable to connect narrative subscriber ");
            let mut narrative_sub = narrative_sub
                .subscribe(&client_id.as_bytes()[..])
                .expect("Unable to subscribe to narrative messages for connection");

            info!(
                "Subscribed on narrative for {:?}, socket addr {}",
                client_id.as_bytes(),
                narrative_address
            );

            // Now provoke welcome message, which is a login command with no arguments, and we
            // don't care about the reply at this point.
            let (_reply, sock) = rpc::make_rpc_call(
                client_id,
                rcp_request_sock,
                RpcRequest::LoginCommand(vec![]),
            )
            .await
            .expect("Unable to send login request to RPC server");

            rcp_request_sock = sock;

            let framed_stream = Framed::new(stream, LinesCodec::new());
            let (write, read): (SplitSink<Framed<TcpStream, LinesCodec>, String>, _) =
                framed_stream.split();

            let mut tcp_connection = TelnetConnection {
                client_id,
                player: connection_oid,
                peer_addr,
                write,
                read,
                last_activity: Instant::now(),
                connect_time: Instant::now(),
            };
            let Ok((player, connect_type, rcp_request_sock)) = tcp_connection
                .authorization_phase(&mut narrative_sub, rcp_request_sock)
                .await
            else {
                bail!("Unable to authorize connection");
            };

            let connect_message = match connect_type {
                ConnectType::Connected => "** Connected **",
                ConnectType::Reconnected => "** Reconnected **",
                ConnectType::Created => "** Created **",
            };
            tcp_connection
                .write
                .send(connect_message.to_string())
                .await?;

            debug!(?player, ?client_id, "Entering command dispatch loop");
            if tcp_connection
                .command_loop(&mut narrative_sub, rcp_request_sock)
                .await
                .is_err()
            {
                info!("Connection closed");
            };
            Ok(())
        });
    }
}
