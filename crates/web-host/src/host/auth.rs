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
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_common::{
    AuthToken, ClientToken, flatbuffers_generated::moor_rpc, mk_detach_msg, mk_login_command_msg,
};
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
    let (client_id, mut rpc_client, client_token) =
        match host.establish_client_connection(addr).await {
            Ok((client_id, rpc_client, client_token)) => (client_id, rpc_client, client_token),
            Err(WsHostError::AuthenticationFailed) => {
                warn!("Authentication failed for {}", player);
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
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
    let login_msg = mk_login_command_msg(&client_token, &host.handler_object, words, false);

    let reply_bytes = rpc_client
        .make_client_rpc_call(client_id, login_msg)
        .await
        .expect("Unable to send login request to RPC server");

    use planus::ReadAsRoot;
    let reply =
        moor_rpc::ReplyResultRef::read_as_root(&reply_bytes).expect("Failed to parse reply");

    let (auth_token, player) = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::LoginResult(login_result) => {
                    if login_result.success().expect("Missing success") {
                        let auth_token_ref = login_result
                            .auth_token()
                            .expect("Missing auth_token")
                            .expect("Missing auth token");
                        let auth_token =
                            AuthToken(auth_token_ref.token().expect("Missing token").to_string());
                        let player_ref = login_result
                            .player()
                            .expect("Missing player")
                            .expect("Missing player obj");
                        let player_struct =
                            moor_rpc::Obj::try_from(player_ref).expect("Failed to convert player");
                        let player = rpc_common::obj_from_flatbuffer_struct(&player_struct)
                            .expect("Failed to decode player");
                        (auth_token, player)
                    } else {
                        error!("Login failed");
                        return Response::builder()
                            .status(StatusCode::UNAUTHORIZED)
                            .body("".to_string())
                            .unwrap();
                    }
                }
                _ => {
                    error!("Unexpected reply type");
                    return Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body("".to_string())
                        .unwrap();
                }
            }
        }
        _ => {
            error!("Login failed");
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body("".to_string())
                .unwrap();
        }
    };

    // We now have a valid auth token for the player, so we return it in the response headers.
    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Moor-Auth-Token",
        HeaderValue::from_str(&auth_token.0).expect("Invalid token"),
    );

    // We now need to wait for the login message completion.

    // We're done with this RPC connection, so we detach it.
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client
        .make_client_rpc_call(client_id, detach_msg)
        .await
        .expect("Unable to send detach to RPC server");

    Response::builder()
        .status(StatusCode::OK)
        .header("X-Moor-Auth-Token", auth_token.0)
        .body(format!("{player} {auth_verb}"))
        .unwrap()
}

pub async fn auth_auth(
    host: WebHost,
    addr: SocketAddr,
    header_map: HeaderMap,
) -> Result<(AuthToken, Uuid, ClientToken, RpcSendClient), StatusCode> {
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
