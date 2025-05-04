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

use crate::attach_worker;
use crate::pubsub_client::workers_events_recv;
use moor_var::Symbol;
use rpc_common::{DaemonToWorkerMessage, WORKER_BROADCAST_TOPIC, WorkerToken};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use thiserror::Error;
use tmq::TmqError;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("Unable to attach worker to daemon: {0}")]
    UnableToConnectToDaemon(TmqError),
    #[error("Unable processing worker event: {0}")]
    RpcError(rpc_common::RpcError),
}

pub async fn worker_loop<ProcessFunc, Fut>(
    kill_switch: &Arc<AtomicBool>,
    my_id: Uuid,
    worker_token: &WorkerToken,
    worker_response_rpc_addr: &String,
    worker_request_rpc_addr: &String,
    worker_type: Symbol,
    perform: ProcessFunc,
) -> Result<(), WorkerError>
where
    ProcessFunc:
        Fn(DaemonToWorkerMessage, tmq::Context, String, Uuid, WorkerToken, Arc<AtomicBool>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let zmq_ctx = tmq::Context::new();

    // First attempt to connect to the daemon and "attach" ourselves.
    let _rpc_client = attach_worker(
        worker_token,
        worker_type,
        my_id,
        zmq_ctx.clone(),
        worker_response_rpc_addr.clone(),
    )
    .await
    .expect("Unable to attach to daemon");

    // Now make the pub-sub client to the daemon and listen.
    let sub = tmq::subscribe(&zmq_ctx)
        .connect(worker_request_rpc_addr)
        .map_err(WorkerError::UnableToConnectToDaemon)?;
    let mut sub = sub
        .subscribe(WORKER_BROADCAST_TOPIC)
        .map_err(WorkerError::UnableToConnectToDaemon)?;

    loop {
        if kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let event = workers_events_recv(&mut sub)
            .await
            .map_err(WorkerError::RpcError)?;

        let addr = worker_response_rpc_addr.clone();
        let ctx = zmq_ctx.clone();
        let worker_token = worker_token.clone();
        tokio::spawn(perform(
            event,
            ctx,
            addr,
            my_id,
            worker_token,
            kill_switch.clone(),
        ));
    }
    Ok(())
}
