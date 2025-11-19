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

use crate::host::{
    WebHost,
    web_host::{LoginType, WsHostError},
};
use axum::{
    Form,
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use moor_schema::rpc as moor_rpc;
use rpc_async_client::rpc_client::RpcClient;
use rpc_common::{AuthToken, ClientToken, mk_detach_msg, mk_login_command_msg};
use serde_derive::Deserialize;
use std::net::SocketAddr;
use tracing::{debug, error, warn};
use uuid::Uuid;

pub fn extract_auth_token_header(header_map: &HeaderMap) -> Result<AuthToken, StatusCode> {
    header_map
        .get("X-Moor-Auth-Token")
        .and_then(|value| value.to_str().ok())
        .map(|token| AuthToken(token.to_string()))
        .ok_or(StatusCode::FORBIDDEN)
}

pub fn extract_client_credentials(header_map: &HeaderMap) -> Option<(Uuid, ClientToken)> {
    let client_token = header_map
        .get("X-Moor-Client-Token")
        .and_then(|value| value.to_str().ok())
        .map(|token| ClientToken(token.to_string()))?;
    let client_id = header_map
        .get("X-Moor-Client-Id")
        .and_then(|value| value.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())?;
    Some((client_id, client_token))
}

#[derive(Deserialize)]
pub struct AuthRequest {
    player: String,
    password: String,
}

pub async fn connect_auth_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    Form(AuthRequest { player, password }): Form<AuthRequest>,
) -> impl IntoResponse {
    auth_handler(LoginType::Connect, addr, ws_host, player, password).await
}

pub async fn create_auth_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    Form(AuthRequest { player, password }): Form<AuthRequest>,
) -> impl IntoResponse {
    auth_handler(LoginType::Create, addr, ws_host, player, password).await
}

/// Stand-alone HTTP POST authentication handler which connects and then gets a valid authentication token
/// which can then be used in the headers/query-string for subsequent websocket request.
async fn auth_handler(
    login_type: LoginType,
    addr: SocketAddr,
    host: WebHost,
    player: String,
    password: String,
) -> impl IntoResponse {
    debug!("Authenticating player: {}", player);
    let (client_id, rpc_client, client_token) = match host.establish_client_connection(addr).await {
        Ok((client_id, rpc_client, client_token)) => (client_id, rpc_client, client_token),
        Err(WsHostError::AuthenticationFailed) => {
            warn!("Authentication failed for {}", player);
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Body::empty())
                .unwrap();
        }
        Err(e) => {
            error!("Unable to establish connection: {}", e);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap();
        }
    };

    let auth_verb = match login_type {
        LoginType::Connect => "connect",
        LoginType::Create => "create",
    };

    let words = vec![auth_verb.to_string(), player, password];
    let login_msg = mk_login_command_msg(&client_token, &host.handler_object, words, false);

    let reply_bytes = rpc_client
        .make_client_rpc_call(client_id, login_msg)
        .await
        .expect("Unable to send login request to RPC server");

    use planus::ReadAsRoot;
    let reply =
        moor_rpc::ReplyResultRef::read_as_root(&reply_bytes).expect("Failed to parse reply");

    // Check if login was successful and extract auth_token
    let auth_token = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::LoginResult(login_result) => {
                    if login_result.success().expect("Missing success") {
                        let auth_token_ref = login_result
                            .auth_token()
                            .expect("Missing auth_token")
                            .expect("Missing auth token");
                        AuthToken(auth_token_ref.token().expect("Missing token").to_string())
                    } else {
                        error!("Login failed");
                        return Response::builder()
                            .status(StatusCode::UNAUTHORIZED)
                            .body(Body::empty())
                            .unwrap();
                    }
                }
                _ => {
                    error!("Unexpected reply type");
                    return Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap();
                }
            }
        }
        _ => {
            error!("Login failed");
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap();
        }
    };

    // Don't detach - keep the connection open so the WebSocket can reattach to it.
    // The connection will be cleaned up when the user logs out or the session times out.

    // Return the entire ReplyResult FlatBuffer which includes player_flags
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .header("X-Moor-Auth-Token", auth_token.0)
        .header("X-Moor-Client-Token", client_token.0.clone())
        .header("X-Moor-Client-Id", client_id.to_string())
        .body(Body::from(reply_bytes))
        .unwrap()
}

pub async fn auth_auth(
    host: WebHost,
    addr: SocketAddr,
    header_map: HeaderMap,
) -> Result<(AuthToken, Uuid, ClientToken, RpcClient), StatusCode> {
    let auth_token = extract_auth_token_header(&header_map)?;

    if let Some((client_id, client_token)) = extract_client_credentials(&header_map) {
        match host
            .reattach_authenticated(auth_token.clone(), client_id, client_token.clone(), addr)
            .await
        {
            Ok((_player, _, confirmed_token, rpc_client)) => {
                return Ok((auth_token, client_id, confirmed_token, rpc_client));
            }
            Err(WsHostError::AuthenticationFailed) => {
                warn!(
                    client_id = ?client_id,
                    "Reattach failed, falling back to attach flow"
                );
            }
            Err(WsHostError::RpcError(e)) => {
                error!("Reattach RPC failure: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    host.attach_authenticated(
        auth_token.clone(),
        Some(moor_rpc::ConnectType::NoConnect),
        addr,
    )
    .await
    .map(|(_player, client_id, client_token, rpc_client)| {
        (auth_token, client_id, client_token, rpc_client)
    })
    .map_err(|e| match e {
        WsHostError::AuthenticationFailed => StatusCode::UNAUTHORIZED,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })
}

pub fn stateless_rpc_client(
    host: &WebHost,
    header_map: &HeaderMap,
) -> Result<(AuthToken, Uuid, RpcClient), StatusCode> {
    let auth_token = extract_auth_token_header(header_map)?;
    let (client_id, rpc_client) = host.new_stateless_client();
    Ok((auth_token, client_id, rpc_client))
}

/// Validate an auth token without establishing a full session
/// Also optionally validates stored client credentials if provided
pub async fn validate_auth_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(host): State<WebHost>,
    header_map: HeaderMap,
) -> impl IntoResponse {
    let auth_token = match extract_auth_token_header(&header_map) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };

    debug!("Validating auth token");

    // Client credentials are required - validate the stored session
    let (client_id, client_token) = match extract_client_credentials(&header_map) {
        Some(creds) => creds,
        None => {
            debug!("No client credentials provided - cannot validate");
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap();
        }
    };

    debug!("Validating stored session with client credentials");

    // Try to reattach with stored credentials to verify the session still exists
    match host
        .reattach_authenticated(auth_token, client_id, client_token, addr)
        .await
    {
        Ok(_) => {
            debug!("Stored session validated successfully");
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        }
        Err(WsHostError::AuthenticationFailed) => {
            debug!("Stored session invalid - connection no longer exists in daemon");
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap()
        }
        Err(e) => {
            error!("Error validating stored session: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        }
    }
}

/// Explicit logout endpoint that notifies daemon that player is disconnecting
pub async fn logout_handler(
    State(host): State<WebHost>,
    header_map: HeaderMap,
) -> impl IntoResponse {
    // Verify auth token is present (though we don't use it directly)
    let _auth_token = match extract_auth_token_header(&header_map) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };

    // Client credentials are required for logout
    let (client_id, client_token) = match extract_client_credentials(&header_map) {
        Some(creds) => creds,
        None => {
            debug!("No client credentials provided for logout");
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap()
                .into_response();
        }
    };

    debug!("Processing explicit logout for client: {}", client_id);

    // Send detach message with disconnected=true to trigger user_disconnected
    let detach_msg = mk_detach_msg(&client_token, true);
    let rpc_client = host.create_rpc_client();

    match rpc_client.make_client_rpc_call(client_id, detach_msg).await {
        Ok(_) => {
            debug!("Logout detach sent successfully");
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
                .into_response()
        }
        Err(e) => {
            error!("Failed to send logout detach: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
                .into_response()
        }
    }
}
