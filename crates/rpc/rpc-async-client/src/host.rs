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

use crate::{ListenersClient, pubsub_client::hosts_events_recv, rpc_client::RpcSendClient};
use moor_common::schema::rpc as moor_rpc;
use rpc_common::{HOST_BROADCAST_TOPIC, HostToken, HostType, RpcError};
use std::{
    net::SocketAddr,
    sync::{Arc, atomic::AtomicBool},
    time::SystemTime,
};
use tmq::request;
use tracing::{error, info, warn};

pub async fn send_host_to_daemon_msg(
    rpc_client: &mut RpcSendClient,
    host_token: &HostToken,
    msg: moor_rpc::HostToDaemonMessage,
) -> Result<Vec<u8>, RpcError> {
    rpc_client.make_host_rpc_call(host_token, msg).await
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

        info!("Registering host with daemon via {}...", rpc_address);
        let timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| RpcError::CouldNotSend(format!("Invalid timestamp: {e}")))?
            .as_nanos() as u64;

        let host_type_fb = moor_rpc::HostType::Tcp;

        let listener_list = listeners
            .get_listeners()
            .await
            .map_err(|e| RpcError::CouldNotSend(e.to_string()))?;

        let listeners_fb: Vec<moor_rpc::Listener> = listener_list
            .iter()
            .map(|(obj, addr)| moor_rpc::Listener {
                handler_object: Box::new(rpc_common::obj_to_flatbuffer_struct(obj)),
                socket_addr: addr.to_string(),
            })
            .collect();

        let host_hello = moor_rpc::HostToDaemonMessage {
            message: moor_rpc::HostToDaemonMessageUnion::RegisterHost(Box::new(
                moor_rpc::RegisterHost {
                    timestamp,
                    host_type: host_type_fb,
                    listeners: listeners_fb,
                },
            )),
        };
        let reply_bytes = send_host_to_daemon_msg(&mut rpc_client, host_token, host_hello).await;
        match reply_bytes {
            Ok(bytes) => {
                use planus::ReadAsRoot;
                let reply_ref = moor_rpc::ReplyResultRef::read_as_root(&bytes)
                    .map_err(|e| RpcError::CouldNotDecode(format!("Invalid flatbuffer: {e}")))?;

                match reply_ref
                    .result()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Missing result: {e}")))?
                {
                    moor_rpc::ReplyResultUnionRef::HostSuccess(host_success) => {
                        let daemon_reply = host_success.reply().map_err(|e| {
                            RpcError::CouldNotDecode(format!("Missing reply: {e}"))
                        })?;
                        match daemon_reply.reply().map_err(|e| {
                            RpcError::CouldNotDecode(format!("Missing reply union: {e}"))
                        })? {
                            moor_rpc::DaemonToHostReplyUnionRef::DaemonToHostAck(_) => {
                                info!("Host token accepted by daemon.");
                                break rpc_client;
                            }
                            moor_rpc::DaemonToHostReplyUnionRef::DaemonToHostReject(reject) => {
                                let reason = reject
                                    .reason()
                                    .map_err(|e| {
                                        RpcError::CouldNotDecode(format!("Missing reason: {e}"))
                                    })?
                                    .to_string();
                                error!("Daemon has rejected this host: {}. Shutting down.", reason);
                                kill_switch.store(true, std::sync::atomic::Ordering::SeqCst);
                                return Err(RpcError::AuthenticationError(format!(
                                    "Daemon rejected host token: {reason}"
                                )));
                            }
                            _ => {
                                return Err(RpcError::UnexpectedReply(
                                    "Expected Ack or Reject".to_string(),
                                ));
                            }
                        }
                    }
                    moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                        let error_ref = failure.error().map_err(|e| {
                            RpcError::CouldNotDecode(format!("Missing error: {e}"))
                        })?;
                        let error_code = error_ref.error_code().map_err(|e| {
                            RpcError::CouldNotDecode(format!("Missing error code: {e}"))
                        })?;
                        return Err(RpcError::CouldNotSend(format!(
                            "Daemon error: {error_code:?}"
                        )));
                    }
                    _ => {
                        return Err(RpcError::UnexpectedReply(
                            "Expected HostSuccess or Failure".to_string(),
                        ));
                    }
                }
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

pub async fn process_hosts_events(
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
        let event = msg.event()?;

        use planus::ReadAsRoot;
        match event
            .event()
            .map_err(|e| RpcError::CouldNotDecode(format!("Missing event: {e}")))?
        {
            moor_rpc::HostBroadcastEventUnionRef::HostBroadcastPingPong(_) => {
                // Respond with a HostPong
                let timestamp = SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| RpcError::CouldNotSend(format!("Invalid timestamp: {e}")))?
                    .as_nanos() as u64;

                let host_type_fb = match our_host_type {
                    HostType::TCP => moor_rpc::HostType::Tcp,
                    HostType::WebSocket => moor_rpc::HostType::WebSocket,
                };

                let listener_list = listeners
                    .get_listeners()
                    .await
                    .map_err(|e| RpcError::CouldNotSend(e.to_string()))?;

                let listeners_fb: Vec<moor_rpc::Listener> = listener_list
                    .iter()
                    .map(|(obj, addr)| moor_rpc::Listener {
                        handler_object: Box::new(rpc_common::obj_to_flatbuffer_struct(obj)),
                        socket_addr: addr.to_string(),
                    })
                    .collect();

                let host_pong = moor_rpc::HostToDaemonMessage {
                    message: moor_rpc::HostToDaemonMessageUnion::HostPong(Box::new(
                        moor_rpc::HostPong {
                            timestamp,
                            host_type: host_type_fb,
                            listeners: listeners_fb,
                        },
                    )),
                };
                let reply_bytes =
                    send_host_to_daemon_msg(&mut rpc_client, &host_token, host_pong).await;
                match reply_bytes {
                    Ok(bytes) => {
                        let reply_ref =
                            moor_rpc::ReplyResultRef::read_as_root(&bytes).map_err(|e| {
                                RpcError::CouldNotDecode(format!("Invalid flatbuffer: {e}"))
                            })?;

                        match reply_ref.result().map_err(|e| {
                            RpcError::CouldNotDecode(format!("Missing result: {e}"))
                        })? {
                            moor_rpc::ReplyResultUnionRef::HostSuccess(host_success) => {
                                let daemon_reply = host_success.reply().map_err(|e| {
                                    RpcError::CouldNotDecode(format!("Missing reply: {e}"))
                                })?;
                                match daemon_reply.reply().map_err(|e| {
                                    RpcError::CouldNotDecode(format!("Missing reply union: {e}"))
                                })? {
                                    moor_rpc::DaemonToHostReplyUnionRef::DaemonToHostAck(_) => {
                                        // All good
                                    }
                                    moor_rpc::DaemonToHostReplyUnionRef::DaemonToHostReject(
                                        reject,
                                    ) => {
                                        let reason = reject
                                            .reason()
                                            .map_err(|e| {
                                                RpcError::CouldNotDecode(format!(
                                                    "Missing reason: {e}"
                                                ))
                                            })?
                                            .to_string();
                                        error!(
                                            "Daemon has rejected this host: {}. Shutting down.",
                                            reason
                                        );
                                        kill_switch
                                            .store(true, std::sync::atomic::Ordering::SeqCst);
                                    }
                                    _ => {
                                        return Err(RpcError::UnexpectedReply(
                                            "Expected Ack or Reject".to_string(),
                                        ));
                                    }
                                }
                            }
                            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                                let error_ref = failure.error().map_err(|e| {
                                    RpcError::CouldNotDecode(format!("Missing error: {e}"))
                                })?;
                                let error_code = error_ref.error_code().map_err(|e| {
                                    RpcError::CouldNotDecode(format!("Missing error code: {e}"))
                                })?;
                                warn!("Daemon error responding to ping: {:?}", error_code);
                            }
                            _ => {
                                return Err(RpcError::UnexpectedReply(
                                    "Expected HostSuccess or Failure".to_string(),
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Error communicating with daemon to respond to ping: {:?}",
                            e
                        );
                    }
                }
            }
            moor_rpc::HostBroadcastEventUnionRef::HostBroadcastListen(listen) => {
                let handler_object_ref = listen.handler_object().map_err(|e| {
                    RpcError::CouldNotDecode(format!("Missing handler_object: {e}"))
                })?;
                let handler_object_struct =
                    moor_rpc::Obj::try_from(handler_object_ref).map_err(|e| {
                        RpcError::CouldNotDecode(format!("Failed to convert handler_object: {e}"))
                    })?;
                let handler_object = rpc_common::obj_from_flatbuffer_struct(&handler_object_struct)
                    .map_err(|e| {
                        RpcError::CouldNotDecode(format!("Failed to decode handler_object: {e}"))
                    })?;

                let host_type_fb = listen
                    .host_type()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Missing host_type: {e}")))?;
                let host_type = match host_type_fb {
                    moor_rpc::HostType::Tcp => HostType::TCP,
                    moor_rpc::HostType::WebSocket => HostType::WebSocket,
                };

                let port = listen
                    .port()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Missing port: {e}")))?;

                if host_type == our_host_type {
                    let listen_addr = format!("{listen_address}:{port}");
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
                            .unwrap_or_else(|_| panic!("Unable to parse address: {listen_addr}"));
                        if let Err(e) = listeners
                            .add_listener(&handler_object, sockaddr_sockaddr)
                            .await
                        {
                            error!("Error starting listener: {}", e);
                        }
                    });
                }
            }
            moor_rpc::HostBroadcastEventUnionRef::HostBroadcastUnlisten(unlisten) => {
                let host_type_fb = unlisten
                    .host_type()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Missing host_type: {e}")))?;
                let host_type = match host_type_fb {
                    moor_rpc::HostType::Tcp => HostType::TCP,
                    moor_rpc::HostType::WebSocket => HostType::WebSocket,
                };

                let port = unlisten
                    .port()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Missing port: {e}")))?;

                if host_type == our_host_type {
                    // Stop listening on the given port, on `listen_address`.
                    let listen_addr = format!("{listen_address}:{port}");
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
