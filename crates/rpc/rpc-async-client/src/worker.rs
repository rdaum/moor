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
use rpc_common::{DaemonToWorkerReply, MOOR_WORKER_TOKEN_FOOTER, RpcError, WorkerToken};
use rusty_paseto::core::{Footer, Key, Paseto, PasetoAsymmetricPrivateKey, Payload, Public, V4};
use tmq::request;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Construct a PASETO token for a worker, to authenticate the worker itself to the daemon.
pub fn make_worker_token(private_key: &Key<64>, worker_id: Uuid) -> WorkerToken {
    let privkey: PasetoAsymmetricPrivateKey<V4, Public> =
        PasetoAsymmetricPrivateKey::from(private_key.as_ref());
    let token = Paseto::<V4, Public>::default()
        .set_footer(Footer::from(MOOR_WORKER_TOKEN_FOOTER))
        .set_payload(Payload::from(worker_id.to_string().as_str()))
        .try_sign(&privkey)
        .expect("Unable to build Paseto worker token");

    WorkerToken(token)
}

/// Start the worker session with the daemon, and return the RPC client to use for further
/// communication.
pub async fn attach_worker(
    worker_token: &WorkerToken,
    worker_type: Symbol,
    worker_id: Uuid,
    zmq_ctx: tmq::Context,
    rpc_address: &str,
) -> Result<WorkerRpcSendClient, RpcError> {
    // Establish the initial connection to the daemon, and send the worker token and our initial
    // listener list.
    let rpc_client = loop {
        let rpc_request_sock = request(&zmq_ctx)
            .set_rcvtimeo(100)
            .set_sndtimeo(100)
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
            .make_worker_rpc_call_fb_attach(worker_token, worker_id, worker_type)
            .await
        {
            Ok(DaemonToWorkerReply::Attached(_, _)) => {
                info!("Worker token accepted by daemon.");
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
