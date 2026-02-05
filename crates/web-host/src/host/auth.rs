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
use rpc_common::{AuthToken, ClientToken, mk_detach_msg, mk_login_command_msg, read_reply_result};
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
    /// Optional event log public key for encryption (typically only for create)
    event_log_pubkey: Option<String>,
}

pub async fn connect_auth_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    Form(AuthRequest {
        player,
        password,
        event_log_pubkey,
    }): Form<AuthRequest>,
) -> impl IntoResponse {
    auth_handler(
        LoginType::Connect,
        addr,
        ws_host,
        player,
        password,
        event_log_pubkey,
    )
    .await
}

pub async fn create_auth_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_host): State<WebHost>,
    Form(AuthRequest {
        player,
        password,
        event_log_pubkey,
    }): Form<AuthRequest>,
) -> impl IntoResponse {
    auth_handler(
        LoginType::Create,
        addr,
        ws_host,
        player,
        password,
        event_log_pubkey,
    )
    .await
}

/// Stand-alone HTTP POST authentication handler which connects and then gets a valid authentication token
/// which can then be used in the headers/query-string for subsequent websocket request.
async fn auth_handler(
    login_type: LoginType,
    addr: SocketAddr,
    host: WebHost,
    player: String,
    password: String,
    event_log_pubkey: Option<String>,
) -> impl IntoResponse {
    debug!("Authenticating player: {}", player);
    let (client_id, rpc_client, client_token) = match host.establish_client_connection(addr).await {
        Ok((client_id, rpc_client, client_token)) => (client_id, rpc_client, client_token),
        Err(WsHostError::AuthenticationFailed) => {
            warn!("Authentication failed for {}", player);
            return StatusCode::FORBIDDEN.into_response();
        }
        Err(e) => {
            error!("Unable to establish connection: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let auth_verb = match login_type {
        LoginType::Connect => "connect",
        LoginType::Create => "create",
    };

    let words = vec![auth_verb.to_string(), player, password];
    let login_msg = mk_login_command_msg(
        &client_token,
        &host.handler_object,
        words,
        false,
        event_log_pubkey,
        None,
    );

    let reply_bytes = rpc_client.make_client_rpc_call(client_id, login_msg).await;
    let reply_bytes = match reply_bytes {
        Ok(reply_bytes) => reply_bytes,
        Err(e) => {
            error!("Unable to send login request to RPC server: {}", e);
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };

    let reply = match read_reply_result(&reply_bytes) {
        Ok(reply) => reply,
        Err(e) => {
            error!("Failed to parse login reply: {}", e);
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    // Check if login was successful and extract auth_token
    let auth_token = match reply.result() {
        Ok(moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success)) => {
            let daemon_reply = match client_success.reply().ok() {
                Some(reply) => reply,
                None => {
                    error!("Login response missing daemon reply");
                    return StatusCode::BAD_GATEWAY.into_response();
                }
            };
            match daemon_reply.reply() {
                Ok(moor_rpc::DaemonToClientReplyUnionRef::LoginResult(login_result)) => {
                    let success = match login_result.success() {
                        Ok(success) => success,
                        Err(e) => {
                            error!("LoginResult missing success flag: {}", e);
                            return StatusCode::BAD_GATEWAY.into_response();
                        }
                    };
                    if success {
                        let auth_token_ref = match login_result.auth_token().ok().flatten() {
                            Some(auth_token_ref) => auth_token_ref,
                            None => {
                                error!("LoginResult missing auth token");
                                return StatusCode::BAD_GATEWAY.into_response();
                            }
                        };
                        match auth_token_ref.token() {
                            Ok(token) => AuthToken(token.to_string()),
                            Err(e) => {
                                error!("LoginResult auth token missing token string: {}", e);
                                return StatusCode::BAD_GATEWAY.into_response();
                            }
                        }
                    } else {
                        error!("Login failed");
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                }
                Ok(_) => {
                    error!("Unexpected reply type");
                    return StatusCode::BAD_GATEWAY.into_response();
                }
                Err(e) => {
                    error!("Login response missing daemon reply union: {}", e);
                    return StatusCode::BAD_GATEWAY.into_response();
                }
            }
        }
        Ok(_) => {
            error!("Login failed");
            return StatusCode::UNAUTHORIZED.into_response();
        }
        Err(e) => {
            error!("Login response missing top-level result: {}", e);
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    // Keep the connection alive - WebSocket will reattach to it, preserving the connection ID
    // from :do_login_command. This is the user's "real" connection.
    match Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .header("X-Moor-Auth-Token", auth_token.0)
        .header("X-Moor-Client-Token", client_token.0.clone())
        .header("X-Moor-Client-Id", client_id.to_string())
        .body(Body::from(reply_bytes))
    {
        Ok(response) => response,
        Err(e) => {
            error!("Failed to build auth response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Authenticate an HTTP request and create an ephemeral connection.
/// Unlike WebSocket, HTTP calls don't need to reattach - they create fresh
/// ephemeral connections that are cleaned up after the request completes.
pub async fn auth_auth(
    host: WebHost,
    addr: SocketAddr,
    header_map: HeaderMap,
) -> Result<(AuthToken, Uuid, ClientToken, RpcClient), StatusCode> {
    let auth_token = extract_auth_token_header(&header_map)?;

    // HTTP calls always create fresh ephemeral connections.
    // Only WebSocket needs reattach to preserve connection ID from login.
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

/// Validate an auth token without establishing a full session.
/// This just checks that the auth token is present and syntactically valid.
/// The actual cryptographic validation happens when the websocket connects.
/// We don't require an existing connection - connections are cleaned up on soft detach,
/// but the auth token remains valid for reconnection.
pub async fn validate_auth_handler(
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    State(_host): State<WebHost>,
    header_map: HeaderMap,
) -> impl IntoResponse {
    // Just check that auth token is present and non-empty
    let auth_token = match extract_auth_token_header(&header_map) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };

    // Basic syntactic check - PASETO tokens start with "v4.public."
    if !auth_token.0.starts_with("v4.public.") {
        debug!("Auth token has invalid format");
        return StatusCode::UNAUTHORIZED.into_response();
    }

    debug!("Auth token validated (syntactic check passed)");
    StatusCode::OK.into_response()
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
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    debug!("Processing explicit logout for client: {}", client_id);

    // Send detach message with disconnected=true to trigger user_disconnected
    let detach_msg = mk_detach_msg(&client_token, true);
    let rpc_client = host.create_rpc_client();

    match rpc_client.make_client_rpc_call(client_id, detach_msg).await {
        Ok(_) => {
            debug!("Logout detach sent successfully");
            StatusCode::OK.into_response()
        }
        Err(e) => {
            error!("Failed to send logout detach: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
