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

use crate::host::{auth, ws_connection::WebSocketConnection};
use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, Path, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use eyre::eyre;
use hickory_resolver::TokioResolver;
use moor_common::model::ObjectRef;
use moor_schema::{convert::obj_from_flatbuffer_struct, rpc as moor_rpc};
use moor_var::{Obj, Symbol};
use rpc_async_client::{rpc_client::RpcClient, zmq};
use rpc_common::{
    AuthToken, CLIENT_BROADCAST_TOPIC, ClientToken, mk_attach_msg, mk_call_system_verb_msg,
    mk_connection_establish_msg, mk_detach_host_msg, mk_detach_msg, mk_eval_msg,
    mk_get_server_features_msg, mk_register_host_msg, mk_request_sys_prop_msg, mk_resolve_msg,
};
use std::{
    net::{IpAddr, SocketAddr},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tmq::{request, subscribe};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Extract the real client IP address from proxy headers or ConnectInfo
/// Checks X-Real-IP and X-Forwarded-For headers first (for nginx/proxy setups),
/// then falls back to the direct connection address
fn get_client_addr(headers: &HeaderMap, connect_addr: SocketAddr) -> SocketAddr {
    // Try X-Real-IP header first (most direct)
    if let Some(real_ip) = headers.get("X-Real-IP") {
        let Ok(ip_str) = real_ip.to_str() else {
            debug!("X-Real-IP header present but invalid UTF-8");
            return connect_addr;
        };

        let Ok(ip) = ip_str.parse::<IpAddr>() else {
            debug!("X-Real-IP header present but invalid IP: {}", ip_str);
            return connect_addr;
        };

        let client_addr = SocketAddr::new(ip, connect_addr.port());
        debug!(
            "Using X-Real-IP: {} (from proxy, connect_addr was {})",
            client_addr, connect_addr
        );
        return client_addr;
    }

    // Try X-Forwarded-For header (may contain multiple IPs, take the first)
    if let Some(forwarded) = headers.get("X-Forwarded-For") {
        let Ok(forwarded_str) = forwarded.to_str() else {
            debug!("X-Forwarded-For header present but invalid UTF-8");
            return connect_addr;
        };

        let Some(first_ip) = forwarded_str.split(',').next() else {
            debug!("X-Forwarded-For header present but empty");
            return connect_addr;
        };

        let Ok(ip) = first_ip.trim().parse::<IpAddr>() else {
            debug!(
                "X-Forwarded-For header present but invalid IP: {}",
                first_ip
            );
            return connect_addr;
        };

        let client_addr = SocketAddr::new(ip, connect_addr.port());
        debug!(
            "Using X-Forwarded-For: {} (from proxy, connect_addr was {})",
            client_addr, connect_addr
        );
        return client_addr;
    }

    // Fall back to direct connection address (no proxy)
    debug!(
        "No proxy headers found, using direct connection address: {}",
        connect_addr
    );
    connect_addr
}

/// Perform async reverse DNS lookup for an IP address with timeout
async fn resolve_hostname(ip: IpAddr) -> Result<String, eyre::Error> {
    // Create a new resolver using system configuration
    let resolver = TokioResolver::builder_tokio()?.build();

    // Perform reverse DNS lookup with 2 second timeout
    let lookup_future = resolver.reverse_lookup(ip);
    let response = timeout(Duration::from_secs(2), lookup_future)
        .await
        .map_err(|_| eyre::eyre!("DNS lookup timeout"))??;

    // Get the first hostname from the response
    if let Some(name) = response.iter().next() {
        Ok(name.to_string().trim_end_matches('.').to_string())
    } else {
        Err(eyre::eyre!("No PTR record found"))
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LoginType {
    Connect,
    Create,
}

#[derive(Clone)]
pub struct WebHost {
    zmq_context: tmq::Context,
    rpc_addr: String,
    pubsub_addr: String,
    pub(crate) handler_object: Obj,
    local_port: u16,
    curve_keys: Option<(String, String, String)>, // (client_secret, client_public, server_public) - Z85 encoded
    pub(crate) host_id: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum WsHostError {
    #[error("RPC system error: {0}")]
    RpcError(eyre::Error),
    #[error("Authentication failed")]
    AuthenticationFailed,
}

impl WebHost {
    pub fn new(
        rpc_addr: String,
        narrative_addr: String,
        handler_object: Obj,
        local_port: u16,
        curve_keys: Option<(String, String, String)>,
        host_id: Uuid,
    ) -> Self {
        let tmq_context = tmq::Context::new();
        Self {
            zmq_context: tmq_context,
            rpc_addr,
            pubsub_addr: narrative_addr,
            handler_object,
            local_port,
            curve_keys,
            host_id,
        }
    }
}

impl WebHost {
    /// Contact the RPC server to validate an auth token, and return the object ID of the player
    /// and the client token and rpc client to use for the connection.
    pub async fn attach_authenticated(
        &self,
        auth_token: AuthToken,
        connect_type: Option<moor_rpc::ConnectType>,
        peer_addr: SocketAddr,
    ) -> Result<(Obj, Uuid, ClientToken, RpcClient), WsHostError> {
        let zmq_ctx = self.zmq_context.clone();
        // Establish a connection to the RPC server
        let client_id = Uuid::new_v4();
        // Configure CURVE encryption if keys provided
        let _socket_builder = request(&zmq_ctx).set_rcvtimeo(5000).set_sndtimeo(5000);
        // Note: socket_builder is no longer used directly since we use ManagedRpcClient

        // Create managed RPC client with connection pooling and cancellation safety
        let rpc_client = RpcClient::new_with_defaults(
            std::sync::Arc::new(zmq_ctx.clone()),
            self.rpc_addr.clone(),
            self.curve_keys
                .as_ref()
                .map(|(client_secret, client_public, server_public)| {
                    rpc_async_client::rpc_client::CurveKeys {
                        client_secret: client_secret.clone(),
                        client_public: client_public.clone(),
                        server_public: server_public.clone(),
                    }
                }),
        );

        // Perform reverse DNS lookup for hostname
        debug!("Starting reverse DNS lookup for {}", peer_addr.ip());
        let hostname = match resolve_hostname(peer_addr.ip()).await {
            Ok(hostname) => {
                debug!("Resolved {} to hostname: {}", peer_addr.ip(), hostname);
                hostname
            }
            Err(e) => {
                debug!(
                    "Failed to resolve {} ({}), using IP address",
                    peer_addr.ip(),
                    e
                );
                peer_addr.to_string()
            }
        };
        debug!("DNS lookup complete, continuing with attach");

        let content_types = vec![
            moor_rpc::Symbol {
                value: "text_html".to_string(),
            },
            moor_rpc::Symbol {
                value: "text_djot".to_string(),
            },
            moor_rpc::Symbol {
                value: "text_plain".to_string(),
            },
        ];

        let attach_msg = mk_attach_msg(
            &auth_token,
            connect_type,
            &self.handler_object,
            hostname,
            self.local_port,
            peer_addr.port(),
            Some(content_types),
        );

        let reply_bytes = match rpc_client.make_client_rpc_call(client_id, attach_msg).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Unable to attach: {}", e);
                return Err(WsHostError::RpcError(eyre!(e)));
            }
        };

        use planus::ReadAsRoot;
        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| WsHostError::RpcError(eyre!("Failed to parse reply: {}", e)))?;

        let (client_token, player) = match reply.result().expect("Missing result") {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success.reply().expect("Missing reply");
                match daemon_reply.reply().expect("Missing reply union") {
                    moor_rpc::DaemonToClientReplyUnionRef::AttachResult(attach_result) => {
                        if attach_result.success().expect("Missing success") {
                            let client_token_ref = attach_result
                                .client_token()
                                .expect("Missing client_token")
                                .expect("Client token is None");
                            let client_token = ClientToken(
                                client_token_ref.token().expect("Missing token").to_string(),
                            );
                            let player_ref = attach_result
                                .player()
                                .expect("Missing player")
                                .expect("Player is None");
                            let player_struct = moor_rpc::Obj::try_from(player_ref)
                                .expect("Failed to convert player");
                            let player = obj_from_flatbuffer_struct(&player_struct)
                                .expect("Failed to decode player");
                            info!("Connection authenticated, player: {}", player);
                            (client_token, player)
                        } else {
                            warn!("Connection authentication failed from {}", peer_addr);
                            return Err(WsHostError::AuthenticationFailed);
                        }
                    }
                    _ => {
                        error!("Unexpected response from RPC server");
                        return Err(WsHostError::RpcError(eyre!(
                            "Unexpected response from RPC server"
                        )));
                    }
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(_) => {
                error!("RPC failure in attach");
                return Err(WsHostError::RpcError(eyre!("RPC failure")));
            }
            moor_rpc::ReplyResultUnionRef::HostSuccess(_) => {
                error!("Unexpected host success response");
                return Err(WsHostError::RpcError(eyre!("Unexpected host success")));
            }
        };

        Ok((player, client_id, client_token, rpc_client))
    }

    /// Actually instantiate the connection now that we've validated the auth token.
    pub async fn start_ws_connection(
        &self,
        handler_object: &Obj,
        player: &Obj,
        client_id: Uuid,
        client_token: ClientToken,
        auth_token: AuthToken,
        rpc_client: RpcClient,
        peer_addr: SocketAddr,
    ) -> Result<WebSocketConnection, eyre::Error> {
        let zmq_ctx = self.zmq_context.clone();

        // We'll need to subscribe to the narrative & broadcast messages for this connection.
        let mut narrative_socket_builder = subscribe(&zmq_ctx);

        // Configure CURVE encryption if keys provided
        if let Some((client_secret, client_public, server_public)) = &self.curve_keys {
            // Decode Z85 keys to bytes
            let client_secret_bytes = zmq::z85_decode(client_secret)
                .map_err(|_| eyre::eyre!("Invalid client secret key"))?;
            let client_public_bytes = zmq::z85_decode(client_public)
                .map_err(|_| eyre::eyre!("Invalid client public key"))?;
            let server_public_bytes = zmq::z85_decode(server_public)
                .map_err(|_| eyre::eyre!("Invalid server public key"))?;

            narrative_socket_builder = narrative_socket_builder
                .set_curve_secretkey(&client_secret_bytes)
                .set_curve_publickey(&client_public_bytes)
                .set_curve_serverkey(&server_public_bytes);
        }

        let narrative_sub = narrative_socket_builder
            .connect(self.pubsub_addr.as_str())
            .expect("Unable to connect narrative subscriber ");
        let narrative_sub = narrative_sub
            .subscribe(&client_id.as_bytes()[..])
            .expect("Unable to subscribe to narrative messages for client connection");

        let mut broadcast_socket_builder = subscribe(&zmq_ctx);

        // Configure CURVE encryption if keys provided
        if let Some((client_secret, client_public, server_public)) = &self.curve_keys {
            // Decode Z85 keys to bytes
            let client_secret_bytes = zmq::z85_decode(client_secret)
                .map_err(|_| eyre::eyre!("Invalid client secret key"))?;
            let client_public_bytes = zmq::z85_decode(client_public)
                .map_err(|_| eyre::eyre!("Invalid client public key"))?;
            let server_public_bytes = zmq::z85_decode(server_public)
                .map_err(|_| eyre::eyre!("Invalid server public key"))?;

            broadcast_socket_builder = broadcast_socket_builder
                .set_curve_secretkey(&client_secret_bytes)
                .set_curve_publickey(&client_public_bytes)
                .set_curve_serverkey(&server_public_bytes);
        }

        let broadcast_sub = broadcast_socket_builder
            .connect(self.pubsub_addr.as_str())
            .expect("Unable to connect broadcast subscriber ");
        let broadcast_sub = broadcast_sub
            .subscribe(CLIENT_BROADCAST_TOPIC)
            .expect("Unable to subscribe to broadcast messages for client connection");

        info!(
            "Subscribed on pubsub socket for {:?}, socket addr {}",
            client_id, self.pubsub_addr
        );

        Ok(WebSocketConnection {
            handler_object: *handler_object,
            player: *player,
            peer_addr,
            broadcast_sub,
            narrative_sub,
            client_id,
            client_token,
            auth_token,
            rpc_client,
            pending_task: None,
        })
    }

    pub async fn establish_client_connection(
        &self,
        addr: SocketAddr,
    ) -> Result<(Uuid, RpcClient, ClientToken), WsHostError> {
        let zmq_ctx = self.zmq_context.clone();
        // Configure CURVE encryption if keys provided
        let _socket_builder = request(&zmq_ctx).set_rcvtimeo(5000).set_sndtimeo(5000);
        // Note: socket_builder is no longer used directly since we use ManagedRpcClient

        // Create managed RPC client with connection pooling and cancellation safety
        let rpc_client = RpcClient::new_with_defaults(
            std::sync::Arc::new(zmq_ctx.clone()),
            self.rpc_addr.clone(),
            self.curve_keys
                .as_ref()
                .map(|(client_secret, client_public, server_public)| {
                    rpc_async_client::rpc_client::CurveKeys {
                        client_secret: client_secret.clone(),
                        client_public: client_public.clone(),
                        server_public: server_public.clone(),
                    }
                }),
        );

        let client_id = Uuid::new_v4();

        // Perform reverse DNS lookup for hostname
        let hostname = match resolve_hostname(addr.ip()).await {
            Ok(hostname) => {
                debug!("Resolved {} to hostname: {}", addr.ip(), hostname);
                hostname
            }
            Err(_) => {
                debug!("Failed to resolve {}, using IP address", addr.ip());
                addr.to_string()
            }
        };

        let content_types = vec![
            moor_rpc::Symbol {
                value: "text_plain".to_string(),
            },
            moor_rpc::Symbol {
                value: "text_html".to_string(),
            },
            moor_rpc::Symbol {
                value: "text_djot".to_string(),
            },
        ];

        let establish_msg = mk_connection_establish_msg(
            hostname,
            self.local_port,
            addr.port(),
            Some(content_types),
            Some(vec![]),
        );

        let reply_bytes = match rpc_client
            .make_client_rpc_call(client_id, establish_msg)
            .await
        {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Unable to establish connection: {}", e);
                return Err(WsHostError::RpcError(eyre!(e)));
            }
        };

        use planus::ReadAsRoot;
        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| WsHostError::RpcError(eyre!("Failed to parse reply: {}", e)))?;

        let client_token = match reply.result().expect("Missing result") {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success.reply().expect("Missing reply");
                match daemon_reply.reply().expect("Missing reply union") {
                    moor_rpc::DaemonToClientReplyUnionRef::NewConnection(new_conn) => {
                        let client_token_ref =
                            new_conn.client_token().expect("Missing client_token");
                        let client_token = ClientToken(
                            client_token_ref.token().expect("Missing token").to_string(),
                        );
                        let objid_ref = new_conn.connection_obj().expect("Missing connection_obj");
                        let objid_struct = moor_rpc::Obj::try_from(objid_ref)
                            .expect("Failed to convert connection_obj");
                        let objid = obj_from_flatbuffer_struct(&objid_struct)
                            .expect("Failed to decode connection_obj");
                        info!("Connection established, connection ID: {}", objid);
                        client_token
                    }
                    _ => {
                        error!("Unexpected response from RPC server");
                        return Err(WsHostError::RpcError(eyre!(
                            "Unexpected response from RPC server"
                        )));
                    }
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(_) => {
                error!("RPC failure in connection establishment");
                return Err(WsHostError::RpcError(eyre!("RPC failure")));
            }
            moor_rpc::ReplyResultUnionRef::HostSuccess(_) => {
                error!("Unexpected host success response");
                return Err(WsHostError::RpcError(eyre!("Unexpected host success")));
            }
        };

        Ok((client_id, rpc_client, client_token))
    }

    pub async fn fetch_server_features(&self) -> Result<Vec<u8>, StatusCode> {
        let zmq_ctx = self.zmq_context.clone();

        let rpc_client = RpcClient::new_with_defaults(
            std::sync::Arc::new(zmq_ctx.clone()),
            self.rpc_addr.clone(),
            self.curve_keys
                .as_ref()
                .map(|(client_secret, client_public, server_public)| {
                    rpc_async_client::rpc_client::CurveKeys {
                        client_secret: client_secret.clone(),
                        client_public: client_public.clone(),
                        server_public: server_public.clone(),
                    }
                }),
        );
        let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos() as u64,
            Err(e) => {
                error!(
                    "Invalid system time while registering temporary host: {}",
                    e
                );
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        let register_msg = mk_register_host_msg(
            self.host_id,
            timestamp,
            moor_rpc::HostType::WebSocket,
            Vec::new(),
        );

        let register_bytes = match rpc_client
            .make_host_rpc_call(self.host_id, register_msg)
            .await
        {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to register temporary host for feature fetch: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        use planus::ReadAsRoot;
        let register_reply = match moor_rpc::ReplyResultRef::read_as_root(&register_bytes) {
            Ok(reply) => reply,
            Err(e) => {
                error!("Failed to parse register reply: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        let register_success = match register_reply.result() {
            Ok(moor_rpc::ReplyResultUnionRef::HostSuccess(success)) => success,
            Ok(_) => {
                error!("Unexpected reply while registering temporary host");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Err(e) => {
                error!("Missing register result: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        let register_reply_body = match register_success.reply() {
            Ok(reply) => reply,
            Err(e) => {
                error!("Missing register reply body: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        match register_reply_body.reply() {
            Ok(moor_rpc::DaemonToHostReplyUnionRef::DaemonToHostAck(_)) => {}
            Ok(_) => {
                error!("Unexpected register reply union");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Err(e) => {
                error!("Missing register reply union: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }

        let request_msg = mk_get_server_features_msg();

        let reply_bytes = match rpc_client
            .make_host_rpc_call(self.host_id, request_msg)
            .await
        {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to fetch server features: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
            Ok(reply) => reply,
            Err(e) => {
                error!("Failed to parse server feature reply: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        let host_success = match reply.result() {
            Ok(moor_rpc::ReplyResultUnionRef::HostSuccess(host_success)) => host_success,
            Ok(_) => {
                error!("Unexpected reply union for server features");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Err(e) => {
                error!("Missing result in server features reply: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        let daemon_reply = match host_success.reply() {
            Ok(reply) => reply,
            Err(e) => {
                error!("Missing host reply for features: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        match daemon_reply.reply() {
            Ok(moor_rpc::DaemonToHostReplyUnionRef::ServerFeatures(_)) => {}
            Ok(_) => {
                error!("Unexpected host reply variant for features");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Err(e) => {
                error!("Missing reply union for features: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }

        let detach_msg = mk_detach_host_msg(self.host_id);
        if let Err(e) = rpc_client
            .make_host_rpc_call(self.host_id, detach_msg)
            .await
        {
            error!("Failed to detach temporary host after feature fetch: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ok(reply_bytes)
    }
}

pub(crate) async fn rpc_call(
    client_id: Uuid,
    rpc_client: &mut RpcClient,
    request: moor_rpc::HostClientToDaemonMessage,
) -> Result<Vec<u8>, StatusCode> {
    match rpc_client.make_client_rpc_call(client_id, request).await {
        Ok(bytes) => Ok(bytes),
        Err(e) => {
            error!("RPC failure: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// FlatBuffer version: Returns raw FlatBuffer bytes instead of JSON
pub async fn features_handler(State(host): State<WebHost>) -> Response {
    match host.fetch_server_features().await {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/x-flatbuffer")
            .body(Body::from(bytes))
            .unwrap_or_else(|e| {
                error!("Failed to build features response: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }),
        Err(status) => status.into_response(),
    }
}

pub async fn system_property_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(path): Path<String>,
) -> Response {
    let (client_id, mut rpc_client, client_token) =
        match host.establish_client_connection(addr).await {
            Ok((client_id, rpc_client, client_token)) => (client_id, rpc_client, client_token),
            Err(WsHostError::AuthenticationFailed) => return StatusCode::FORBIDDEN.into_response(),
            Err(e) => {
                error!("Unable to establish connection: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

    // Parse the path into object reference and property name
    let path_parts: Vec<&str> = path.split('/').collect();
    let (obj_path, property_name) = if path_parts.len() < 2 {
        return StatusCode::BAD_REQUEST.into_response();
    } else {
        // Multiple parts: last is property, rest is object path
        let obj_parts = &path_parts[..path_parts.len() - 1];
        let prop = path_parts[path_parts.len() - 1];
        (obj_parts.iter().map(|&s| Symbol::mk(s)).collect(), prop)
    };

    let sysprop_msg = mk_request_sys_prop_msg(
        &client_token,
        &ObjectRef::SysObj(obj_path),
        &Symbol::mk(property_name),
    );

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, sysprop_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    // Just return the raw FlatBuffer bytes!
    // No parsing, no JSON conversion - the client will handle it
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap();

    // We're done with this RPC connection, so we detach it.
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client
        .make_client_rpc_call(client_id, detach_msg)
        .await
        .expect("Unable to send detach to RPC server");

    response
}

/// Attach a websocket connection to an existing player.
async fn attach(
    ws: WebSocketUpgrade,
    addr: SocketAddr,
    connect_type: moor_rpc::ConnectType,
    host: &WebHost,
    auth_token: String,
) -> impl IntoResponse + use<> {
    debug!("Connection from {}", addr);

    let auth_token = AuthToken(auth_token);

    let (player, client_id, client_token, rpc_client) = match host
        .attach_authenticated(auth_token.clone(), Some(connect_type), addr)
        .await
    {
        Ok(connection_details) => connection_details,
        Err(WsHostError::AuthenticationFailed) => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap();
        }
        Err(e) => {
            error!("Unable to validate auth token: {}", e);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap();
        }
    };

    let Ok(mut connection) = host
        .start_ws_connection(
            &host.handler_object,
            &player,
            client_id,
            client_token,
            auth_token,
            rpc_client,
            addr,
        )
        .await
    else {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::empty())
            .unwrap();
    };

    ws.on_upgrade(move |socket| async move { connection.handle(connect_type, socket).await })
}

/// Websocket upgrade handler for authenticated users who are connecting to an existing user
pub async fn ws_connect_attach_handler(
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    Path(token): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse + use<> {
    debug!(
        "ws_connect_attach_handler called, ConnectInfo addr: {}",
        addr
    );
    let client_addr = get_client_addr(&headers, addr);
    info!("WebSocket connection from {}", client_addr);

    attach(
        ws,
        client_addr,
        moor_rpc::ConnectType::Connected,
        &ws_host,
        token,
    )
    .await
}

/// Websocket upgrade handler for authenticated users who are connecting to a new user
pub async fn ws_create_attach_handler(
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    Path(token): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse + use<> {
    debug!(
        "ws_create_attach_handler called, ConnectInfo addr: {}",
        addr
    );
    let client_addr = get_client_addr(&headers, addr);
    info!("WebSocket connection from {}", client_addr);

    attach(
        ws,
        client_addr,
        moor_rpc::ConnectType::Created,
        &ws_host,
        token,
    )
    .await
}

/// FlatBuffer version: GET /fb/objects/{object} - resolve object reference
pub async fn resolve_objref_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path(object): Path<String>,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    let objref = match ObjectRef::parse_curie(&object) {
        None => {
            return StatusCode::BAD_REQUEST.into_response();
        }
        Some(oref) => oref,
    };

    let resolve_msg = mk_resolve_msg(&client_token, &auth_token, &objref);

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, resolve_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap();

    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client
        .make_client_rpc_call(client_id, detach_msg)
        .await
        .expect("Unable to send detach to RPC server");

    response
}

/// FlatBuffer version: POST /fb/eval - evaluate expression
pub async fn eval_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    expression: Bytes,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };
    let expression = String::from_utf8_lossy(&expression).to_string();

    let eval_msg = mk_eval_msg(&client_token, &auth_token, expression);

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, eval_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap();

    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client
        .make_client_rpc_call(client_id, detach_msg)
        .await
        .expect("Unable to send detach to RPC server");

    response
}

/// FlatBuffer version: GET /fb/invoke_welcome_message - invoke #0:do_login_command
pub async fn invoke_welcome_message_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    let (client_id, mut rpc_client, client_token) =
        match host.establish_client_connection(addr).await {
            Ok((client_id, rpc_client, client_token)) => (client_id, rpc_client, client_token),
            Err(WsHostError::AuthenticationFailed) => return StatusCode::FORBIDDEN.into_response(),
            Err(e) => {
                error!("Unable to establish connection: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

    // Create the system verb call for #0:do_login_command
    let verb = Symbol::mk("do_login_command");
    let args: Vec<&moor_var::Var> = vec![]; // No arguments for do_login_command

    let call_system_verb_msg = match mk_call_system_verb_msg(&client_token, &verb, args) {
        Some(msg) => msg,
        None => {
            error!("Failed to create CallSystemVerb message");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, call_system_verb_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap();

    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client
        .make_client_rpc_call(client_id, detach_msg)
        .await
        .expect("Unable to send detach to RPC server");

    response
}
