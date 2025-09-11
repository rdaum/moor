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
use eyre::bail;
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use hickory_resolver::TokioResolver;
use moor_var::{Obj, Symbol};
use nectar::{TelnetCodec, option::TelnetOption, subnegotiation::SubnegotiationType};
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

            // Set up the telnet connection stream first
            let framed_stream = Framed::new(stream, TelnetCodec::new(1024));
            let (mut write, mut read) = framed_stream.split();

            // Initialize basic connection attributes for telnet
            let mut connection_attributes = std::collections::HashMap::new();
            connection_attributes.insert(Symbol::mk("host-type"), moor_var::Var::from("telnet"));
            connection_attributes.insert(
                Symbol::mk("supports-telnet-protocol"),
                moor_var::Var::mk_bool(true),
            );

            // Perform telnet capability negotiation first
            debug!("Starting telnet capability negotiation");
            if let Err(e) = negotiate_telnet_capabilities(&mut write).await {
                warn!("Failed to negotiate telnet capabilities: {}", e);
            }

            // Collect negotiation results with timeout
            if let Ok(negotiated_attrs) =
                collect_negotiation_results(&mut read, &connection_attributes).await
            {
                // Check if client supports charset and send UTF-8 request
                if negotiated_attrs.contains_key(&Symbol::mk("supports-charset")) {
                    debug!("Client supports charset, sending UTF-8 request");
                    if let Err(e) = send_utf8_charset_request(&mut write).await {
                        debug!("Failed to send UTF-8 charset request: {}", e);
                    } else {
                        debug!("UTF-8 charset request sent");

                        // Collect UTF-8 response with short timeout
                        if let Ok(charset_attrs) = collect_charset_response(&mut read).await {
                            connection_attributes.extend(charset_attrs);
                        }
                    }
                }

                connection_attributes.extend(negotiated_attrs);
            }

            debug!(
                "Telnet negotiation complete, establishing connection with {} attributes",
                connection_attributes.len()
            );

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

            // Use the already-split stream from negotiation
            let mut tcp_connection = TelnetConnection {
                handler_object,
                peer_addr,
                connection_oid,
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
            };

            tcp_connection.run().await?;
            Ok(())
        });
        Ok(())
    }
}

/// Send telnet negotiation requests before connection establishment
async fn negotiate_telnet_capabilities(
    write: &mut SplitSink<Framed<TcpStream, TelnetCodec>, nectar::event::TelnetEvent>,
) -> Result<(), eyre::Error> {
    use nectar::{event::TelnetEvent, option::TelnetOption};

    // Request NAWS (window size)
    let naws_request = TelnetEvent::Do(TelnetOption::NAWS);
    write.send(naws_request).await?;

    // Request terminal type
    let term_type_request = TelnetEvent::Do(TelnetOption::Unknown(24)); // Terminal Type
    write.send(term_type_request).await?;

    // Request GMCP for modern MUD clients
    let gmcp_request = TelnetEvent::Do(TelnetOption::GMCP);
    write.send(gmcp_request).await?;

    // Request charset negotiation
    let charset_request = TelnetEvent::Do(TelnetOption::Charset);
    write.send(charset_request).await?;

    Ok(())
}

/// Collect telnet negotiation results with timeout
async fn collect_negotiation_results(
    read: &mut SplitStream<Framed<TcpStream, TelnetCodec>>,
    _base_attributes: &std::collections::HashMap<Symbol, moor_var::Var>,
) -> Result<std::collections::HashMap<Symbol, moor_var::Var>, eyre::Error> {
    use futures_util::StreamExt;
    use nectar::event::TelnetEvent;
    use std::time::Duration;

    let mut negotiated_attrs = std::collections::HashMap::new();
    let timeout_duration = Duration::from_millis(500); // Short timeout for negotiation

    // Collect negotiation responses for a short time
    let deadline = tokio::time::Instant::now() + timeout_duration;

    while tokio::time::Instant::now() < deadline {
        let timeout_result = tokio::time::timeout(Duration::from_millis(50), read.next()).await;

        let Some(event_result) = timeout_result.ok().flatten() else {
            continue;
        };

        let event = match event_result {
            Ok(event) => event,
            Err(_) => continue,
        };

        match event {
            TelnetEvent::Will(option) => {
                handle_will_option(&mut negotiated_attrs, option);
            }
            TelnetEvent::Subnegotiate(subneg_type) => {
                handle_subnegotiation(&mut negotiated_attrs, subneg_type);
            }
            TelnetEvent::Message(_) => {
                // Client sent actual text data, negotiation phase is likely over
                break;
            }
            _ => {}
        }
    }

    debug!(
        "Negotiation complete, collected {} attributes",
        negotiated_attrs.len()
    );
    Ok(negotiated_attrs)
}

fn handle_will_option(
    negotiated_attrs: &mut std::collections::HashMap<Symbol, moor_var::Var>,
    option: TelnetOption,
) {
    match option {
        TelnetOption::NAWS => {
            negotiated_attrs.insert(Symbol::mk("supports-naws"), moor_var::Var::mk_bool(true));
            debug!("Client supports NAWS");
        }
        TelnetOption::Unknown(24) => {
            // Terminal Type
            negotiated_attrs.insert(
                Symbol::mk("supports-terminal-type"),
                moor_var::Var::mk_bool(true),
            );
            debug!("Client supports terminal type");
        }
        TelnetOption::GMCP => {
            negotiated_attrs.insert(Symbol::mk("supports-gmcp"), moor_var::Var::mk_bool(true));
            debug!("Client supports GMCP");
        }
        TelnetOption::Charset => {
            negotiated_attrs.insert(Symbol::mk("supports-charset"), moor_var::Var::mk_bool(true));
            debug!("Client supports charset negotiation");
        }
        _ => debug!("Client will: {:?}", option),
    }
}

fn handle_subnegotiation(
    negotiated_attrs: &mut std::collections::HashMap<Symbol, moor_var::Var>,
    subneg_type: SubnegotiationType,
) {
    match subneg_type {
        SubnegotiationType::WindowSize(width, height) => {
            negotiated_attrs.insert(
                Symbol::mk("terminal-width"),
                moor_var::Var::from(width as i64),
            );
            negotiated_attrs.insert(
                Symbol::mk("terminal-height"),
                moor_var::Var::from(height as i64),
            );
            debug!("NAWS: terminal size {}x{}", width, height);
        }
        SubnegotiationType::Unknown(option, data) => {
            handle_unknown_subnegotiation(negotiated_attrs, option, &data);
        }
        _ => debug!("Unhandled subnegotiation: {:?}", subneg_type),
    }
}

fn handle_unknown_subnegotiation(
    negotiated_attrs: &mut std::collections::HashMap<Symbol, moor_var::Var>,
    option: TelnetOption,
    data: &[u8],
) {
    match option {
        TelnetOption::Unknown(24) => {
            // Terminal Type
            handle_terminal_type_subneg(negotiated_attrs, data);
        }
        opt if format!("{opt:?}").contains("GMCP") => {
            handle_gmcp_subneg(negotiated_attrs, data);
        }
        _ => debug!("Unknown subnegotiation option: {:?}", option),
    }
}

fn handle_terminal_type_subneg(
    negotiated_attrs: &mut std::collections::HashMap<Symbol, moor_var::Var>,
    data: &[u8],
) {
    if data.is_empty() || data[0] != 0 {
        return;
    }

    let Ok(terminal_type) = String::from_utf8(data[1..].to_vec()) else {
        return;
    };

    negotiated_attrs.insert(
        Symbol::mk("terminal-type"),
        moor_var::Var::from(terminal_type),
    );
    debug!("Terminal type received");
}

fn handle_gmcp_subneg(
    negotiated_attrs: &mut std::collections::HashMap<Symbol, moor_var::Var>,
    data: &[u8],
) {
    let Ok(gmcp_msg) = String::from_utf8(data.to_vec()) else {
        return;
    };

    let Some(space_pos) = gmcp_msg.find(' ') else {
        return;
    };

    let package_msg = &gmcp_msg[..space_pos];
    let json_data = &gmcp_msg[space_pos + 1..];

    if package_msg != "Core.Hello" {
        return;
    }

    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_data) else {
        return;
    };

    let Some(obj) = parsed.as_object() else {
        return;
    };

    let pairs: Vec<moor_var::Var> = obj
        .iter()
        .map(|(key, value)| {
            let moo_key = moor_var::Var::from(key.clone());
            let moo_value = match value {
                serde_json::Value::String(s) => moor_var::Var::from(s.clone()),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        moor_var::Var::from(i)
                    } else {
                        moor_var::Var::from(n.to_string())
                    }
                }
                serde_json::Value::Bool(b) => moor_var::Var::from(if *b { 1i64 } else { 0i64 }),
                _ => moor_var::Var::from(value.to_string()),
            };
            moor_var::v_list(&[moo_key, moo_value])
        })
        .collect();

    negotiated_attrs.insert(Symbol::mk("gmcp-client"), moor_var::v_list(&pairs));
    debug!("GMCP client info received");
}

/// Send UTF-8 charset request to client
async fn send_utf8_charset_request(
    write: &mut SplitSink<Framed<TcpStream, TelnetCodec>, nectar::event::TelnetEvent>,
) -> Result<(), eyre::Error> {
    use nectar::{event::TelnetEvent, subnegotiation::SubnegotiationType};

    // Send UTF-8 charset request
    let utf8_request = b"REQUEST UTF-8".to_vec();
    let charset_request = TelnetEvent::Subnegotiate(SubnegotiationType::CharsetRequest(vec![
        utf8_request.into(),
    ]));

    write.send(charset_request).await?;
    Ok(())
}

/// Collect charset response from client with timeout
async fn collect_charset_response(
    read: &mut SplitStream<Framed<TcpStream, TelnetCodec>>,
) -> Result<std::collections::HashMap<Symbol, moor_var::Var>, eyre::Error> {
    use futures_util::StreamExt;
    use nectar::{event::TelnetEvent, subnegotiation::SubnegotiationType};
    use std::time::Duration;

    let mut charset_attrs = std::collections::HashMap::new();
    let timeout_duration = Duration::from_millis(500); // Short timeout for charset response

    let deadline = tokio::time::Instant::now() + timeout_duration;

    while tokio::time::Instant::now() < deadline {
        let timeout_result = tokio::time::timeout(Duration::from_millis(50), read.next()).await;

        let Some(event_result) = timeout_result.ok().flatten() else {
            continue;
        };

        let event = match event_result {
            Ok(event) => event,
            Err(_) => continue,
        };

        match event {
            TelnetEvent::Subnegotiate(subneg_type) => {
                match subneg_type {
                    SubnegotiationType::CharsetAccepted(charset) => {
                        debug!("Client accepted charset: {:?}", charset);
                        if let Ok(charset_str) = String::from_utf8(charset.to_vec()) {
                            charset_attrs.insert(
                                Symbol::mk("charset"),
                                moor_var::Var::from(charset_str.clone()),
                            );
                            debug!("Client accepted charset: {}", charset_str);
                        }
                        break; // Got response, we're done
                    }
                    SubnegotiationType::CharsetRejected => {
                        debug!("Client rejected charset request");
                        charset_attrs
                            .insert(Symbol::mk("charset-rejected"), moor_var::Var::mk_bool(true));
                        break; // Got response, we're done
                    }
                    _ => {
                        // Other subnegotiation, continue waiting
                    }
                }
            }
            TelnetEvent::Message(_) => {
                // Client sent text, charset negotiation is probably done
                break;
            }
            _ => {
                // Other events, continue waiting
            }
        }
    }

    debug!(
        "Charset response collection complete, collected {} attributes",
        charset_attrs.len()
    );
    Ok(charset_attrs)
}
