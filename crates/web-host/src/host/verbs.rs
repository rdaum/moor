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
    Json,
    body::Bytes,
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use moor_common::model::ObjectRef;
use moor_schema::{
    convert::{
        compilation_error_from_ref, obj_from_flatbuffer_struct, symbol_from_flatbuffer_struct,
    },
    rpc as moor_rpc,
};
use moor_var::Symbol;
use planus::ReadAsRoot;
use rpc_common::{mk_detach_msg, mk_program_msg, mk_retrieve_msg, mk_verbs_msg};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use tracing::error;

#[derive(Deserialize)]
pub struct VerbsQuery {
    inherited: Option<bool>,
}

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

    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::VerbProgramResponseReply(
                    prog_response_reply,
                ) => {
                    let prog_response = prog_response_reply.response().expect("Missing response");
                    let response_union = prog_response.response().expect("Missing response union");
                    match response_union {
                        moor_rpc::VerbProgramResponseUnionRef::VerbProgramSuccess(success) => {
                            let objid_ref = success.obj().expect("Missing obj");
                            let objid_struct =
                                moor_rpc::Obj::try_from(objid_ref).expect("Failed to convert obj");
                            let objid = obj_from_flatbuffer_struct(&objid_struct)
                                .expect("Failed to decode obj");

                            let verb_name = success.verb_name().expect("Missing verb_name");

                            Json(json!({
                                "location": objid.as_u64(),
                                "name": verb_name,
                            }))
                            .into_response()
                        }
                        moor_rpc::VerbProgramResponseUnionRef::VerbProgramFailure(failure) => {
                            let error_wrapper = failure.error().expect("Missing error");
                            let error_type = error_wrapper.error().expect("Missing error union");
                            match error_type {
                                moor_rpc::VerbProgramErrorUnionRef::NoVerbToProgram(_) => {
                                    StatusCode::NOT_FOUND.into_response()
                                }
                                moor_rpc::VerbProgramErrorUnionRef::VerbDatabaseError(_) => {
                                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                                }
                                moor_rpc::VerbProgramErrorUnionRef::VerbCompilationError(
                                    comp_error_wrapper,
                                ) => {
                                    let comp_error_ref =
                                        comp_error_wrapper.error().expect("Missing error");
                                    let error_struct = compilation_error_from_ref(comp_error_ref)
                                        .expect("Failed to convert compilation error");
                                    Json(json!({
                                        "errors": serde_json::to_value(error_struct).unwrap()
                                    }))
                                    .into_response()
                                }
                            }
                        }
                    }
                }
                _ => {
                    error!("Unexpected response from RPC server");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        _ => {
            error!("RPC failure");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
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

    response
}

pub async fn verb_retrieval_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path((object, name)): Path<(String, String)>,
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

    let retrieve_msg = mk_retrieve_msg(
        &client_token,
        &auth_token,
        &object_ref,
        moor_rpc::EntityType::Verb,
        &name,
    );

    let reply_bytes = match web_host::rpc_call(client_id, &mut rpc_client, retrieve_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::VerbValue(verb_value) => {
                    let verb_info = verb_value.verb_info().expect("Missing verb_info");

                    let location_ref = verb_info.location().expect("Missing location");
                    let location_struct =
                        moor_rpc::Obj::try_from(location_ref).expect("Failed to convert location");
                    let location = obj_from_flatbuffer_struct(&location_struct)
                        .expect("Failed to decode location");

                    let owner_ref = verb_info.owner().expect("Missing owner");
                    let owner_struct =
                        moor_rpc::Obj::try_from(owner_ref).expect("Failed to convert owner");
                    let owner =
                        obj_from_flatbuffer_struct(&owner_struct).expect("Failed to decode owner");

                    let names_vec = verb_info.names().expect("Missing names");
                    let names: Vec<String> = names_vec
                        .iter()
                        .map(|name_result| {
                            let name_ref = name_result.expect("Failed to get name");
                            let name_struct = moor_rpc::Symbol::try_from(name_ref)
                                .expect("Failed to convert name");
                            symbol_from_flatbuffer_struct(&name_struct).to_string()
                        })
                        .collect();

                    let arg_spec_vec = verb_info.arg_spec().expect("Missing arg_spec");
                    let arg_spec: Vec<String> = arg_spec_vec
                        .iter()
                        .map(|arg_result| {
                            let arg_ref = arg_result.expect("Failed to get arg");
                            let arg_struct =
                                moor_rpc::Symbol::try_from(arg_ref).expect("Failed to convert arg");
                            symbol_from_flatbuffer_struct(&arg_struct).to_string()
                        })
                        .collect();

                    let code_vec = verb_value.code().expect("Missing code");
                    let code: Vec<String> = code_vec
                        .iter()
                        .map(|line| line.expect("Failed to get code line").to_string())
                        .collect();

                    Json(json!({
                        "location": location.as_u64(),
                        "owner": owner.as_u64(),
                        "names": names,
                        "code": code,
                        "r": verb_info.r().expect("Missing r"),
                        "w": verb_info.w().expect("Missing w"),
                        "x": verb_info.x().expect("Missing x"),
                        "d": verb_info.d().expect("Missing d"),
                        "arg_spec": arg_spec
                    }))
                    .into_response()
                }
                _ => {
                    error!("Unexpected response from RPC server");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        _ => {
            error!("RPC failure");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
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

    response
}

pub async fn verbs_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path(object): Path<String>,
    Query(query): Query<VerbsQuery>,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    let Some(object_ref) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let inherited = query.inherited.unwrap_or(false);

    let verbs_msg = mk_verbs_msg(&client_token, &auth_token, &object_ref, inherited);

    let reply_bytes = match web_host::rpc_call(client_id, &mut rpc_client, verbs_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    let reply = match moor_rpc::ReplyResultRef::read_as_root(&reply_bytes) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse reply: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::VerbsReply(verbs_reply) => {
                    let verbs_vec = verbs_reply.verbs().expect("Missing verbs");
                    Json(
                        verbs_vec
                            .iter()
                            .map(|verb_result| {
                                let verb = verb_result.expect("Failed to get verb");

                                let location_ref = verb.location().expect("Missing location");
                                let location_struct = moor_rpc::Obj::try_from(location_ref)
                                    .expect("Failed to convert location");
                                let location = obj_from_flatbuffer_struct(&location_struct)
                                    .expect("Failed to decode location");

                                let owner_ref = verb.owner().expect("Missing owner");
                                let owner_struct = moor_rpc::Obj::try_from(owner_ref)
                                    .expect("Failed to convert owner");
                                let owner = obj_from_flatbuffer_struct(&owner_struct)
                                    .expect("Failed to decode owner");

                                let names_vec = verb.names().expect("Missing names");
                                let names: Vec<String> = names_vec
                                    .iter()
                                    .map(|name_result| {
                                        let name_ref = name_result.expect("Failed to get name");
                                        let name_struct = moor_rpc::Symbol::try_from(name_ref)
                                            .expect("Failed to convert name");
                                        symbol_from_flatbuffer_struct(&name_struct).to_string()
                                    })
                                    .collect();

                                let arg_spec_vec = verb.arg_spec().expect("Missing arg_spec");
                                let arg_spec: Vec<String> = arg_spec_vec
                                    .iter()
                                    .map(|arg_result| {
                                        let arg_ref = arg_result.expect("Failed to get arg");
                                        let arg_struct = moor_rpc::Symbol::try_from(arg_ref)
                                            .expect("Failed to convert arg");
                                        symbol_from_flatbuffer_struct(&arg_struct).to_string()
                                    })
                                    .collect();

                                json!({
                                    "location": location.as_u64(),
                                    "owner": owner.as_u64(),
                                    "names": names,
                                    "r": verb.r().expect("Missing r"),
                                    "w": verb.w().expect("Missing w"),
                                    "x": verb.x().expect("Missing x"),
                                    "d": verb.d().expect("Missing d"),
                                    "arg_spec": arg_spec
                                })
                            })
                            .collect::<Vec<serde_json::Value>>(),
                    )
                    .into_response()
                }
                _ => {
                    error!("Unexpected response from RPC server");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        _ => {
            error!("RPC failure");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
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

    response
}
