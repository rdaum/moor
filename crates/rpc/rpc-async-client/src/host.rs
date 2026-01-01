// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::{ListenersClient, pubsub_client::hosts_events_recv, rpc_client::RpcClient};
use moor_schema::{convert::obj_from_ref, rpc as moor_rpc, var as moor_var_fb};
use rpc_common::{
    HOST_BROADCAST_TOPIC, HostType, RpcError, mk_host_pong_msg, mk_register_host_msg, obj_fb,
    read_reply_result,
};
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::SystemTime,
};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Extract a boolean option from a HostBroadcastListen options map.
/// Returns true if the option exists and is truthy (non-zero int or true bool).
fn extract_bool_option(listen: &moor_rpc::HostBroadcastListenRef<'_>, key_name: &str) -> bool {
    let Ok(Some(options)) = listen.options() else {
        return false;
    };

    for pair_result in options.iter() {
        let Ok(pair) = pair_result else {
            continue;
        };
        let Ok(key) = pair.key() else {
            continue;
        };
        let Ok(value) = pair.value() else {
            continue;
        };

        // Check if key is a symbol matching our key_name
        let Ok(key_variant) = key.variant() else {
            continue;
        };
        let moor_var_fb::VarUnionRef::VarSym(var_sym) = key_variant else {
            continue;
        };
        let Ok(sym) = var_sym.symbol() else {
            continue;
        };
        let Ok(sym_val) = sym.value() else {
            continue;
        };
        if sym_val != key_name {
            continue;
        }

        // Check if value is truthy
        let Ok(value_variant) = value.variant() else {
            return false;
        };
        return match value_variant {
            moor_var_fb::VarUnionRef::VarInt(i) => i.value().unwrap_or(0) != 0,
            moor_var_fb::VarUnionRef::VarBool(b) => b.value().unwrap_or(false),
            _ => false,
        };
    }

    false
}

/// Start the host session with the daemon, and return the RPC client and host_id to use for further
/// communication.
pub async fn start_host_session(
    host_id: Uuid,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    kill_switch: Arc<AtomicBool>,
    listeners: ListenersClient,
    curve_keys: Option<(String, String, String)>, // (client_secret, client_public, server_public) - Z85 encoded
) -> Result<(RpcClient, Uuid), RpcError> {
    // Establish the initial connection to the daemon, and send the host_id and our initial
    // listener list.
    let rpc_client = loop {
        // Check if shutdown was requested before attempting connection
        if kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
            info!("Host shutdown requested during connection attempt");
            return Err(RpcError::CouldNotInitiateSession(
                "Host shutdown requested during connection".to_string(),
            ));
        }

        // Create managed RPC client with connection pooling and cancellation safety
        let rpc_client = RpcClient::new_with_defaults(
            std::sync::Arc::new(zmq_ctx.clone()),
            rpc_address.clone(),
            curve_keys
                .as_ref()
                .map(|(client_secret, client_public, server_public)| {
                    crate::rpc_client::CurveKeys {
                        client_secret: client_secret.clone(),
                        client_public: client_public.clone(),
                        server_public: server_public.clone(),
                    }
                }),
        );

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
            .map(|info| moor_rpc::Listener {
                handler_object: obj_fb(&info.handler),
                socket_addr: info.addr.to_string(),
            })
            .collect();

        let host_hello = mk_register_host_msg(host_id, timestamp, host_type_fb, listeners_fb);
        let reply_bytes = rpc_client.make_host_rpc_call(host_id, host_hello).await;
        match reply_bytes {
            Ok(bytes) => {
                let reply_ref = read_reply_result(&bytes)
                    .map_err(|e| RpcError::CouldNotDecode(format!("Invalid flatbuffer: {e}")))?;

                match reply_ref
                    .result()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Missing result: {e}")))?
                {
                    moor_rpc::ReplyResultUnionRef::HostSuccess(host_success) => {
                        let daemon_reply = host_success
                            .reply()
                            .map_err(|e| RpcError::CouldNotDecode(format!("Missing reply: {e}")))?;
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
                        let error_ref = failure
                            .error()
                            .map_err(|e| RpcError::CouldNotDecode(format!("Missing error: {e}")))?;
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
    Ok((rpc_client, host_id))
}

pub async fn process_hosts_events(
    rpc_client: RpcClient,
    host_id: Uuid,
    zmq_ctx: tmq::Context,
    events_zmq_address: String,
    listen_address: String,
    kill_switch: Arc<AtomicBool>,
    listeners: ListenersClient,
    our_host_type: HostType,
    curve_keys: Option<(String, String, String)>, // (client_secret, client_public, server_public) - Z85 encoded
    last_daemon_ping: Option<Arc<AtomicU64>>,
) -> Result<(), RpcError> {
    // Handle inbound events from the daemon specifically to the host
    let mut socket_builder = tmq::subscribe(&zmq_ctx);

    // Configure CURVE encryption if keys provided
    if let Some((client_secret, client_public, server_public)) = &curve_keys {
        // Decode Z85 keys to bytes
        let client_secret_bytes = zmq::z85_decode(client_secret).map_err(|_| {
            RpcError::CouldNotInitiateSession("Invalid client secret key".to_string())
        })?;
        let client_public_bytes = zmq::z85_decode(client_public).map_err(|_| {
            RpcError::CouldNotInitiateSession("Invalid client public key".to_string())
        })?;
        let server_public_bytes = zmq::z85_decode(server_public).map_err(|_| {
            RpcError::CouldNotInitiateSession("Invalid server public key".to_string())
        })?;

        socket_builder = socket_builder
            .set_curve_secretkey(&client_secret_bytes)
            .set_curve_publickey(&client_public_bytes)
            .set_curve_serverkey(&server_public_bytes);

        info!("CURVE encryption enabled for host events connection");
    }

    let events_sub = socket_builder
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

        match event
            .event()
            .map_err(|e| RpcError::CouldNotDecode(format!("Missing event: {e}")))?
        {
            moor_rpc::HostBroadcastEventUnionRef::HostBroadcastPingPong(_) => {
                // Update last ping timestamp for health checks
                let timestamp = SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| RpcError::CouldNotSend(format!("Invalid timestamp: {e}")))?
                    .as_secs();
                if let Some(ref ping_atomic) = last_daemon_ping {
                    ping_atomic.store(timestamp, Ordering::Relaxed);
                }

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
                    .map(|info| moor_rpc::Listener {
                        handler_object: obj_fb(&info.handler),
                        socket_addr: info.addr.to_string(),
                    })
                    .collect();

                let host_pong = mk_host_pong_msg(host_id, timestamp, host_type_fb, listeners_fb);
                let reply_bytes = rpc_client.make_host_rpc_call(host_id, host_pong).await;
                match reply_bytes {
                    Ok(bytes) => {
                        let reply_ref = read_reply_result(&bytes).map_err(|e| {
                            RpcError::CouldNotDecode(format!("Invalid flatbuffer: {e}"))
                        })?;

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
                let handler_object = obj_from_ref(handler_object_ref).map_err(|e| {
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

                let use_tls = extract_bool_option(&listen, "tls");

                if host_type != our_host_type {
                    continue;
                }

                let listen_addr = format!("{listen_address}:{port}");
                let sockaddr = listen_addr.parse::<SocketAddr>().unwrap();
                let tls_label = if use_tls { " (TLS)" } else { "" };
                info!(
                    "Starting listener for {} on {}{}",
                    host_type.id_str(),
                    sockaddr,
                    tls_label
                );
                let listeners = listeners.clone();
                tokio::spawn(async move {
                    let sockaddr = listen_addr
                        .parse::<SocketAddr>()
                        .unwrap_or_else(|_| panic!("Unable to parse address: {listen_addr}"));
                    let result = if use_tls {
                        listeners.add_tls_listener(&handler_object, sockaddr).await
                    } else {
                        listeners.add_listener(&handler_object, sockaddr).await
                    };
                    if let Err(e) = result {
                        error!("Error starting listener: {}", e);
                    }
                });
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
