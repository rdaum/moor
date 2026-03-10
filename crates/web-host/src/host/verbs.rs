// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::host::{
    WebHost,
    auth::{EphemeralAuth, StatelessAuth},
    flatbuffer_response,
    negotiate::{
        BOTH_FORMATS, FLATBUFFERS_CONTENT_TYPE, ResponseFormat, TEXT_PLAIN_CONTENT_TYPE,
        negotiate_response_format, reply_result_to_json, require_content_type,
        verb_call_response_to_json,
    },
    web_host,
};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use moor_common::model::ObjectRef;
use moor_common::tasks::NarrativeEvent;
use moor_schema::{
    common as moor_common_fb,
    convert::{narrative_event_to_flatbuffer_struct, var_from_flatbuffer_ref, var_to_flatbuffer},
    rpc as moor_rpc, var as moor_var_schema,
};
use moor_var::Symbol;
use planus::ReadAsRoot;
use rpc_async_client::task_client::{SessionEvent, TaskClient, TaskResult};
use rpc_common::{
    mk_program_msg, mk_retrieve_msg, mk_verbs_msg, scheduler_error_to_flatbuffer_struct,
};
use serde::Deserialize;
use tracing::{debug, error};

#[derive(Deserialize)]
pub struct VerbsQuery {
    inherited: Option<bool>,
}

pub async fn verb_retrieval_handler(
    StatelessAuth {
        auth_token,
        client_id,
        rpc_client,
    }: StatelessAuth,
    header_map: HeaderMap,
    Path((object, name)): Path<(String, String)>,
) -> Response {
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
        Err(status) => return status.into_response(),
    };

    let Some(object_ref) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let name = Symbol::mk(&name);

    let retrieve_msg = mk_retrieve_msg(&auth_token, &object_ref, moor_rpc::EntityType::Verb, &name);

    let reply_bytes = match web_host::rpc_call(client_id, &rpc_client, retrieve_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(reply_bytes),
        ResponseFormat::Json => match reply_result_to_json(&reply_bytes) {
            Ok(resp) => resp,
            Err(status) => status.into_response(),
        },
    }
}

pub async fn verbs_handler(
    StatelessAuth {
        auth_token,
        client_id,
        rpc_client,
    }: StatelessAuth,
    header_map: HeaderMap,
    Path(object): Path<String>,
    Query(query): Query<VerbsQuery>,
) -> Response {
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
        Err(status) => return status.into_response(),
    };

    let Some(object_ref) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let inherited = query.inherited.unwrap_or(false);

    let verbs_msg = mk_verbs_msg(&auth_token, &object_ref, inherited);

    let reply_bytes = match web_host::rpc_call(client_id, &rpc_client, verbs_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(reply_bytes),
        ResponseFormat::Json => match reply_result_to_json(&reply_bytes) {
            Ok(resp) => resp,
            Err(status) => status.into_response(),
        },
    }
}

pub async fn invoke_verb_handler(
    State(host): State<WebHost>,
    header_map: HeaderMap,
    Path((object_path, verb_name)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    if let Err(status) = require_content_type(
        header_map.get(header::CONTENT_TYPE),
        &[FLATBUFFERS_CONTENT_TYPE],
        true, // allow missing for backwards compat
    ) {
        return status.into_response();
    }
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
        Err(status) => return status.into_response(),
    };

    let auth_token = match crate::host::auth::extract_auth_token_header(&header_map) {
        Ok(t) => t,
        Err(status) => return status.into_response(),
    };

    debug!(
        "Invoke verb handler: object={}, verb={}, body_len={}",
        object_path,
        verb_name,
        body.len()
    );

    let object_ref = match ObjectRef::parse_curie(&object_path) {
        Some(oref) => oref,
        None => {
            error!("Invalid object CURIE: {}", object_path);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let verb_symbol = Symbol::mk(&verb_name);

    // Parse the FlatBuffer request body containing args as a Var (list)
    let args_var = match moor_var_schema::VarRef::read_as_root(&body) {
        Ok(var_ref) => match var_from_flatbuffer_ref(var_ref) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to parse args var: {}", e);
                return StatusCode::BAD_REQUEST.into_response();
            }
        },
        Err(e) => {
            error!("Failed to parse FlatBuffer args: {}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let moo_args: Vec<moor_var::Var> = match args_var.variant() {
        moor_var::Variant::List(l) => l.iter().collect(),
        _ => {
            error!("Args must be a list");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Create a per-request TaskClient (session cache optimization can come later)
    let task_client = match TaskClient::connect(host.task_client_config(auth_token)).await {
        Ok(tc) => tc,
        Err(e) => {
            error!("Failed to create TaskClient: {}", e);
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };

    // Subscribe to session events to collect narrative output
    let mut session_rx = task_client.session_events();

    let args_refs: Vec<&moor_var::Var> = moo_args.iter().collect();
    let task_result = task_client
        .invoke_verb(&object_ref, &verb_symbol, args_refs)
        .await;

    // Drain any narrative events that arrived during execution
    let mut collected_events: Vec<NarrativeEvent> = Vec::new();
    while let Ok(event) = session_rx.try_recv() {
        if let SessionEvent::Narrative(_, narrative_event) = event {
            collected_events.push(narrative_event);
        }
    }

    task_client.shutdown().await;

    let response = match task_result {
        Ok(TaskResult::Success(result_var)) => {
            let result_fb = match var_to_flatbuffer(&result_var) {
                Ok(fb) => fb,
                Err(e) => {
                    error!("Failed to encode result: {}", e);
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

            let output_fb: Vec<moor_common_fb::NarrativeEvent> = collected_events
                .iter()
                .filter_map(|event| match narrative_event_to_flatbuffer_struct(event) {
                    Ok(fb_event) => Some(fb_event),
                    Err(e) => {
                        error!("Failed to convert narrative event: {e}");
                        None
                    }
                })
                .collect();

            debug!(
                "VerbCallResponse with {} events for {}:{}",
                output_fb.len(),
                object_path,
                verb_name
            );

            moor_rpc::VerbCallResponse {
                response: moor_rpc::VerbCallResponseUnion::VerbCallSuccess(Box::new(
                    moor_rpc::VerbCallSuccess {
                        result: Box::new(result_fb),
                        output: output_fb,
                    },
                )),
            }
        }
        Ok(TaskResult::Error(scheduler_error)) => {
            let scheduler_error_fb = match scheduler_error_to_flatbuffer_struct(&scheduler_error) {
                Ok(fb) => fb,
                Err(e) => {
                    error!("Failed to encode scheduler error: {}", e);
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };
            moor_rpc::VerbCallResponse {
                response: moor_rpc::VerbCallResponseUnion::VerbCallError(Box::new(
                    moor_rpc::VerbCallError {
                        error: Box::new(scheduler_error_fb),
                    },
                )),
            }
        }
        Ok(TaskResult::Suspended(_)) => {
            // Task went to background — return empty success
            let result_fb = match var_to_flatbuffer(&moor_var::v_none()) {
                Ok(fb) => fb,
                Err(e) => {
                    error!("Failed to encode result: {}", e);
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };
            moor_rpc::VerbCallResponse {
                response: moor_rpc::VerbCallResponseUnion::VerbCallSuccess(Box::new(
                    moor_rpc::VerbCallSuccess {
                        result: Box::new(result_fb),
                        output: vec![],
                    },
                )),
            }
        }
        Err(e) => {
            error!("TaskClient error: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match format {
        ResponseFormat::FlatBuffers => {
            let mut builder = planus::Builder::new();
            let response_bytes = builder.finish(&response, None).to_vec();
            flatbuffer_response(response_bytes)
        }
        ResponseFormat::Json => match verb_call_response_to_json(&response) {
            Ok(resp) => resp,
            Err(status) => status.into_response(),
        },
    }
}

pub async fn verb_program_handler(
    EphemeralAuth {
        auth_token,
        client_id,
        client_token,
        rpc_client,
        ..
    }: EphemeralAuth,
    header_map: HeaderMap,
    Path((object, name)): Path<(String, String)>,
    expression: Bytes,
) -> Response {
    if let Err(status) = require_content_type(
        header_map.get(header::CONTENT_TYPE),
        &[TEXT_PLAIN_CONTENT_TYPE],
        true, // allow missing for backwards compat
    ) {
        return status.into_response();
    }
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
        Err(status) => return status.into_response(),
    };

    let Some(object_ref) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let name = Symbol::mk(&name);

    let expression = String::from_utf8_lossy(&expression).to_string();

    let code = expression
        .split('\n')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    let program_msg = mk_program_msg(&client_token, &auth_token, &object_ref, &name, code);

    let reply_bytes = match web_host::rpc_call(client_id, &rpc_client, program_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    // DetachGuard in EphemeralAuth handles cleanup automatically

    match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(reply_bytes),
        ResponseFormat::Json => match reply_result_to_json(&reply_bytes) {
            Ok(resp) => resp,
            Err(status) => status.into_response(),
        },
    }
}
