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

// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::host::{auth, web_host, WebHost};
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use moor_values::model::ObjectRef;
use moor_values::tasks::VerbProgramError;
use moor_values::Symbol;
use rpc_common::{
    DaemonToClientReply, EntityType, HostClientToDaemonMessage, VerbInfo, VerbProgramResponse,
};
use serde_json::json;
use std::net::SocketAddr;
use tracing::error;

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

    let Some(object) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let name = Symbol::mk(&name);

    let expression = String::from_utf8_lossy(&expression).to_string();

    let code = expression
        .split('\n')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    let response = match web_host::rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::Program(
            client_token.clone(),
            auth_token.clone(),
            object,
            name,
            code,
        ),
    )
    .await
    {
        Ok(DaemonToClientReply::ProgramResponse(VerbProgramResponse::Success(
            objid,
            verb_name,
        ))) => Json(json!({
            "location": objid.id().0,
            "name": verb_name,
        }))
        .into_response(),
        Ok(DaemonToClientReply::ProgramResponse(VerbProgramResponse::Failure(
            VerbProgramError::NoVerbToProgram,
        ))) => {
            // 404
            StatusCode::NOT_FOUND.into_response()
        }
        Ok(DaemonToClientReply::ProgramResponse(VerbProgramResponse::Failure(
            VerbProgramError::DatabaseError,
        ))) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        Ok(DaemonToClientReply::ProgramResponse(VerbProgramResponse::Failure(
            VerbProgramError::CompilationError(errors),
        ))) => Json(json!({
            "errors": errors.iter().map(|e| e.to_string()).collect::<Vec<String>>()
        }))
        .into_response(),
        Ok(r) => {
            error!("Unexpected response from RPC server: {:?}", r);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(status) => status.into_response(),
    };

    // We're done with this RPC connection, so we detach it.
    let _ = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Detach(client_token.clone()),
        )
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

    let Some(object) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let name = Symbol::mk(&name);

    let response = match web_host::rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::Retrieve(
            client_token.clone(),
            auth_token.clone(),
            object,
            EntityType::Verb,
            name,
        ),
    )
    .await
    {
        Ok(DaemonToClientReply::VerbValue(
            VerbInfo {
                location,
                owner,
                names,
                r,
                w,
                x,
                d,
                arg_spec,
            },
            code,
        )) => Json(json!({
            "location": location.id().0,
            "owner": owner.id().0,
            "names": names.iter().map(|s| s.to_string()).collect::<Vec<String>>(),
            "code": code,
            "r": r,
            "w": w,
            "x": x,
            "d": d,
            "arg_spec": arg_spec.iter().map(|s| s.to_string()).collect::<Vec<String>>()
        }))
        .into_response(),
        Ok(r) => {
            error!("Unexpected response from RPC server: {:?}", r);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(status) => status.into_response(),
    };

    // We're done with this RPC connection, so we detach it.
    let _ = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Detach(client_token.clone()),
        )
        .await
        .expect("Unable to send detach to RPC server");

    response
}

pub async fn verbs_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path(object): Path<String>,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    let Some(object) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let response = match web_host::rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::Verbs(client_token.clone(), auth_token.clone(), object),
    )
        .await
    {
        Ok(DaemonToClientReply::Verbs(verbs)) => Json(
            verbs
                .iter()
                .map(|verb| {
                    json!({
                        "location": verb.location.id().0,
                        "owner": verb.owner.id().0,
                        "names": verb.names.iter().map(|s| s.to_string()).collect::<Vec<String>>(),
                        "r": verb.r,
                        "w": verb.w,
                        "x": verb.x,
                        "d": verb.d,
                        "arg_spec": verb.arg_spec.iter().map(|s| s.to_string()).collect::<Vec<String>>()
                    })
                })
                .collect::<Vec<serde_json::Value>>(),
        )
            .into_response(),
        Ok(r) => {
            error!("Unexpected response from RPC server: {:?}", r);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(status) => status.into_response(),
    };

    // We're done with this RPC connection, so we detach it.
    let _ = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Detach(client_token.clone()),
        )
        .await
        .expect("Unable to send detach to RPC server");

    response
}
