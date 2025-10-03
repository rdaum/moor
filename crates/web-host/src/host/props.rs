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

use crate::host::{WebHost, auth, var_as_json, web_host};
use axum::{
    Json,
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use moor_common::model::ObjectRef;
use moor_schema::{
    convert::{obj_from_flatbuffer_struct, symbol_from_flatbuffer_struct, var_from_flatbuffer},
    rpc as moor_rpc, var as moor_var_schema,
};
use moor_var::Symbol;
use planus::ReadAsRoot;
use rpc_common::{mk_detach_msg, mk_properties_msg, mk_retrieve_msg};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use tracing::{debug, error};

#[derive(Deserialize)]
pub struct PropertiesQuery {
    inherited: Option<bool>,
}

pub async fn properties_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path(object): Path<String>,
    Query(query): Query<PropertiesQuery>,
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

    let props_msg = mk_properties_msg(&client_token, &auth_token, &object_ref, inherited);

    let reply_bytes = match web_host::rpc_call(client_id, &mut rpc_client, props_msg).await {
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
                moor_rpc::DaemonToClientReplyUnionRef::PropertiesReply(properties) => {
                    let props_vec = properties.properties().expect("Missing properties");
                    Json(
                        props_vec
                            .iter()
                            .map(|prop_result| {
                                let prop = prop_result.expect("Failed to get property");
                                let definer_ref = prop.definer().expect("Missing definer");
                                let definer_struct = moor_rpc::Obj::try_from(definer_ref)
                                    .expect("Failed to convert definer");
                                let definer = obj_from_flatbuffer_struct(&definer_struct)
                                    .expect("Failed to decode definer");

                                let location_ref = prop.location().expect("Missing location");
                                let location_struct = moor_rpc::Obj::try_from(location_ref)
                                    .expect("Failed to convert location");
                                let location = obj_from_flatbuffer_struct(&location_struct)
                                    .expect("Failed to decode location");

                                let name_ref = prop.name().expect("Missing name");
                                let name_struct = moor_rpc::Symbol::try_from(name_ref)
                                    .expect("Failed to convert name");
                                let name = symbol_from_flatbuffer_struct(&name_struct);

                                let owner_ref = prop.owner().expect("Missing owner");
                                let owner_struct = moor_rpc::Obj::try_from(owner_ref)
                                    .expect("Failed to convert owner");
                                let owner = obj_from_flatbuffer_struct(&owner_struct)
                                    .expect("Failed to decode owner");

                                json!({
                                    "definer": definer.as_u64(),
                                    "location": location.as_u64(),
                                    "name": name.to_string(),
                                    "owner": owner.as_u64(),
                                    "r": prop.r().expect("Missing r"),
                                    "w": prop.w().expect("Missing w"),
                                    "chown": prop.chown().expect("Missing chown"),
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

pub async fn property_retrieval_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    header_map: HeaderMap,
    Path((object, prop_name)): Path<(String, String)>,
) -> Response {
    let (auth_token, client_id, client_token, mut rpc_client) =
        match auth::auth_auth(host.clone(), addr, header_map.clone()).await {
            Ok(connection_details) => connection_details,
            Err(status) => return status.into_response(),
        };

    let Some(object_ref) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let prop_name = Symbol::mk(&prop_name);

    let retrieve_msg = mk_retrieve_msg(
        &client_token,
        &auth_token,
        &object_ref,
        moor_rpc::EntityType::Property,
        &prop_name,
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
                moor_rpc::DaemonToClientReplyUnionRef::PropertyValue(prop_value) => {
                    let prop = prop_value.prop_info().expect("Missing prop_info");

                    let definer_ref = prop.definer().expect("Missing definer");
                    let definer_struct =
                        moor_rpc::Obj::try_from(definer_ref).expect("Failed to convert definer");
                    let definer = obj_from_flatbuffer_struct(&definer_struct)
                        .expect("Failed to decode definer");

                    let location_ref = prop.location().expect("Missing location");
                    let location_struct =
                        moor_rpc::Obj::try_from(location_ref).expect("Failed to convert location");
                    let location = obj_from_flatbuffer_struct(&location_struct)
                        .expect("Failed to decode location");

                    let name_ref = prop.name().expect("Missing name");
                    let name_struct =
                        moor_rpc::Symbol::try_from(name_ref).expect("Failed to convert name");
                    let name = symbol_from_flatbuffer_struct(&name_struct);

                    let owner_ref = prop.owner().expect("Missing owner");
                    let owner_struct =
                        moor_rpc::Obj::try_from(owner_ref).expect("Failed to convert owner");
                    let owner =
                        obj_from_flatbuffer_struct(&owner_struct).expect("Failed to decode owner");

                    let value_ref = prop_value.value().expect("Missing value");
                    let value_struct =
                        moor_var_schema::Var::try_from(value_ref).expect("Failed to convert value");
                    let value = var_from_flatbuffer(&value_struct).expect("Failed to decode value");

                    debug!("Property value: {:?}", value);
                    Json(json!({
                        "definer": definer.as_u64(),
                        "name": name.to_string(),
                        "location": location.as_u64(),
                        "owner": owner.as_u64(),
                        "r": prop.r().expect("Missing r"),
                        "w": prop.w().expect("Missing w"),
                        "chown": prop.chown().expect("Missing chown"),
                        "value": var_as_json(&value)
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
