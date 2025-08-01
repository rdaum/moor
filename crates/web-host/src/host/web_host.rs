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

use crate::host::ws_connection::WebSocketConnection;
use crate::host::{auth, var_as_json};
use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, Path, Query, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use eyre::eyre;

use moor_common::model::ObjectRef;
use moor_common::tasks::Event;
use moor_var::v_obj;
use moor_var::{E_INVIND, Obj, Symbol, v_err};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_common::AuthToken;
use rpc_common::HostClientToDaemonMessage::{Attach, ConnectionEstablish};
use rpc_common::{
    CLIENT_BROADCAST_TOPIC, ConnectType, DaemonToClientReply, HistoryRecall,
    HostClientToDaemonMessage, ReplyResult,
};
use rpc_common::{ClientToken, RpcMessageError};
use serde_derive::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use tmq::{request, subscribe};
use tracing::warn;
use tracing::{debug, error, info};
use uuid::Uuid;

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
}

#[derive(Debug, thiserror::Error)]
pub enum WsHostError {
    #[error("RPC request error: {0}")]
    RpcFailure(RpcMessageError),
    #[error("RPC system error: {0}")]
    RpcError(eyre::Error),
    #[error("Authentication failed")]
    AuthenticationFailed,
}

impl WebHost {
    pub fn new(rpc_addr: String, narrative_addr: String, handler_object: Obj) -> Self {
        let tmq_context = tmq::Context::new();
        Self {
            zmq_context: tmq_context,
            rpc_addr,
            pubsub_addr: narrative_addr,
            handler_object,
        }
    }
}

impl WebHost {
    /// Contact the RPC server to validate an auth token, and return the object ID of the player
    /// and the client token and rpc client to use for the connection.
    pub async fn attach_authenticated(
        &self,
        auth_token: AuthToken,
        connect_type: Option<ConnectType>,
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

        let (client_token, player) = match rpc_client
            .make_client_rpc_call(
                client_id,
                Attach {
                    auth_token,
                    connect_type,
                    handler_object: self.handler_object,
                    peer_addr: peer_addr.to_string(),
                    acceptable_content_types: Some(vec![
                        Symbol::mk("text_html"),
                        Symbol::mk("text_djot"),
                        Symbol::mk("text_plain"),
                    ]),
                },
            )
            .await
        {
            Ok(ReplyResult::ClientSuccess(DaemonToClientReply::AttachResult(Some((
                client_token,
                player,
            ))))) => {
                info!("Connection authenticated, player: {}", player);
                (client_token, player)
            }
            Ok(ReplyResult::ClientSuccess(DaemonToClientReply::AttachResult(None))) => {
                warn!("Connection authentication failed from {}", peer_addr);
                return Err(WsHostError::AuthenticationFailed);
            }
            Ok(ReplyResult::Failure(f)) => {
                error!("RPC failure in connection establishment: {}", f);
                return Err(WsHostError::RpcFailure(f));
            }
            Ok(resp) => {
                return Err(WsHostError::RpcError(eyre::eyre!(
                    "Unexpected response from RPC server: {:?}",
                    resp
                )));
            }
            Err(e) => {
                return Err(WsHostError::RpcError(eyre!(e)));
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

        let client_token = match rpc_client
            .make_client_rpc_call(
                client_id,
                ConnectionEstablish {
                    peer_addr: addr.to_string(),
                    acceptable_content_types: Some(vec![
                        Symbol::mk("text_plain"),
                        Symbol::mk("text_html"),
                        Symbol::mk("text_djot"),
                    ]),
                },
            )
            .await
        {
            Ok(ReplyResult::ClientSuccess(DaemonToClientReply::NewConnection(
                client_token,
                objid,
            ))) => {
                info!("Connection established, connection ID: {}", objid);
                client_token
            }
            Ok(ReplyResult::Failure(f)) => {
                error!("RPC failure in connection establishment: {}", f);
                return Err(WsHostError::RpcFailure(f));
            }
            Ok(ReplyResult::ClientSuccess(r)) => {
                error!("Unexpected response from RPC server");
                return Err(WsHostError::RpcError(eyre!(
                    "Unexpected response from RPC server: {:?}",
                    r
                )));
            }
            Err(e) => {
                error!("Unable to establish connection: {}", e);
                return Err(WsHostError::RpcError(eyre!(e)));
            }
            Ok(ReplyResult::HostSuccess(hs)) => {
                error!("Unexpected response from RPC server: {:?}", hs);
                return Err(WsHostError::RpcError(eyre!(
                    "Unexpected response from RPC server: {:?}",
                    hs
                )));
            }
        };

        Ok((client_id, rpc_client, client_token))
    }
}

pub(crate) async fn rpc_call(
    client_id: Uuid,
    rpc_client: &mut RpcSendClient,
    request: HostClientToDaemonMessage,
) -> Result<DaemonToClientReply, StatusCode> {
    match rpc_client.make_client_rpc_call(client_id, request).await {
        Ok(rpc_response) => match rpc_response {
            ReplyResult::ClientSuccess(r) => Ok(r),

            ReplyResult::Failure(RpcMessageError::PermissionDenied) => {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
            ReplyResult::Failure(f) => {
                error!("RPC failure in welcome message retrieval: {:?}", f);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
            ReplyResult::HostSuccess(hs) => {
                error!("Unexpected response from RPC server: {:?}", hs);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        },
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
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

    let response = match rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::RequestSysProp(
            client_token.clone(),
            ObjectRef::SysObj(obj_path),
            Symbol::mk(property_name),
        ),
    )
    .await
    {
        Ok(DaemonToClientReply::SysPropValue(Some(value))) => {
            Json(var_as_json(&value)).into_response()
        }
        Ok(DaemonToClientReply::SysPropValue(None)) => StatusCode::NOT_FOUND.into_response(),
        Ok(r) => {
            error!("Unexpected response from RPC server: {:?}", r);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(status) => status.into_response(),
    };

    // We're done with this RPC connection, so we detach it.
    let _ = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Detach(client_token.clone()),
        )
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

    let response = match rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::Eval(client_token.clone(), auth_token.clone(), expression),
    )
    .await
    {
        Ok(DaemonToClientReply::EvalResult(value)) => {
            debug!("Eval result: {:?}", value);
            Json(var_as_json(&value)).into_response()
        }
        Ok(r) => {
            error!("Unexpected response from RPC server: {:?}", r);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(status) => status.into_response(),
    };

    // We're done with this RPC connection, so we detach it.
    let _ = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Detach(client_token.clone()),
        )
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

    let response = match rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::Resolve(client_token.clone(), auth_token.clone(), objref),
    )
    .await
    {
        Ok(DaemonToClientReply::ResolveResult(obj)) => {
            if obj == v_err(E_INVIND) {
                StatusCode::NOT_FOUND.into_response()
            } else {
                Json(var_as_json(&obj)).into_response()
            }
        }
        Ok(r) => {
            error!("Unexpected response from RPC server: {:?}", r);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(status) => status.into_response(),
    };

    // We're done with this RPC connection, so we detach it.
    let _ = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Detach(client_token.clone()),
        )
        .await
        .expect("Unable to send detach to RPC server");

    response
}

/// Attach a websocket connection to an existing player.
async fn attach(
    ws: WebSocketUpgrade,
    addr: SocketAddr,
    connect_type: ConnectType,
    host: &WebHost,
    auth_token: String,
) -> impl IntoResponse + use<> {
    info!("Connection from {}", addr);

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

    attach(ws, addr, ConnectType::Connected, &ws_host, token).await
}

/// Websocket upgrade handler for authenticated users who are connecting to a new user
pub async fn ws_create_attach_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    Path(token): Path<String>,
) -> impl IntoResponse + use<> {
    info!("Connection from {}", addr);

    attach(ws, addr, ConnectType::Created, &ws_host, token).await
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

    let history_recall = if let Some(since_seconds) = query.since_seconds {
        HistoryRecall::SinceSeconds(since_seconds, query.limit)
    } else if let Some(since_event_str) = query.since_event {
        match Uuid::parse_str(&since_event_str) {
            Ok(uuid) => HistoryRecall::SinceEvent(uuid, query.limit),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    } else if let Some(until_event_str) = query.until_event {
        match Uuid::parse_str(&until_event_str) {
            Ok(uuid) => HistoryRecall::UntilEvent(uuid, query.limit),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    } else {
        HistoryRecall::None
    };

    let response = match rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::RequestHistory(
            client_token.clone(),
            auth_token.clone(),
            history_recall,
        ),
    )
    .await
    {
        Ok(DaemonToClientReply::HistoryResponse(history)) => Json(json!({
            "events": history.events.iter().map(|e| {
                json!({
                    "event_id": e.event.event_id(),
                    "author": var_as_json(e.event.author()),
                    "message": match e.event.event() {
                        Event::Notify(msg, content_type) => json!({
                            "type": "notify",
                            "content": var_as_json(&msg),
                            "content_type": content_type
                        }),
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
                    "timestamp": e.event.timestamp(),
                    "is_historical": e.is_historical,
                    "player": var_as_json(&v_obj(e.player))
                })
            }).collect::<Vec<_>>(),
            "meta": {
                "total_events": history.total_events,
                "time_range": history.time_range,
                "has_more_before": history.has_more_before,
                "earliest_event_id": history.earliest_event_id,
                "latest_event_id": history.latest_event_id
            }
        })),
        Ok(other) => {
            error!("Unexpected daemon response: {:?}", other);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(e) => {
            error!("RPC error getting history: {:?}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // We're done with this RPC connection, so we detach it.
    let _ = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Detach(client_token.clone()),
        )
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

    let response = match rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::RequestCurrentPresentations(
            client_token.clone(),
            auth_token.clone(),
        ),
    )
    .await
    {
        Ok(DaemonToClientReply::CurrentPresentations(presentations)) => Json(json!({
            "presentations": presentations
        })),
        Ok(other) => {
            error!("Unexpected daemon response: {:?}", other);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(e) => {
            error!("RPC error getting presentations: {:?}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // We're done with this RPC connection, so we detach it.
    let _ = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Detach(client_token.clone()),
        )
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

    let response = match rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::DismissPresentation(
            client_token.clone(),
            auth_token.clone(),
            presentation_id.clone(),
        ),
    )
    .await
    {
        Ok(DaemonToClientReply::PresentationDismissed) => Json(json!({
            "dismissed": true,
            "presentation_id": presentation_id
        })),
        Ok(other) => {
            error!("Unexpected daemon response: {:?}", other);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(e) => {
            error!("RPC error dismissing presentation: {:?}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // We're done with this RPC connection, so we detach it.
    let _ = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Detach(client_token.clone()),
        )
        .await
        .expect("Unable to send detach to RPC server");

    response.into_response()
}
