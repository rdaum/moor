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

/// RPC related functions, for talking to/from the RPC daemon over ZMQ.
use futures_util::StreamExt;
use tmq::subscribe::Subscribe;
use uuid::Uuid;

use rpc_common::{
    ClientEvent, ClientsBroadcastEvent, DaemonToWorkerMessage, HostBroadcastEvent, RpcError,
};

pub async fn events_recv(
    client_id: Uuid,
    subscribe: &mut Subscribe,
) -> Result<ClientEvent, RpcError> {
    let Some(Ok(mut inbound)) = subscribe.next().await else {
        return Err(RpcError::CouldNotReceive(
            "Unable to receive published event".to_string(),
        ));
    };

    // bincode decode the message, and it should be ConnectionEvent
    if inbound.len() != 2 {
        return Err(RpcError::CouldNotDecode(format!(
            "Unexpected message length: {}",
            inbound.len()
        )));
    }
    let (Some(received_client_id), Some(event)) = (inbound.pop_front(), inbound.pop_front()) else {
        return Err(RpcError::CouldNotDecode(
            "Unexpected message format".to_string(),
        ));
    };

    let Ok(received_client_id) = Uuid::from_slice(&received_client_id) else {
        return Err(RpcError::CouldNotDecode(
            "Unable to decode client ID".to_string(),
        ));
    };

    if received_client_id != client_id {
        return Err(RpcError::CouldNotDecode("Unexpected client ID".to_string()));
    }

    let decode_result = bincode::decode_from_slice(event.as_ref(), bincode::config::standard());
    let (msg, _msg_size): (ClientEvent, usize) = decode_result
        .map_err(|e| RpcError::CouldNotDecode(format!("Unable to decode published event: {e}")))?;

    Ok(msg)
}

pub async fn broadcast_recv(subscribe: &mut Subscribe) -> Result<ClientsBroadcastEvent, RpcError> {
    let Some(Ok(mut inbound)) = subscribe.next().await else {
        return Err(RpcError::CouldNotReceive(
            "Unable to receive broadcast message".to_string(),
        ));
    };

    if inbound.len() != 2 {
        return Err(RpcError::CouldNotDecode(format!(
            "Unexpected message length: {}",
            inbound.len()
        )));
    }

    let Some(topic) = inbound.pop_front() else {
        return Err(RpcError::CouldNotDecode(
            "Unexpected message format".to_string(),
        ));
    };

    if &topic[..] != b"broadcast" {
        return Err(RpcError::CouldNotDecode(format!(
            "Unexpected topic: {topic:?}"
        )));
    }

    let Some(event) = inbound.pop_front() else {
        return Err(RpcError::CouldNotDecode(
            "Unexpected message format".to_string(),
        ));
    };

    let (msg, _msg_size): (ClientsBroadcastEvent, usize) =
        bincode::decode_from_slice(event.as_ref(), bincode::config::standard()).map_err(|e| {
            RpcError::CouldNotDecode(format!("Unable to decode broadcast message: {e}"))
        })?;
    Ok(msg)
}

pub async fn hosts_events_recv(subscribe: &mut Subscribe) -> Result<HostBroadcastEvent, RpcError> {
    let Some(Ok(mut inbound)) = subscribe.next().await else {
        return Err(RpcError::CouldNotReceive(
            "Unable to receive host broadcast message".to_string(),
        ));
    };

    if inbound.len() != 2 {
        return Err(RpcError::CouldNotDecode(format!(
            "Unexpected message length: {}",
            inbound.len()
        )));
    }

    let Some(topic) = inbound.pop_front() else {
        return Err(RpcError::CouldNotDecode(
            "Unexpected message format".to_string(),
        ));
    };

    if &topic[..] != b"hosts" {
        return Err(RpcError::CouldNotDecode(format!(
            "Unexpected topic: {topic:?}"
        )));
    }

    let Some(event) = inbound.pop_front() else {
        return Err(RpcError::CouldNotDecode(
            "Unexpected message format".to_string(),
        ));
    };

    let (msg, _msg_size): (HostBroadcastEvent, usize) =
        bincode::decode_from_slice(event.as_ref(), bincode::config::standard()).map_err(|e| {
            RpcError::CouldNotDecode(format!("Unable to decode host broadcast message: {e}"))
        })?;
    Ok(msg)
}

pub async fn workers_events_recv(
    subscribe: &mut Subscribe,
) -> Result<DaemonToWorkerMessage, RpcError> {
    let Some(Ok(mut inbound)) = subscribe.next().await else {
        return Err(RpcError::CouldNotReceive(
            "Unable to receive worker broadcast message".to_string(),
        ));
    };

    if inbound.len() != 2 {
        return Err(RpcError::CouldNotDecode(format!(
            "Unexpected message length: {}",
            inbound.len()
        )));
    }

    let Some(topic) = inbound.pop_front() else {
        return Err(RpcError::CouldNotDecode(
            "Unexpected message format".to_string(),
        ));
    };

    if &topic[..] != b"workers" {
        return Err(RpcError::CouldNotDecode(format!(
            "Unexpected topic: {topic:?}"
        )));
    }

    let Some(event) = inbound.pop_front() else {
        return Err(RpcError::CouldNotDecode(
            "Unexpected message format".to_string(),
        ));
    };

    let (msg, _msg_size): (DaemonToWorkerMessage, usize) =
        bincode::decode_from_slice(event.as_ref(), bincode::config::standard()).map_err(|e| {
            RpcError::CouldNotDecode(format!("Unable to decode worker broadcast message: {e}"))
        })?;
    Ok(msg)
}
