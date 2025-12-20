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

//! Event log encryption and history endpoints

use crate::host::{auth, web_host::WebHost, web_host::rpc_call};
use axum::{
    Json,
    body::Body,
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use moor_schema::rpc as moor_rpc;
use rpc_common::{
    mk_dismiss_presentation_msg, mk_request_current_presentations_msg, mk_request_history_msg,
    read_reply_result,
};
use serde_derive::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use tracing::error;
use uuid::Uuid;

/// Helper function to extract the daemon reply from an RPC response
/// Handles the common boilerplate of unwrapping the nested RPC structures
fn extract_daemon_reply(
    reply_bytes: &[u8],
) -> Result<moor_rpc::DaemonToClientReplyUnionRef<'_>, Box<Response>> {
    let reply = match read_reply_result(reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return Err(Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()));
        }
    };

    let Ok(result) = reply.result() else {
        error!("Missing result in RPC reply");
        return Err(Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()));
    };

    let moor_rpc::ReplyResultUnionRef::ClientSuccess(host_success) = result else {
        error!("RPC failure");
        return Err(Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()));
    };

    let Ok(daemon_reply) = host_success.reply() else {
        error!("Missing daemon reply");
        return Err(Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()));
    };

    let Ok(reply_union) = daemon_reply.reply() else {
        error!("Missing reply union");
        return Err(Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()));
    };

    Ok(reply_union)
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    since_seconds: Option<u64>,
    since_event: Option<String>, // UUID as string
    until_event: Option<String>, // UUID as string
    limit: Option<usize>,
}

/// REST endpoint to retrieve player event history as encrypted FlatBuffer blobs
/// Client-side decryption: returns encrypted events, no key needed in header
pub async fn history_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> Response {
    let (auth_token, client_id, mut rpc_client) =
        match auth::stateless_rpc_client(&host, &header_map) {
            Ok(ctx) => ctx,
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
        &auth_token,
        Box::new(moor_rpc::HistoryRecall {
            recall: history_recall_union,
        }),
    );

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, history_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let reply_union = match extract_daemon_reply(&reply_bytes) {
        Ok(r) => r,
        Err(response) => return *response,
    };

    let moor_rpc::DaemonToClientReplyUnionRef::HistoryResponseReply(history_ref) = reply_union
    else {
        error!("Unexpected response type: expected HistoryResponseReply");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let Ok(_history_response) = history_ref.response() else {
        error!("Missing history response");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    // Return the entire HistoryResponse as FlatBuffer bytes
    // Client will parse the FlatBuffer and decrypt the encrypted_blob field of each event
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap()
}

/// REST endpoint to get player's event log public key
pub async fn get_pubkey_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
) -> Response {
    let (auth_token, client_id, mut rpc_client) =
        match auth::stateless_rpc_client(&host, &header_map) {
            Ok(ctx) => ctx,
            Err(status) => return status.into_response(),
        };

    let get_pubkey_msg = rpc_common::mk_get_event_log_pubkey_msg(&auth_token);

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, get_pubkey_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let reply_union = match extract_daemon_reply(&reply_bytes) {
        Ok(r) => r,
        Err(response) => return *response,
    };

    let moor_rpc::DaemonToClientReplyUnionRef::EventLogPublicKey(pubkey_ref) = reply_union else {
        error!("Unexpected response type: expected EventLogPublicKey");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let public_key = pubkey_ref
        .public_key()
        .ok()
        .flatten()
        .unwrap_or_default()
        .to_string();

    let response = Json(json!({
        "public_key": public_key
    }));

    response.into_response()
}

/// REST endpoint to delete all event history for the authenticated player
pub async fn delete_history_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
) -> Response {
    let (auth_token, client_id, mut rpc_client) =
        match auth::stateless_rpc_client(&host, &header_map) {
            Ok(ctx) => ctx,
            Err(status) => return status.into_response(),
        };

    let delete_msg = rpc_common::mk_delete_event_log_history_msg(&auth_token);

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, delete_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let reply_union = match extract_daemon_reply(&reply_bytes) {
        Ok(r) => r,
        Err(response) => return *response,
    };

    let moor_rpc::DaemonToClientReplyUnionRef::EventLogHistoryDeleted(deleted_ref) = reply_union
    else {
        error!("Unexpected response type: expected EventLogHistoryDeleted");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let success = deleted_ref.success().unwrap_or(false);

    let response = Json(json!({
        "success": success
    }));

    response.into_response()
}

/// REST endpoint to set player's event log public key
/// Expects JSON body with `public_key` field containing age public key string (age1...)
pub async fn set_pubkey_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    let (auth_token, client_id, mut rpc_client) =
        match auth::stateless_rpc_client(&host, &header_map) {
            Ok(ctx) => ctx,
            Err(status) => return status.into_response(),
        };

    // Extract public key from request
    let public_key = match payload.get("public_key").and_then(|v| v.as_str()) {
        Some(key) => key.to_string(),
        None => {
            return (StatusCode::BAD_REQUEST, "Missing public_key field").into_response();
        }
    };

    let set_pubkey_msg = rpc_common::mk_set_event_log_pubkey_msg(&auth_token, public_key);

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, set_pubkey_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let _ = match read_reply_result(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let reply_union = match extract_daemon_reply(&reply_bytes) {
        Ok(r) => r,
        Err(response) => return *response,
    };

    let moor_rpc::DaemonToClientReplyUnionRef::EventLogPublicKey(pubkey_ref) = reply_union else {
        error!("Unexpected response type: expected EventLogPublicKey");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let public_key = pubkey_ref
        .public_key()
        .ok()
        .flatten()
        .unwrap_or_default()
        .to_string();

    let response = Json(json!({
        "public_key": public_key,
        "status": "set"
    }));

    response.into_response()
}

/// REST endpoint to dismiss a specific presentation for the authenticated player
pub async fn dismiss_presentation_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path(presentation_id): Path<String>,
) -> Response {
    let (auth_token, client_id, mut rpc_client) =
        match auth::stateless_rpc_client(&host, &header_map) {
            Ok(ctx) => ctx,
            Err(status) => return status.into_response(),
        };

    let dismiss_msg = mk_dismiss_presentation_msg(&auth_token, presentation_id.clone());

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, dismiss_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let reply_union = match extract_daemon_reply(&reply_bytes) {
        Ok(r) => r,
        Err(response) => return *response,
    };

    let moor_rpc::DaemonToClientReplyUnionRef::PresentationDismissed(_) = reply_union else {
        error!("Unexpected response type: expected PresentationDismissed");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let response = Json(json!({
        "dismissed": true,
        "presentation_id": presentation_id
    }));

    response.into_response()
}

/// FlatBuffer version: GET /fb/api/presentations - return raw flatbuffer bytes
pub async fn presentations_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
) -> Response {
    let (auth_token, client_id, mut rpc_client) =
        match auth::stateless_rpc_client(&host, &header_map) {
            Ok(ctx) => ctx,
            Err(status) => return status.into_response(),
        };

    let presentations_msg = mk_request_current_presentations_msg(&auth_token);

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, presentations_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    // Return raw FlatBuffer bytes
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap()
}
