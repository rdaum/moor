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

// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

/// RPC related functions, for talking to/from the RPC daemon over ZMQ.
use tracing::trace;
use uuid::Uuid;
use zmq::Socket;

use rpc_common::{ClientEvent, ClientsBroadcastEvent, RpcError};

/// Blocking receive on the narrative channel, returning a `ConnectionEvent`.
pub fn events_recv(client_id: Uuid, subscribe: &Socket) -> Result<ClientEvent, RpcError> {
    let Ok(inbound) = subscribe.recv_multipart(0) else {
        return Err(RpcError::CouldNotReceive(
            "Unable to receive narrative message".to_string(),
        ));
    };

    // bincode decode the message, and it should be ConnectionEvent
    if inbound.len() != 2 {
        return Err(RpcError::CouldNotDecode(format!(
            "Unexpected message length: {}",
            inbound.len()
        )));
    }

    let (received_client_id, event) = (&inbound[0], &inbound[1]);

    let Ok(received_client_id) = Uuid::from_slice(received_client_id) else {
        return Err(RpcError::CouldNotDecode(
            "Unable to decode client ID".to_string(),
        ));
    };

    if received_client_id != client_id {
        return Err(RpcError::CouldNotDecode("Unexpected client ID".to_string()));
    }

    let decode_result = bincode::decode_from_slice(event.as_ref(), bincode::config::standard());
    let (msg, _msg_size): (ClientEvent, usize) = decode_result.map_err(|e| {
        RpcError::CouldNotDecode(format!("Unable to decode narrative message: {}", e))
    })?;

    Ok(msg)
}

/// Blocking receive on the broadcast channel, returning a `BroadcastEvent`.
pub fn broadcast_recv(subscribe: &mut Socket) -> Result<ClientsBroadcastEvent, RpcError> {
    let Ok(inbound) = subscribe.recv_multipart(0) else {
        return Err(RpcError::CouldNotReceive(
            "Unable to receive broadcast message".to_string(),
        ));
    };

    trace!(message = ?inbound, "broadcast_message");
    if inbound.len() != 2 {
        return Err(RpcError::CouldNotDecode(format!(
            "Unexpected message length: {}",
            inbound.len()
        )));
    }

    let (topic, event) = (&inbound[0], &inbound[1]);

    if &topic[..] != b"broadcast" {
        return Err(RpcError::CouldNotDecode(format!(
            "Unexpected topic: {:?}",
            topic
        )));
    }

    let (msg, _msg_size): (ClientsBroadcastEvent, usize) =
        bincode::decode_from_slice(event.as_ref(), bincode::config::standard()).map_err(|e| {
            RpcError::CouldNotDecode(format!("Unable to decode broadcast message: {}", e))
        })?;
    Ok(msg)
}
