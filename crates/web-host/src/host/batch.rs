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

//! Batch world state operation endpoints

use crate::host::{
    auth::StatelessAuth,
    flatbuffer_response,
    negotiate::{
        BOTH_FORMATS, FLATBUFFERS_CONTENT_TYPE, JSON_CONTENT_TYPE, ResponseFormat,
        negotiate_response_format, reply_result_to_json, require_content_type,
    },
    web_host,
};
use axum::{
    body::Bytes,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use moor_schema::rpc as moor_rpc;
use planus::ReadAsRoot;
use rpc_common::BatchAction;
use tracing::error;

pub async fn batch_handler(
    StatelessAuth {
        auth_token,
        client_id,
        rpc_client,
    }: StatelessAuth,
    header_map: HeaderMap,
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

    let content_type = header_map.get(header::CONTENT_TYPE);

    let (actions, rollback) = if let Ok(()) =
        require_content_type(content_type, &[FLATBUFFERS_CONTENT_TYPE], false)
    {
        match moor_rpc::BatchWorldStateRef::read_as_root(&body) {
            Ok(batch_ref) => {
                let actions_ref = match batch_ref.actions() {
                    Ok(a) => a,
                    Err(e) => {
                        error!("Failed to get actions from BatchWorldState: {}", e);
                        return StatusCode::BAD_REQUEST.into_response();
                    }
                };
                let mut actions = Vec::new();
                for entry_ref_result in actions_ref.iter() {
                    let entry_ref = match entry_ref_result {
                        Ok(e) => e,
                        Err(_) => continue,
                    };
                    let id = match entry_ref.id() {
                        Ok(i) => i.to_string(),
                        Err(_) => continue,
                    };
                    let action_union_ref = match entry_ref.action() {
                        Ok(a) => a,
                        Err(_) => continue,
                    };

                    let action = match moor_rpc::WorldStateActionUnion::try_from(action_union_ref) {
                        Ok(a) => a,
                        Err(e) => {
                            error!("Failed to convert action union: {}", e);
                            return StatusCode::BAD_REQUEST.into_response();
                        }
                    };
                    actions.push(BatchAction { id, action });
                }
                let rollback = batch_ref.rollback().unwrap_or(false);
                (actions, rollback)
            }
            Err(e) => {
                error!("Failed to parse BatchWorldState FlatBuffer: {}", e);
                return StatusCode::BAD_REQUEST.into_response();
            }
        }
    } else if let Ok(()) = require_content_type(content_type, &[JSON_CONTENT_TYPE], false) {
        #[derive(serde::Deserialize)]
        struct BatchRequest {
            actions: Vec<BatchActionJson>,
            #[serde(default)]
            rollback: bool,
        }
        #[derive(serde::Deserialize)]
        struct BatchActionJson {
            id: String,
            action: moor_rpc::WorldStateActionUnion,
        }

        match serde_json::from_slice::<BatchRequest>(&body) {
            Ok(req) => {
                let actions = req
                    .actions
                    .into_iter()
                    .map(|a| BatchAction {
                        id: a.id,
                        action: a.action,
                    })
                    .collect();
                (actions, req.rollback)
            }
            Err(e) => {
                error!("Failed to parse BatchWorldState JSON: {}", e);
                return StatusCode::BAD_REQUEST.into_response();
            }
        }
    } else {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    };

    let batch_msg = rpc_common::mk_batch_world_state_msg(&auth_token, actions, rollback);

    let reply_bytes = match web_host::rpc_call(client_id, &rpc_client, batch_msg).await {
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
