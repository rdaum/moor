use crate::host::ws_connection::WebSocketConnection;
use anyhow::anyhow;
use axum::body::{boxed, Empty};
use axum::extract::{ConnectInfo, Path, State, WebSocketUpgrade};
use axum::headers::HeaderValue;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use metrics_macros::increment_counter;
use moor_values::var::objid::Objid;
use rpc_common::rpc_client::RpcSendClient;
use rpc_common::AuthToken;
use rpc_common::RpcRequest::{Attach, ConnectionEstablish};
use rpc_common::{ClientToken, RpcRequestError};
use rpc_common::{ConnectType, RpcRequest, RpcResponse, RpcResult, BROADCAST_TOPIC};
use serde_derive::Deserialize;
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
pub struct WebSocketHost {
    zmq_context: tmq::Context,
    rpc_addr: String,
    pubsub_addr: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WsHostError {
    #[error("RPC request error: {0}")]
    RpcFailure(RpcRequestError),
    #[error("RPC system error: {0}")]
    RpcError(anyhow::Error),
    #[error("Authentication failed")]
    AuthenticationFailed,
}

impl WebSocketHost {
    pub fn new(rpc_addr: String, narrative_addr: String) -> Self {
        let tmq_context = tmq::Context::new();
        Self {
            zmq_context: tmq_context,
            rpc_addr,
            pubsub_addr: narrative_addr,
        }
    }
}

impl WebSocketHost {
    /// Contact the RPC server to validate an auth token, and return the object ID of the player
    /// and the client token and rpc client to use for the connection.
    pub async fn validate_auth(
        &self,
        auth_token: AuthToken,
        connect_type: ConnectType,
        peer_addr: SocketAddr,
    ) -> Result<(Objid, Uuid, ClientToken, RpcSendClient), WsHostError> {
        let zmq_ctx = self.zmq_context.clone();
        // Establish a connection to the RPC server
        let client_id = Uuid::new_v4();
        let rcp_request_sock = request(&zmq_ctx)
            .set_rcvtimeo(100)
            .set_sndtimeo(100)
            .connect(self.rpc_addr.as_str())
            .map_err(|e| WsHostError::RpcError(anyhow!(e)))?;

        // Establish a connection to the RPC server
        debug!(
            self.rpc_addr,
            "Contacting RPC server to establish connection"
        );
        let mut rpc_client = RpcSendClient::new(rcp_request_sock);

        let (client_token, player) = match rpc_client
            .make_rpc_call(
                client_id,
                Attach(auth_token, connect_type, peer_addr.to_string()),
            )
            .await
        {
            Ok(RpcResult::Success(RpcResponse::AttachResult(Some((client_token, player))))) => {
                info!("Connection authenticated, player: {}", player);
                (client_token, player)
            }
            Ok(RpcResult::Success(RpcResponse::AttachResult(None))) => {
                warn!("Connection authentication failed from {}", peer_addr);
                return Err(WsHostError::AuthenticationFailed);
            }
            Ok(RpcResult::Failure(f)) => {
                error!("RPC failure in connection establishment: {}", f);
                return Err(WsHostError::RpcFailure(f));
            }
            Ok(resp) => {
                return Err(WsHostError::RpcError(anyhow::anyhow!(
                    "Unexpected response from RPC server: {:?}",
                    resp
                )));
            }
            Err(e) => {
                return Err(WsHostError::RpcError(anyhow!(e)));
            }
        };

        Ok((player, client_id, client_token, rpc_client))
    }

    /// Actually instantiate the connection now that we've validated the auth token.
    pub async fn start_connection(
        &self,
        player: Objid,
        client_id: Uuid,
        client_token: ClientToken,
        auth_token: AuthToken,
        rpc_client: RpcSendClient,
        peer_addr: SocketAddr,
    ) -> Result<WebSocketConnection, anyhow::Error> {
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
            .subscribe(BROADCAST_TOPIC)
            .expect("Unable to subscribe to broadcast messages for client connection");

        info!(
            "Subscribed on pubsub socket for {:?}, socket addr {}",
            client_id, self.pubsub_addr
        );

        Ok(WebSocketConnection {
            player,
            peer_addr,
            broadcast_sub,
            narrative_sub,
            client_id,
            client_token,
            auth_token,
            rpc_client,
        })
    }
}

#[derive(Deserialize)]
pub struct AuthRequest {
    player: String,
    password: String,
}

pub async fn connect_auth_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebSocketHost>,
    Form(AuthRequest { player, password }): Form<AuthRequest>,
) -> impl IntoResponse {
    auth_handler(LoginType::Connect, addr, ws_host, player, password).await
}

pub async fn create_auth_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebSocketHost>,
    Form(AuthRequest { player, password }): Form<AuthRequest>,
) -> impl IntoResponse {
    auth_handler(LoginType::Create, addr, ws_host, player, password).await
}

/// Stand-alone HTTP POST authentication handler which connects and then gets a valid authentication token
/// which can then be used in the headers for subsequent websocket request.
async fn auth_handler(
    login_type: LoginType,
    addr: SocketAddr,
    ws_host: WebSocketHost,
    player: String,
    password: String,
) -> impl IntoResponse {
    increment_counter!("ws_host.auth");

    let zmq_ctx = ws_host.zmq_context.clone();
    let rcp_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(ws_host.rpc_addr.as_str())
        .expect("Unable to bind RPC server for connection");

    let client_id = Uuid::new_v4();
    let mut rpc_client = RpcSendClient::new(rcp_request_sock);

    let client_token = match rpc_client
        .make_rpc_call(client_id, ConnectionEstablish(addr.to_string()))
        .await
    {
        Ok(RpcResult::Success(RpcResponse::NewConnection(client_token, objid))) => {
            info!("Connection established, connection ID: {}", objid);
            client_token
        }
        Ok(RpcResult::Failure(f)) => {
            error!("RPC failure in connection establishment: {}", f);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("".to_string())
                .unwrap();
        }
        Ok(_) => {
            error!("Unexpected response from RPC server");
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("".to_string())
                .unwrap();
        }
        Err(e) => {
            error!("Unable to establish connection: {}", e);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("".to_string())
                .unwrap();
        }
    };

    let auth_verb = match login_type {
        LoginType::Connect => "connect",
        LoginType::Create => "create",
    };

    let words = vec![auth_verb.to_string(), player, password];
    let response = rpc_client
        .make_rpc_call(
            client_id,
            RpcRequest::LoginCommand(client_token.clone(), words, false),
        )
        .await
        .expect("Unable to send login request to RPC server");
    let RpcResult::Success(RpcResponse::LoginResult(Some((auth_token, _connect_type, player)))) =
        response
    else {
        error!(?response, "Login failed");

        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body("".to_string())
            .unwrap();
    };

    // We now have a valid auth token for the player, so we return it in the response headers.
    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Moor-Auth-Token",
        HeaderValue::from_str(&auth_token.0).expect("Invalid token"),
    );

    // We now need to wait for the login message completion.

    // We're done with this RPC connection, so we detach it.
    let _ = rpc_client
        .make_rpc_call(client_id, RpcRequest::Detach(client_token.clone()))
        .await
        .expect("Unable to send detach to RPC server");

    Response::builder()
        .status(StatusCode::OK)
        .header("X-Moor-Auth-Token", auth_token.0)
        .body(format!("{} {}", player, auth_verb))
        .unwrap()
}

async fn attach(
    ws: WebSocketUpgrade,
    addr: SocketAddr,
    connect_type: ConnectType,
    ws_host: &WebSocketHost,
    auth_token: String,
) -> impl IntoResponse {
    info!("Connection from {}", addr);

    let auth_token = AuthToken(auth_token);

    let (player, client_id, client_token, rpc_client) = match ws_host
        .validate_auth(auth_token.clone(), connect_type, addr)
        .await
    {
        Ok(connection_details) => connection_details,
        Err(WsHostError::AuthenticationFailed) => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(boxed(Empty::new()))
                .unwrap();
        }
        Err(e) => {
            error!("Unable to validate auth token: {}", e);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(boxed(Empty::new()))
                .unwrap();
        }
    };

    let Ok(mut connection) = ws_host
        .start_connection(
            player,
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
            .body(boxed(Empty::new()))
            .unwrap();
    };

    ws.on_upgrade(move |socket| async move { connection.handle(connect_type, socket).await })
}
pub async fn ws_connect_attach_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebSocketHost>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    increment_counter!("ws_host.ws_connect_attach_handler");
    info!("Connection from {}", addr);

    attach(ws, addr, ConnectType::Connected, &ws_host, token).await
}

pub async fn ws_create_attach_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebSocketHost>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    increment_counter!("ws_host.ws_create_attach_handler");
    info!("Connection from {}", addr);

    attach(ws, addr, ConnectType::Created, &ws_host, token).await
}
