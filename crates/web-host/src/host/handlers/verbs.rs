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

//! Verb listing, retrieval, invocation, and programming endpoints.

use super::{CapturedInvocationQuery, run_captured_invocation};

use crate::host::{
    auth::StatelessAuth,
    flatbuffer_response,
    negotiate::{
        BOTH_FORMATS, FLATBUFFERS_CONTENT_TYPE, JSON_CONTENT_TYPE, ResponseFormat,
        TEXT_PLAIN_CONTENT_TYPE, invocation_response_to_json, negotiate_response_format,
        reply_result_to_json, require_content_type,
    },
    web_host,
};
use axum::{
    body::Bytes,
    extract::{Path, Query},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use moor_common::{
    model::ObjectRef,
    tasks::{SchedulerError, VerbProgramError},
};
use moor_runtime_api::{
    api::{ClientReply, ClientRequest, EntityType, InvocationMode, VerbProgramResponse},
    api_codec::encode_client_success_bytes,
};
use moor_schema::{
    convert::{var_from_flatbuffer, var_from_flatbuffer_ref},
    var as moor_var_schema,
};
use moor_var::Symbol;
use planus::ReadAsRoot;
use serde::Deserialize;
use tracing::{debug, error};

#[derive(Deserialize)]
pub struct VerbsQuery {
    inherited: Option<bool>,
}

#[derive(Deserialize)]
struct InvokeVerbJsonRequest {
    args: Vec<moor_var_schema::VarUnion>,
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

    let retrieve_msg = ClientRequest::Retrieve {
        auth_token,
        object: object_ref,
        entity_type: EntityType::Verb,
        name,
    };

    let reply_bytes = match web_host::rpc_call(client_id, &rpc_client, retrieve_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(reply_bytes),
        ResponseFormat::Json => {
            reply_result_to_json(&reply_bytes).unwrap_or_else(|status| status.into_response())
        }
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

    let verbs_msg = ClientRequest::Verbs {
        auth_token,
        object: object_ref,
        inherited,
    };

    let reply_bytes = match web_host::rpc_call(client_id, &rpc_client, verbs_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(reply_bytes),
        ResponseFormat::Json => {
            reply_result_to_json(&reply_bytes).unwrap_or_else(|status| status.into_response())
        }
    }
}

pub async fn invoke_verb_handler(
    StatelessAuth {
        auth_token,
        client_id,
        rpc_client,
    }: StatelessAuth,
    header_map: HeaderMap,
    Path((object_path, verb_name)): Path<(String, String)>,
    Query(query): Query<CapturedInvocationQuery>,
    body: Bytes,
) -> Response {
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
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

    let content_type = header_map.get(header::CONTENT_TYPE);
    let moo_args = if require_content_type(content_type, &[FLATBUFFERS_CONTENT_TYPE], true).is_ok()
    {
        decode_flatbuffer_args(&body)
    } else if require_content_type(content_type, &[JSON_CONTENT_TYPE], false).is_ok() {
        decode_json_args(&body)
    } else {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    };
    let moo_args = match moo_args {
        Ok(args) => args,
        Err(e) => {
            error!("Failed to parse verb arguments: {e}");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    let timeout = match query.timeout() {
        Ok(timeout) => timeout,
        Err(status) => return status.into_response(),
    };

    // The daemon runs the verb with no connection behind it, collects the narrative output the
    // root task commits, and answers once the call finishes.
    let invoke_msg = ClientRequest::InvokeVerb {
        auth_token,
        object: object_ref,
        verb: verb_symbol,
        args: moo_args,
        mode: InvocationMode::CaptureOutput { timeout },
    };

    // This endpoint answers with a bare InvocationResponse rather than the ReplyResult envelope the
    // other endpoints use, so take the typed reply and encode only the final HTTP form.
    let response =
        match run_captured_invocation(client_id, &rpc_client, invoke_msg, "invoke verb").await {
            Ok(response) => response,
            Err(status) => return status.into_response(),
        };

    match format {
        ResponseFormat::FlatBuffers => {
            let mut builder = planus::Builder::new();
            let response_bytes = builder.finish(&response, None).to_vec();
            flatbuffer_response(response_bytes)
        }
        ResponseFormat::Json => {
            invocation_response_to_json(&response).unwrap_or_else(|status| status.into_response())
        }
    }
}

fn decode_flatbuffer_args(body: &[u8]) -> Result<Vec<moor_var::Var>, String> {
    let args_ref = moor_var_schema::VarRef::read_as_root(body).map_err(|e| e.to_string())?;
    let args = var_from_flatbuffer_ref(args_ref).map_err(|e| e.to_string())?;
    let moor_var::Variant::List(args) = args.variant() else {
        return Err("arguments must be a list".to_string());
    };
    Ok(args.iter().collect())
}

fn decode_json_args(body: &[u8]) -> Result<Vec<moor_var::Var>, String> {
    let request: InvokeVerbJsonRequest = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let elements = request
        .args
        .into_iter()
        .map(|variant| moor_var_schema::Var { variant })
        .collect();
    let args = var_from_flatbuffer(moor_var_schema::Var {
        variant: moor_var_schema::VarUnion::VarList(Box::new(moor_var_schema::VarList {
            elements,
        })),
    })
    .map_err(|e| e.to_string())?;
    let moor_var::Variant::List(args) = args.variant() else {
        return Err("decoded JSON arguments were not a list".to_string());
    };
    Ok(args.iter().collect())
}

pub async fn verb_program_handler(
    StatelessAuth {
        auth_token,
        client_id,
        rpc_client,
    }: StatelessAuth,
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

    let program_msg = ClientRequest::Program {
        auth_token,
        object: object_ref,
        verb: name,
        code,
    };

    let reply = match rpc_client.client_call(client_id, program_msg).await {
        Ok(reply) => reply,
        Err(error) => return super::captured_rpc_error_status(&error).into_response(),
    };
    verb_program_response(reply, format)
}

fn verb_program_response(reply: ClientReply, format: ResponseFormat) -> Response {
    let status = match &reply {
        ClientReply::VerbProgramResponseReply {
            response: VerbProgramResponse::Success { .. },
        } => StatusCode::OK,
        ClientReply::VerbProgramResponseReply {
            response:
                VerbProgramResponse::Failure {
                    error: SchedulerError::VerbProgramFailed(error),
                },
        } => match error {
            VerbProgramError::CompilationError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            VerbProgramError::PermissionDenied => StatusCode::FORBIDDEN,
            VerbProgramError::NoVerbToProgram => StatusCode::NOT_FOUND,
            VerbProgramError::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
        },
        _ => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let bytes = match encode_client_success_bytes(reply) {
        Ok(bytes) => bytes,
        Err(error) => {
            error!("Failed to encode verb programming response: {error:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let mut response = match format {
        ResponseFormat::FlatBuffers => flatbuffer_response(bytes),
        ResponseFormat::Json => match reply_result_to_json(&bytes) {
            Ok(response) => response,
            Err(status) => return status.into_response(),
        },
    };
    *response.status_mut() = status;
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::handlers::invocation_response;
    use moor_common::tasks::{NarrativeEvent, SchedulerError};
    use moor_runtime_api::api::{ClientReply, InvocationOutcome, InvocationResponse};
    use moor_schema::rpc as moor_rpc;
    use moor_var::{Obj, v_int, v_obj, v_str};

    #[tokio::test]
    async fn programming_failures_keep_diagnostics_with_error_statuses() {
        let compile_error = moor_compiler::compile("return (;", Default::default()).unwrap_err();
        let cases = [
            (
                VerbProgramError::CompilationError(compile_error),
                StatusCode::UNPROCESSABLE_ENTITY,
                "VerbCompilationError",
            ),
            (
                VerbProgramError::PermissionDenied,
                StatusCode::FORBIDDEN,
                "VerbPermissionDenied",
            ),
            (
                VerbProgramError::NoVerbToProgram,
                StatusCode::NOT_FOUND,
                "NoVerbToProgram",
            ),
            (
                VerbProgramError::DatabaseError,
                StatusCode::INTERNAL_SERVER_ERROR,
                "VerbDatabaseError",
            ),
        ];
        for (error, status, diagnostic) in cases {
            for format in [ResponseFormat::Json, ResponseFormat::FlatBuffers] {
                let response = verb_program_response(
                    ClientReply::VerbProgramResponseReply {
                        response: VerbProgramResponse::Failure {
                            error: SchedulerError::VerbProgramFailed(error.clone()),
                        },
                    },
                    format,
                );
                assert_eq!(response.status(), status);
                let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap();
                let json = match format {
                    ResponseFormat::Json => body.to_vec(),
                    ResponseFormat::FlatBuffers => {
                        let decoded = reply_result_to_json(&body).unwrap();
                        axum::body::to_bytes(decoded.into_body(), usize::MAX)
                            .await
                            .unwrap()
                            .to_vec()
                    }
                };
                let text = String::from_utf8(json).unwrap();
                assert!(text.contains(diagnostic), "{text}");
                if status == StatusCode::UNPROCESSABLE_ENTITY {
                    assert!(text.contains("error_position"), "{text}");
                }
            }
        }
    }

    #[test]
    fn json_verb_arguments_decode_losslessly() {
        let body = br#"{
            "args": [
                {"VarInt": {"value": 7}},
                {"VarStr": {"value": "argument"}}
            ]
        }"#;

        assert_eq!(
            decode_json_args(body).unwrap(),
            vec![v_int(7), v_str("argument")]
        );
    }

    #[test]
    fn invalid_json_verb_argument_is_rejected() {
        let body = br#"{"args":[{"NotAVar":{"value":7}}]}"#;

        assert!(decode_json_args(body).is_err());
    }

    #[test]
    fn a_verb_call_result_and_its_output_survive_encoding() {
        let event = NarrativeEvent::notify(
            v_obj(Obj::mk_id(1)),
            v_str("said something"),
            None,
            false,
            false,
            None,
        );
        let reply = ClientReply::InvocationResponse {
            response: InvocationResponse {
                outcome: InvocationOutcome::Success { result: v_int(7) },
                output: vec![event],
            },
        };

        let encoded = invocation_response(reply).expect("Should encode the response");
        let moor_rpc::InvocationOutcome::InvocationSuccess(success) = encoded.outcome else {
            panic!("Expected a success response");
        };
        assert_eq!(var_from_flatbuffer(*success.result).unwrap(), v_int(7));
        assert_eq!(encoded.output.len(), 1);
        let moor_schema::common::EventUnion::NotifyEvent(notify) = &encoded.output[0].event.event
        else {
            panic!("Expected a notification");
        };
        assert_eq!(
            var_from_flatbuffer(*notify.value.clone()).unwrap(),
            v_str("said something")
        );
    }

    #[test]
    fn a_task_error_is_encoded_as_an_invocation_error() {
        let reply = ClientReply::InvocationResponse {
            response: InvocationResponse {
                outcome: InvocationOutcome::Error {
                    error: SchedulerError::TaskAbortedCancelled,
                },
                output: vec![NarrativeEvent::notify(
                    v_obj(Obj::mk_id(1)),
                    v_str("before failure"),
                    None,
                    false,
                    false,
                    None,
                )],
            },
        };

        let encoded = invocation_response(reply).expect("Should encode the response");
        let moor_rpc::InvocationOutcome::InvocationError(error) = encoded.outcome else {
            panic!("Expected an invocation error");
        };
        assert!(matches!(
            error.error.error,
            moor_rpc::SchedulerErrorUnion::TaskAbortedCancelled(_)
        ));
        assert_eq!(encoded.output.len(), 1);
        let moor_schema::common::EventUnion::NotifyEvent(notify) = &encoded.output[0].event.event
        else {
            panic!("Expected a notification");
        };
        assert_eq!(
            var_from_flatbuffer(*notify.value.clone()).unwrap(),
            v_str("before failure")
        );
    }

    #[test]
    fn a_reply_that_is_not_an_invocation_response_is_an_error() {
        let reply = ClientReply::TaskSubmitted { task_id: 1 };

        assert_eq!(
            invocation_response(reply),
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        );
    }
}
