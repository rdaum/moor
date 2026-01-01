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

//! Object browser endpoints

use crate::host::{WebHost, auth, web_host};
use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use moor_common::model::ObjectRef;
use moor_var::Symbol;
use rpc_common::{mk_list_objects_msg, mk_update_property_msg};
use std::net::SocketAddr;
use tracing::error;

/// FlatBuffer version: GET /fb/objects - list all accessible objects
pub async fn list_objects_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
) -> Response {
    let auth_token = match auth::extract_auth_token_header(&header_map) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };
    let (client_id, mut rpc_client) = host.new_stateless_client();

    let list_msg = mk_list_objects_msg(&auth_token);

    let reply_bytes = match web_host::rpc_call(client_id, &mut rpc_client, list_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap()
}

/// FlatBuffer version: POST /fb/properties/{object}/{name} - update property value
pub async fn update_property_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path((object, prop_name)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let auth_token = match auth::extract_auth_token_header(&header_map) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };
    let (client_id, mut rpc_client) = host.new_stateless_client();

    let Some(object_ref) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let prop_symbol = Symbol::mk(&prop_name);

    // Parse body as MOO literal string
    let literal_str = match String::from_utf8(body.to_vec()) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to parse body as UTF-8: {}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Parse the MOO literal into a Var
    let value = match moor_compiler::parse_literal_value(&literal_str) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to parse MOO literal '{}': {:?}", literal_str, e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let Some(update_msg) = mk_update_property_msg(&auth_token, &object_ref, &prop_symbol, &value)
    else {
        error!("Failed to create update_property message");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let reply_bytes = match web_host::rpc_call(client_id, &mut rpc_client, update_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap()
}
