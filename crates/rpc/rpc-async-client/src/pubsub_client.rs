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

use moor_schema::{
    convert::{obj_from_flatbuffer_struct, var_from_flatbuffer_ref},
    rpc,
};
use moor_var::{Obj, Var};
use planus::ReadAsRoot;
use rpc_common::RpcError;
use std::time::Duration;

/// Type alias for the complex return type of worker request extraction
type WorkerRequestData = (Uuid, Uuid, Obj, Vec<Var>, Option<Duration>);

/// Owned wrapper around worker message flatbuffer data that provides zero-copy access
pub struct WorkerMessage {
    buffer: Vec<u8>,
}

impl WorkerMessage {
    pub fn from_buffer(buffer: Vec<u8>) -> Result<Self, RpcError> {
        // Validate it's a valid flatbuffer by attempting to parse
        let _msg = rpc::DaemonToWorkerMessageRef::read_as_root(&buffer)
            .map_err(|e| RpcError::CouldNotDecode(format!("Invalid flatbuffer: {e}")))?;
        Ok(WorkerMessage { buffer })
    }

    /// Get zero-copy reference to the message union
    pub fn message(&self) -> Result<rpc::DaemonToWorkerMessageUnionRef<'_>, RpcError> {
        let fb_msg = rpc::DaemonToWorkerMessageRef::read_as_root(&self.buffer)
            .map_err(|e| RpcError::CouldNotDecode(format!("Failed to parse flatbuffer: {e}")))?;
        fb_msg
            .message()
            .map_err(|e| RpcError::CouldNotDecode(format!("Failed to access message: {e}")))
    }

    /// Extract WorkerRequest data with all business logic conversions
    pub fn extract_worker_request(&self) -> Result<WorkerRequestData, RpcError> {
        match self.message()? {
            rpc::DaemonToWorkerMessageUnionRef::WorkerRequest(req) => {
                let worker_id_data = req
                    .worker_id()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Failed to get worker_id: {e}")))?
                    .data()
                    .map_err(|e| {
                        RpcError::CouldNotDecode(format!("Failed to get worker_id data: {e}"))
                    })?;
                let worker_id = Uuid::from_slice(worker_id_data)
                    .map_err(|e| RpcError::CouldNotDecode(format!("Invalid worker UUID: {e}")))?;

                let request_id_data = req
                    .id()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Failed to get id: {e}")))?
                    .data()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Failed to get id data: {e}")))?;
                let request_id = Uuid::from_slice(request_id_data)
                    .map_err(|e| RpcError::CouldNotDecode(format!("Invalid request UUID: {e}")))?;

                let perms_ref = req
                    .perms()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Failed to get perms: {e}")))?;
                let perms_obj = rpc::Obj::try_from(perms_ref).map_err(|e| {
                    RpcError::CouldNotDecode(format!("Failed to convert perms ref: {e}"))
                })?;
                let perms = obj_from_flatbuffer_struct(&perms_obj).map_err(|e| {
                    RpcError::CouldNotDecode(format!("Failed to decode perms: {e}"))
                })?;

                let request_vec = req
                    .request()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Failed to get request: {e}")))?;
                let request = request_vec
                    .iter()
                    .map(|var_ref_result| {
                        let var_ref = var_ref_result.map_err(|e| {
                            RpcError::CouldNotDecode(format!("Failed to get var: {e}"))
                        })?;
                        var_from_flatbuffer_ref(var_ref).map_err(|e| {
                            RpcError::CouldNotDecode(format!("Failed to decode var: {e}"))
                        })
                    })
                    .collect::<Result<Vec<_>, RpcError>>()?;

                let timeout_ms = req.timeout_ms().map_err(|e| {
                    RpcError::CouldNotDecode(format!("Failed to get timeout_ms: {e}"))
                })?;
                let timeout = if timeout_ms == 0 {
                    None
                } else {
                    Some(Duration::from_millis(timeout_ms))
                };

                Ok((worker_id, request_id, perms, request, timeout))
            }
            _ => Err(RpcError::CouldNotDecode(
                "Expected WorkerRequest message".to_string(),
            )),
        }
    }

    /// Extract PleaseDie data with all business logic conversions
    pub fn extract_please_die(&self) -> Result<Uuid, RpcError> {
        match self.message()? {
            rpc::DaemonToWorkerMessageUnionRef::PleaseDie(die) => {
                let worker_id_data = die
                    .worker_id()
                    .map_err(|e| RpcError::CouldNotDecode(format!("Failed to get worker_id: {e}")))?
                    .data()
                    .map_err(|e| {
                        RpcError::CouldNotDecode(format!("Failed to get worker_id data: {e}"))
                    })?;
                let worker_id = Uuid::from_slice(worker_id_data)
                    .map_err(|e| RpcError::CouldNotDecode(format!("Invalid worker UUID: {e}")))?;

                Ok(worker_id)
            }
            _ => Err(RpcError::CouldNotDecode(
                "Expected PleaseDie message".to_string(),
            )),
        }
    }

    /// Check if this is a PingWorkers message
    pub fn is_ping_workers(&self) -> Result<bool, RpcError> {
        match self.message()? {
            rpc::DaemonToWorkerMessageUnionRef::PingWorkers(_) => Ok(true),
            _ => Ok(false),
        }
    }
}

/// Owned wrapper for ClientEvent that keeps the buffer alive for zero-copy access
pub struct ClientEventMessage {
    buffer: Vec<u8>,
}

impl ClientEventMessage {
    pub fn event(&self) -> Result<rpc::ClientEventRef<'_>, RpcError> {
        rpc::ClientEventRef::read_as_root(&self.buffer)
            .map_err(|e| RpcError::CouldNotDecode(format!("Failed to parse flatbuffer: {e}")))
    }

    /// Consume self and pull the underlying buffer.
    pub fn consume(self) -> Vec<u8> {
        self.buffer
    }
}

pub async fn events_recv(
    client_id: Uuid,
    subscribe: &mut Subscribe,
) -> Result<ClientEventMessage, RpcError> {
    let Some(Ok(mut inbound)) = subscribe.next().await else {
        return Err(RpcError::CouldNotReceive(
            "Unable to receive published event".to_string(),
        ));
    };

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

    // Validate it's a valid flatbuffer
    let _fb_event = rpc::ClientEventRef::read_as_root(event.as_ref())
        .map_err(|e| RpcError::CouldNotDecode(format!("Unable to parse FlatBuffer: {e:?}")))?;

    Ok(ClientEventMessage {
        buffer: event.to_vec(),
    })
}

/// Owned wrapper for ClientsBroadcastEvent that keeps the buffer alive for zero-copy access
pub struct BroadcastEventMessage {
    buffer: Vec<u8>,
}

impl BroadcastEventMessage {
    pub fn event(&self) -> Result<rpc::ClientsBroadcastEventRef<'_>, RpcError> {
        rpc::ClientsBroadcastEventRef::read_as_root(&self.buffer)
            .map_err(|e| RpcError::CouldNotDecode(format!("Failed to parse flatbuffer: {e}")))
    }
}

pub async fn broadcast_recv(subscribe: &mut Subscribe) -> Result<BroadcastEventMessage, RpcError> {
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

    // Validate it's a valid flatbuffer
    let _fb_event = rpc::ClientsBroadcastEventRef::read_as_root(event.as_ref())
        .map_err(|e| RpcError::CouldNotDecode(format!("Unable to parse FlatBuffer: {e:?}")))?;

    Ok(BroadcastEventMessage {
        buffer: event.to_vec(),
    })
}

/// Owned wrapper for HostBroadcastEvent that keeps the buffer alive for zero-copy access
pub struct HostBroadcastMessage {
    buffer: Vec<u8>,
}

impl HostBroadcastMessage {
    pub fn event(&self) -> Result<rpc::HostBroadcastEventRef<'_>, RpcError> {
        rpc::HostBroadcastEventRef::read_as_root(&self.buffer)
            .map_err(|e| RpcError::CouldNotDecode(format!("Failed to parse flatbuffer: {e}")))
    }
}

pub async fn hosts_events_recv(
    subscribe: &mut Subscribe,
) -> Result<HostBroadcastMessage, RpcError> {
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

    // Validate it's a valid flatbuffer
    let _fb_event = rpc::HostBroadcastEventRef::read_as_root(event.as_ref())
        .map_err(|e| RpcError::CouldNotDecode(format!("Unable to parse FlatBuffer: {e:?}")))?;

    Ok(HostBroadcastMessage {
        buffer: event.to_vec(),
    })
}

pub async fn workers_events_recv(subscribe: &mut Subscribe) -> Result<WorkerMessage, RpcError> {
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

    // Create owned wrapper with zero-copy access
    WorkerMessage::from_buffer(event.to_vec())
}
