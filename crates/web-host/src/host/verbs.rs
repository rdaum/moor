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

use crate::host::{WebHost, auth, web_host};
use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use moor_common::model::ObjectRef;
use moor_common::tasks::NarrativeEvent;
use moor_schema::{
    common as moor_common_fb,
    convert::{
        narrative_event_from_ref, narrative_event_to_flatbuffer_struct, var_from_flatbuffer_ref,
        var_to_flatbuffer,
    },
    rpc as moor_rpc,
    var as moor_var_schema,
};
use moor_var::Symbol;
use planus::ReadAsRoot;
use rpc_async_client::pubsub_client::events_recv;
use rpc_common::{
    mk_detach_msg, mk_invoke_verb_msg, mk_program_msg, mk_retrieve_msg, mk_verbs_msg,
    read_reply_result, scheduler_error_from_ref, scheduler_error_to_flatbuffer_struct,
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error};

#[derive(Deserialize)]
pub struct VerbsQuery {
    inherited: Option<bool>,
}

/// FlatBuffer version: GET /fb/verbs/{object}/{name} - retrieve verb code
pub async fn verb_retrieval_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path((object, name)): Path<(String, String)>,
) -> Response {
    let auth_token = match auth::extract_auth_token_header(&header_map) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };
    let (client_id, mut rpc_client) = host.new_stateless_client();

    let Some(object_ref) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let name = Symbol::mk(&name);

    let retrieve_msg = mk_retrieve_msg(&auth_token, &object_ref, moor_rpc::EntityType::Verb, &name);

    let reply_bytes = match web_host::rpc_call(client_id, &mut rpc_client, retrieve_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap()
}

/// FlatBuffer version: GET /fb/verbs/{object} - list verbs
pub async fn verbs_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path(object): Path<String>,
    Query(query): Query<VerbsQuery>,
) -> Response {
    let auth_token = match auth::extract_auth_token_header(&header_map) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };
    let (client_id, mut rpc_client) = host.new_stateless_client();

    let Some(object_ref) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let inherited = query.inherited.unwrap_or(false);

    let verbs_msg = mk_verbs_msg(&auth_token, &object_ref, inherited);

    let reply_bytes = match web_host::rpc_call(client_id, &mut rpc_client, verbs_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap()
}

/// Extract task_id from a TaskSubmitted response
fn extract_task_id(reply_bytes: &[u8]) -> Result<u64, StatusCode> {
    let reply_result = read_reply_result(reply_bytes).map_err(|e| {
        error!("Failed to parse ReplyResult: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let result_union = reply_result.result().map_err(|e| {
        error!("Failed to parse result union: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) = result_union else {
        error!("Expected ClientSuccess from verb invocation");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let daemon_reply = client_success.reply().map_err(|e| {
        error!("Failed to parse daemon reply: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let Ok(moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted)) =
        daemon_reply.reply()
    else {
        error!("Expected TaskSubmitted from verb invocation");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    task_submitted.task_id().map_err(|e| {
        error!("Failed to parse task_id: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

/// Result of waiting for task completion
enum TaskCompletionResult {
    /// Task succeeded with a result Var and collected narrative events
    Success(moor_var::Var, Vec<NarrativeEvent>),
    /// Task failed with an error (using moor_common error type for processing)
    Error(moor_common::tasks::SchedulerError),
}

/// Wait for a task completion event matching the given task_id, collecting narrative events
async fn wait_for_task_completion(
    client_id: uuid::Uuid,
    mut narrative_sub: tmq::subscribe::Subscribe,
    task_id: u64,
) -> Result<TaskCompletionResult, StatusCode> {
    let mut collected_events: Vec<NarrativeEvent> = Vec::new();

    loop {
        let event_msg = events_recv(client_id, &mut narrative_sub)
            .await
            .map_err(|e| {
                error!("Error receiving event: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let event = event_msg.event().map_err(|e| {
            error!("Failed to parse event: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let event_ref = event.event().map_err(|e| {
            error!("Failed to parse event union: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        match event_ref {
            moor_rpc::ClientEventUnionRef::NarrativeEventMessage(narrative_msg) => {
                // Collect narrative events during execution
                let Ok(event_ref) = narrative_msg.event() else {
                    continue;
                };
                match narrative_event_from_ref(event_ref) {
                    Ok(narrative_event) => {
                        debug!("Collected narrative event during verb execution");
                        collected_events.push(narrative_event);
                    }
                    Err(e) => {
                        error!("Failed to parse narrative event: {}", e);
                    }
                }
            }
            moor_rpc::ClientEventUnionRef::TaskSuccessEvent(success) => {
                let Ok(event_task_id) = success.task_id() else {
                    continue;
                };

                if event_task_id == task_id {
                    let Ok(result_ref) = success.result() else {
                        error!("Failed to get result from TaskSuccessEvent");
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    };
                    let result_var = var_from_flatbuffer_ref(result_ref).map_err(|e| {
                        error!("Failed to parse result Var: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;
                    debug!(
                        "Task {} completed successfully with {} collected events",
                        task_id,
                        collected_events.len()
                    );
                    return Ok(TaskCompletionResult::Success(result_var, collected_events));
                }
            }
            moor_rpc::ClientEventUnionRef::TaskErrorEvent(error_event) => {
                let Ok(event_task_id) = error_event.task_id() else {
                    continue;
                };

                if event_task_id == task_id {
                    let Ok(error_ref) = error_event.error() else {
                        error!("Failed to get error from TaskErrorEvent");
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    };
                    let scheduler_error = scheduler_error_from_ref(error_ref).map_err(|e| {
                        error!("Failed to parse scheduler error: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;
                    debug!("Task {} failed with error", task_id);
                    return Ok(TaskCompletionResult::Error(scheduler_error));
                }
            }
            _ => continue,
        }
    }
}

/// FlatBuffer version: POST /fb/verbs/{object}/{name}/invoke - invoke a verb
pub async fn invoke_verb_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path((object_path, verb_name)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    tracing::info!(
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
            Ok(v) => {
                tracing::info!("Successfully parsed args var: {:?}", v);
                v
            }
            Err(e) => {
                error!("Failed to parse args var: {}", e);
                return StatusCode::BAD_REQUEST.into_response();
            }
        },
        Err(e) => {
            error!(
                "Failed to parse FlatBuffer args (VarRef::read_as_root): {}",
                e
            );
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Extract args from the Var (should be a list)
    let moo_args = match args_var.variant() {
        moor_var::Variant::List(l) => l.iter().collect::<Vec<_>>(),
        _ => {
            error!("Args must be a list");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    // Subscribe to events BEFORE submitting the task to avoid race condition
    let narrative_sub = match host.events_sub(client_id).await {
        Ok(sub) => sub,
        Err(e) => {
            error!("Failed to subscribe to events: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Give ZMQ subscription time to establish (slow joiner problem)
    tokio::time::sleep(Duration::from_millis(10)).await;

    let args_refs: Vec<&moor_var::Var> = moo_args.iter().collect();
    let Some(invoke_msg) = mk_invoke_verb_msg(
        &client_token,
        &auth_token,
        &object_ref,
        &verb_symbol,
        args_refs,
    ) else {
        error!("Failed to create invoke_verb message");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let reply_bytes = match web_host::rpc_call(client_id, &mut rpc_client, invoke_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    // Extract task_id from the reply
    let task_id = match extract_task_id(&reply_bytes) {
        Ok(id) => {
            tracing::debug!("Extracted task_id: {} for verb {}", id, verb_name);
            id
        }
        Err(status) => return status.into_response(),
    };

    // Wait for task completion with 60 second timeout
    let completion_result = match timeout(
        Duration::from_secs(60),
        wait_for_task_completion(client_id, narrative_sub, task_id),
    )
    .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(status)) => return status.into_response(),
        Err(_) => {
            error!(
                "Task {} for verb {} timed out after 60 seconds - no completion event received",
                task_id, verb_name
            );
            return StatusCode::GATEWAY_TIMEOUT.into_response();
        }
    };

    // Send detach
    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client.make_client_rpc_call(client_id, detach_msg).await;

    // Build VerbCallResponse based on the completion result
    let response = match completion_result {
        TaskCompletionResult::Success(result_var, collected_events) => {
            // Convert result Var to FlatBuffer
            let result_fb = match var_to_flatbuffer(&result_var) {
                Ok(fb) => fb,
                Err(e) => {
                    error!("Failed to encode result: {}", e);
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

            // Convert collected events to FlatBuffer format
            let output_fb: Vec<moor_common_fb::NarrativeEvent> = collected_events
                .iter()
                .filter_map(|event| match narrative_event_to_flatbuffer_struct(event) {
                    Ok(fb_event) => Some(fb_event),
                    Err(e) => {
                        error!("Failed to convert narrative event to FlatBuffer: {e}");
                        None
                    }
                })
                .collect();

            debug!(
                "Building VerbCallResponse with {} events for task {}",
                output_fb.len(),
                task_id
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
        TaskCompletionResult::Error(scheduler_error) => {
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
    };

    // Serialize the response to FlatBuffer bytes
    let mut builder = planus::Builder::new();
    let response_bytes = builder.finish(&response, None).to_vec();

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(response_bytes))
        .unwrap()
}

/// FlatBuffer version: POST /fb/verbs/{object}/{name} - compile/program a verb
pub async fn verb_program_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path((object, name)): Path<(String, String)>,
    expression: Bytes,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
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

    let reply_bytes = match web_host::rpc_call(client_id, &mut rpc_client, program_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap();

    let detach_msg = moor_rpc::HostClientToDaemonMessage {
        message: mk_detach_msg(&client_token, false).message,
    };
    let _ = rpc_client
        .make_client_rpc_call(client_id, detach_msg)
        .await
        .expect("Unable to send detach to RPC server");

    response
}
