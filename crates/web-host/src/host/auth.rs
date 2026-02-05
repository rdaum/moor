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
    extract::{ConnectInfo, FromRequestParts, State},
    http::{HeaderMap, StatusCode, request::Parts},
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
        .ok_or(StatusCode::UNAUTHORIZED)
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

/// Auth extractor for stateless (fire-and-forget) RPC calls.
/// No daemon-side connection state — just a token, a throwaway client_id, and an RPC client.
pub struct StatelessAuth {
    pub auth_token: AuthToken,
    pub client_id: Uuid,
    pub rpc_client: RpcClient,
}

impl FromRequestParts<WebHost> for StatelessAuth {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &WebHost,
    ) -> Result<Self, Self::Rejection> {
        let auth_token = extract_auth_token_header(&parts.headers)?;
        let (client_id, rpc_client) = state.new_stateless_client();
        Ok(StatelessAuth {
            auth_token,
            client_id,
            rpc_client,
        })
    }
}

/// RAII guard that detaches an ephemeral connection when dropped.
struct DetachGuard {
    client_id: Uuid,
    client_token: ClientToken,
    host: WebHost,
}

impl Drop for DetachGuard {
    fn drop(&mut self) {
        let rpc_client = self.host.create_rpc_client();
        let client_id = self.client_id;
        let client_token = self.client_token.clone();
        tokio::spawn(async move {
            let detach = mk_detach_msg(&client_token, true);
            let _ = rpc_client.make_client_rpc_call(client_id, detach).await;
        });
    }
}

/// Auth extractor for ephemeral (attach + detach) RPC calls.
/// Creates a real daemon connection that is automatically cleaned up when
/// the `EphemeralAuth` value is dropped (on handler return or early error).
pub struct EphemeralAuth {
    pub auth_token: AuthToken,
    pub client_id: Uuid,
    pub client_token: ClientToken,
    pub rpc_client: RpcClient,
    _guard: DetachGuard,
}

impl FromRequestParts<WebHost> for EphemeralAuth {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &WebHost,
    ) -> Result<Self, Self::Rejection> {
        let auth_token = extract_auth_token_header(&parts.headers)?;

        // Extract peer address from ConnectInfo stored in extensions.
        // Missing ConnectInfo means the router is misconfigured (no .into_make_service_with_connect_info).
        let addr = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0)
            .ok_or_else(|| {
                error!("ConnectInfo<SocketAddr> missing from request extensions");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let (_player, client_id, client_token, rpc_client) = state
            .attach_authenticated(
                auth_token.clone(),
                Some(moor_rpc::ConnectType::NoConnect),
                addr,
            )
            .await
            .map_err(|e| match e {
                WsHostError::AuthenticationFailed => StatusCode::UNAUTHORIZED,
                WsHostError::RpcError(_) => StatusCode::SERVICE_UNAVAILABLE,
            })?;

        let guard = DetachGuard {
            client_id,
            client_token: client_token.clone(),
            host: state.clone(),
        };

        Ok(EphemeralAuth {
            auth_token,
            client_id,
            client_token,
            rpc_client,
            _guard: guard,
        })
    }
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
        .header("Content-Type", "application/x-flatbuffers")
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

/// Validate an auth token by round-tripping to the daemon.
/// If `EphemeralAuth` extraction succeeds, the token is valid — return 200.
/// The `DetachGuard` handles cleanup automatically.
/// Returns:
///   200 — token is valid
///   401 — token is invalid or expired
///   503 — daemon is unreachable
pub async fn validate_auth_handler(_auth: EphemeralAuth) -> impl IntoResponse {
    debug!("Auth token validated via daemon");
    StatusCode::OK
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
