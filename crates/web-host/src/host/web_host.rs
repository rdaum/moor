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

#![allow(clippy::too_many_arguments)]

use crate::host::{
    auth, flatbuffer_response,
    negotiate::{
        BOTH_FORMATS, ResponseFormat, TEXT_PLAIN_CONTENT_TYPE, negotiate_response_format,
        reply_result_to_json, require_content_type,
    },
    ws_connection::WebSocketConnection,
};
use axum::{
    Json,
    body::Bytes,
    extract::{ConnectInfo, Path, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use eyre::eyre;
use hickory_resolver::TokioResolver;
use ipnet::IpNet;
use moor_common::model::ObjectRef;
use moor_schema::{convert::obj_from_ref, rpc as moor_rpc};
use moor_var::{Obj, Symbol};
use rpc_async_client::{rpc_client::RpcClient, zmq};
use rpc_common::{
    AuthToken, CLIENT_BROADCAST_TOPIC, ClientToken, mk_attach_msg, mk_call_system_verb_msg,
    mk_connection_establish_msg, mk_detach_host_msg, mk_eval_msg, mk_get_server_features_msg,
    mk_reattach_msg, mk_register_host_msg, mk_request_sys_prop_msg, mk_resolve_msg,
    read_reply_result,
};
use std::{
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tmq::subscribe;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Extract the real client IP address from proxy headers or ConnectInfo.
/// Only honours X-Real-IP / X-Forwarded-For if the direct peer is within
/// a trusted proxy CIDR. Otherwise returns the direct connection address.
fn get_client_addr(
    headers: &HeaderMap,
    connect_addr: SocketAddr,
    trusted_cidrs: &[IpNet],
) -> SocketAddr {
    debug!(
        "Extracting client address. Direct connect_addr: {}",
        connect_addr
    );

    // Only trust proxy headers when the direct peer is in a trusted CIDR
    let peer_trusted = !trusted_cidrs.is_empty()
        && trusted_cidrs
            .iter()
            .any(|cidr| cidr.contains(&connect_addr.ip()));

    if !peer_trusted {
        if !trusted_cidrs.is_empty() {
            debug!(
                "Peer {} not in trusted proxy CIDRs, ignoring proxy headers",
                connect_addr.ip()
            );
        }
        return connect_addr;
    }

    debug!(
        "  X-Real-IP header: {:?}",
        headers.get("X-Real-IP").and_then(|h| h.to_str().ok())
    );
    debug!(
        "  X-Forwarded-For header: {:?}",
        headers.get("X-Forwarded-For").and_then(|h| h.to_str().ok())
    );

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

fn extract_ws_attach_info(headers: &HeaderMap) -> Result<WsAttachInfo, StatusCode> {
    let mut auth_token = headers
        .get("X-Moor-Auth-Token")
        .and_then(|value| value.to_str().ok())
        .map(|token| token.to_string());
    let mut client_id = headers
        .get("X-Moor-Client-Id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok());
    let mut client_token = headers
        .get("X-Moor-Client-Token")
        .and_then(|value| value.to_str().ok())
        .map(|value| ClientToken(value.to_string()));
    let mut is_initial_attach = false;

    if let Some(protocols_header) = headers.get(header::SEC_WEBSOCKET_PROTOCOL) {
        let protocols_str = protocols_header
            .to_str()
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        for protocol in protocols_str.split(',').map(|p| p.trim()) {
            if let Some(token) = protocol.strip_prefix("paseto.") {
                if !token.is_empty() {
                    auth_token = Some(token.to_string());
                }
                continue;
            }

            if let Some(id_str) = protocol.strip_prefix("client_id.") {
                if let Ok(parsed_id) = Uuid::parse_str(id_str) {
                    client_id = Some(parsed_id);
                }
                continue;
            }

            if let Some(token) = protocol.strip_prefix("client_token.") {
                if !token.is_empty() {
                    client_token = Some(ClientToken(token.to_string()));
                }
                continue;
            }

            if protocol
                .strip_prefix("initial_attach.")
                .is_some_and(|f| f.eq_ignore_ascii_case("true"))
            {
                is_initial_attach = true;
            }
        }
    }

    let auth_token = auth_token.ok_or(StatusCode::UNAUTHORIZED)?;
    let client_hint = match (client_id, client_token) {
        (Some(id), Some(token)) => Some((id, token)),
        _ => None,
    };

    debug!(
        "extract_ws_attach_info: is_initial_attach={}, client_hint={:?}",
        is_initial_attach,
        client_hint.as_ref().map(|(id, _)| id)
    );
    Ok(WsAttachInfo {
        auth_token,
        client_hint,
        is_initial_attach,
    })
}

fn failure_message(failure: moor_rpc::FailureRef<'_>) -> String {
    failure
        .error()
        .ok()
        .and_then(|e| e.message().ok().flatten())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown error".to_string())
}

fn decode_attach_result(
    attach_result: moor_rpc::AttachResultRef<'_>,
) -> Result<(ClientToken, Obj), WsHostError> {
    let success = attach_result
        .success()
        .map_err(|e| WsHostError::RpcError(eyre!("AttachResult missing success flag: {}", e)))?;
    if !success {
        return Err(WsHostError::AuthenticationFailed);
    }

    let client_token_ref = attach_result
        .client_token()
        .ok()
        .flatten()
        .ok_or_else(|| WsHostError::RpcError(eyre!("AttachResult missing client_token")))?;
    let client_token_value = client_token_ref.token().map_err(|e| {
        WsHostError::RpcError(eyre!(
            "AttachResult client token missing token string: {}",
            e
        ))
    })?;

    let player_ref = attach_result
        .player()
        .ok()
        .flatten()
        .ok_or_else(|| WsHostError::RpcError(eyre!("AttachResult missing player")))?;
    let player = obj_from_ref(player_ref)
        .map_err(|e| WsHostError::RpcError(eyre!("Failed to decode player: {}", e)))?;

    Ok((ClientToken(client_token_value.to_string()), player))
}

fn decode_new_connection_token(
    new_conn: moor_rpc::NewConnectionRef<'_>,
) -> Result<ClientToken, WsHostError> {
    let client_token_ref = new_conn
        .client_token()
        .ok()
        .ok_or_else(|| WsHostError::RpcError(eyre!("NewConnection missing client_token")))?;
    let client_token_value = client_token_ref.token().map_err(|e| {
        WsHostError::RpcError(eyre!(
            "NewConnection client token missing token string: {}",
            e
        ))
    })?;

    let objid_ref = new_conn
        .connection_obj()
        .ok()
        .ok_or_else(|| WsHostError::RpcError(eyre!("NewConnection missing connection_obj")))?;
    let objid = obj_from_ref(objid_ref)
        .map_err(|e| WsHostError::RpcError(eyre!("Failed to decode connection_obj: {}", e)))?;
    info!("Connection established, connection ID: {}", objid);

    Ok(ClientToken(client_token_value.to_string()))
}

/// Cached DNS resolver to avoid recreating on every connection
/// Initialized lazily on first use
static DNS_RESOLVER: LazyLock<Result<TokioResolver, String>> = LazyLock::new(|| {
    debug!("DNS resolver initialization STARTING");
    let builder = TokioResolver::builder_tokio().map_err(|e| e.to_string())?;
    debug!("DNS resolver builder created, calling build()");
    let resolver = builder.build();
    debug!("DNS resolver initialization COMPLETE");
    Ok(resolver)
});

/// Perform async reverse DNS lookup for an IP address with timeout
async fn resolve_hostname(ip: IpAddr) -> Result<String, eyre::Error> {
    debug!(
        "resolve_hostname: Acquiring DNS resolver reference for {}",
        ip
    );

    // Get the cached resolver (created once, reused for all connections)
    let resolver = DNS_RESOLVER
        .as_ref()
        .map_err(|e| eyre::eyre!("DNS resolver initialization failed: {}", e))?;

    debug!("resolve_hostname: DNS resolver acquired, creating lookup future");

    // Perform reverse DNS lookup with 2 second timeout
    let lookup_future = resolver.reverse_lookup(ip);

    debug!("resolve_hostname: Starting timeout wrapper (2s) for reverse lookup");
    let timeout_result = timeout(Duration::from_secs(2), lookup_future).await;

    debug!("resolve_hostname: Timeout wrapper returned for {}", ip);

    let response = timeout_result.map_err(|_| {
        warn!("DNS lookup timeout (2s) for {}", ip);
        eyre::eyre!("DNS lookup timeout")
    })??;

    debug!("resolve_hostname: Got DNS response, extracting hostname");

    // Get the first hostname from the response
    if let Some(name) = response.iter().next() {
        let hostname = name.to_string().trim_end_matches('.').to_string();
        debug!(
            "resolve_hostname: Successfully resolved {} to {}",
            ip, hostname
        );
        Ok(hostname)
    } else {
        debug!("resolve_hostname: No PTR record found for {}", ip);
        Err(eyre::eyre!("No PTR record found"))
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LoginType {
    Connect,
    Create,
}

#[derive(Debug, Default)]
struct WsAttachInfo {
    auth_token: String,
    client_hint: Option<(Uuid, ClientToken)>,
    is_initial_attach: bool,
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
    last_daemon_ping: Arc<AtomicU64>,
    pub(crate) trusted_proxy_cidrs: Arc<Vec<IpNet>>,
    /// Cached server features response (features don't change at runtime).
    features_cache: Arc<tokio::sync::OnceCell<Vec<u8>>>,
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
        last_daemon_ping: Arc<AtomicU64>,
        trusted_proxy_cidrs: Arc<Vec<IpNet>>,
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
            last_daemon_ping,
            trusted_proxy_cidrs,
            features_cache: Arc::new(tokio::sync::OnceCell::new()),
        }
    }
}

impl WebHost {
    pub fn create_rpc_client(&self) -> RpcClient {
        self.build_rpc_client()
    }

    pub fn new_stateless_client(&self) -> (Uuid, RpcClient) {
        (Uuid::new_v4(), self.create_rpc_client())
    }

    fn build_rpc_client(&self) -> RpcClient {
        let zmq_ctx = self.zmq_context.clone();
        RpcClient::new_with_defaults(
            Arc::new(zmq_ctx.clone()),
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
        )
    }

    /// Contact the RPC server to validate an auth token, and return the object ID of the player
    /// and the client token and rpc client to use for the connection.
    pub async fn attach_authenticated(
        &self,
        auth_token: AuthToken,
        connect_type: Option<moor_rpc::ConnectType>,
        peer_addr: SocketAddr,
    ) -> Result<(Obj, Uuid, ClientToken, RpcClient), WsHostError> {
        let client_id = Uuid::new_v4();
        let rpc_client = self.build_rpc_client();

        // Perform reverse DNS lookup for hostname
        debug!(
            "attach_authenticated: About to call resolve_hostname for {}",
            peer_addr.ip()
        );
        let hostname = match resolve_hostname(peer_addr.ip()).await {
            Ok(hostname) => {
                debug!(
                    "attach_authenticated: Resolved {} to hostname: {}",
                    peer_addr.ip(),
                    hostname
                );
                hostname
            }
            Err(e) => {
                debug!(
                    "attach_authenticated: Failed to resolve {} ({}), using IP address",
                    peer_addr.ip(),
                    e
                );
                peer_addr.to_string()
            }
        };
        debug!("attach_authenticated: DNS lookup complete, continuing with attach");

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

        let reply = read_reply_result(&reply_bytes)
            .map_err(|e| WsHostError::RpcError(eyre!("Failed to parse reply: {}", e)))?;

        let (client_token, player) = match reply.result() {
            Ok(moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success)) => {
                let daemon_reply = client_success.reply().ok().ok_or_else(|| {
                    WsHostError::RpcError(eyre!("Attach response missing daemon reply"))
                })?;
                match daemon_reply.reply().map_err(|e| {
                    WsHostError::RpcError(eyre!(
                        "Attach response missing daemon reply union: {}",
                        e
                    ))
                })? {
                    moor_rpc::DaemonToClientReplyUnionRef::AttachResult(attach_result) => {
                        match decode_attach_result(attach_result) {
                            Ok((client_token, player)) => {
                                debug!("Connection authenticated, player: {}", player);
                                (client_token, player)
                            }
                            Err(WsHostError::AuthenticationFailed) => {
                                warn!("Connection authentication failed from {}", peer_addr);
                                return Err(WsHostError::AuthenticationFailed);
                            }
                            Err(e) => return Err(e),
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
            Ok(moor_rpc::ReplyResultUnionRef::Failure(failure)) => {
                let msg = failure_message(failure);
                debug!("Attach rejected: {}", msg);
                return Err(WsHostError::AuthenticationFailed);
            }
            Ok(moor_rpc::ReplyResultUnionRef::HostSuccess(_)) => {
                error!("Unexpected host success response");
                return Err(WsHostError::RpcError(eyre!("Unexpected host success")));
            }
            Err(e) => {
                return Err(WsHostError::RpcError(eyre!(
                    "Attach response missing top-level result: {}",
                    e
                )));
            }
        };

        Ok((player, client_id, client_token, rpc_client))
    }

    pub async fn reattach_authenticated(
        &self,
        auth_token: AuthToken,
        client_id: Uuid,
        client_token: ClientToken,
        peer_addr: SocketAddr,
    ) -> Result<(Obj, Uuid, ClientToken, RpcClient), WsHostError> {
        let rpc_client = self.build_rpc_client();

        debug!(
            "reattach_authenticated: About to call resolve_hostname for {}",
            peer_addr.ip()
        );
        let hostname = match resolve_hostname(peer_addr.ip()).await {
            Ok(hostname) => {
                debug!(
                    "reattach_authenticated: Resolved {} to hostname: {}",
                    peer_addr.ip(),
                    hostname
                );
                hostname
            }
            Err(e) => {
                debug!(
                    "reattach_authenticated: Failed to resolve {} ({}), using IP address",
                    peer_addr.ip(),
                    e
                );
                peer_addr.to_string()
            }
        };
        debug!("reattach_authenticated: DNS lookup complete, continuing with reattach");

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

        let reattach_msg = mk_reattach_msg(
            &client_token,
            &auth_token,
            Some(hostname),
            Some(self.local_port),
            Some(peer_addr.port()),
            Some(content_types),
            None,
        );

        let reply_bytes = match rpc_client
            .make_client_rpc_call(client_id, reattach_msg)
            .await
        {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Unable to reattach: {}", e);
                return Err(WsHostError::RpcError(eyre!(e)));
            }
        };

        let reply = read_reply_result(&reply_bytes)
            .map_err(|e| WsHostError::RpcError(eyre!("Failed to parse reply: {}", e)))?;

        let (client_token, player) = match reply.result() {
            Ok(moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success)) => {
                let daemon_reply = client_success.reply().ok().ok_or_else(|| {
                    WsHostError::RpcError(eyre!("Reattach response missing daemon reply"))
                })?;
                match daemon_reply.reply().map_err(|e| {
                    WsHostError::RpcError(eyre!(
                        "Reattach response missing daemon reply union: {}",
                        e
                    ))
                })? {
                    moor_rpc::DaemonToClientReplyUnionRef::AttachResult(attach_result) => {
                        match decode_attach_result(attach_result) {
                            Ok((confirmed_client_token, player)) => {
                                (confirmed_client_token, player)
                            }
                            Err(WsHostError::AuthenticationFailed) => {
                                warn!("Connection reattach failed from {}", peer_addr);
                                return Err(WsHostError::AuthenticationFailed);
                            }
                            Err(e) => return Err(e),
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
            Ok(moor_rpc::ReplyResultUnionRef::Failure(failure)) => {
                let msg = failure_message(failure);
                debug!("Reattach rejected: {}", msg);
                return Err(WsHostError::AuthenticationFailed);
            }
            Ok(moor_rpc::ReplyResultUnionRef::HostSuccess(_)) => {
                error!("Unexpected host success response");
                return Err(WsHostError::RpcError(eyre!("Unexpected host success")));
            }
            Err(e) => {
                return Err(WsHostError::RpcError(eyre!(
                    "Reattach response missing top-level result: {}",
                    e
                )));
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
            .map_err(|e| eyre::eyre!("Unable to connect narrative subscriber: {}", e))?;
        let narrative_sub = narrative_sub
            .subscribe(&client_id.as_bytes()[..])
            .map_err(|e| eyre::eyre!("Unable to subscribe to narrative messages: {}", e))?;

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
            .map_err(|e| eyre::eyre!("Unable to connect broadcast subscriber: {}", e))?;
        let broadcast_sub = broadcast_sub
            .subscribe(CLIENT_BROADCAST_TOPIC)
            .map_err(|e| eyre::eyre!("Unable to subscribe to broadcast messages: {}", e))?;

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
            close_code: None,
            is_logout: false,
        })
    }

    /// Create an event subscription for a specific client_id
    /// Used for HTTP handlers that need to wait for task completion events
    pub async fn events_sub(
        &self,
        client_id: Uuid,
    ) -> Result<tmq::subscribe::Subscribe, eyre::Error> {
        let zmq_ctx = self.zmq_context.clone();

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
            .map_err(|e| eyre::eyre!("Unable to connect narrative subscriber: {}", e))?;
        let narrative_sub = narrative_sub
            .subscribe(&client_id.as_bytes()[..])
            .map_err(|e| eyre::eyre!("Unable to subscribe to narrative messages: {}", e))?;

        Ok(narrative_sub)
    }

    pub async fn establish_client_connection(
        &self,
        addr: SocketAddr,
    ) -> Result<(Uuid, RpcClient, ClientToken), WsHostError> {
        let rpc_client = self.build_rpc_client();

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

        let reply = read_reply_result(&reply_bytes)
            .map_err(|e| WsHostError::RpcError(eyre!("Failed to parse reply: {}", e)))?;

        let client_token = match reply.result() {
            Ok(moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success)) => {
                let daemon_reply = client_success.reply().ok().ok_or_else(|| {
                    WsHostError::RpcError(eyre!("Connection establishment missing daemon reply"))
                })?;
                match daemon_reply.reply().map_err(|e| {
                    WsHostError::RpcError(eyre!(
                        "Connection establishment missing daemon reply union: {}",
                        e
                    ))
                })? {
                    moor_rpc::DaemonToClientReplyUnionRef::NewConnection(new_conn) => {
                        decode_new_connection_token(new_conn)?
                    }
                    _ => {
                        error!("Unexpected response from RPC server");
                        return Err(WsHostError::RpcError(eyre!(
                            "Unexpected response from RPC server"
                        )));
                    }
                }
            }
            Ok(moor_rpc::ReplyResultUnionRef::Failure(_)) => {
                error!("RPC failure in connection establishment");
                return Err(WsHostError::RpcError(eyre!("RPC failure")));
            }
            Ok(moor_rpc::ReplyResultUnionRef::HostSuccess(_)) => {
                error!("Unexpected host success response");
                return Err(WsHostError::RpcError(eyre!("Unexpected host success")));
            }
            Err(e) => {
                return Err(WsHostError::RpcError(eyre!(
                    "Connection establishment missing top-level result: {}",
                    e
                )));
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

        let register_reply = match read_reply_result(&register_bytes) {
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

        let reply = match read_reply_result(&reply_bytes) {
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
    rpc_client: &RpcClient,
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

pub async fn features_handler(State(host): State<WebHost>, header_map: HeaderMap) -> Response {
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
        Err(status) => return status.into_response(),
    };

    // Features don't change at runtime — cache the result of the first fetch.
    let bytes = match host
        .features_cache
        .get_or_try_init(|| host.fetch_server_features())
        .await
    {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(bytes.clone()),
        ResponseFormat::Json => match reply_result_to_json(bytes) {
            Ok(resp) => resp,
            Err(status) => status.into_response(),
        },
    }
}

pub async fn system_property_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path(path): Path<String>,
) -> Response {
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
        Err(status) => return status.into_response(),
    };

    let auth_token = auth::extract_auth_token_header(&header_map).ok();
    let rpc_client = host.create_rpc_client();
    let client_id = Uuid::new_v4();

    let path_parts: Vec<&str> = path.split('/').collect();
    let (obj_path, property_name) = if path_parts.len() < 2 {
        return StatusCode::BAD_REQUEST.into_response();
    } else {
        let obj_parts = &path_parts[..path_parts.len() - 1];
        let prop = path_parts[path_parts.len() - 1];
        (obj_parts.iter().map(|&s| Symbol::mk(s)).collect(), prop)
    };

    let sysprop_msg = mk_request_sys_prop_msg(
        auth_token.as_ref(),
        &ObjectRef::SysObj(obj_path),
        &Symbol::mk(property_name),
    );

    let reply_bytes = match rpc_call(client_id, &rpc_client, sysprop_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(reply_bytes),
        ResponseFormat::Json => match reply_result_to_json(&reply_bytes) {
            Ok(resp) => resp,
            Err(status) => status.into_response(),
        },
    }
}

fn should_attempt_reattach(is_initial_attach: bool, has_client_hint: bool) -> bool {
    has_client_hint && !is_initial_attach
}

fn effective_connect_type_for_fresh_attach(
    connect_type: moor_rpc::ConnectType,
    is_initial_attach: bool,
    has_client_hint: bool,
) -> moor_rpc::ConnectType {
    if has_client_hint && !is_initial_attach {
        return moor_rpc::ConnectType::Reconnected;
    }
    connect_type
}

/// Attach a websocket connection to an existing player.
async fn attach(
    ws: WebSocketUpgrade,
    addr: SocketAddr,
    connect_type: moor_rpc::ConnectType,
    host: &WebHost,
    auth_token: String,
    client_hint: Option<(Uuid, ClientToken)>,
    is_initial_attach: bool,
) -> Response {
    debug!(
        "Connection from {}, is_initial_attach={}, has_client_hint={}",
        addr,
        is_initial_attach,
        client_hint.is_some()
    );

    let auth_token = AuthToken(auth_token);

    let has_client_hint = client_hint.is_some();
    let attempt_reattach = should_attempt_reattach(is_initial_attach, has_client_hint);

    let reattach_details = if attempt_reattach {
        let Some((hint_id, hint_token)) = client_hint.clone() else {
            error!("attach decision bug: attempt_reattach=true without client_hint");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        debug!(
            client_id = ?hint_id,
            "WebSocket attach: attempting reattach with existing credentials"
        );
        match host
            .reattach_authenticated(auth_token.clone(), hint_id, hint_token.clone(), addr)
            .await
        {
            Ok(details) => {
                debug!(client_id = ?hint_id, "WebSocket reattach succeeded");
                Some(details)
            }
            Err(WsHostError::AuthenticationFailed) => {
                warn!(client_id = ?hint_id, "WebSocket reattach failed - will create new connection");
                None
            }
            Err(e) => {
                error!("Reattach attempt failed: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    } else {
        debug!(
            "WebSocket attach: skipping reattach (is_initial_attach={}, has_client_hint={})",
            is_initial_attach, has_client_hint
        );
        None
    };
    let reattach_succeeded = reattach_details.is_some();

    let (effective_connect_type, connection_details) = if let Some(details) = reattach_details {
        debug!("Reattach succeeded, using Reconnected");
        (moor_rpc::ConnectType::Reconnected, details)
    } else {
        let ct =
            effective_connect_type_for_fresh_attach(connect_type, is_initial_attach, has_client_hint);
        debug!("Fresh attach effective connect_type={:?}", ct);
        match host
            .attach_authenticated(auth_token.clone(), Some(ct), addr)
            .await
        {
            Ok(details) => (ct, details),
            Err(WsHostError::AuthenticationFailed) => {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            Err(e) => {
                error!("Unable to validate auth token: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };
    debug!(
        "WebSocket attach decision: is_initial_attach={}, had_client_hint={}, attempt_reattach={}, reattach_succeeded={}, effective_connect_type={:?}",
        is_initial_attach,
        has_client_hint,
        attempt_reattach,
        reattach_succeeded,
        effective_connect_type
    );
    if has_client_hint && !reattach_succeeded {
        warn!(
            "WebSocket attach fallback: client_hint_present_but_reattach_failed; is_initial_attach={}, effective_connect_type={:?}",
            is_initial_attach,
            effective_connect_type
        );
    }
    let (player, client_id, client_token, rpc_client) = connection_details;

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
        return StatusCode::UNAUTHORIZED.into_response();
    };

    ws.on_upgrade(
        move |socket| async move { connection.handle(effective_connect_type, socket).await },
    )
}

/// Websocket upgrade handler for authenticated users who are connecting to an existing user
pub async fn ws_connect_attach_handler(
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    ws: WebSocketUpgrade,
) -> Response {
    debug!(
        "ws_connect_attach_handler called, ConnectInfo addr: {}",
        addr
    );
    let client_addr = get_client_addr(&headers, addr, &ws_host.trusted_proxy_cidrs);
    info!("WebSocket connection from {}", client_addr);

    let attach_info = match extract_ws_attach_info(&headers) {
        Ok(info) => info,
        Err(status) => return status.into_response(),
    };

    let ws = ws.protocols(["moor"]);
    attach(
        ws,
        client_addr,
        moor_rpc::ConnectType::Connected,
        &ws_host,
        attach_info.auth_token,
        attach_info.client_hint,
        attach_info.is_initial_attach,
    )
    .await
}

/// Websocket upgrade handler for authenticated users who are connecting to a new user
pub async fn ws_create_attach_handler(
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    ws: WebSocketUpgrade,
) -> Response {
    debug!(
        "ws_create_attach_handler called, ConnectInfo addr: {}",
        addr
    );
    let client_addr = get_client_addr(&headers, addr, &ws_host.trusted_proxy_cidrs);
    info!("WebSocket connection from {}", client_addr);

    let attach_info = match extract_ws_attach_info(&headers) {
        Ok(info) => info,
        Err(status) => return status.into_response(),
    };

    let ws = ws.protocols(["moor"]);
    attach(
        ws,
        client_addr,
        moor_rpc::ConnectType::Created,
        &ws_host,
        attach_info.auth_token,
        attach_info.client_hint,
        attach_info.is_initial_attach,
    )
    .await
}

pub async fn resolve_objref_handler(
    auth::StatelessAuth {
        auth_token,
        client_id,
        rpc_client,
    }: auth::StatelessAuth,
    header_map: HeaderMap,
    Path(object): Path<String>,
) -> Response {
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
        Err(status) => return status.into_response(),
    };

    let objref = match ObjectRef::parse_curie(&object) {
        None => {
            return StatusCode::BAD_REQUEST.into_response();
        }
        Some(oref) => oref,
    };

    let resolve_msg = mk_resolve_msg(&auth_token, &objref);

    let reply_bytes = match rpc_call(client_id, &rpc_client, resolve_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(reply_bytes),
        ResponseFormat::Json => match reply_result_to_json(&reply_bytes) {
            Ok(resp) => resp,
            Err(status) => status.into_response(),
        },
    }
}

pub async fn eval_handler(
    auth::EphemeralAuth {
        auth_token,
        client_id,
        client_token,
        rpc_client,
        ..
    }: auth::EphemeralAuth,
    header_map: HeaderMap,
    expression: Bytes,
) -> Response {
    if let Err(status) = require_content_type(
        header_map.get(header::CONTENT_TYPE),
        &[TEXT_PLAIN_CONTENT_TYPE],
        true, // allow missing for backwards compat
    ) {
        return status.into_response();
    }
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
        Err(status) => return status.into_response(),
    };

    let expression = String::from_utf8_lossy(&expression).to_string();

    let eval_msg = mk_eval_msg(&client_token, &auth_token, expression);

    let reply_bytes = match rpc_call(client_id, &rpc_client, eval_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    // DetachGuard in EphemeralAuth handles cleanup automatically

    match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(reply_bytes),
        ResponseFormat::Json => match reply_result_to_json(&reply_bytes) {
            Ok(resp) => resp,
            Err(status) => status.into_response(),
        },
    }
}

pub async fn invoke_welcome_message_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
) -> Response {
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
        Err(status) => return status.into_response(),
    };

    let rpc_client = host.create_rpc_client();
    let client_id = Uuid::new_v4();

    let verb = Symbol::mk("do_login_command");
    let args: Vec<&moor_var::Var> = vec![];

    let call_system_verb_msg = match mk_call_system_verb_msg(None, &verb, args) {
        Some(msg) => msg,
        None => {
            error!("Failed to create CallSystemVerb message");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let reply_bytes = match rpc_call(client_id, &rpc_client, call_system_verb_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(reply_bytes),
        ResponseFormat::Json => match reply_result_to_json(&reply_bytes) {
            Ok(resp) => resp,
            Err(status) => status.into_response(),
        },
    }
}

/// Health check endpoint - verifies host is healthy and can communicate with daemon
/// Checks that we've received a ping from the daemon recently (within last 30 seconds)
/// Does NOT invoke any MOO code - just checks infrastructure connectivity
pub async fn health_handler(State(host): State<WebHost>) -> Response {
    let last_ping = host.last_daemon_ping.load(Ordering::Relaxed);
    let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(e) => {
            error!("Invalid system time in health check: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Report healthy if: no ping yet (last_ping == 0, still starting up) OR ping within last 30s
    // This proves: daemon is alive, CURVE auth working, host is registered
    if last_ping == 0 || now - last_ping < 30 {
        StatusCode::OK.into_response()
    } else {
        StatusCode::SERVICE_UNAVAILABLE.into_response()
    }
}

/// Server version information
#[derive(serde::Serialize)]
pub struct VersionInfo {
    pub version: &'static str,
    pub commit: &'static str,
}

/// Version endpoint - returns server version and git commit
pub async fn version_handler() -> Json<VersionInfo> {
    Json(VersionInfo {
        version: moor_common::build::PKG_VERSION,
        commit: moor_common::build::short_commit(),
    })
}

const OPENAPI_SPEC: &str = include_str!("../../openapi.yaml");

pub async fn openapi_handler() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/yaml")], OPENAPI_SPEC)
}

#[cfg(test)]
mod tests {
    use super::{effective_connect_type_for_fresh_attach, should_attempt_reattach};
    use super::moor_rpc::ConnectType;

    #[test]
    fn attach_decision_matrix_for_reattach_attempt() {
        assert!(should_attempt_reattach(false, true));
        assert!(!should_attempt_reattach(true, true));
        assert!(!should_attempt_reattach(false, false));
        assert!(!should_attempt_reattach(true, false));
    }

    #[test]
    fn attach_decision_matrix_for_fresh_connect_type() {
        assert_eq!(
            effective_connect_type_for_fresh_attach(
                ConnectType::Connected,
                true,
                true
            ),
            ConnectType::Connected
        );
        assert_eq!(
            effective_connect_type_for_fresh_attach(
                ConnectType::Connected,
                false,
                true
            ),
            ConnectType::Reconnected
        );
        assert_eq!(
            effective_connect_type_for_fresh_attach(
                ConnectType::Connected,
                false,
                false
            ),
            ConnectType::Connected
        );
        assert_eq!(
            effective_connect_type_for_fresh_attach(
                ConnectType::Created,
                false,
                false
            ),
            ConnectType::Created
        );
    }
}
