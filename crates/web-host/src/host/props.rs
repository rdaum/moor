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

use crate::host::{WebHost, auth, web_host};
use axum::{
    body::Body,
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use moor_common::model::ObjectRef;
use moor_schema::rpc as moor_rpc;
use moor_var::Symbol;
use rpc_common::{mk_properties_msg, mk_retrieve_msg};
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Deserialize)]
pub struct PropertiesQuery {
    inherited: Option<bool>,
}

/// FlatBuffer version: GET /fb/properties/{object} - list properties
pub async fn properties_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path(object): Path<String>,
    Query(query): Query<PropertiesQuery>,
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

    let props_msg = mk_properties_msg(&auth_token, &object_ref, inherited);

    let reply_bytes = match web_host::rpc_call(client_id, &mut rpc_client, props_msg).await {
        Ok(bytes) => bytes,
        Err(status) => return status.into_response(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-flatbuffer")
        .body(Body::from(reply_bytes))
        .unwrap()
}

/// FlatBuffer version: GET /fb/properties/{object}/{name} - retrieve property value
pub async fn property_retrieval_handler(
    State(host): State<WebHost>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path((object, prop_name)): Path<(String, String)>,
) -> Response {
    let auth_token = match auth::extract_auth_token_header(&header_map) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };
    let (client_id, mut rpc_client) = host.new_stateless_client();

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
