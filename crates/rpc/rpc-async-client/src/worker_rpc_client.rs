// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
use std::sync::Arc;
use tmq::{Multipart, request_reply::RequestSender};
use tokio::sync::Mutex;
use tracing::error;
use uuid::Uuid;

/// Socket guard that ensures socket cleanup regardless of cancellation
struct WorkerSocketGuard {
    client: Arc<Mutex<Option<RequestSender>>>,
    socket: Option<RequestSender>,
}

impl WorkerSocketGuard {
    /// Create a new socket guard with the given socket
    fn new(client: Arc<Mutex<Option<RequestSender>>>, socket: RequestSender) -> Self {
        Self {
            client,
            socket: Some(socket),
        }
    }

    /// Take the socket for use in an RPC call
    fn take_socket(&mut self) -> RequestSender {
        self.socket.take().expect("Socket should be present")
    }

    /// Return the socket after successful completion
    async fn return_socket(&mut self, socket: RequestSender) {
        self.socket = Some(socket);
    }
}

impl Drop for WorkerSocketGuard {
    fn drop(&mut self) {
        // If the socket is still present when the guard is dropped,
        // return it to the client. This handles cancellation scenarios.
        if let Some(socket) = self.socket.take() {
            // Use tokio::spawn to return the socket asynchronously
            let client = self.client.clone();
            tokio::spawn(async move {
                let mut socket_guard = client.lock().await;
                *socket_guard = Some(socket);
            });
        }
    }
}

/// Lightweight wrapper around the TMQ RequestSender to make it slightly simpler to make RPC
/// requests, reducing some boiler plate.
pub struct WorkerRpcSendClient {
    socket: Arc<Mutex<Option<RequestSender>>>,
}

impl WorkerRpcSendClient {
    pub fn new(request_sender: RequestSender) -> Self {
        Self {
            socket: Arc::new(Mutex::new(Some(request_sender))),
        }
    }

    pub async fn make_worker_rpc_call_fb_pong(
        &self,
        worker_id: Uuid,
        worker_type: Symbol,
    ) -> Result<(), RpcError> {
        // Acquire socket and create guard for cancellation safety
        let socket = self.acquire_socket().await?;
        let mut socket_guard = WorkerSocketGuard::new(self.socket.clone(), socket);
        let socket = socket_guard.take_socket();

        let fb_message = mk_worker_pong_msg(worker_id, &worker_type);

        let worker_id_bytes = worker_id.as_bytes().to_vec();

        let mut builder = Builder::new();
        let rpc_msg_payload = builder.finish(&fb_message, None);

        let message = Multipart::from(vec![worker_id_bytes, rpc_msg_payload.to_vec()]);

        // Perform the RPC call - socket cleanup is guaranteed by the guard
        match Self::perform_worker_rpc_call(socket, message).await {
            Ok(socket) => {
                // Successfully completed - return socket
                socket_guard.return_socket(socket).await;
                Ok(())
            }
            Err(error) => {
                // Socket is already cleaned up by the guard on error
                Err(error)
            }
        }
    }

    pub async fn make_worker_rpc_call_fb_result(
        &self,
        worker_id: Uuid,
        request_id: Uuid,
        result: Var,
    ) -> Result<(), RpcError> {
        // Acquire socket and create guard for cancellation safety
        let socket = self.acquire_socket().await?;
        let mut socket_guard = WorkerSocketGuard::new(self.socket.clone(), socket);
        let socket = socket_guard.take_socket();

        let result_fb = var_to_flatbuffer(&result)
            .map_err(|e| RpcError::CouldNotSend(format!("Failed to serialize result: {e}")))?;

        let fb_message = mk_request_result_msg(worker_id, request_id, result_fb);

        let worker_id_bytes = worker_id.as_bytes().to_vec();

        let mut builder = Builder::new();
        let rpc_msg_payload = builder.finish(&fb_message, None);

        let message = Multipart::from(vec![worker_id_bytes, rpc_msg_payload.to_vec()]);

        // Perform the RPC call - socket cleanup is guaranteed by the guard
        match Self::perform_worker_rpc_call(socket, message).await {
            Ok(socket) => {
                // Successfully completed - return socket
                socket_guard.return_socket(socket).await;
                Ok(())
            }
            Err(error) => {
                // Socket is already cleaned up by the guard on error
                Err(error)
            }
        }
    }

    pub async fn make_worker_rpc_call_fb_attach(
        &self,
        worker_id: Uuid,
        worker_type: Symbol,
    ) -> Result<DaemonToWorkerReply, RpcError> {
        // Acquire socket and create guard for cancellation safety
        let socket = self.acquire_socket().await?;
        let mut socket_guard = WorkerSocketGuard::new(self.socket.clone(), socket);
        let socket = socket_guard.take_socket();

        let fb_message = mk_attach_worker_msg(worker_id, &worker_type);

        let worker_id_bytes = worker_id.as_bytes().to_vec();

        let mut builder = Builder::new();
        let rpc_msg_payload = builder.finish(&fb_message, None);

        let message = Multipart::from(vec![worker_id_bytes, rpc_msg_payload.to_vec()]);

        // Perform the RPC call - socket cleanup is guaranteed by the guard
        match Self::perform_worker_rpc_call_with_response(socket, message).await {
            Ok((reply_bytes, socket)) => {
                // Successfully completed - return socket
                socket_guard.return_socket(socket).await;

                // Decode flatbuffer response
                let fb_reply = moor_rpc::DaemonToWorkerReplyRef::read_as_root(&reply_bytes)
                    .map_err(|e| {
                        RpcError::CouldNotDecode(format!(
                            "Unable to decode flatbuffer daemon reply: {e}"
                        ))
                    })?;

                let reply_union = fb_reply.reply().map_err(|e| {
                    RpcError::CouldNotDecode(format!("Unable to decode reply union: {e}"))
                })?;

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
                            .map_err(|e| {
                                RpcError::CouldNotDecode(format!("Failed to get worker_id: {e}"))
                            })?
                            .data()
                            .map_err(|e| {
                                RpcError::CouldNotDecode(format!(
                                    "Failed to get worker_id data: {e}"
                                ))
                            })?;
                        let worker_id = Uuid::from_slice(worker_id_data).map_err(|e| {
                            RpcError::CouldNotDecode(format!("Invalid worker UUID: {e}"))
                        })?;

                        DaemonToWorkerReply::Attached(worker_id)
                    }
                    moor_rpc::DaemonToWorkerReplyUnionRef::WorkerAuthFailed(auth_failed) => {
                        let reason = auth_failed
                            .reason()
                            .map_err(|e| {
                                RpcError::CouldNotDecode(format!("Failed to get reason: {e}"))
                            })?
                            .to_string();
                        DaemonToWorkerReply::AuthFailed(reason)
                    }
                    moor_rpc::DaemonToWorkerReplyUnionRef::WorkerInvalidPayload(invalid) => {
                        let reason = invalid
                            .reason()
                            .map_err(|e| {
                                RpcError::CouldNotDecode(format!("Failed to get reason: {e}"))
                            })?
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
                                RpcError::CouldNotDecode(format!(
                                    "Failed to get request_id data: {e}"
                                ))
                            })?;
                        let request_id = Uuid::from_slice(request_id_data).map_err(|e| {
                            RpcError::CouldNotDecode(format!("Invalid request UUID: {e}"))
                        })?;
                        DaemonToWorkerReply::UnknownRequest(request_id)
                    }
                    moor_rpc::DaemonToWorkerReplyUnionRef::WorkerNotRegistered(not_registered) => {
                        let worker_id_data = not_registered
                            .worker_id()
                            .map_err(|e| {
                                RpcError::CouldNotDecode(format!("Failed to get worker_id: {e}"))
                            })?
                            .data()
                            .map_err(|e| {
                                RpcError::CouldNotDecode(format!(
                                    "Failed to get worker_id data: {e}"
                                ))
                            })?;
                        let worker_id = Uuid::from_slice(worker_id_data).map_err(|e| {
                            RpcError::CouldNotDecode(format!("Invalid worker UUID: {e}"))
                        })?;
                        DaemonToWorkerReply::NotRegistered(worker_id)
                    }
                };

                Ok(reply)
            }
            Err(error) => {
                // Socket is already cleaned up by the guard on error
                Err(error)
            }
        }
    }

    pub async fn make_worker_rpc_call_fb_error(
        &self,
        worker_id: Uuid,
        request_id: Uuid,
        error: WorkerError,
    ) -> Result<(), RpcError> {
        // Acquire socket and create guard for cancellation safety
        let socket = self.acquire_socket().await?;
        let mut socket_guard = WorkerSocketGuard::new(self.socket.clone(), socket);
        let socket = socket_guard.take_socket();

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

        // Perform the RPC call - socket cleanup is guaranteed by the guard
        match Self::perform_worker_rpc_call(socket, message).await {
            Ok(socket) => {
                // Successfully completed - return socket
                socket_guard.return_socket(socket).await;
                Ok(())
            }
            Err(error) => {
                // Socket is already cleaned up by the guard on error
                Err(error)
            }
        }
    }

    /// Acquire the socket from the shared state
    async fn acquire_socket(&self) -> Result<RequestSender, RpcError> {
        let mut socket_guard = self.socket.lock().await;
        socket_guard
            .take()
            .ok_or_else(|| RpcError::CouldNotSend("RPC request socket not initialized".to_string()))
    }

    /// Perform a worker RPC call that doesn't return response data
    async fn perform_worker_rpc_call(
        socket: RequestSender,
        message: Multipart,
    ) -> Result<RequestSender, RpcError> {
        let rpc_reply_sock = match socket.send(message).await {
            Ok(rpc_reply_sock) => rpc_reply_sock,
            Err(e) => {
                error!("Unable to send worker request to RPC server: {}", e);
                return Err(RpcError::CouldNotSend(e.to_string()));
            }
        };

        let (_msg, recv_sock) = match rpc_reply_sock.recv().await {
            Ok((msg, recv_sock)) => (msg, recv_sock),
            Err(e) => {
                error!("Unable to receive worker reply from RPC server: {}", e);
                return Err(RpcError::CouldNotReceive(e.to_string()));
            }
        };

        Ok(recv_sock)
    }

    /// Perform a worker RPC call that returns response data
    async fn perform_worker_rpc_call_with_response(
        socket: RequestSender,
        message: Multipart,
    ) -> Result<(Vec<u8>, RequestSender), RpcError> {
        let rpc_reply_sock = match socket.send(message).await {
            Ok(rpc_reply_sock) => rpc_reply_sock,
            Err(e) => {
                error!("Unable to send worker request to RPC server: {}", e);
                return Err(RpcError::CouldNotSend(e.to_string()));
            }
        };

        let (msg, recv_sock) = match rpc_reply_sock.recv().await {
            Ok((msg, recv_sock)) => (msg, recv_sock),
            Err(e) => {
                error!("Unable to receive worker reply from RPC server: {}", e);
                return Err(RpcError::CouldNotReceive(e.to_string()));
            }
        };

        // Return raw reply bytes
        let reply_bytes = msg[0].to_vec();
        Ok((reply_bytes, recv_sock))
    }
}
