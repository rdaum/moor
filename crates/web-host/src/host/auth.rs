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

    // We're done with this RPC connection, so we detach it.
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client
        .make_client_rpc_call(client_id, detach_msg)
        .await
        .expect("Unable to send detach to RPC server");

    // Return the entire ReplyResult FlatBuffer which includes player_flags
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .header("X-Moor-Auth-Token", auth_token.0)
        .body(Body::from(reply_bytes))
        .unwrap()
}

pub async fn auth_auth(
    host: WebHost,
    addr: SocketAddr,
    header_map: HeaderMap,
) -> Result<(AuthToken, Uuid, ClientToken, RpcClient), StatusCode> {
    let auth_token = match header_map.get("X-Moor-Auth-Token") {
        Some(auth_token) => match auth_token.to_str() {
            Ok(auth_token) => AuthToken(auth_token.to_string()),
            Err(e) => {
                error!("Unable to parse auth token: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        },
        None => {
            error!("No auth token provided");
            return Err(StatusCode::FORBIDDEN);
        }
    };

    let (_player, client_id, client_token, rpc_client) = host
        .attach_authenticated(auth_token.clone(), None, addr)
        .await
        .map_err(|e| match e {
            WsHostError::AuthenticationFailed => StatusCode::UNAUTHORIZED,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    Ok((auth_token, client_id, client_token, rpc_client))
}
