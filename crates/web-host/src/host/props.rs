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

use crate::host::{auth, var_as_json, web_host, WebHost};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use moor_values::model::ObjectRef;
use moor_values::Symbol;
use rpc_common::{DaemonToClientReply, EntityType, HostClientToDaemonMessage, PropInfo};
use serde_json::json;
use std::net::SocketAddr;
use tracing::{debug, error};

pub async fn properties_handler(
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
        HostClientToDaemonMessage::Properties(client_token.clone(), auth_token.clone(), object),
    )
    .await
    {
        Ok(DaemonToClientReply::Properties(properties)) => Json(
            properties
                .iter()
                .map(|prop| {
                    json!({
                        "definer": prop.definer.0,
                        "location": prop.location.0,
                        "name": prop.name.to_string(),
                        "owner": prop.owner.0,
                        "r": prop.r,
                        "w": prop.w,
                        "chown": prop.chown,
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

    let Some(object) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let prop_name = Symbol::mk(&prop_name);

    let response = match web_host::rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::Retrieve(
            client_token.clone(),
            auth_token.clone(),
            object,
            EntityType::Property,
            prop_name,
        ),
    )
    .await
    {
        Ok(DaemonToClientReply::PropertyValue(
            PropInfo {
                definer,
                location,
                name,
                owner,
                r,
                w,
                chown,
            },
            value,
        )) => {
            debug!("Property value: {:?}", value);
            Json(json!({
                "definer": definer.0,
                "name": name.to_string(),
                "location": location.0,
                "owner": owner.0,
                "r": r,
                "w": w,
                "chown": chown,
                "value": var_as_json(&value)
            }))
            .into_response()
        }
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
