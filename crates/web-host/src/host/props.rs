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

use crate::host::{
    auth::StatelessAuth,
    flatbuffer_response,
    negotiate::{BOTH_FORMATS, ResponseFormat, negotiate_response_format, reply_result_to_json},
    web_host,
};
use axum::{
    extract::{Path, Query},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use moor_common::model::ObjectRef;
use moor_schema::rpc as moor_rpc;
use moor_var::Symbol;
use rpc_common::{mk_properties_msg, mk_retrieve_msg};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PropertiesQuery {
    inherited: Option<bool>,
}

pub async fn properties_handler(
    StatelessAuth {
        auth_token,
        client_id,
        rpc_client,
    }: StatelessAuth,
    header_map: HeaderMap,
    Path(object): Path<String>,
    Query(query): Query<PropertiesQuery>,
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

    let props_msg = mk_properties_msg(&auth_token, &object_ref, inherited);

    let reply_bytes = match web_host::rpc_call(client_id, &rpc_client, props_msg).await {
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

pub async fn property_retrieval_handler(
    StatelessAuth {
        auth_token,
        client_id,
        rpc_client,
    }: StatelessAuth,
    header_map: HeaderMap,
    Path((object, prop_name)): Path<(String, String)>,
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

    let prop_name = Symbol::mk(&prop_name);

    let retrieve_msg = mk_retrieve_msg(
        &auth_token,
        &object_ref,
        moor_rpc::EntityType::Property,
        &prop_name,
    );

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
