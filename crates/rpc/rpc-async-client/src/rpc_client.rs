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

use moor_schema::rpc as moor_rpc;
use planus::Builder;
use rpc_common::{RpcError, uuid_fb};
use std::collections::VecDeque;
use std::sync::Arc;
use tmq::{Multipart, request_reply::RequestSender};
use tokio::sync::Mutex;
use tracing::{debug, error};
use uuid::Uuid;

const DEFAULT_SOCK_CONNECT_TIMEOUT_MS: i32 = 5000;
const DEFAULT_SOCK_RECEIVE_TIMEOUT_MS: i32 = 5000;

/// Configuration for the RPC client
#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub max_pool_size: usize,
    pub connect_timeout_ms: i32,
    pub receive_timeout_ms: i32,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            max_pool_size: 10,
            connect_timeout_ms: DEFAULT_SOCK_CONNECT_TIMEOUT_MS,
            receive_timeout_ms: DEFAULT_SOCK_RECEIVE_TIMEOUT_MS,
        }
    }
}

/// CURVE encryption keys for secure connections
#[derive(Debug, Clone)]
pub struct CurveKeys {
    pub client_secret: String,
    pub client_public: String,
    pub server_public: String,
}

/// RPC client with connection pooling and cancellation safety
pub struct RpcClient {
    zmq_context: Arc<tmq::Context>,
    rpc_addr: String,
    curve_keys: Option<CurveKeys>,
    config: RpcConfig,
    connection_pool: Mutex<VecDeque<RequestSender>>,
}

impl Clone for RpcClient {
    fn clone(&self) -> Self {
        Self {
            zmq_context: self.zmq_context.clone(),
            rpc_addr: self.rpc_addr.clone(),
            curve_keys: self.curve_keys.clone(),
            config: self.config.clone(),
            connection_pool: Mutex::new(VecDeque::new()), // New pool for clone
        }
    }
}

/// Socket guard that ensures socket cleanup regardless of cancellation
struct SocketGuard<'a> {
    client: &'a RpcClient,
    socket: Option<RequestSender>,
}

impl<'a> SocketGuard<'a> {
    /// Create a new socket guard, acquiring a socket from the pool
    async fn new(client: &'a RpcClient) -> Result<Self, RpcError> {
        let socket = client.acquire_socket().await?;
        Ok(Self {
            client,
            socket: Some(socket),
        })
    }

    /// Take the socket for use in an RPC call
    fn take_socket(&mut self) -> RequestSender {
        self.socket.take().expect("Socket should be present")
    }

    /// Return the socket to the pool
    async fn return_socket(&mut self, socket: RequestSender) {
        self.socket = Some(socket);
    }
}

impl<'a> Drop for SocketGuard<'a> {
    fn drop(&mut self) {
        // If the socket is still present when the guard is dropped,
        // return it to the pool. This handles cancellation scenarios.
        if let Some(socket) = self.socket.take() {
            // Use tokio::spawn to return the socket asynchronously
            let client = self.client.clone();
            tokio::spawn(async move {
                client.return_socket(socket).await;
            });
        }
    }
}

impl RpcClient {
    /// Create a new managed RPC client
    pub fn new(
        zmq_context: Arc<tmq::Context>,
        rpc_addr: String,
        curve_keys: Option<CurveKeys>,
        config: RpcConfig,
    ) -> Self {
        Self {
            zmq_context,
            rpc_addr,
            curve_keys,
            config,
            connection_pool: Mutex::new(VecDeque::new()),
        }
    }

    /// Create a new managed RPC client with default configuration
    pub fn new_with_defaults(
        zmq_context: Arc<tmq::Context>,
        rpc_addr: String,
        curve_keys: Option<CurveKeys>,
    ) -> Self {
        Self::new(zmq_context, rpc_addr, curve_keys, RpcConfig::default())
    }

    /// Make a client RPC call with cancellation safety and connection pooling
    pub async fn make_client_rpc_call(
        &self,
        client_id: Uuid,
        rpc_msg: moor_rpc::HostClientToDaemonMessage,
    ) -> Result<Vec<u8>, RpcError> {
        // Use a guard pattern to ensure socket cleanup regardless of cancellation
        let mut socket_guard = SocketGuard::new(self).await?;
        let socket = socket_guard.take_socket();

        // Perform the RPC call - socket cleanup is guaranteed by the guard
        match Self::perform_rpc_call(socket, client_id, rpc_msg).await {
            Ok((response, socket)) => {
                // Successfully completed - return socket to pool
                socket_guard.return_socket(socket).await;
                Ok(response)
            }
            Err((error, socket)) => {
                // Even on error, attempt to return the socket if we have it
                if let Some(socket) = socket {
                    socket_guard.return_socket(socket).await;
                }
                Err(error)
            }
        }
    }

    /// Make a host RPC call with cancellation safety and connection pooling
    pub async fn make_host_rpc_call(
        &self,
        host_id: Uuid,
        rpc_message: moor_rpc::HostToDaemonMessage,
    ) -> Result<Vec<u8>, RpcError> {
        // Use a guard pattern to ensure socket cleanup regardless of cancellation
        let mut socket_guard = SocketGuard::new(self).await?;
        let socket = socket_guard.take_socket();

        // Perform the RPC call - socket cleanup is guaranteed by the guard
        match Self::perform_host_rpc_call(socket, host_id, rpc_message).await {
            Ok((response, socket)) => {
                // Successfully completed - return socket to pool
                socket_guard.return_socket(socket).await;
                Ok(response)
            }
            Err((error, socket)) => {
                // Even on error, attempt to return the socket if we have it
                if let Some(socket) = socket {
                    socket_guard.return_socket(socket).await;
                }
                Err(error)
            }
        }
    }

    /// Acquire a socket from the pool or create a new one
    async fn acquire_socket(&self) -> Result<RequestSender, RpcError> {
        let mut pool = self.connection_pool.lock().await;

        if let Some(socket) = pool.pop_front() {
            debug!("Reusing socket from pool");
            return Ok(socket);
        }

        // Create a new socket
        self.create_socket().await
    }

    /// Return a socket to the pool, discarding if pool is full
    async fn return_socket(&self, socket: RequestSender) {
        let mut pool = self.connection_pool.lock().await;

        if pool.len() < self.config.max_pool_size {
            pool.push_back(socket);
        } else {
            drop(socket);
        }
    }

    /// Create a new socket with proper configuration
    async fn create_socket(&self) -> Result<RequestSender, RpcError> {
        let mut socket_builder = tmq::request(&self.zmq_context)
            .set_rcvtimeo(self.config.receive_timeout_ms)
            .set_sndtimeo(self.config.connect_timeout_ms)
            // Fail immediately if no connection instead of queuing messages indefinitely
            .set_immediate(true)
            // Don't linger on close - drop queued messages immediately
            .set_linger(0);

        // Configure CURVE encryption if keys provided
        if let Some(curve_keys) = &self.curve_keys {
            socket_builder = super::configure_curve_client(
                socket_builder,
                &curve_keys.client_secret,
                &curve_keys.client_public,
                &curve_keys.server_public,
            )
            .map_err(|e| RpcError::Fatal(format!("Failed to configure CURVE: {}", e)))?;
        }

        socket_builder
            .connect(&self.rpc_addr)
            .map_err(|e| RpcError::Fatal(format!("Failed to connect to RPC server: {}", e)))
    }

    /// Perform an RPC call with guaranteed socket cleanup
    async fn perform_rpc_call(
        socket: RequestSender,
        client_id: Uuid,
        rpc_msg: moor_rpc::HostClientToDaemonMessage,
    ) -> Result<(Vec<u8>, RequestSender), (RpcError, Option<RequestSender>)> {
        // Serialize the message to FlatBuffer bytes
        let mut builder = Builder::new();
        let rpc_msg_payload = builder.finish(&rpc_msg, None).to_vec();

        // Build the MessageType discriminator
        let client_msg = moor_rpc::HostClientToDaemonMsg {
            client_data: client_id.as_bytes().to_vec(),
            message: Box::new(rpc_msg),
        };
        let message_type = moor_rpc::MessageType {
            message: moor_rpc::MessageTypeUnion::HostClientToDaemonMsg(Box::new(client_msg)),
        };
        let mut discriminator_builder = Builder::new();
        let message_type_bytes = discriminator_builder.finish(&message_type, None).to_vec();

        let message = Multipart::from(vec![message_type_bytes, rpc_msg_payload]);

        let rpc_reply_sock = match socket.send(message).await {
            Ok(rpc_reply_sock) => rpc_reply_sock,
            Err(e) => {
                error!(
                    "Unable to send connection establish request to RPC server: {}",
                    e
                );
                // Note: socket is consumed by send(), so we can't return it here
                // The socket is lost on send failure - this is a limitation of the tmq API
                return Err((RpcError::CouldNotSend(e.to_string()), None));
            }
        };

        let (msg, recv_sock) = match rpc_reply_sock.recv().await {
            Ok((msg, recv_sock)) => (msg, recv_sock),
            Err(e) => {
                error!(
                    "Unable to receive connection establish reply from RPC server: {}",
                    e
                );
                // Note: rpc_reply_sock is consumed by recv() even on failure
                // The socket is lost on recv failure - this is a limitation of the tmq API
                return Err((RpcError::CouldNotReceive(e.to_string()), None));
            }
        };

        // Return raw reply bytes - caller will decode the FlatBuffer
        let reply_bytes = msg[0].to_vec();

        Ok((reply_bytes, recv_sock))
    }

    /// Perform a host RPC call with guaranteed socket cleanup
    async fn perform_host_rpc_call(
        socket: RequestSender,
        host_id: Uuid,
        rpc_message: moor_rpc::HostToDaemonMessage,
    ) -> Result<(Vec<u8>, RequestSender), (RpcError, Option<RequestSender>)> {
        // Serialize the message to FlatBuffer bytes
        let mut builder = Builder::new();
        let rpc_msg_payload = builder.finish(&rpc_message, None).to_vec();

        // Build the MessageType discriminator
        let host_msg = moor_rpc::HostToDaemonMsg {
            host_id: uuid_fb(host_id),
            message: Box::new(rpc_message),
        };
        let message_type = moor_rpc::MessageType {
            message: moor_rpc::MessageTypeUnion::HostToDaemonMsg(Box::new(host_msg)),
        };
        let mut discriminator_builder = Builder::new();
        let message_type_bytes = discriminator_builder.finish(&message_type, None).to_vec();

        let message = Multipart::from(vec![message_type_bytes, rpc_msg_payload]);

        let rpc_reply_sock = match socket.send(message).await {
            Ok(rpc_reply_sock) => rpc_reply_sock,
            Err(e) => {
                error!(
                    "Unable to send connection establish request to RPC server: {}",
                    e
                );
                // Note: socket is consumed by send(), so we can't return it here
                // The socket is lost on send failure - this is a limitation of the tmq API
                return Err((RpcError::CouldNotSend(e.to_string()), None));
            }
        };

        let (msg, recv_sock) = match rpc_reply_sock.recv().await {
            Ok((msg, recv_sock)) => (msg, recv_sock),
            Err(e) => {
                error!(
                    "Unable to receive connection establish reply from RPC server: {}",
                    e
                );
                // Note: rpc_reply_sock is consumed by recv() even on failure
                // The socket is lost on recv failure - this is a limitation of the tmq API
                return Err((RpcError::CouldNotReceive(e.to_string()), None));
            }
        };

        // Return raw reply bytes - caller will decode the FlatBuffer
        let reply_bytes = msg[0].to_vec();

        Ok((reply_bytes, recv_sock))
    }

    /// Get current pool size for monitoring
    pub async fn pool_size(&self) -> usize {
        let pool = self.connection_pool.lock().await;
        pool.len()
    }

    /// Clear the connection pool (useful for cleanup)
    pub async fn clear_pool(&self) {
        let mut pool = self.connection_pool.lock().await;
        pool.clear();
        debug!("Cleared RPC connection pool");
    }
}
