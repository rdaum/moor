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

//! HTTP handlers for OAuth2 authentication endpoints

use crate::host::{WebHost, oauth2::OAuth2Manager};
use axum::{
    Json,
    extract::{ConnectInfo, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use moor_schema::rpc as moor_rpc;
use rpc_common::mk_login_command_msg;
use serde_derive::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tracing::{debug, error, info, warn};

/// Shared state for OAuth2 handlers
#[derive(Clone)]
pub struct OAuth2State {
    pub manager: Arc<OAuth2Manager>,
    pub web_host: WebHost,
}

/// Response for authorization URL request
#[derive(Serialize)]
pub struct AuthUrlResponse {
    pub auth_url: String,
    pub state: String,
}

/// Response for OAuth2 configuration
#[derive(Serialize)]
pub struct OAuth2ConfigResponse {
    pub enabled: bool,
    pub providers: Vec<String>,
}

/// Query parameters for OAuth2 callback
#[derive(Deserialize)]
pub struct OAuth2CallbackQuery {
    pub code: String,
    pub state: String,
}

/// Response for successful OAuth2 login
#[derive(Serialize)]
pub struct OAuth2LoginResponse {
    pub success: bool,
    pub auth_token: Option<String>,
    pub player: Option<String>,
    pub error: Option<String>,
}

/// Request body for account choice submission
#[derive(Deserialize)]
pub struct AccountChoiceRequest {
    pub mode: String, // "oauth2_create" or "oauth2_connect"
    pub provider: String,
    pub external_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub username: Option<String>,
    pub player_name: Option<String>,       // For oauth2_create
    pub existing_email: Option<String>,    // For oauth2_connect
    pub existing_password: Option<String>, // For oauth2_connect
}

/// GET /auth/oauth2/:provider/authorize
/// Generate and return OAuth2 authorization URL for the specified provider
pub async fn oauth2_authorize_handler(
    State(oauth2_state): State<OAuth2State>,
    Path(provider): Path<String>,
) -> impl IntoResponse {
    debug!("OAuth2 authorization request for provider: {}", provider);

    if !oauth2_state.manager.is_enabled() {
        warn!("OAuth2 is not enabled");
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "OAuth2 not enabled"})),
        )
            .into_response();
    }

    match oauth2_state.manager.get_authorization_url(&provider) {
        Ok((auth_url, csrf_token)) => {
            info!("Generated OAuth2 authorization URL for {}", provider);
            Json(AuthUrlResponse {
                auth_url,
                state: csrf_token.secret().clone(),
            })
            .into_response()
        }
        Err(e) => {
            error!("Failed to generate authorization URL: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid provider: {}", e)})),
            )
                .into_response()
        }
    }
}

/// GET /auth/oauth2/:provider/callback
/// Handle OAuth2 provider callback with authorization code
pub async fn oauth2_callback_handler(
    State(oauth2_state): State<OAuth2State>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(provider): Path<String>,
    Query(query): Query<OAuth2CallbackQuery>,
) -> impl IntoResponse {
    debug!("OAuth2 callback from provider: {} with code", provider);

    if !oauth2_state.manager.is_enabled() {
        warn!("OAuth2 is not enabled");
        return Redirect::to("/?error=oauth2_disabled").into_response();
    }

    // Complete OAuth2 flow: exchange code and get user info
    let user_info = match oauth2_state
        .manager
        .complete_oauth2_flow(&provider, query.code)
        .await
    {
        Ok(info) => info,
        Err(e) => {
            error!("OAuth2 flow failed: {}", e);
            return Redirect::to(&format!("/?error=oauth2_failed&details={}", e)).into_response();
        }
    };

    info!(
        "OAuth2 flow completed for provider {}, external_id: {}",
        provider, user_info.external_id
    );

    // Check if this OAuth2 identity already exists in the system
    // We do this by calling the daemon with oauth2_check mode
    let (client_id, mut rpc_client, client_token) = match oauth2_state
        .web_host
        .establish_client_connection(addr)
        .await
    {
        Ok(connection) => connection,
        Err(e) => {
            error!("Failed to establish RPC connection: {}", e);
            return Redirect::to("/?error=internal_error").into_response();
        }
    };

    let check_args = vec![
        "oauth2_check".to_string(),
        provider.clone(),
        user_info.external_id.clone(),
    ];

    let login_msg = mk_login_command_msg(
        &client_token,
        &oauth2_state.web_host.handler_object,
        check_args,
        false, // just checking, not attaching yet
    );

    let reply_bytes = match rpc_client.make_client_rpc_call(client_id, login_msg).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("RPC call failed: {}", e);
            return Redirect::to("/?error=internal_error").into_response();
        }
    };

    use planus::ReadAsRoot;
    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return Redirect::to("/?error=internal_error").into_response();
        }
    };

    // Check if user exists
    let Ok(result) = reply.result() else {
        error!("Missing result in reply");
        return Redirect::to("/?error=internal_error").into_response();
    };

    let moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) = result else {
        error!("Login check failed");
        return Redirect::to("/?error=login_check_failed").into_response();
    };

    let Ok(daemon_reply) = client_success.reply() else {
        error!("Missing daemon reply");
        return Redirect::to("/?error=internal_error").into_response();
    };

    let Ok(reply_union) = daemon_reply.reply() else {
        error!("Missing reply union");
        return Redirect::to("/?error=internal_error").into_response();
    };

    let moor_rpc::DaemonToClientReplyUnionRef::LoginResult(login_result) = reply_union else {
        error!("Unexpected reply type from daemon");
        return Redirect::to("/?error=unexpected_reply").into_response();
    };

    let Ok(success) = login_result.success() else {
        error!("Missing success field in login result");
        return Redirect::to("/?error=internal_error").into_response();
    };

    if success {
        // Existing user - extract auth token and player OID, then redirect
        let Ok(Some(token_ref)) = login_result.auth_token() else {
            error!("Missing auth_token in login result");
            return Redirect::to("/?error=internal_error").into_response();
        };

        let Ok(auth_token) = token_ref.token() else {
            error!("Missing token string");
            return Redirect::to("/?error=internal_error").into_response();
        };

        let Ok(Some(player_ref)) = login_result.player() else {
            error!("Missing player in login result");
            return Redirect::to("/?error=internal_error").into_response();
        };

        let Ok(player_struct) = moor_rpc::Obj::try_from(player_ref) else {
            error!("Failed to convert player");
            return Redirect::to("/?error=internal_error").into_response();
        };

        let Ok(player_obj) = moor_schema::convert::obj_from_flatbuffer_struct(&player_struct)
        else {
            error!("Failed to decode player");
            return Redirect::to("/?error=internal_error").into_response();
        };

        info!("Existing OAuth2 user logged in: {}", player_obj);
        // URL-encode the player OID to handle # character
        let player_oid_str = player_obj.to_string();
        let player_oid_encoded = urlencoding::encode(&player_oid_str);
        let redirect_url = format!("/?auth_token={}&player={}", auth_token, player_oid_encoded);
        Redirect::to(&redirect_url).into_response()
    } else {
        // New user - redirect to account choice page with user info
        let user_info_json = serde_json::to_string(&user_info).unwrap_or_default();
        let encoded_info = urlencoding::encode(&user_info_json);
        Redirect::to(&format!(
            "/?oauth2_user_info={}&state={}",
            encoded_info, query.state
        ))
        .into_response()
    }
}

/// POST /auth/oauth2/account
/// Handle account choice submission (create new or link existing)
pub async fn oauth2_account_choice_handler(
    State(oauth2_state): State<OAuth2State>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(choice): Json<AccountChoiceRequest>,
) -> impl IntoResponse {
    debug!(
        "OAuth2 account choice: mode={}, provider={}",
        choice.mode, choice.provider
    );

    if !oauth2_state.manager.is_enabled() {
        warn!("OAuth2 is not enabled");
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "OAuth2 not enabled"})),
        )
            .into_response();
    }

    // Validate mode
    if choice.mode != "oauth2_create" && choice.mode != "oauth2_connect" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid mode, must be oauth2_create or oauth2_connect"})),
        ).into_response();
    }

    // Build arguments for LoginCommand based on mode
    let final_args = if choice.mode == "oauth2_create" {
        vec![
            choice.mode.clone(),
            choice.provider.clone(),
            choice.external_id.clone(),
            choice.email.clone().unwrap_or_default(),
            choice.name.clone().unwrap_or_default(),
            choice.username.clone().unwrap_or_default(),
            choice.player_name.clone().unwrap_or_default(),
        ]
    } else {
        // oauth2_connect
        vec![
            choice.mode.clone(),
            choice.provider.clone(),
            choice.external_id.clone(),
            choice.email.clone().unwrap_or_default(),
            choice.name.clone().unwrap_or_default(),
            choice.username.clone().unwrap_or_default(),
            choice.existing_email.clone().unwrap_or_default(),
            choice.existing_password.clone().unwrap_or_default(),
        ]
    };

    // Establish RPC connection
    let (client_id, mut rpc_client, client_token) = match oauth2_state
        .web_host
        .establish_client_connection(addr)
        .await
    {
        Ok(connection) => connection,
        Err(e) => {
            error!("Failed to establish RPC connection: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to establish connection"})),
            )
                .into_response();
        }
    };

    // Call daemon with complete user info
    let login_msg = mk_login_command_msg(
        &client_token,
        &oauth2_state.web_host.handler_object,
        final_args,
        true, // do_attach
    );

    let reply_bytes = match rpc_client.make_client_rpc_call(client_id, login_msg).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("RPC call failed: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "RPC call failed"})),
            )
                .into_response();
        }
    };

    use planus::ReadAsRoot;
    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to parse reply"})),
            )
                .into_response();
        }
    };

    // Return login result
    let Ok(result) = reply.result() else {
        error!("Missing result in reply");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Internal error"})),
        )
            .into_response();
    };

    let moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) = result else {
        error!("Account choice failed");
        return (
            StatusCode::UNAUTHORIZED,
            Json(OAuth2LoginResponse {
                success: false,
                auth_token: None,
                player: None,
                error: Some("Authentication failed".to_string()),
            }),
        )
            .into_response();
    };

    let Ok(daemon_reply) = client_success.reply() else {
        error!("Missing daemon reply");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Internal error"})),
        )
            .into_response();
    };

    let Ok(reply_union) = daemon_reply.reply() else {
        error!("Missing reply union");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Internal error"})),
        )
            .into_response();
    };

    let moor_rpc::DaemonToClientReplyUnionRef::LoginResult(login_result) = reply_union else {
        error!("Unexpected reply type from daemon");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Unexpected reply type"})),
        )
            .into_response();
    };

    let Ok(success) = login_result.success() else {
        error!("Missing success field in login result");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Internal error"})),
        )
            .into_response();
    };

    if success {
        let Ok(Some(token_ref)) = login_result.auth_token() else {
            error!("Missing auth_token in login result");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"})),
            )
                .into_response();
        };

        let Ok(auth_token) = token_ref.token() else {
            error!("Missing token string");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"})),
            )
                .into_response();
        };

        let Ok(Some(player_ref)) = login_result.player() else {
            error!("Missing player in login result");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"})),
            )
                .into_response();
        };

        let Ok(player_struct) = moor_rpc::Obj::try_from(player_ref) else {
            error!("Failed to convert player");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"})),
            )
                .into_response();
        };

        let Ok(player_obj) = moor_schema::convert::obj_from_flatbuffer_struct(&player_struct)
        else {
            error!("Failed to decode player");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"})),
            )
                .into_response();
        };

        info!("OAuth2 account {} successful", choice.mode);

        // Return JSON with auth token and player OID
        Json(OAuth2LoginResponse {
            success: true,
            auth_token: Some(auth_token.to_string()),
            player: Some(player_obj.to_string()),
            error: None,
        })
        .into_response()
    } else {
        // Login failed
        warn!("OAuth2 account {} failed", choice.mode);
        (
            StatusCode::UNAUTHORIZED,
            Json(OAuth2LoginResponse {
                success: false,
                auth_token: None,
                player: None,
                error: Some("Authentication failed".to_string()),
            }),
        )
            .into_response()
    }
}

/// GET /api/oauth2/config
/// Return OAuth2 configuration including enabled status and available providers
pub async fn oauth2_config_handler(State(oauth2_state): State<OAuth2State>) -> impl IntoResponse {
    debug!("OAuth2 config request");

    let enabled = oauth2_state.manager.is_enabled();
    let providers = if enabled {
        oauth2_state.manager.available_providers()
    } else {
        Vec::new()
    };

    Json(OAuth2ConfigResponse { enabled, providers }).into_response()
}
