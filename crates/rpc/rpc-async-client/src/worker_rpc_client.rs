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

use moor_common::tasks::WorkerError;
use moor_schema::{convert::var_to_flatbuffer, rpc as moor_rpc};
use moor_var::{Symbol, Var};
use planus::{Builder, ReadAsRoot};
use rpc_common::{
    DaemonToWorkerReply, RpcError, mk_attach_worker_msg, mk_request_error_msg,
    mk_request_result_msg, mk_worker_pong_msg,
};
use tmq::{Multipart, request_reply::RequestSender};
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

    pub async fn make_worker_rpc_call_fb_pong(
        &mut self,
        worker_id: Uuid,
        worker_type: Symbol,
    ) -> Result<(), RpcError> {
        let fb_message = mk_worker_pong_msg(worker_id, &worker_type);

        let worker_id_bytes = worker_id.as_bytes().to_vec();

        let mut builder = Builder::new();
        let rpc_msg_payload = builder.finish(&fb_message, None);

        let message = Multipart::from(vec![worker_id_bytes, rpc_msg_payload.to_vec()]);
        let rpc_request_sock = self.rcp_request_sock.take().ok_or_else(|| {
            RpcError::CouldNotSend("RPC request socket not initialized".to_string())
        })?;
        let rpc_reply_sock = match rpc_request_sock.send(message).await {
            Ok(rpc_reply_sock) => rpc_reply_sock,
            Err(e) => {
                error!("Unable to send worker pong request to RPC server: {}", e);
                return Err(RpcError::CouldNotSend(e.to_string()));
            }
        };

        let (_msg, recv_sock) = match rpc_reply_sock.recv().await {
            Ok((msg, recv_sock)) => (msg, recv_sock),
            Err(e) => {
                error!("Unable to receive worker pong reply from RPC server: {}", e);
                return Err(RpcError::CouldNotReceive(e.to_string()));
            }
        };

        self.rcp_request_sock = Some(recv_sock);
        Ok(())
    }

    pub async fn make_worker_rpc_call_fb_result(
        &mut self,
        worker_id: Uuid,
        request_id: Uuid,
        result: Var,
    ) -> Result<(), RpcError> {
        let result_fb = var_to_flatbuffer(&result)
            .map_err(|e| RpcError::CouldNotSend(format!("Failed to serialize result: {e}")))?;

        let fb_message = mk_request_result_msg(worker_id, request_id, result_fb);

        let worker_id_bytes = worker_id.as_bytes().to_vec();

        let mut builder = Builder::new();
        let rpc_msg_payload = builder.finish(&fb_message, None);

        let message = Multipart::from(vec![worker_id_bytes, rpc_msg_payload.to_vec()]);
        let rcp_request_sock = self.rcp_request_sock.take().ok_or_else(|| {
            RpcError::CouldNotSend("RPC request socket not initialized".to_string())
        })?;
        let rpc_reply_sock = match rcp_request_sock.send(message).await {
            Ok(rpc_reply_sock) => rpc_reply_sock,
            Err(e) => {
                error!("Unable to send request result to RPC server: {}", e);
                return Err(RpcError::CouldNotSend(e.to_string()));
            }
        };

        let (_msg, recv_sock) = match rpc_reply_sock.recv().await {
            Ok((msg, recv_sock)) => (msg, recv_sock),
            Err(e) => {
                error!(
                    "Unable to receive request result reply from RPC server: {}",
                    e
                );
                return Err(RpcError::CouldNotReceive(e.to_string()));
            }
        };

        self.rcp_request_sock = Some(recv_sock);
        Ok(())
    }

    pub async fn make_worker_rpc_call_fb_attach(
        &mut self,
        worker_id: Uuid,
        worker_type: Symbol,
    ) -> Result<DaemonToWorkerReply, RpcError> {
        let fb_message = mk_attach_worker_msg(worker_id, &worker_type);

        let worker_id_bytes = worker_id.as_bytes().to_vec();

        let mut builder = Builder::new();
        let rpc_msg_payload = builder.finish(&fb_message, None);

        let message = Multipart::from(vec![worker_id_bytes, rpc_msg_payload.to_vec()]);
        let rpc_request_sock = self.rcp_request_sock.take().ok_or_else(|| {
            RpcError::CouldNotSend("RPC request socket not initialized".to_string())
        })?;
        let rpc_reply_sock = match rpc_request_sock.send(message).await {
            Ok(rpc_reply_sock) => rpc_reply_sock,
            Err(e) => {
                error!("Unable to send worker attach request to RPC server: {}", e);
                return Err(RpcError::CouldNotSend(e.to_string()));
            }
        };

        let (msg, recv_sock) = match rpc_reply_sock.recv().await {
            Ok((msg, recv_sock)) => (msg, recv_sock),
            Err(e) => {
                error!(
                    "Unable to receive worker attach reply from RPC server: {}",
                    e
                );
                return Err(RpcError::CouldNotReceive(e.to_string()));
            }
        };

        // Decode flatbuffer response
        let fb_reply = moor_rpc::DaemonToWorkerReplyRef::read_as_root(&msg[0]).map_err(|e| {
            RpcError::CouldNotDecode(format!("Unable to decode flatbuffer daemon reply: {e}"))
        })?;

        let reply_union = fb_reply
            .reply()
            .map_err(|e| RpcError::CouldNotDecode(format!("Unable to decode reply union: {e}")))?;

        let reply = match reply_union {
            moor_rpc::DaemonToWorkerReplyUnionRef::WorkerAck(_) => DaemonToWorkerReply::Ack,
            moor_rpc::DaemonToWorkerReplyUnionRef::WorkerRejected(rejected) => {
                let reason = rejected
                    .reason()
                    .ok()
                    .flatten()
                    .unwrap_or("Unknown reason")
                    .to_string();
                DaemonToWorkerReply::Rejected(reason)
            }
            moor_rpc::DaemonToWorkerReplyUnionRef::WorkerAttached(attached) => {
                let worker_id_data = attached
                    .worker_id()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Failed to get worker_id: {e}")))?
                    .data()
                    .map_err(|e| {
                        RpcError::CouldNotDecode(format!("Failed to get worker_id data: {e}"))
                    })?;
                let worker_id = Uuid::from_slice(worker_id_data)
                    .map_err(|e| RpcError::CouldNotDecode(format!("Invalid worker UUID: {e}")))?;

                DaemonToWorkerReply::Attached(worker_id)
            }
            moor_rpc::DaemonToWorkerReplyUnionRef::WorkerAuthFailed(auth_failed) => {
                let reason = auth_failed
                    .reason()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Failed to get reason: {e}")))?
                    .to_string();
                DaemonToWorkerReply::AuthFailed(reason)
            }
            moor_rpc::DaemonToWorkerReplyUnionRef::WorkerInvalidPayload(invalid) => {
                let reason = invalid
                    .reason()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Failed to get reason: {e}")))?
                    .to_string();
                DaemonToWorkerReply::InvalidPayload(reason)
            }
            moor_rpc::DaemonToWorkerReplyUnionRef::WorkerUnknownRequest(unknown) => {
                let request_id_data = unknown
                    .request_id()
                    .map_err(|e| {
                        RpcError::CouldNotDecode(format!("Failed to get request_id: {e}"))
                    })?
                    .data()
                    .map_err(|e| {
                        RpcError::CouldNotDecode(format!("Failed to get request_id data: {e}"))
                    })?;
                let request_id = Uuid::from_slice(request_id_data)
                    .map_err(|e| RpcError::CouldNotDecode(format!("Invalid request UUID: {e}")))?;
                DaemonToWorkerReply::UnknownRequest(request_id)
            }
            moor_rpc::DaemonToWorkerReplyUnionRef::WorkerNotRegistered(not_registered) => {
                let worker_id_data = not_registered
                    .worker_id()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Failed to get worker_id: {e}")))?
                    .data()
                    .map_err(|e| {
                        RpcError::CouldNotDecode(format!("Failed to get worker_id data: {e}"))
                    })?;
                let worker_id = Uuid::from_slice(worker_id_data)
                    .map_err(|e| RpcError::CouldNotDecode(format!("Invalid worker UUID: {e}")))?;
                DaemonToWorkerReply::NotRegistered(worker_id)
            }
        };

        self.rcp_request_sock = Some(recv_sock);
        Ok(reply)
    }

    pub async fn make_worker_rpc_call_fb_error(
        &mut self,
        worker_id: Uuid,
        request_id: Uuid,
        error: WorkerError,
    ) -> Result<(), RpcError> {
        let fb_error = match error {
            WorkerError::PermissionDenied(msg) => {
                moor_rpc::WorkerErrorUnion::WorkerPermissionDenied(Box::new(
                    moor_rpc::WorkerPermissionDenied { message: msg },
                ))
            }
            WorkerError::InvalidRequest(msg) => moor_rpc::WorkerErrorUnion::WorkerInvalidRequest(
                Box::new(moor_rpc::WorkerInvalidRequest { message: msg }),
            ),
            WorkerError::InternalError(msg) => moor_rpc::WorkerErrorUnion::WorkerInternalError(
                Box::new(moor_rpc::WorkerInternalError { message: msg }),
            ),
            WorkerError::RequestTimedOut(msg) => moor_rpc::WorkerErrorUnion::WorkerRequestTimedOut(
                Box::new(moor_rpc::WorkerRequestTimedOut { message: msg }),
            ),
            WorkerError::RequestError(msg) => moor_rpc::WorkerErrorUnion::WorkerRequestError(
                Box::new(moor_rpc::WorkerRequestError { message: msg }),
            ),
            WorkerError::WorkerDetached(msg) => {
                moor_rpc::WorkerErrorUnion::WorkerDetached(Box::new(moor_rpc::WorkerDetached {
                    message: msg,
                }))
            }
            WorkerError::NoWorkerAvailable(symbol) => {
                moor_rpc::WorkerErrorUnion::NoWorkerAvailable(Box::new(
                    moor_rpc::NoWorkerAvailable {
                        worker_type: Box::new(moor_rpc::Symbol {
                            value: symbol.as_arc_str().to_string(),
                        }),
                    },
                ))
            }
        };

        let fb_message = mk_request_error_msg(
            worker_id,
            request_id,
            moor_rpc::WorkerError { error: fb_error },
        );

        let worker_id_bytes = worker_id.as_bytes().to_vec();

        let mut builder = Builder::new();
        let rpc_msg_payload = builder.finish(&fb_message, None);

        let message = Multipart::from(vec![worker_id_bytes, rpc_msg_payload.to_vec()]);
        let rcp_request_sock = self.rcp_request_sock.take().ok_or_else(|| {
            RpcError::CouldNotSend("RPC request socket not initialized".to_string())
        })?;
        let rpc_reply_sock = match rcp_request_sock.send(message).await {
            Ok(rpc_reply_sock) => rpc_reply_sock,
            Err(e) => {
                error!("Unable to send request error to RPC server: {}", e);
                return Err(RpcError::CouldNotSend(e.to_string()));
            }
        };

        let (_msg, recv_sock) = match rpc_reply_sock.recv().await {
            Ok((msg, recv_sock)) => (msg, recv_sock),
            Err(e) => {
                error!(
                    "Unable to receive request error reply from RPC server: {}",
                    e
                );
                return Err(RpcError::CouldNotReceive(e.to_string()));
            }
        };

        self.rcp_request_sock = Some(recv_sock);
        Ok(())
    }
}
