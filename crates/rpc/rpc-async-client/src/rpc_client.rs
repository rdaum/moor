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

use moor_schema::rpc as moor_rpc;
use planus::Builder;
use rpc_common::{RpcError, uuid_fb};
use tmq::{Multipart, request_reply::RequestSender};
use tracing::{debug, error};
use uuid::Uuid;

/// Lightweight wrapper around the TMQ RequestSender to make it slightly simpler to make RPC
/// requests, reducing some boiler plate.
pub struct RpcSendClient {
    // Note: this becomes None while a request is in flight, and is replaced with Some() as the
    // response is received.
    rcp_request_sock: Option<RequestSender>,
}

impl RpcSendClient {
    pub fn new(request_sender: RequestSender) -> Self {
        Self {
            rcp_request_sock: Some(request_sender),
        }
    }

    /// Call the ZMQ RPC (REQ/REPLY) endpoint with a `ClientRequest`, and receive a `ServerResponse`.
    pub async fn make_client_rpc_call(
        &mut self,
        client_id: Uuid,
        rpc_msg: moor_rpc::HostClientToDaemonMessage,
    ) -> Result<Vec<u8>, RpcError> {
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
        let rpc_request_sock = self.rcp_request_sock.take().ok_or_else(|| {
            RpcError::CouldNotSend("RPC request socket not initialized".to_string())
        })?;

        debug!(client_id = %client_id, "Sending RPC client request to daemon");
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

        debug!(client_id = %client_id, "Waiting for RPC client response from daemon");
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

        // Return raw reply bytes - caller will decode the FlatBuffer
        let reply_bytes = msg[0].to_vec();
        debug!(client_id = %client_id, response_bytes = reply_bytes.len(), "Received RPC client response from daemon");
        self.rcp_request_sock = Some(recv_sock);
        Ok(reply_bytes)
    }

    pub async fn make_host_rpc_call(
        &mut self,
        host_id: Uuid,
        rpc_message: moor_rpc::HostToDaemonMessage,
    ) -> Result<Vec<u8>, RpcError> {
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
        let rpc_request_sock = self.rcp_request_sock.take().ok_or_else(|| {
            RpcError::CouldNotSend("RPC request socket not initialized".to_string())
        })?;

        debug!(host_id = %host_id, "Sending RPC host request to daemon");
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

        debug!(host_id = %host_id, "Waiting for RPC host response from daemon");
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

        // Return raw reply bytes - caller will decode the FlatBuffer
        let reply_bytes = msg[0].to_vec();
        debug!(host_id = %host_id, response_bytes = reply_bytes.len(), "Received RPC host response from daemon");
        self.rcp_request_sock = Some(recv_sock);
        Ok(reply_bytes)
    }
}
