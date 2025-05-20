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

use crate::pubsub_client::workers_events_recv;
use crate::{WorkerRpcSendClient, attach_worker};
use moor_common::tasks::WorkerError;
use moor_var::{Obj, Symbol, Var};
use rpc_common::{
    DaemonToWorkerMessage, WORKER_BROADCAST_TOPIC, WorkerToDaemonMessage, WorkerToken,
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
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
    Fut: Future<Output = Result<Vec<Var>, WorkerError>> + Send + 'static + Sync,
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
        tokio::spawn(process(
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

async fn process<ProcessFunc, Fut>(
    event: DaemonToWorkerMessage,
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
    Fut: Future<Output = Result<Vec<Var>, WorkerError>> + Send + 'static + Sync,
{
    let rpc_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(&rpc_address)
        .expect("Unable to bind RPC server for connection");
    let mut rpc_client = WorkerRpcSendClient::new(rpc_request_sock);

    match event {
        DaemonToWorkerMessage::PingWorkers => {
            rpc_client
                .make_worker_rpc_call(
                    &worker_token,
                    my_id,
                    WorkerToDaemonMessage::Pong(worker_token.clone(), worker_type),
                )
                .await
                .expect("Unable to send pong to daemon");
        }
        DaemonToWorkerMessage::WorkerRequest {
            worker_id,
            token,
            id: request_id,
            perms,
            request,
            timeout,
        } => {
            if worker_id != my_id {
                return;
            }

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
                        .make_worker_rpc_call(
                            &worker_token,
                            my_id,
                            WorkerToDaemonMessage::RequestResult(
                                worker_token.clone(),
                                request_id,
                                r,
                            ),
                        )
                        .await
                        .expect("Unable to send response to daemon");
                }
                Err(e) => {
                    info!("Error performing request: {}", e);
                    rpc_client
                        .make_worker_rpc_call(
                            &worker_token,
                            my_id,
                            WorkerToDaemonMessage::RequestError(
                                worker_token.clone(),
                                request_id,
                                e,
                            ),
                        )
                        .await
                        .expect("Unable to send error response to daemon");
                }
            }
        }
        DaemonToWorkerMessage::PleaseDie(token, _) => {
            if token == worker_token {
                info!("Received please die from daemon");
                kill_switch.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}
