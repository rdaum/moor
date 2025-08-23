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
use axum::Json;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use moor_common::model::ObjectRef;
use moor_var::Symbol;
use rpc_common::{DaemonToClientReply, EntityType, HostClientToDaemonMessage, PropInfo};
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

    let Some(object) = ObjectRef::parse_curie(&object) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let inherited = query.inherited.unwrap_or(false);

    let response = match web_host::rpc_call(
        client_id,
        &mut rpc_client,
        HostClientToDaemonMessage::Properties(
            client_token.clone(),
            auth_token.clone(),
            object,
            inherited,
        ),
    )
    .await
    {
        Ok(DaemonToClientReply::Properties(properties)) => Json(
            properties
                .iter()
                .map(|prop| {
                    json!({
                        "definer": prop.definer.id().0,
                        "location": prop.location.id().0,
                        "name": prop.name.to_string(),
                        "owner": prop.owner.id().0,
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
            HostClientToDaemonMessage::Detach(client_token.clone(), false),
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
                "definer": definer.id().0,
                "name": name.to_string(),
                "location": location.id().0,
                "owner": owner.id().0,
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
            HostClientToDaemonMessage::Detach(client_token.clone(), false),
        )
        .await
        .expect("Unable to send detach to RPC server");

    response
}
