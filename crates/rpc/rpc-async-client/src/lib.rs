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

#![allow(clippy::too_many_arguments)]

use crate::rpc_client::RpcSendClient;
use rpc_common::{
    DaemonToHostReply, HostBroadcastEvent, HostToDaemonMessage, HostToken, HostType, ReplyResult,
    RpcError, HOST_BROADCAST_TOPIC, MOOR_HOST_TOKEN_FOOTER,
};
use rusty_paseto::prelude::{Footer, Key, Paseto, PasetoAsymmetricPrivateKey, Payload, Public, V4};
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;
use tmq::request;
use tracing::{error, info, warn};

use crate::pubsub_client::hosts_events_recv;
pub use listeners::{ListenersClient, ListenersError, ListenersMessage};

mod listeners;
pub mod pubsub_client;
pub mod rpc_client;

/// Construct a PASETO token for this host, to authenticate the host itself to the daemon.
pub fn make_host_token(private_key: &Key<64>, host_type: HostType) -> HostToken {
    let privkey: PasetoAsymmetricPrivateKey<V4, Public> =
        PasetoAsymmetricPrivateKey::from(private_key.as_ref());
    let token = Paseto::<V4, Public>::default()
        .set_footer(Footer::from(MOOR_HOST_TOKEN_FOOTER))
        .set_payload(Payload::from(host_type.id_str()))
        .try_sign(&privkey)
        .expect("Unable to build Paseto host token");

    HostToken(token)
}

pub async fn send_host_to_daemon_msg(
    rpc_client: &mut RpcSendClient,
    host_token: &HostToken,
    msg: HostToDaemonMessage,
) -> Result<DaemonToHostReply, RpcError> {
    match rpc_client.make_host_rpc_call(host_token, msg).await {
        Ok(ReplyResult::HostSuccess(msg)) => Ok(msg),
        Ok(ReplyResult::Failure(f)) => Err(RpcError::CouldNotSend(f.to_string())),
        Ok(m) => Err(RpcError::UnexpectedReply(format!(
            "Unexpected reply from daemon: {:?}",
            m
        ))),
        Err(e) => {
            error!("Error communicating with daemon: {}", e);
            Err(RpcError::CouldNotSend(e.to_string()))
        }
    }
}

/// Start the host session with the daemon, and return the RPC client to use for further
/// communication.
pub async fn start_host_session(
    host_token: &HostToken,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    kill_switch: Arc<AtomicBool>,
    listeners: ListenersClient,
) -> Result<RpcSendClient, RpcError> {
    // Establish the initial connection to the daemon, and send the host token and our initial
    // listener list.
    let rpc_client = loop {
        let rpc_request_sock = request(&zmq_ctx)
            .set_rcvtimeo(100)
            .set_sndtimeo(100)
            .connect(rpc_address.as_str())
            .expect("Unable to bind RPC server for connection");

        // And let the RPC server know we're here, and it should start sending events on the
        // narrative subscription.
        let mut rpc_client = RpcSendClient::new(rpc_request_sock);

        info!("Registering host with daemon...");
        let host_hello = HostToDaemonMessage::RegisterHost(
            SystemTime::now(),
            HostType::TCP,
            listeners
                .get_listeners()
                .await
                .map_err(|e| RpcError::CouldNotSend(e.to_string()))?,
        );
        match send_host_to_daemon_msg(&mut rpc_client, host_token, host_hello).await {
            Ok(DaemonToHostReply::Ack) => {
                info!("Host token accepted by daemon.");
                break rpc_client;
            }
            Ok(DaemonToHostReply::Reject(reason)) => {
                error!("Daemon has rejected this host: {}. Shutting down.", reason);
                kill_switch.store(true, std::sync::atomic::Ordering::SeqCst);
                return Err(RpcError::AuthenticationError(format!(
                    "Daemon rejected host token: {}",
                    reason
                )));
            }
            Err(e) => {
                warn!("Error communicating with daemon: {} to send host token", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
        }
    };
    Ok(rpc_client)
}

pub async fn proces_hosts_events(
    mut rpc_client: RpcSendClient,
    host_token: HostToken,
    zmq_ctx: tmq::Context,
    events_zmq_address: String,
    listen_address: String,
    kill_switch: Arc<AtomicBool>,
    listeners: ListenersClient,
    our_host_type: HostType,
) -> Result<(), RpcError> {
    // Handle inbound events from the daemon specifically to the host
    let events_sub = tmq::subscribe(&zmq_ctx)
        .connect(&events_zmq_address)
        .expect("Unable to connect host events subscriber ");
    let mut events_sub = events_sub.subscribe(HOST_BROADCAST_TOPIC).unwrap();

    loop {
        if kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
            info!("Kill switch activated, stopping...");
            return Ok(());
        }
        let msg = hosts_events_recv(&mut events_sub).await?;

        match msg {
            HostBroadcastEvent::PingPong(_) => {
                // Respond with a HostPong
                let host_pong = HostToDaemonMessage::HostPong(
                    SystemTime::now(),
                    our_host_type,
                    listeners
                        .get_listeners()
                        .await
                        .map_err(|e| RpcError::CouldNotSend(e.to_string()))?,
                );
                match send_host_to_daemon_msg(&mut rpc_client, &host_token, host_pong).await {
                    Ok(DaemonToHostReply::Ack) => {
                        // All good
                    }
                    Ok(DaemonToHostReply::Reject(reason)) => {
                        error!("Daemon has rejected this host: {}. Shutting down.", reason);
                        kill_switch.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    Err(e) => {
                        warn!(
                            "Error communicating with daemon: {} to respond to ping: {:?}",
                            e, msg
                        );
                    }
                }
            }
            HostBroadcastEvent::Listen {
                handler_object,
                host_type,
                port,
                print_messages: _,
            } => {
                if host_type == our_host_type {
                    let listen_addr = format!("{}:{}", listen_address, port);
                    let sockaddr = listen_addr.parse::<SocketAddr>().unwrap();
                    info!(
                        "Starting listener for {} on {}",
                        host_type.id_str(),
                        sockaddr
                    );
                    let listeners = listeners.clone();
                    tokio::spawn(async move {
                        let sockaddr_sockaddr = listen_addr
                            .parse::<SocketAddr>()
                            .unwrap_or_else(|_| panic!("Unable to parse address: {}", listen_addr));
                        if let Err(e) = listeners
                            .add_listener(&handler_object, sockaddr_sockaddr)
                            .await
                        {
                            error!("Error starting listener: {}", e);
                        }
                    });
                }
            }
            HostBroadcastEvent::Unlisten { host_type, port } => {
                if host_type == our_host_type {
                    // Stop listening on the given port, on `listen_address`.
                    let listen_addr = format!("{}:{}", listen_address, port);
                    let sockaddr = listen_addr.parse::<SocketAddr>().unwrap();
                    info!(
                        "Stopping listener for {} on {}",
                        host_type.id_str(),
                        sockaddr
                    );
                    listeners
                        .remove_listener(sockaddr)
                        .await
                        .expect("Unable to stop listener");
                }
            }
        }
    }
}
