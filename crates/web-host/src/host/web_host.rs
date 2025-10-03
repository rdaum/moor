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

use crate::host::{auth, var_as_json, ws_connection::WebSocketConnection};
use axum::{
    Json,
    body::{Body, Bytes},
    extract::{ConnectInfo, Path, Query, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use eyre::eyre;
use hickory_resolver::TokioResolver;

use moor_common::{model::ObjectRef, tasks::Event};
use moor_schema::{
    convert::{
        narrative_event_from_ref, obj_from_flatbuffer_struct, presentation_from_ref,
        var_from_flatbuffer,
    },
    rpc as moor_rpc, var as moor_var_schema,
};
use moor_var::{E_INVIND, Obj, Symbol, Var, v_err, v_obj};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_common::{
    AuthToken, CLIENT_BROADCAST_TOPIC, ClientToken, mk_attach_msg, mk_connection_establish_msg,
    mk_detach_msg, mk_dismiss_presentation_msg, mk_eval_msg, mk_invoke_verb_msg,
    mk_request_current_presentations_msg, mk_request_history_msg, mk_request_sys_prop_msg,
    mk_resolve_msg,
};
use serde_derive::Deserialize;
use serde_json::json;
use std::net::{IpAddr, SocketAddr};
use tmq::{request, subscribe};
use tracing::{debug, error, info, warn};
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
    ) -> Self {
        let tmq_context = tmq::Context::new();
        Self {
            zmq_context: tmq_context,
            rpc_addr,
            pubsub_addr: narrative_addr,
            handler_object,
            local_port,
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
    ) -> Result<(Obj, Uuid, ClientToken, RpcSendClient), WsHostError> {
        let zmq_ctx = self.zmq_context.clone();
        // Establish a connection to the RPC server
        let client_id = Uuid::new_v4();
        let rcp_request_sock = request(&zmq_ctx)
            .set_rcvtimeo(100)
            .set_sndtimeo(100)
            .connect(self.rpc_addr.as_str())
            .map_err(|e| WsHostError::RpcError(eyre!(e)))?;

        // Establish a connection to the RPC server
        debug!(
            self.rpc_addr,
            "Contacting RPC server to establish connection"
        );
        let mut rpc_client = RpcSendClient::new(rcp_request_sock);

        // Perform reverse DNS lookup for hostname
        let hostname = match resolve_hostname(peer_addr.ip()).await {
            Ok(hostname) => {
                debug!("Resolved {} to hostname: {}", peer_addr.ip(), hostname);
                hostname
            }
            Err(_) => {
                debug!("Failed to resolve {}, using IP address", peer_addr.ip());
                peer_addr.to_string()
            }
        };

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
        rpc_client: RpcSendClient,
        peer_addr: SocketAddr,
    ) -> Result<WebSocketConnection, eyre::Error> {
        let zmq_ctx = self.zmq_context.clone();

        // We'll need to subscribe to the narrative & broadcast messages for this connection.
        let narrative_sub = subscribe(&zmq_ctx)
            .connect(self.pubsub_addr.as_str())
            .expect("Unable to connect narrative subscriber ");
        let narrative_sub = narrative_sub
            .subscribe(&client_id.as_bytes()[..])
            .expect("Unable to subscribe to narrative messages for client connection");

        let broadcast_sub = subscribe(&zmq_ctx)
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
    ) -> Result<(Uuid, RpcSendClient, ClientToken), WsHostError> {
        let zmq_ctx = self.zmq_context.clone();
        let rcp_request_sock = request(&zmq_ctx)
            .set_rcvtimeo(100)
            .set_sndtimeo(100)
            .connect(self.rpc_addr.as_str())
            .expect("Unable to bind RPC server for connection");

        let client_id = Uuid::new_v4();
        let mut rpc_client = RpcSendClient::new(rcp_request_sock);

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
}

pub(crate) async fn rpc_call(
    client_id: Uuid,
    rpc_client: &mut RpcSendClient,
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

/// Stand-alone HTTP GET handler for getting system properties.
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
        &moor_common::model::ObjectRef::SysObj(obj_path),
        &Symbol::mk(property_name),
    );

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, sysprop_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    use planus::ReadAsRoot;
    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(host_success) => {
            let daemon_reply = host_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::SysPropValue(sysprop) => {
                    if let Ok(Some(value_ref)) = sysprop.value() {
                        let value_struct = moor_var_schema::Var::try_from(value_ref)
                            .expect("Failed to convert value");
                        let value =
                            var_from_flatbuffer(&value_struct).expect("Failed to decode value");
                        Json(var_as_json(&value)).into_response()
                    } else {
                        StatusCode::NOT_FOUND.into_response()
                    }
                }
                _ => {
                    error!("Unexpected response from RPC server");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        _ => {
            error!("RPC failure");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    };

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

/// Evaluate a MOO expression and return the result.
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

    use planus::ReadAsRoot;
    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::EvalResult(eval_result) => {
                    let value_ref = eval_result.result().expect("Missing value");
                    let value_struct =
                        moor_var_schema::Var::try_from(value_ref).expect("Failed to convert value");
                    let value = var_from_flatbuffer(&value_struct).expect("Failed to decode value");
                    debug!("Eval result: {:?}", value);
                    Json(var_as_json(&value)).into_response()
                }
                _ => {
                    error!("Unexpected response from RPC server");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        _ => {
            error!("RPC failure");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    };

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

    use planus::ReadAsRoot;
    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::ResolveResult(resolve_result) => {
                    let value_ref = resolve_result.result().expect("Missing value");
                    let value_struct =
                        moor_var_schema::Var::try_from(value_ref).expect("Failed to convert value");
                    let obj = var_from_flatbuffer(&value_struct).expect("Failed to decode value");
                    if obj == v_err(E_INVIND) {
                        StatusCode::NOT_FOUND.into_response()
                    } else {
                        Json(var_as_json(&obj)).into_response()
                    }
                }
                _ => {
                    error!("Unexpected response from RPC server");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        _ => {
            error!("RPC failure");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    };

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
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    Path(token): Path<String>,
) -> impl IntoResponse + use<> {
    info!("Connection from {}", addr);

    attach(ws, addr, moor_rpc::ConnectType::Connected, &ws_host, token).await
}

/// Websocket upgrade handler for authenticated users who are connecting to a new user
pub async fn ws_create_attach_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    Path(token): Path<String>,
) -> impl IntoResponse + use<> {
    info!("Connection from {}", addr);

    attach(ws, addr, moor_rpc::ConnectType::Created, &ws_host, token).await
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    since_seconds: Option<u64>,
    since_event: Option<String>, // UUID as string
    until_event: Option<String>, // UUID as string
    limit: Option<usize>,
}

/// REST endpoint to retrieve player event history
pub async fn history_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    let history_recall_union = if let Some(since_seconds) = query.since_seconds {
        moor_rpc::HistoryRecallUnion::HistoryRecallSinceSeconds(Box::new(
            moor_rpc::HistoryRecallSinceSeconds {
                seconds_ago: since_seconds,
                limit: query.limit.unwrap_or(0) as u64,
            },
        ))
    } else if let Some(since_event_str) = query.since_event {
        match Uuid::parse_str(&since_event_str) {
            Ok(uuid) => {
                let uuid_bytes = uuid.as_bytes().to_vec();
                moor_rpc::HistoryRecallUnion::HistoryRecallSinceEvent(Box::new(
                    moor_rpc::HistoryRecallSinceEvent {
                        event_id: Box::new(moor_rpc::Uuid { data: uuid_bytes }),
                        limit: query.limit.unwrap_or(0) as u64,
                    },
                ))
            }
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    } else if let Some(until_event_str) = query.until_event {
        match Uuid::parse_str(&until_event_str) {
            Ok(uuid) => {
                let uuid_bytes = uuid.as_bytes().to_vec();
                moor_rpc::HistoryRecallUnion::HistoryRecallUntilEvent(Box::new(
                    moor_rpc::HistoryRecallUntilEvent {
                        event_id: Box::new(moor_rpc::Uuid { data: uuid_bytes }),
                        limit: query.limit.unwrap_or(0) as u64,
                    },
                ))
            }
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    } else {
        moor_rpc::HistoryRecallUnion::HistoryRecallNone(Box::new(moor_rpc::HistoryRecallNone {}))
    };

    let history_msg = mk_request_history_msg(
        &client_token,
        &auth_token,
        Box::new(moor_rpc::HistoryRecall {
            recall: history_recall_union,
        }),
    );

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, history_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    use planus::ReadAsRoot;
    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(host_success) => {
            let daemon_reply = host_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::HistoryResponseReply(history_ref) => {
                    let history_response = history_ref.response().expect("Missing response");
                    let events_ref = history_response.events().expect("Missing events");
                    let events: Vec<_> = events_ref
                        .iter()
                        .filter_map(|event_result| {
                            let historical_event = event_result.ok()?;
                            let narrative_event_ref = historical_event.event().ok()?;
                            let narrative_event = narrative_event_from_ref(narrative_event_ref).ok()?;
                            let is_historical = historical_event.is_historical().ok()?;
                            let player_ref = historical_event.player().ok()?;
                            let player_struct = moor_rpc::Obj::try_from(player_ref).ok()?;
                            let player = obj_from_flatbuffer_struct(&player_struct).ok()?;

                            Some(json!({
                                "event_id": narrative_event.event_id(),
                                "author": var_as_json(narrative_event.author()),
                                "message": match narrative_event.event() {
                                    Event::Notify { value: msg, content_type, no_flush, no_newline } => {
                                        let _ = (no_flush, no_newline);
                                        let normalized_content_type = content_type.as_ref().map(|ct| {
                                            match ct.as_string().as_str() {
                                                "text_djot" => "text/djot".to_string(),
                                                "text_html" => "text/html".to_string(),
                                                "text_plain" => "text/plain".to_string(),
                                                _ => ct.as_string(),
                                            }
                                        });
                                        json!({
                                            "type": "notify",
                                            "content": var_as_json(&msg),
                                            "content_type": normalized_content_type
                                        })
                                    },
                                    Event::Traceback(ex) => json!({
                                        "type": "traceback",
                                        "error": format!("{}", ex)
                                    }),
                                    Event::Present(p) => json!({
                                        "type": "present",
                                        "presentation": p
                                    }),
                                    Event::Unpresent(id) => json!({
                                        "type": "unpresent",
                                        "id": id
                                    })
                                },
                                "timestamp": narrative_event.timestamp(),
                                "is_historical": is_historical,
                                "player": var_as_json(&v_obj(player))
                            }))
                        })
                        .collect();

                    let total_events = history_response
                        .total_events()
                        .expect("Missing total_events");
                    let time_range_start = history_response
                        .time_range_start()
                        .expect("Missing time_range_start");
                    let time_range_end = history_response
                        .time_range_end()
                        .expect("Missing time_range_end");
                    let has_more_before = history_response
                        .has_more_before()
                        .expect("Missing has_more_before");

                    let earliest_event_id = history_response
                        .earliest_event_id()
                        .ok()
                        .flatten()
                        .and_then(|uuid_ref| uuid_ref.data().ok())
                        .and_then(|bytes| {
                            if bytes.len() == 16 {
                                let mut uuid_bytes = [0u8; 16];
                                uuid_bytes.copy_from_slice(bytes);
                                Some(Uuid::from_bytes(uuid_bytes))
                            } else {
                                None
                            }
                        });

                    let latest_event_id = history_response
                        .latest_event_id()
                        .ok()
                        .flatten()
                        .and_then(|uuid_ref| uuid_ref.data().ok())
                        .and_then(|bytes| {
                            if bytes.len() == 16 {
                                let mut uuid_bytes = [0u8; 16];
                                uuid_bytes.copy_from_slice(bytes);
                                Some(Uuid::from_bytes(uuid_bytes))
                            } else {
                                None
                            }
                        });

                    Json(json!({
                        "events": events,
                        "meta": {
                            "total_events": total_events,
                            "time_range": (time_range_start, time_range_end),
                            "has_more_before": has_more_before,
                            "earliest_event_id": earliest_event_id,
                            "latest_event_id": latest_event_id
                        }
                    }))
                }
                _ => {
                    error!("Unexpected response from RPC server");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            }
        }
        _ => {
            error!("RPC failure");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // We're done with this RPC connection, so we detach it.
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client
        .make_client_rpc_call(client_id, detach_msg)
        .await
        .expect("Unable to send detach to RPC server");

    response.into_response()
}

/// REST endpoint to retrieve current presentations for the authenticated player
pub async fn presentations_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    let presentations_msg = mk_request_current_presentations_msg(&client_token, &auth_token);

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, presentations_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    use planus::ReadAsRoot;
    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(host_success) => {
            let daemon_reply = host_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::CurrentPresentations(presentations_ref) => {
                    let presentations_vec = presentations_ref
                        .presentations()
                        .expect("Missing presentations");
                    let presentations: Vec<_> = presentations_vec
                        .iter()
                        .filter_map(|p| presentation_from_ref(p.ok()?).ok())
                        .collect();
                    Json(json!({
                        "presentations": presentations
                    }))
                }
                _ => {
                    error!("Unexpected response from RPC server");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            }
        }
        _ => {
            error!("RPC failure");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // We're done with this RPC connection, so we detach it.
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client
        .make_client_rpc_call(client_id, detach_msg)
        .await
        .expect("Unable to send detach to RPC server");

    response.into_response()
}

/// REST endpoint to dismiss a specific presentation for the authenticated player
pub async fn dismiss_presentation_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path(presentation_id): Path<String>,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    let dismiss_msg =
        mk_dismiss_presentation_msg(&client_token, &auth_token, presentation_id.clone());

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, dismiss_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    use planus::ReadAsRoot;
    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(host_success) => {
            let daemon_reply = host_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::PresentationDismissed(_) => Json(json!({
                    "dismissed": true,
                    "presentation_id": presentation_id
                })),
                _ => {
                    error!("Unexpected response from RPC server");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            }
        }
        _ => {
            error!("RPC failure");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // We're done with this RPC connection, so we detach it.
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client
        .make_client_rpc_call(client_id, detach_msg)
        .await
        .expect("Unable to send detach to RPC server");

    response.into_response()
}

/// RESTful verb invocation handler: POST /verbs/{object}/{name}/invoke
pub async fn invoke_verb_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path((object_path, verb_name)): Path<(String, String)>,
    Json(args): Json<Vec<serde_json::Value>>,
) -> Response {
    // Parse the object reference from the path
    let object_ref = match ObjectRef::parse_curie(&object_path) {
        Some(oref) => oref,
        None => {
            warn!("Invalid object reference \"{}\"", object_path);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Convert verb name to Symbol
    let verb_symbol = Symbol::mk(&verb_name);

    // Convert JSON arguments to MOO Vars
    let moo_args: Result<Vec<Var>, _> = args.iter().map(crate::host::json_as_var).collect();

    let moo_args = match moo_args {
        Ok(args) => args,
        Err(e) => {
            warn!("Invalid arguments: {:?}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Get authenticated connection
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    // Make the InvokeVerb RPC call
    let args_refs: Vec<&Var> = moo_args.iter().collect();
    let invoke_msg = mk_invoke_verb_msg(
        &client_token,
        &auth_token,
        &object_ref,
        &verb_symbol,
        args_refs,
    )
    .expect("Failed to create invoke_verb message");

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, invoke_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    use planus::ReadAsRoot;
    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let result = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::EvalResult(eval_result) => {
                    let value_ref = eval_result.result().expect("Missing value");
                    let value_struct =
                        moor_var_schema::Var::try_from(value_ref).expect("Failed to convert value");
                    let result =
                        var_from_flatbuffer(&value_struct).expect("Failed to decode value");
                    Json(var_as_json(&result)).into_response()
                }
                moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted) => {
                    let task_id = task_submitted.task_id().expect("Missing task_id");
                    Json(json!({"task_submitted": task_id, "status": "success"})).into_response()
                }
                _ => {
                    error!("Unexpected response from RPC server");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        _ => {
            error!("RPC failure");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    };

    // Clean up the RPC connection
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client.make_client_rpc_call(client_id, detach_msg).await;

    result
}
