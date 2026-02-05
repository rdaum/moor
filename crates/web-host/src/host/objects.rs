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

use crate::host::{
    auth::StatelessAuth,
    flatbuffer_response,
    negotiate::{
        BOTH_FORMATS, ResponseFormat, TEXT_PLAIN_CONTENT_TYPE, negotiate_response_format,
        reply_result_to_json, require_content_type,
    },
    web_host,
};
use axum::{
    body::Bytes,
    extract::Path,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use moor_common::model::ObjectRef;
use moor_var::Symbol;
use rpc_common::{mk_list_objects_msg, mk_update_property_msg};
use tracing::error;

pub async fn list_objects_handler(
    StatelessAuth {
        auth_token,
        client_id,
        rpc_client,
    }: StatelessAuth,
    header_map: HeaderMap,
) -> Response {
    let format = match negotiate_response_format(
        header_map.get(header::ACCEPT),
        BOTH_FORMATS,
        ResponseFormat::FlatBuffers,
    ) {
        Ok(f) => f,
        Err(status) => return status.into_response(),
    };

    let list_msg = mk_list_objects_msg(&auth_token);

    let reply_bytes = match web_host::rpc_call(client_id, &rpc_client, list_msg).await {
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

pub async fn update_property_handler(
    StatelessAuth {
        auth_token,
        client_id,
        rpc_client,
    }: StatelessAuth,
    header_map: HeaderMap,
    Path((object, prop_name)): Path<(String, String)>,
    body: Bytes,
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

    let prop_symbol = Symbol::mk(&prop_name);

    let literal_str = match String::from_utf8(body.to_vec()) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to parse body as UTF-8: {}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

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

    let reply_bytes = match web_host::rpc_call(client_id, &rpc_client, update_msg).await {
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
