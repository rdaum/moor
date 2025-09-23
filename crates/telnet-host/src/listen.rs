// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::connection::TelnetConnection;
use crate::connection_codec::ConnectionCodec;
use eyre::bail;
use futures_util::StreamExt;
use hickory_resolver::TokioResolver;
use moor_var::{Obj, Symbol};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_async_client::{ListenersClient, ListenersMessage};
use rpc_common::HostClientToDaemonMessage::ConnectionEstablish;
use rpc_common::{CLIENT_BROADCAST_TOPIC, DaemonToClientReply, ReplyResult};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tmq::{request, subscribe};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio_util::codec::Framed;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Perform async reverse DNS lookup for an IP address
async fn resolve_hostname(ip: IpAddr) -> Result<String, eyre::Error> {
    // Create a new resolver using system configuration
    let resolver = TokioResolver::builder_tokio()?.build();

    // Perform reverse DNS lookup
    let response = resolver.reverse_lookup(ip).await?;

    // Get the first hostname from the response
    if let Some(name) = response.iter().next() {
        Ok(name.to_string().trim_end_matches('.').to_string())
    } else {
        Err(eyre::eyre!("No PTR record found"))
    }
}

pub struct Listeners {
    listeners: HashMap<SocketAddr, Listener>,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    events_address: String,
    kill_switch: Arc<AtomicBool>,
}

impl Listeners {
    pub fn new(
        zmq_ctx: tmq::Context,
        rpc_address: String,
        events_address: String,
        kill_switch: Arc<AtomicBool>,
    ) -> (
        Self,
        tokio::sync::mpsc::Receiver<ListenersMessage>,
        ListenersClient,
    ) {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let listeners = Self {
            listeners: HashMap::new(),
            zmq_ctx,
            rpc_address,
            events_address,
            kill_switch,
        };
        let listeners_client = ListenersClient::new(tx);
        (listeners, rx, listeners_client)
    }

    pub async fn run(
        &mut self,
        mut listeners_channel: tokio::sync::mpsc::Receiver<ListenersMessage>,
    ) {
        self.zmq_ctx
            .set_io_threads(8)
            .expect("Unable to set ZMQ IO threads");

        loop {
            if self.kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                info!("Host kill switch activated, stopping...");
                return;
            }

            match listeners_channel.recv().await {
                Some(ListenersMessage::AddListener(handler, addr)) => {
                    let listener = TcpListener::bind(addr)
                        .await
                        .expect("Unable to bind listener");
                    let (terminate_send, terminate_receive) = tokio::sync::watch::channel(false);
                    self.listeners
                        .insert(addr, Listener::new(terminate_send, handler));

                    info!("Listening @ {}", addr);
                    let zmq_ctx = self.zmq_ctx.clone();
                    let rpc_address = self.rpc_address.clone();
                    let events_address = self.events_address.clone();
                    let kill_switch = self.kill_switch.clone();

                    // One task per listener.
                    tokio::spawn(async move {
                        loop {
                            let mut term_receive = terminate_receive.clone();
                            select! {
                                _ = term_receive.changed() => {
                                    info!("Listener terminated, stopping...");
                                    break;
                                }
                                result = listener.accept() => {
                                    match result {
                                        Ok((stream, addr)) => {
                                            info!(?addr, "Accepted connection for listener");
                                            let listener_port = addr.port();
                                            let zmq_ctx = zmq_ctx.clone();
                                            let rpc_address = rpc_address.clone();
                                            let events_address = events_address.clone();
                                            let kill_switch = kill_switch.clone();

                                            // Spawn a task to handle the accepted connection.
                                            tokio::spawn(Listener::handle_accepted_connection(
                                                zmq_ctx,
                                                rpc_address,
                                                events_address,
                                                handler,
                                                kill_switch,
                                                listener_port,
                                                stream,
                                                addr,
                                            ));
                                        }
                                        Err(e) => {
                                            warn!(?e, "Accept failed, can't handle connection");
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
                Some(ListenersMessage::RemoveListener(addr)) => {
                    let listener = self.listeners.remove(&addr);
                    info!(?addr, "Removing listener");
                    if let Some(listener) = listener {
                        listener
                            .terminate
                            .send(true)
                            .expect("Unable to send terminate message");
                    }
                }
                Some(ListenersMessage::GetListeners(tx)) => {
                    let listeners = self
                        .listeners
                        .iter()
                        .map(|(addr, listener)| (listener.handler_object, *addr))
                        .collect();
                    tx.send(listeners).expect("Unable to send listeners list");
                }
                None => {
                    warn!("Listeners channel closed, stopping...");
                    return;
                }
            }
        }
    }
}

pub struct Listener {
    pub(crate) handler_object: Obj,
    pub(crate) terminate: tokio::sync::watch::Sender<bool>,
}

impl Listener {
    pub fn new(terminate: tokio::sync::watch::Sender<bool>, handler_object: Obj) -> Self {
        Self {
            handler_object,
            terminate,
        }
    }

    async fn handle_accepted_connection(
        zmq_ctx: tmq::Context,
        rpc_address: String,
        events_address: String,
        handler_object: Obj,
        kill_switch: Arc<AtomicBool>,
        listener_port: u16,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<(), eyre::Report> {
        let connection_kill_switch = kill_switch.clone();
        let rpc_address = rpc_address.clone();
        let events_address = events_address.clone();
        let zmq_ctx = zmq_ctx.clone();
        tokio::spawn(async move {
            let client_id = Uuid::new_v4();
            info!(peer_addr = ?peer_addr, client_id = ?client_id, port = listener_port,
                "Accepted connection for listener"
            );

            let rpc_request_sock = request(&zmq_ctx)
                .set_rcvtimeo(100)
                .set_sndtimeo(100)
                .connect(rpc_address.as_str())
                .expect("Unable to bind RPC server for connection");

            // And let the RPC server know we're here, and it should start sending events on the
            // narrative subscription.
            debug!(rpc_address, "Setting up telnet connection for negotiation");
            let mut rpc_client = RpcSendClient::new(rpc_request_sock);

            // Initialize basic connection attributes for telnet
            let mut connection_attributes = std::collections::HashMap::new();
            connection_attributes.insert(Symbol::mk("host-type"), moor_var::Var::from("telnet"));
            connection_attributes.insert(
                Symbol::mk("supports-telnet-protocol"),
                moor_var::Var::mk_bool(true),
            );
            // Default telnet echo state: client echoes input (client-echo = true)
            connection_attributes.insert(Symbol::mk("client-echo"), moor_var::Var::mk_bool(true));

            // Perform reverse DNS lookup for hostname
            let hostname = {
                match resolve_hostname(peer_addr.ip()).await {
                    Ok(hostname) => {
                        debug!("Resolved {} to hostname: {}", peer_addr.ip(), hostname);
                        hostname
                    }
                    Err(_) => {
                        debug!("Failed to resolve {}, using IP address", peer_addr.ip());
                        peer_addr.to_string()
                    }
                }
            };

            // Now establish the connection with all negotiated attributes
            let (client_token, connection_oid) = match rpc_client
                .make_client_rpc_call(
                    client_id,
                    ConnectionEstablish {
                        peer_addr: hostname,
                        local_port: listener_port,
                        remote_port: peer_addr.port(),
                        acceptable_content_types: Some(vec![
                            Symbol::mk("text_djot"),
                            Symbol::mk("text_markdown"),
                            Symbol::mk("text_plain"),
                        ]),
                        connection_attributes: Some(connection_attributes.clone()),
                    },
                )
                .await
            {
                Ok(ReplyResult::ClientSuccess(DaemonToClientReply::NewConnection(
                    token,
                    objid,
                ))) => {
                    info!(
                        "Connection established with {} attributes, connection ID: {}",
                        connection_attributes.len(),
                        objid
                    );
                    (token, objid)
                }
                Ok(ReplyResult::Failure(f)) => {
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

            // Before attempting login, we subscribe to the events socket, using our client
            // id. The daemon should be sending events here.
            let events_sub = subscribe(&zmq_ctx)
                .connect(events_address.as_str())
                .expect("Unable to connect narrative subscriber ");
            let events_sub = events_sub
                .subscribe(&client_id.as_bytes()[..])
                .expect("Unable to subscribe to narrative messages for client connection");
            let broadcast_sub = subscribe(&zmq_ctx)
                .connect(events_address.as_str())
                .expect("Unable to connect broadcast subscriber ");
            let broadcast_sub = broadcast_sub
                .subscribe(CLIENT_BROADCAST_TOPIC)
                .expect("Unable to subscribe to broadcast messages for client connection");

            info!(
                "Subscribed on pubsub events socket for {:?}, socket addr {}",
                client_id, events_address
            );

            // Re-ify the connection.
            let framed_stream = Framed::new(stream, ConnectionCodec::new());
            let (write, read) = framed_stream.split();
            let mut tcp_connection = TelnetConnection {
                handler_object,
                peer_addr,
                connection_object: connection_oid,
                player_object: None,
                client_token,
                client_id,
                write,
                read,
                kill_switch: connection_kill_switch,
                broadcast_sub,
                narrative_sub: events_sub,
                auth_token: None,
                rpc_client,
                pending_task: None,
                output_prefix: None,
                output_suffix: None,
                flush_command: crate::connection::DEFAULT_FLUSH_COMMAND.to_string(),
                connection_attributes,
                is_binary_mode: false,
                hold_input: None,
                disable_oob: false,
            };

            tcp_connection.run().await?;
            Ok(())
        });
        Ok(())
    }
}
