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

use crate::{
    WorkerRpcSendClient, attach_worker,
    pubsub_client::{WorkerMessage, workers_events_recv},
};
use moor_common::{schema::rpc as moor_rpc, tasks::WorkerError};
use moor_var::{Obj, Symbol, Var};
use rpc_common::{WORKER_BROADCAST_TOPIC, WorkerToken};
use std::{
    future::Future,
    sync::{Arc, atomic::AtomicBool},
};
use thiserror::Error;
use tmq::{TmqError, request};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WorkerRpcError {
    #[error("Unable to attach worker to daemon: {0}")]
    UnableToConnectToDaemon(TmqError),
    #[error("Unable processing worker event: {0}")]
    RpcError(rpc_common::RpcError),
}

pub async fn worker_loop<ProcessFunc, Fut>(
    kill_switch: &Arc<AtomicBool>,
    my_id: Uuid,
    worker_token: &WorkerToken,
    worker_response_rpc_addr: &str,
    worker_request_rpc_addr: &str,
    worker_type: Symbol,
    perform: Arc<ProcessFunc>,
) -> Result<(), WorkerRpcError>
where
    ProcessFunc: Fn(WorkerToken, Uuid, Symbol, Obj, Vec<Var>, Option<std::time::Duration>) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = Result<Var, WorkerError>> + Send + 'static + Sync,
{
    let zmq_ctx = tmq::Context::new();

    // First attempt to connect to the daemon and "attach" ourselves.
    let _rpc_client = attach_worker(
        worker_token,
        worker_type,
        my_id,
        zmq_ctx.clone(),
        worker_response_rpc_addr,
    )
    .await
    .expect("Unable to attach to daemon");

    // Now make the pub-sub client to the daemon and listen.
    let sub = tmq::subscribe(&zmq_ctx)
        .connect(worker_request_rpc_addr)
        .map_err(WorkerRpcError::UnableToConnectToDaemon)?;
    let mut sub = sub
        .subscribe(WORKER_BROADCAST_TOPIC)
        .map_err(WorkerRpcError::UnableToConnectToDaemon)?;

    loop {
        if kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let event = workers_events_recv(&mut sub)
            .await
            .map_err(WorkerRpcError::RpcError)?;

        let ctx = zmq_ctx.clone();
        let worker_token = worker_token.clone();
        let perform_p = perform.clone();
        tokio::spawn(process_fb(
            event,
            ctx,
            worker_response_rpc_addr.to_string(),
            my_id,
            worker_token,
            worker_type,
            kill_switch.clone(),
            perform_p,
        ));
    }
    Ok(())
}

async fn process_fb<ProcessFunc, Fut>(
    event: WorkerMessage,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    my_id: Uuid,
    worker_token: WorkerToken,
    worker_type: Symbol,
    kill_switch: Arc<AtomicBool>,
    perform: Arc<ProcessFunc>,
) where
    ProcessFunc: Fn(WorkerToken, Uuid, Symbol, Obj, Vec<Var>, Option<std::time::Duration>) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = Result<Var, WorkerError>> + Send + 'static + Sync,
{
    let rpc_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(&rpc_address)
        .expect("Unable to bind RPC server for connection");
    let mut rpc_client = WorkerRpcSendClient::new(rpc_request_sock);

    // Work directly with flatbuffer references to avoid copying
    let message_union = match event.message() {
        Ok(msg) => msg,
        Err(e) => {
            info!("Failed to parse worker message: {}", e);
            return;
        }
    };

    match message_union {
        moor_rpc::DaemonToWorkerMessageUnionRef::PingWorkers(_) => {
            rpc_client
                .make_worker_rpc_call_fb_pong(&worker_token, my_id, worker_type)
                .await
                .expect("Unable to send pong to daemon");
        }
        moor_rpc::DaemonToWorkerMessageUnionRef::WorkerRequest(req) => {
            // Extract data directly from flatbuffer references - only copying the minimal data we need
            let worker_id_data = match req.worker_id().and_then(|id| id.data()) {
                Ok(data) => data,
                Err(e) => {
                    info!("Failed to get worker_id: {}", e);
                    return;
                }
            };
            let worker_id = match Uuid::from_slice(worker_id_data) {
                Ok(id) => id,
                Err(e) => {
                    info!("Invalid worker UUID: {}", e);
                    return;
                }
            };

            if worker_id != my_id {
                return;
            }

            let request_id_data = match req.id().and_then(|id| id.data()) {
                Ok(data) => data,
                Err(e) => {
                    info!("Failed to get request_id: {}", e);
                    return;
                }
            };
            let request_id = match Uuid::from_slice(request_id_data) {
                Ok(id) => id,
                Err(e) => {
                    info!("Invalid request UUID: {}", e);
                    return;
                }
            };

            let token_str = match req.token().and_then(|t| t.token()) {
                Ok(token) => token,
                Err(e) => {
                    info!("Failed to get token: {}", e);
                    return;
                }
            };
            let token = WorkerToken(token_str.to_owned());

            let perms_ref = match req.perms() {
                Ok(perms) => perms,
                Err(e) => {
                    info!("Failed to get perms: {}", e);
                    return;
                }
            };
            let perms_obj = match moor_rpc::Obj::try_from(perms_ref) {
                Ok(obj) => obj,
                Err(e) => {
                    info!("Failed to convert perms ref: {}", e);
                    return;
                }
            };
            let perms = match rpc_common::obj_from_flatbuffer_struct(&perms_obj) {
                Ok(obj) => obj,
                Err(e) => {
                    info!("Failed to decode perms: {}", e);
                    return;
                }
            };

            let request_vec = match req.request() {
                Ok(req) => req,
                Err(e) => {
                    info!("Failed to get request: {}", e);
                    return;
                }
            };
            let request = match request_vec
                .iter()
                .map(|var_bytes_result| {
                    let var_bytes = var_bytes_result.map_err(|e| {
                        rpc_common::RpcError::CouldNotDecode(format!(
                            "Failed to get var_bytes: {e}"
                        ))
                    })?;
                    let data = var_bytes.data().map_err(|e| {
                        rpc_common::RpcError::CouldNotDecode(format!(
                            "Failed to get var_bytes data: {e}"
                        ))
                    })?;
                    rpc_common::var_from_flatbuffer_bytes(data).map_err(|e| {
                        rpc_common::RpcError::CouldNotDecode(format!("Failed to decode var: {e}"))
                    })
                })
                .collect::<Result<Vec<_>, rpc_common::RpcError>>()
            {
                Ok(req) => req,
                Err(e) => {
                    info!("Failed to decode request: {}", e);
                    return;
                }
            };

            let timeout_ms = match req.timeout_ms() {
                Ok(ms) => ms,
                Err(e) => {
                    info!("Failed to get timeout_ms: {}", e);
                    return;
                }
            };
            let timeout = if timeout_ms == 0 {
                None
            } else {
                Some(std::time::Duration::from_millis(timeout_ms))
            };

            // Make an outbound HTTP request w/ request, pass timeout if needed
            let result = perform(
                token.clone(),
                request_id,
                worker_type,
                perms,
                request,
                timeout,
            )
            .await;
            match result {
                Ok(r) => {
                    rpc_client
                        .make_worker_rpc_call_fb_result(&worker_token, my_id, request_id, r)
                        .await
                        .expect("Unable to send response to daemon");
                }
                Err(e) => {
                    info!("Error performing request: {}", e);
                    rpc_client
                        .make_worker_rpc_call_fb_error(&worker_token, my_id, request_id, e)
                        .await
                        .expect("Unable to send error response to daemon");
                }
            }
        }
        moor_rpc::DaemonToWorkerMessageUnionRef::PleaseDie(die) => {
            let token_str = match die.token().and_then(|t| t.token()) {
                Ok(token) => token,
                Err(e) => {
                    info!("Failed to get token: {}", e);
                    return;
                }
            };
            let token = WorkerToken(token_str.to_owned());

            if token == worker_token {
                info!("Received please die from daemon");
                kill_switch.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}
