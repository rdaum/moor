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

use rpc_common::{DaemonToWorkerReply, RpcError, WorkerToDaemonMessage, WorkerToken};
use tmq::Multipart;
use tmq::request_reply::RequestSender;
use tracing::error;
use uuid::Uuid;

/// Lightweight wrapper around the TMQ RequestSender to make it slightly simpler to make RPC
/// requests, reducing some boiler plate.
pub struct WorkerRpcSendClient {
    // Note: this becomes None while a request is in flight, and is replaced with Some() as the
    // response is received.
    rcp_request_sock: Option<RequestSender>,
}

impl WorkerRpcSendClient {
    pub fn new(request_sender: RequestSender) -> Self {
        Self {
            rcp_request_sock: Some(request_sender),
        }
    }

    pub async fn make_worker_rpc_call(
        &mut self,
        worker_token: &WorkerToken,
        worker_id: Uuid,
        rpc_message: WorkerToDaemonMessage,
    ) -> Result<DaemonToWorkerReply, RpcError> {
        // (worker_token, worker_id, request)
        let worker_token_bytes = worker_token.0.as_bytes().to_vec();
        let worker_id_bytes = worker_id.as_bytes().to_vec();

        let rpc_msg_payload = bincode::encode_to_vec(&rpc_message, bincode::config::standard())
            .map_err(|e| RpcError::CouldNotSend(e.to_string()))?;
        let message = Multipart::from(vec![worker_token_bytes, worker_id_bytes, rpc_msg_payload]);
        let rpc_request_sock = self.rcp_request_sock.take().ok_or_else(|| {
            RpcError::CouldNotSend("RPC request socket not initialized".to_string())
        })?;
        let rpc_reply_sock = match rpc_request_sock.send(message).await {
            Ok(rpc_reply_sock) => rpc_reply_sock,
            Err(e) => {
                error!(
                    "Unable to send connection establish request to RPC server: {}",
                    e
                );
                return Err(RpcError::CouldNotSend(e.to_string()));
            }
        };

        let (msg, recv_sock) = match rpc_reply_sock.recv().await {
            Ok((msg, recv_sock)) => (msg, recv_sock),
            Err(e) => {
                error!(
                    "Unable to receive connection establish reply from RPC server: {}",
                    e
                );
                return Err(RpcError::CouldNotReceive(e.to_string()));
            }
        };

        match bincode::decode_from_slice(&msg[0], bincode::config::standard()) {
            Ok((msg, _)) => {
                self.rcp_request_sock = Some(recv_sock);
                Ok(msg)
            }
            Err(e) => {
                error!("Unable to decode RPC response: {}", e);
                Err(RpcError::CouldNotDecode(e.to_string()))
            }
        }
    }
}
