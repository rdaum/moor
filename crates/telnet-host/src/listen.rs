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

use crate::{connection::TelnetConnection, connection_codec::ConnectionCodec};
use eyre::bail;
use futures_util::StreamExt;
use hickory_resolver::TokioResolver;
use moor_schema::{convert::var_to_flatbuffer, rpc as moor_rpc};
use moor_var::{Obj, Symbol};
use rpc_async_client::{ListenersClient, ListenersMessage, rpc_client::RpcSendClient};
use rpc_common::{CLIENT_BROADCAST_TOPIC, extract_obj, mk_connection_establish_msg};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, atomic::AtomicBool},
};
use tmq::{request, subscribe};
use tokio::{
    net::{TcpListener, TcpStream},
    select,
};
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
                    Ok(hostname) => hostname,
                    Err(_) => peer_addr.to_string(),
                }
            };

            // Now establish the connection with all negotiated attributes
            let acceptable_types_fb: Vec<moor_rpc::Symbol> = vec![
                moor_rpc::Symbol {
                    value: "text_djot".to_string(),
                },
                moor_rpc::Symbol {
                    value: "text_markdown".to_string(),
                },
                moor_rpc::Symbol {
                    value: "text_plain".to_string(),
                },
            ];

            let connection_attrs_fb: Vec<moor_rpc::ConnectionAttribute> = connection_attributes
                .iter()
                .filter_map(|(k, v)| {
                    let var_fb = var_to_flatbuffer(v).ok()?;
                    Some(moor_rpc::ConnectionAttribute {
                        key: Box::new(moor_rpc::Symbol {
                            value: k.as_string(),
                        }),
                        value: Box::new(var_fb),
                    })
                })
                .collect();

            let conn_establish_msg = mk_connection_establish_msg(
                hostname.clone(),
                listener_port,
                peer_addr.port(),
                Some(acceptable_types_fb),
                Some(connection_attrs_fb),
            );

            let (client_token, connection_oid) = match rpc_client
                .make_client_rpc_call(client_id, conn_establish_msg)
                .await
            {
                Ok(bytes) => {
                    use planus::ReadAsRoot;
                    let reply_ref = moor_rpc::ReplyResultRef::read_as_root(&bytes)
                        .map_err(|e| eyre::eyre!("Invalid flatbuffer: {}", e))?;

                    match reply_ref
                        .result()
                        .map_err(|e| eyre::eyre!("Missing result: {}", e))?
                    {
                        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                            let daemon_reply = client_success
                                .reply()
                                .map_err(|e| eyre::eyre!("Missing reply: {}", e))?;
                            match daemon_reply
                                .reply()
                                .map_err(|e| eyre::eyre!("Missing reply union: {}", e))?
                            {
                                moor_rpc::DaemonToClientReplyUnionRef::NewConnection(new_conn) => {
                                    let token_ref = new_conn
                                        .client_token()
                                        .map_err(|e| eyre::eyre!("Missing client_token: {}", e))?;
                                    let token = rpc_common::ClientToken(
                                        token_ref
                                            .token()
                                            .map_err(|e| eyre::eyre!("Missing token: {}", e))?
                                            .to_string(),
                                    );

                                    let objid = extract_obj(&new_conn, "connection_obj", |n| {
                                        n.connection_obj()
                                    })
                                    .map_err(|e| eyre::eyre!("{}", e))?;

                                    info!(
                                        "Connection established with {} attributes, connection ID: {}",
                                        connection_attributes.len(),
                                        objid
                                    );
                                    (token, objid)
                                }
                                _ => {
                                    bail!("Unexpected response from RPC server");
                                }
                            }
                        }
                        moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                            let error_ref = failure
                                .error()
                                .map_err(|e| eyre::eyre!("Missing error: {}", e))?;
                            let error_msg = error_ref
                                .message()
                                .unwrap_or(Some("Unknown error"))
                                .unwrap_or("Unknown error");
                            bail!("RPC failure in connection establishment: {}", error_msg);
                        }
                        _ => {
                            bail!("Unexpected response type");
                        }
                    }
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
