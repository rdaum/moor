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

use crate::host::{auth, var_as_json, web_host::WebHost, web_host::rpc_call};
use axum::{
    Json,
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use base64::Engine;
use moor_common::tasks::Event;
use moor_schema::{
    convert::{narrative_event_from_ref, obj_from_flatbuffer_struct},
    rpc as moor_rpc,
};
use moor_var::v_obj;
use planus::ReadAsRoot;
use rpc_common::{
    mk_detach_msg, mk_dismiss_presentation_msg, mk_request_current_presentations_msg,
    mk_request_history_msg,
};
use serde_derive::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use tracing::{error, warn};
use uuid::Uuid;

/// Helper function to extract the daemon reply from an RPC response
/// Handles the common boilerplate of unwrapping the nested RPC structures
fn extract_daemon_reply(
    reply_bytes: &[u8],
) -> Result<moor_rpc::DaemonToClientReplyUnionRef<'_>, Response> {
    let reply = match moor_rpc::ReplyResultRef::read_as_root(reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
    };

    let Ok(result) = reply.result() else {
        error!("Missing result in RPC reply");
        return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };

    let moor_rpc::ReplyResultUnionRef::ClientSuccess(host_success) = result else {
        error!("RPC failure");
        return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };

    let Ok(daemon_reply) = host_success.reply() else {
        error!("Missing daemon reply");
        return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };

    let Ok(reply_union) = daemon_reply.reply() else {
        error!("Missing reply union");
        return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };

    Ok(reply_union)
}

/// Create an age identity from 32 derived bytes (from Argon2)
/// Encodes bytes as bech32 AGE-SECRET-KEY string that age can parse
fn identity_from_derived_bytes(bytes: &[u8; 32]) -> Result<age::x25519::Identity, String> {
    use bech32::{ToBase32, Variant};

    // Encode as bech32 with AGE-SECRET-KEY- prefix (same format age uses)
    let base32_bytes = bytes.to_base32();
    let encoded = bech32::encode("age-secret-key-", base32_bytes, Variant::Bech32)
        .map_err(|e| format!("Failed to encode identity: {}", e))?;

    // Age expects uppercase
    encoded
        .to_uppercase()
        .parse()
        .map_err(|e| format!("Failed to parse identity: {}", e))
}

/// Decrypt an age-encrypted event blob using the provided identity
fn decrypt_event_blob(
    encrypted_blob: &[u8],
    identity: &age::x25519::Identity,
) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let decryptor = age::Decryptor::new(encrypted_blob)
        .map_err(|e| format!("Failed to create decryptor: {}", e))?;

    let mut decrypted = Vec::new();
    let mut reader = decryptor
        .decrypt(std::iter::once(identity as &dyn age::Identity))
        .map_err(|e| format!("Failed to decrypt: {}", e))?;

    reader
        .read_to_end(&mut decrypted)
        .map_err(|e| format!("Failed to read decrypted data: {}", e))?;

    Ok(decrypted)
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
        &client_token,
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
        Err(response) => return response,
    };

    let moor_rpc::DaemonToClientReplyUnionRef::HistoryResponseReply(history_ref) = reply_union
    else {
        error!("Unexpected response type: expected HistoryResponseReply");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let Ok(history_response) = history_ref.response() else {
        error!("Missing history response");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let Ok(events_ref) = history_response.events() else {
        error!("Missing events in history response");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let events: Vec<_> = events_ref
        .iter()
        .filter_map(|event_result| {
            let historical_event = event_result.ok()?;
            let encrypted_blob = historical_event.encrypted_blob().ok()?;

            // Get decryption key from header
            let Some(key_header) = header_map.get("X-Moor-Event-Log-Key") else {
                warn!("No decryption key provided in X-Moor-Event-Log-Key header - skipping encrypted event");
                return None;
            };

            let key_str = key_header.to_str().ok()?;

            // Try to parse as age identity string first, then as base64 derived bytes
            let identity = if let Ok(id) = key_str.parse::<age::x25519::Identity>() {
                id
            } else if let Ok(derived_bytes) = base64::engine::general_purpose::STANDARD.decode(key_str) {
                let Ok(bytes_array) = <[u8; 32]>::try_from(derived_bytes.as_slice()) else {
                    warn!("Invalid derived key length: {}", derived_bytes.len());
                    return None;
                };
                match identity_from_derived_bytes(&bytes_array) {
                    Ok(id) => id,
                    Err(e) => {
                        warn!("Failed to create identity from derived bytes: {}", e);
                        return None;
                    }
                }
            } else {
                warn!("Invalid key format in X-Moor-Event-Log-Key header");
                return None;
            };

            // Decrypt with provided key
            let event_bytes = match decrypt_event_blob(encrypted_blob, &identity) {
                Ok(decrypted) => decrypted,
                Err(e) => {
                    warn!("Failed to decrypt event: {}", e);
                    return None;
                }
            };

            let narrative_event_ref = <moor_schema::common::NarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&event_bytes).ok()?;
            let narrative_event = narrative_event_from_ref(narrative_event_ref).ok()?;
            let is_historical = historical_event.is_historical().ok()?;
            let player_ref = historical_event.player().ok()?;
            let player_struct = moor_rpc::Obj::try_from(player_ref).ok()?;
            let player = obj_from_flatbuffer_struct(&player_struct).ok()?;

            Some(json!({
                "event_id": narrative_event.event_id(),
                "author": var_as_json(narrative_event.author()),
                "message": match narrative_event.event() {
                    Event::Notify { value: msg, content_type, no_flush, no_newline } => {
                        let _ = (no_flush, no_newline);
                        let normalized_content_type = content_type.as_ref().map(|ct| {
                            match ct.as_string().as_str() {
                                "text_djot" => "text/djot".to_string(),
                                "text_html" => "text/html".to_string(),
                                "text_plain" => "text/plain".to_string(),
                                _ => ct.as_string(),
                            }
                        });
                        json!({
                            "type": "notify",
                            "content": var_as_json(&msg),
                            "content_type": normalized_content_type
                        })
                    },
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
                "timestamp": narrative_event.timestamp(),
                "is_historical": is_historical,
                "player": var_as_json(&v_obj(player))
            }))
        })
        .collect();

    let Ok(total_events) = history_response.total_events() else {
        error!("Missing total_events in history response");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let Ok(time_range_start) = history_response.time_range_start() else {
        error!("Missing time_range_start in history response");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let Ok(time_range_end) = history_response.time_range_end() else {
        error!("Missing time_range_end in history response");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let Ok(has_more_before) = history_response.has_more_before() else {
        error!("Missing has_more_before in history response");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let earliest_event_id = history_response
        .earliest_event_id()
        .ok()
        .flatten()
        .and_then(|uuid_ref| uuid_ref.data().ok())
        .and_then(|bytes| {
            let bytes_array = <[u8; 16]>::try_from(bytes).ok()?;
            Some(Uuid::from_bytes(bytes_array))
        });

    let latest_event_id = history_response
        .latest_event_id()
        .ok()
        .flatten()
        .and_then(|uuid_ref| uuid_ref.data().ok())
        .and_then(|bytes| {
            let bytes_array = <[u8; 16]>::try_from(bytes).ok()?;
            Some(Uuid::from_bytes(bytes_array))
        });

    let response = Json(json!({
        "events": events,
        "meta": {
            "total_events": total_events,
            "time_range": (time_range_start, time_range_end),
            "has_more_before": has_more_before,
            "earliest_event_id": earliest_event_id,
            "latest_event_id": latest_event_id
        }
    }));

    // We're done with this RPC connection, so we detach it.
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    if let Err(e) = rpc_client.make_client_rpc_call(client_id, detach_msg).await {
        error!("Failed to send detach to RPC server: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    response.into_response()
}

/// REST endpoint to get player's event log public key
pub async fn get_pubkey_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    let get_pubkey_msg = rpc_common::mk_get_event_log_pubkey_msg(&client_token, &auth_token);

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, get_pubkey_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let reply_union = match extract_daemon_reply(&reply_bytes) {
        Ok(r) => r,
        Err(response) => return response,
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

    // Detach RPC connection
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    if let Err(e) = rpc_client.make_client_rpc_call(client_id, detach_msg).await {
        error!("Failed to send detach to RPC server: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    response.into_response()
}

/// REST endpoint to set player's event log public key
/// Expects either:
/// - `derived_key_bytes`: base64-encoded 32 bytes from Argon2 (will derive age keypair)
/// - `public_key`: age public key string (legacy)
pub async fn set_pubkey_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    // Derive public key from Argon2 bytes or use provided public key
    let public_key = if let Some(derived_bytes_b64) =
        payload.get("derived_key_bytes").and_then(|v| v.as_str())
    {
        // Decode base64
        let derived_bytes =
            match base64::engine::general_purpose::STANDARD.decode(derived_bytes_b64) {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("Failed to decode derived_key_bytes: {}", e);
                    return (
                        StatusCode::BAD_REQUEST,
                        "Invalid base64 in derived_key_bytes",
                    )
                        .into_response();
                }
            };

        // Ensure 32 bytes
        if derived_bytes.len() != 32 {
            return (
                StatusCode::BAD_REQUEST,
                "derived_key_bytes must be 32 bytes",
            )
                .into_response();
        }

        // Create age identity from bytes
        let Ok(bytes_array) = derived_bytes.as_slice().try_into() else {
            error!("Failed to convert derived bytes to array (expected 32 bytes)");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to convert derived bytes",
            )
                .into_response();
        };

        let identity = match identity_from_derived_bytes(&bytes_array) {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to create identity from derived bytes: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to create identity",
                )
                    .into_response();
            }
        };
        let recipient = identity.to_public();
        recipient.to_string()
    } else if let Some(key) = payload.get("public_key").and_then(|v| v.as_str()) {
        key.to_string()
    } else {
        return (
            StatusCode::BAD_REQUEST,
            "Missing derived_key_bytes or public_key field",
        )
            .into_response();
    };

    let set_pubkey_msg =
        rpc_common::mk_set_event_log_pubkey_msg(&client_token, &auth_token, public_key);

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, set_pubkey_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let _ = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let reply_union = match extract_daemon_reply(&reply_bytes) {
        Ok(r) => r,
        Err(response) => return response,
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

    // Detach RPC connection
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    if let Err(e) = rpc_client.make_client_rpc_call(client_id, detach_msg).await {
        error!("Failed to send detach to RPC server: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

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

    let presentations_msg = mk_request_current_presentations_msg(&client_token, &auth_token);

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, presentations_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let reply_union = match extract_daemon_reply(&reply_bytes) {
        Ok(r) => r,
        Err(response) => return response,
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

    // We're done with this RPC connection, so we detach it.
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    if let Err(e) = rpc_client.make_client_rpc_call(client_id, detach_msg).await {
        error!("Failed to send detach to RPC server: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

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

    let dismiss_msg =
        mk_dismiss_presentation_msg(&client_token, &auth_token, presentation_id.clone());

    let reply_bytes = match rpc_call(client_id, &mut rpc_client, dismiss_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let reply_union = match extract_daemon_reply(&reply_bytes) {
        Ok(r) => r,
        Err(response) => return response,
    };

    let moor_rpc::DaemonToClientReplyUnionRef::PresentationDismissed(_) = reply_union else {
        error!("Unexpected response type: expected PresentationDismissed");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let response = Json(json!({
        "dismissed": true,
        "presentation_id": presentation_id
    }));

    // We're done with this RPC connection, so we detach it.
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    if let Err(e) = rpc_client.make_client_rpc_call(client_id, detach_msg).await {
        error!("Failed to send detach to RPC server: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    response.into_response()
}
