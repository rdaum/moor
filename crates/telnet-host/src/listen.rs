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

use crate::{
    connection::{BoxedAsyncIo, TelnetConnection},
    connection_codec::ConnectionCodec,
};
use eyre::bail;
use futures_util::StreamExt;
use hickory_resolver::TokioResolver;
use moor_schema::{convert::var_to_flatbuffer, rpc as moor_rpc};
use moor_var::{Obj, Symbol};
use rpc_async_client::{
    ListenerInfo, ListenersClient, ListenersError, ListenersMessage, rpc_client::RpcClient, zmq,
};
use rpc_common::{
    CLIENT_BROADCAST_TOPIC, extract_obj, mk_connection_establish_msg, read_reply_result,
};
use rustls_pemfile::{certs, private_key};
use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    net::{IpAddr, SocketAddr},
    os::fd::{AsRawFd, RawFd},
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
use tmq::subscribe;
use tokio::{
    net::{TcpListener, TcpStream},
    select,
};
use tokio_rustls::{TlsAcceptor, rustls::ServerConfig};
use tokio_util::codec::Framed;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Load TLS configuration from certificate and key files.
pub fn load_tls_config(cert_path: &Path, key_path: &Path) -> Result<Arc<ServerConfig>, eyre::Error> {
    let cert_file = File::open(cert_path)
        .map_err(|e| eyre::eyre!("Failed to open certificate file {:?}: {}", cert_path, e))?;
    let key_file = File::open(key_path)
        .map_err(|e| eyre::eyre!("Failed to open key file {:?}: {}", key_path, e))?;

    let mut cert_reader = BufReader::new(cert_file);
    let mut key_reader = BufReader::new(key_file);

    let cert_chain: Vec<_> = certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| eyre::eyre!("Failed to parse certificate: {}", e))?;

    if cert_chain.is_empty() {
        return Err(eyre::eyre!("No certificates found in {:?}", cert_path));
    }

    let key = private_key(&mut key_reader)
        .map_err(|e| eyre::eyre!("Failed to parse private key: {}", e))?
        .ok_or_else(|| eyre::eyre!("No private key found in {:?}", key_path))?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .map_err(|e| eyre::eyre!("Failed to build TLS config: {}", e))?;

    Ok(Arc::new(config))
}

/// Perform async reverse DNS lookup for an IP address
async fn resolve_hostname(ip: IpAddr) -> Result<String, eyre::Error> {
    let resolver = TokioResolver::builder_tokio()?.build();
    let response = resolver.reverse_lookup(ip).await?;

    if let Some(name) = response.iter().next() {
        Ok(name.to_string().trim_end_matches('.').to_string())
    } else {
        Err(eyre::eyre!("No PTR record found"))
    }
}

/// Configure CURVE encryption on a SUB socket builder
fn configure_curve_subscriber(
    mut builder: tmq::SocketBuilder<tmq::subscribe::SubscribeWithoutTopic>,
    curve_keys: &Option<(String, String, String)>,
) -> tmq::SocketBuilder<tmq::subscribe::SubscribeWithoutTopic> {
    if let Some((client_secret, client_public, server_public)) = curve_keys {
        let client_secret_bytes =
            zmq::z85_decode(client_secret).expect("Failed to decode client secret key");
        let client_public_bytes =
            zmq::z85_decode(client_public).expect("Failed to decode client public key");
        let server_public_bytes =
            zmq::z85_decode(server_public).expect("Failed to decode server public key");

        builder = builder
            .set_curve_secretkey(&client_secret_bytes)
            .set_curve_publickey(&client_public_bytes)
            .set_curve_serverkey(&server_public_bytes);
    }
    builder
}

pub struct Listeners {
    listeners: HashMap<SocketAddr, Listener>,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    events_address: String,
    kill_switch: Arc<AtomicBool>,
    curve_keys: Option<(String, String, String)>, // (client_secret, client_public, server_public)
    tls_config: Option<Arc<ServerConfig>>,
}

impl Listeners {
    pub fn new(
        zmq_ctx: tmq::Context,
        rpc_address: String,
        events_address: String,
        kill_switch: Arc<AtomicBool>,
        curve_keys: Option<(String, String, String)>,
        tls_config: Option<Arc<ServerConfig>>,
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
            curve_keys,
            tls_config,
        };
        let listeners_client = ListenersClient::new(tx);
        (listeners, rx, listeners_client)
    }

    async fn start_listener(
        &mut self,
        handler: Obj,
        addr: SocketAddr,
        reply: tokio::sync::oneshot::Sender<Result<(), ListenersError>>,
        is_tls: bool,
    ) {
        let listener = match TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(e) => {
                let _ = reply.send(Err(ListenersError::AddListenerFailed(handler, addr)));
                error!(?addr, "Unable to bind listener: {}", e);
                return;
            }
        };

        let (terminate_send, terminate_receive) = tokio::sync::watch::channel(false);
        self.listeners
            .insert(addr, Listener::new(terminate_send, handler, is_tls));

        let tls_label = if is_tls { " (TLS)" } else { "" };
        info!("Listening @ {}{}", addr, tls_label);

        let zmq_ctx = self.zmq_ctx.clone();
        let rpc_address = self.rpc_address.clone();
        let events_address = self.events_address.clone();
        let kill_switch = self.kill_switch.clone();
        let curve_keys = self.curve_keys.clone();
        let tls_acceptor = if is_tls {
            self.tls_config.as_ref().map(|c| TlsAcceptor::from(c.clone()))
        } else {
            None
        };

        // Signal that the listener is successfully bound
        let _ = reply.send(Ok(()));

        let local_listener_port = listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or(addr.port());

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
                            Ok((stream, peer_addr)) => {
                                info!(?peer_addr, is_tls, "Accepted connection for listener");

                                // Get the raw fd before wrapping (for keep-alive support)
                                let socket_fd = stream.as_raw_fd();

                                let listener_port = local_listener_port;
                                let zmq_ctx = zmq_ctx.clone();
                                let rpc_address = rpc_address.clone();
                                let events_address = events_address.clone();
                                let kill_switch = kill_switch.clone();
                                let curve_keys = curve_keys.clone();
                                let tls_acceptor = tls_acceptor.clone();

                                // Spawn a task to handle the accepted connection.
                                tokio::spawn(Listener::handle_accepted_connection(
                                    zmq_ctx,
                                    rpc_address,
                                    events_address,
                                    handler,
                                    kill_switch,
                                    listener_port,
                                    stream,
                                    peer_addr,
                                    curve_keys,
                                    socket_fd,
                                    tls_acceptor,
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
                Some(ListenersMessage::AddListener(handler, addr, reply)) => {
                    self.start_listener(handler, addr, reply, false).await;
                }
                Some(ListenersMessage::AddTlsListener(handler, addr, reply)) => {
                    if self.tls_config.is_none() {
                        error!("TLS listener requested but no TLS config available");
                        let _ = reply.send(Err(ListenersError::AddListenerFailed(handler, addr)));
                        continue;
                    }
                    self.start_listener(handler, addr, reply, true).await;
                }
                Some(ListenersMessage::RemoveListener(addr, reply)) => {
                    let listener = self.listeners.remove(&addr);
                    info!(?addr, "Removing listener");
                    if let Some(listener) = listener {
                        listener
                            .terminate
                            .send(true)
                            .expect("Unable to send terminate message");
                        let _ = reply.send(Ok(()));
                    } else {
                        let _ = reply.send(Err(ListenersError::RemoveListenerFailed(addr)));
                    }
                }
                Some(ListenersMessage::GetListeners(tx)) => {
                    let listeners = self
                        .listeners
                        .iter()
                        .map(|(addr, listener)| ListenerInfo {
                            handler: listener.handler_object,
                            addr: *addr,
                            is_tls: listener.is_tls,
                        })
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
    pub(crate) is_tls: bool,
}

impl Listener {
    pub fn new(
        terminate: tokio::sync::watch::Sender<bool>,
        handler_object: Obj,
        is_tls: bool,
    ) -> Self {
        Self {
            handler_object,
            terminate,
            is_tls,
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
        curve_keys: Option<(String, String, String)>,
        socket_fd: RawFd,
        tls_acceptor: Option<TlsAcceptor>,
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

            let rpc_client = RpcClient::new_with_defaults(
                std::sync::Arc::new(zmq_ctx.clone()),
                rpc_address.clone(),
                curve_keys
                    .as_ref()
                    .map(|(client_secret, client_public, server_public)| {
                        rpc_async_client::rpc_client::CurveKeys {
                            client_secret: client_secret.clone(),
                            client_public: client_public.clone(),
                            server_public: server_public.clone(),
                        }
                    }),
            );

            // Initialize basic connection attributes for telnet
            let mut connection_attributes = std::collections::HashMap::new();
            connection_attributes.insert(Symbol::mk("host_type"), moor_var::Var::from("telnet"));
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
                    let reply_ref = read_reply_result(&bytes)
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

            // Subscribe to the events socket for this client's narrative events
            let events_sub = configure_curve_subscriber(subscribe(&zmq_ctx), &curve_keys)
                .connect(events_address.as_str())
                .expect("Unable to connect narrative subscriber ");
            let events_sub = events_sub
                .subscribe(&client_id.as_bytes()[..])
                .expect("Unable to subscribe to narrative messages for client connection");

            let broadcast_sub = configure_curve_subscriber(subscribe(&zmq_ctx), &curve_keys)
                .connect(events_address.as_str())
                .expect("Unable to connect broadcast subscriber ");
            let broadcast_sub = broadcast_sub
                .subscribe(CLIENT_BROADCAST_TOPIC)
                .expect("Unable to subscribe to broadcast messages for client connection");

            info!(
                "Subscribed on pubsub events socket for {:?}, socket addr {}",
                client_id, events_address
            );

            // Perform TLS handshake if this is a TLS connection, then box the stream
            let is_tls = tls_acceptor.is_some();
            let boxed_stream: BoxedAsyncIo = if let Some(acceptor) = tls_acceptor {
                match acceptor.accept(stream).await {
                    Ok(tls_stream) => Box::pin(tls_stream),
                    Err(e) => {
                        error!(?peer_addr, "TLS handshake failed: {}", e);
                        bail!("TLS handshake failed: {}", e);
                    }
                }
            } else {
                Box::pin(stream)
            };

            // Add TLS status to connection attributes
            connection_attributes.insert(Symbol::mk("tls"), moor_var::Var::mk_bool(is_tls));

            // Re-ify the connection.
            let framed_stream = Framed::new(boxed_stream, ConnectionCodec::new());
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
                pending_line_mode: None,
                collecting_input: false,
                socket_fd,
            };

            tcp_connection.run().await?;
            Ok(())
        });
        Ok(())
    }
}
