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

use crate::worker_rpc_client::WorkerRpcSendClient;
use moor_var::Symbol;
use rpc_common::{DaemonToWorkerReply, RpcError};
use tmq::request;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Start the worker session with the daemon, and return the RPC client to use for further
/// communication.
pub async fn attach_worker(
    worker_type: Symbol,
    worker_id: Uuid,
    zmq_ctx: tmq::Context,
    rpc_address: &str,
    curve_keys: Option<(String, String, String)>, // (client_secret, client_public, server_public) - Z85 encoded
) -> Result<WorkerRpcSendClient, RpcError> {
    // Establish the initial connection to the daemon, and send the worker token and our initial
    // listener list.
    let rpc_client = loop {
        let mut socket_builder = request(&zmq_ctx).set_rcvtimeo(5000).set_sndtimeo(5000);

        // Configure CURVE encryption if keys provided
        if let Some((client_secret, client_public, server_public)) = &curve_keys {
            // Decode Z85 keys to bytes
            let client_secret_bytes = zmq::z85_decode(client_secret).map_err(|_| {
                RpcError::CouldNotInitiateSession("Invalid client secret key".to_string())
            })?;
            let client_public_bytes = zmq::z85_decode(client_public).map_err(|_| {
                RpcError::CouldNotInitiateSession("Invalid client public key".to_string())
            })?;
            let server_public_bytes = zmq::z85_decode(server_public).map_err(|_| {
                RpcError::CouldNotInitiateSession("Invalid server public key".to_string())
            })?;

            socket_builder = socket_builder
                .set_curve_secretkey(&client_secret_bytes)
                .set_curve_publickey(&client_public_bytes)
                .set_curve_serverkey(&server_public_bytes);

            info!("CURVE encryption enabled for worker connection");
        }

        let rpc_request_sock = socket_builder
            .connect(rpc_address)
            .expect("Unable to bind RPC server for connection");

        // And let the RPC server know we're here, and it should start sending events on the
        // narrative subscription.
        let mut rpc_client = WorkerRpcSendClient::new(rpc_request_sock);

        info!(
            "Attaching worker type {} to daemon with worker id {} via {}",
            worker_type, worker_id, rpc_address
        );
        match rpc_client
            .make_worker_rpc_call_fb_attach(worker_id, worker_type)
            .await
        {
            Ok(DaemonToWorkerReply::Attached(_)) => {
                info!("Worker attached to daemon.");
                break rpc_client;
            }
            Ok(DaemonToWorkerReply::Ack) => {
                break rpc_client;
            }
            Ok(r) => {
                error!(
                    "Worker request rejected by daemon, unexpected reply: {:?}",
                    r
                );
                return Err(RpcError::CouldNotSend(
                    "Worker token rejected by daemon".to_string(),
                ));
            }
            Err(e) => {
                warn!(
                    "Error communicating with daemon: {} to send worker attachment",
                    e
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
        }
    };
    Ok(rpc_client)
}
