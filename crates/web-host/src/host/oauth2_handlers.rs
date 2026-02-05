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

//! HTTP handlers for OAuth2 authentication endpoints

use crate::host::{
    WebHost,
    oauth2::{OAuth2Manager, PendingOAuth2Code, PendingOAuth2Store},
};
use axum::{
    Json,
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use moor_common::model::ObjectRef;
use moor_schema::rpc as moor_rpc;
use rpc_common::{mk_login_command_msg, read_reply_result};
use serde_derive::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tracing::{debug, error, info, warn};

const OAUTH2_NONCE_COOKIE: &str = "moor_oauth_nonce";
const OAUTH2_NONCE_COOKIE_MAX_AGE: u64 = 600;

fn extract_cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some((cookie_name, value)) = trimmed.split_once('=')
            && cookie_name == name
        {
            return Some(value.to_string());
        }
    }
    None
}

fn make_nonce_cookie_value(nonce: &str, max_age_seconds: u64, secure: bool) -> String {
    let mut value = format!(
        "{}={}; Max-Age={}; Path=/; HttpOnly; SameSite=Lax",
        OAUTH2_NONCE_COOKIE, nonce, max_age_seconds
    );
    if secure {
        value.push_str("; Secure");
    }
    value
}

fn attach_set_cookie(mut response: Response, cookie: &str) -> Response {
    if let Ok(cookie_value) = cookie.parse() {
        response
            .headers_mut()
            .append(header::SET_COOKIE, cookie_value);
    }
    response
}

/// Shared state for OAuth2 handlers
#[derive(Clone)]
pub struct OAuth2State {
    pub manager: Arc<OAuth2Manager>,
    pub web_host: WebHost,
    pub pending: Arc<PendingOAuth2Store>,
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
    pub player_flags: Option<u16>,
    pub client_token: Option<String>,
    pub client_id: Option<String>,
    pub error: Option<String>,
}

/// Request body for code exchange (both existing-user auth and new-user identity)
#[derive(Deserialize)]
pub struct CodeExchangeRequest {
    pub code: String,
}

/// Request body for account choice submission.
/// The `oauth2_code` is a one-time server-side code that resolves to the verified identity.
#[derive(Deserialize)]
pub struct AccountChoiceRequest {
    pub mode: String,       // "oauth2_create" or "oauth2_connect"
    pub oauth2_code: String, // One-time code from callback redirect
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
            let state = csrf_token.secret().clone();
            let browser_nonce = uuid::Uuid::new_v4().to_string();
            oauth2_state
                .pending
                .store_csrf_token(&provider, &state, browser_nonce.clone());
            info!("Generated OAuth2 authorization URL for {}", provider);
            let response = Json(AuthUrlResponse { auth_url, state }).into_response();
            attach_set_cookie(
                response,
                &make_nonce_cookie_value(
                    &browser_nonce,
                    OAUTH2_NONCE_COOKIE_MAX_AGE,
                    oauth2_state.manager.oauth_cookie_secure(),
                ),
            )
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
    headers: HeaderMap,
) -> impl IntoResponse {
    debug!("OAuth2 callback from provider: {} with code", provider);

    if !oauth2_state.manager.is_enabled() {
        warn!("OAuth2 is not enabled");
        return Redirect::to("/?error=oauth2_disabled").into_response();
    }

    let Some(browser_nonce) = extract_cookie_value(&headers, OAUTH2_NONCE_COOKIE) else {
        warn!("Missing OAuth2 browser nonce cookie in callback");
        return Redirect::to("/?error=invalid_state").into_response();
    };

    // Validate CSRF state token (bound to provider)
    if !oauth2_state
        .pending
        .validate_csrf_token(&provider, &query.state, &browser_nonce)
    {
        warn!("Invalid or expired CSRF state token in OAuth2 callback");
        return Redirect::to("/?error=invalid_state").into_response();
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
            return Redirect::to(&format!("/?error=oauth2_failed&details={e}")).into_response();
        }
    };

    info!(
        "OAuth2 flow completed for provider {}, external_id: {}",
        provider, user_info.external_id
    );

    // Check if this OAuth2 identity already exists in the system
    let (client_id, rpc_client, client_token) = match oauth2_state
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
        None,
        None,
    );

    let reply_bytes = match rpc_client.make_client_rpc_call(client_id, login_msg).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("RPC call failed: {}", e);
            return Redirect::to("/?error=internal_error").into_response();
        }
    };

    let reply = match read_reply_result(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return Redirect::to("/?error=internal_error").into_response();
        }
    };

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
        // Existing user — store auth session server-side, redirect with one-time code
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

        let Ok(player_obj) = moor_schema::convert::obj_from_ref(player_ref) else {
            error!("Failed to decode player");
            return Redirect::to("/?error=internal_error").into_response();
        };

        let Ok(player_flags) = login_result.player_flags() else {
            error!("Missing player_flags in login result");
            return Redirect::to("/?error=internal_error").into_response();
        };

        info!(
            "Existing OAuth2 user logged in: {} (flags: {})",
            player_obj, player_flags
        );

        let player_curie = ObjectRef::Id(player_obj).to_curie();
        let pending = PendingOAuth2Code::AuthSession {
            auth_token: rpc_common::AuthToken(auth_token.to_string()),
            player_curie,
            player_flags,
            client_token,
            client_id,
        };
        let Some(code) = oauth2_state
            .pending
            .store_pending_code(pending, browser_nonce.clone())
        else {
            error!("Failed to store pending auth code");
            return Redirect::to("/?error=internal_error").into_response();
        };
        Redirect::to(&format!("/#auth_code={}", code)).into_response()
    } else {
        // New user — store verified identity server-side, redirect with one-time code + display hints
        let display_info = serde_json::json!({
            "email": user_info.email,
            "name": user_info.name,
            "username": user_info.username,
            "provider": user_info.provider,
        });
        let pending = PendingOAuth2Code::Identity(user_info);
        let Some(code) = oauth2_state
            .pending
            .store_pending_code(pending, browser_nonce)
        else {
            error!("Failed to store pending identity code");
            return Redirect::to("/?error=internal_error").into_response();
        };
        let display_str = display_info.to_string();
        let redirect_url = format!(
            "/#oauth2_code={}&oauth2_display={}",
            code,
            urlencoding::encode(&display_str),
        );
        Redirect::to(&redirect_url).into_response()
    }
}

/// POST /auth/oauth2/exchange
/// Exchange a one-time code for auth tokens (existing user) or identity info (new user).
/// Both code types are stored server-side and consumed on use.
pub async fn oauth2_exchange_handler(
    State(oauth2_state): State<OAuth2State>,
    headers: HeaderMap,
    Json(request): Json<CodeExchangeRequest>,
) -> impl IntoResponse {
    let Some(browser_nonce) = extract_cookie_value(&headers, OAUTH2_NONCE_COOKIE) else {
        warn!("Missing OAuth2 browser nonce cookie in exchange request");
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Missing OAuth2 browser nonce"})),
        )
            .into_response();
    };

    let payload = match oauth2_state
        .pending
        .redeem_pending_code(&request.code, &browser_nonce)
    {
        Some(payload) => payload,
        None => {
            warn!("Invalid or expired code in exchange request");
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Invalid or expired code"})),
            )
                .into_response();
        }
    };

    match payload {
        PendingOAuth2Code::AuthSession {
            auth_token,
            player_curie,
            player_flags,
            client_token,
            client_id,
        } => Json(serde_json::json!({
            "type": "auth_session",
            "auth_token": auth_token.0,
            "player": player_curie,
            "player_flags": player_flags,
            "client_token": client_token.0,
            "client_id": client_id.to_string(),
        }))
        .into_response(),

        PendingOAuth2Code::Identity(user_info) => Json(serde_json::json!({
            "type": "identity",
            "provider": user_info.provider,
            "email": user_info.email,
            "name": user_info.name,
            "username": user_info.username,
        }))
        .into_response(),
    }
}

/// POST /auth/oauth2/account
/// Handle account choice submission (create new or link existing).
/// The `oauth2_code` in the request is a one-time server-side code that resolves
/// to the verified provider identity. This prevents client-side tampering with
/// external_id or other identity fields.
pub async fn oauth2_account_choice_handler(
    State(oauth2_state): State<OAuth2State>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(choice): Json<AccountChoiceRequest>,
) -> impl IntoResponse {
    debug!("OAuth2 account choice: mode={}", choice.mode);

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

    let Some(browser_nonce) = extract_cookie_value(&headers, OAUTH2_NONCE_COOKIE) else {
        warn!("Missing OAuth2 browser nonce cookie in account choice");
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Missing OAuth2 browser nonce"})),
        )
            .into_response();
    };

    // Redeem the one-time code — must resolve to an Identity variant
    let user_info = match oauth2_state
        .pending
        .redeem_pending_code(&choice.oauth2_code, &browser_nonce)
    {
        Some(PendingOAuth2Code::Identity(info)) => info,
        Some(_) => {
            warn!("Code resolved to wrong type (expected identity)");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid code type for account choice"})),
            )
                .into_response();
        }
        None => {
            warn!("Invalid or expired OAuth2 code in account choice");
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Invalid or expired OAuth2 code"})),
            )
                .into_response();
        }
    };

    info!(
        "Verified OAuth2 identity: provider={}, external_id={}",
        user_info.provider, user_info.external_id
    );

    // Build arguments for LoginCommand based on mode, using server-verified identity fields
    let final_args = if choice.mode == "oauth2_create" {
        vec![
            choice.mode.clone(),
            user_info.provider,
            user_info.external_id,
            user_info.email.unwrap_or_default(),
            user_info.name.unwrap_or_default(),
            user_info.username.unwrap_or_default(),
            choice.player_name.clone().unwrap_or_default(),
        ]
    } else {
        // oauth2_connect
        vec![
            choice.mode.clone(),
            user_info.provider,
            user_info.external_id,
            user_info.email.unwrap_or_default(),
            user_info.name.unwrap_or_default(),
            user_info.username.unwrap_or_default(),
            choice.existing_email.clone().unwrap_or_default(),
            choice.existing_password.clone().unwrap_or_default(),
        ]
    };

    // Establish RPC connection
    let (client_id, rpc_client, client_token) = match oauth2_state
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

    let login_msg = mk_login_command_msg(
        &client_token,
        &oauth2_state.web_host.handler_object,
        final_args,
        true, // do_attach
        None,
        None,
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

    let reply = match read_reply_result(&reply_bytes) {
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
                player_flags: None,
                client_token: None,
                client_id: None,
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

        let Ok(player_obj) = moor_schema::convert::obj_from_ref(player_ref) else {
            error!("Failed to decode player");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"})),
            )
                .into_response();
        };

        let Ok(player_flags) = login_result.player_flags() else {
            error!("Missing player_flags in login result");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"})),
            )
                .into_response();
        };

        info!(
            "OAuth2 account {} successful (player: {}, flags: {})",
            choice.mode, player_obj, player_flags
        );

        Json(OAuth2LoginResponse {
            success: true,
            auth_token: Some(auth_token.to_string()),
            player: Some(ObjectRef::Id(player_obj).to_curie()),
            player_flags: Some(player_flags),
            client_token: Some(client_token.0.clone()),
            client_id: Some(client_id.to_string()),
            error: None,
        })
        .into_response()
    } else {
        warn!("OAuth2 account {} failed", choice.mode);
        (
            StatusCode::UNAUTHORIZED,
            Json(OAuth2LoginResponse {
                success: false,
                auth_token: None,
                player: None,
                player_flags: None,
                client_token: None,
                client_id: None,
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
