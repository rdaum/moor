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

use rpc_common::{
    HostClientToDaemonMessage, HostToDaemonMessage, HostToken, MessageType, ReplyResult, RpcError,
};
use tmq::request_reply::RequestSender;
use tmq::Multipart;
use tracing::error;
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
        rpc_msg: HostClientToDaemonMessage,
    ) -> Result<ReplyResult, RpcError> {
        let rpc_msg_payload = bincode::encode_to_vec(&rpc_msg, bincode::config::standard())
            .map_err(|e| RpcError::CouldNotSend(e.to_string()))?;
        let client_message_type = MessageType::HostClientToDaemon(client_id.into_bytes().to_vec());
        let message_type_bytes =
            bincode::encode_to_vec(&client_message_type, bincode::config::standard())
                .map_err(|e| RpcError::CouldNotSend(e.to_string()))?;
        let message = Multipart::from(vec![message_type_bytes, rpc_msg_payload]);
        let rpc_request_sock = self.rcp_request_sock.take().ok_or(RpcError::CouldNotSend(
            "RPC request socket not initialized".to_string(),
        ))?;
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

    pub async fn make_host_rpc_call(
        &mut self,
        host_token: &HostToken,
        rpc_message: HostToDaemonMessage,
    ) -> Result<ReplyResult, RpcError> {
        let host_message_type = MessageType::HostToDaemon(host_token.clone());
        let message_type_bytes =
            bincode::encode_to_vec(&host_message_type, bincode::config::standard())
                .map_err(|e| RpcError::CouldNotSend(e.to_string()))?;

        let rpc_msg_payload = bincode::encode_to_vec(&rpc_message, bincode::config::standard())
            .map_err(|e| RpcError::CouldNotSend(e.to_string()))?;
        let message = Multipart::from(vec![message_type_bytes, rpc_msg_payload]);
        let rpc_request_sock = self.rcp_request_sock.take().ok_or(RpcError::CouldNotSend(
            "RPC request socket not initialized".to_string(),
        ))?;
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
