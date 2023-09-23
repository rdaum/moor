/// RPC related functions, for talking to/from the RPC daemon over ZMQ.
use anyhow::bail;
use futures_util::StreamExt;
use tmq::subscribe::Subscribe;
use tracing::trace;
use uuid::Uuid;

use crate::{BroadcastEvent, ConnectionEvent};

pub async fn narrative_recv(
    client_id: Uuid,
    subscribe: &mut Subscribe,
) -> Result<ConnectionEvent, anyhow::Error> {
    let Some(Ok(mut inbound)) = subscribe.next().await else {
        bail!("Unable to receive narrative message");
    };

    trace!(message = ?inbound, "narrative_message");
    // bincode decode the message, and it should be ConnectionEvent
    if inbound.len() != 2 {
        bail!("Unexpected message length: {}", inbound.len());
    }
    let (Some(received_client_id), Some(event)) = (inbound.pop_front(), inbound.pop_front()) else {
        bail!("Unexpected message format");
    };

    let Ok(received_client_id) = Uuid::from_slice(&received_client_id) else {
        bail!("Unable to decode client ID");
    };

    if received_client_id != client_id {
        bail!("Unexpected client ID");
    }

    let (msg, _msg_size): (ConnectionEvent, usize) =
        match bincode::decode_from_slice(event.as_ref(), bincode::config::standard()) {
            Ok((msg, size)) => (msg, size),
            Err(e) => {
                bail!("Unable to decode narrative message: {}", e);
            }
        };
    Ok(msg)
}

pub async fn broadcast_recv(subscribe: &mut Subscribe) -> Result<BroadcastEvent, anyhow::Error> {
    let Some(Ok(mut inbound)) = subscribe.next().await else {
        bail!("Unable to receive broadcast message");
    };

    trace!(message = ?inbound, "broadcast_message");
    if inbound.len() != 2 {
        bail!("Unexpected message length: {}", inbound.len());
    }

    let Some(topic) = inbound.pop_front() else {
        bail!("Unexpected message format");
    };

    if &topic[..] != b"broadcast" {
        bail!("Unexpected topic: {:?}", topic);
    }

    let Some(event) = inbound.pop_front() else {
        bail!("Unexpected message format");
    };

    let (msg, _msg_size): (BroadcastEvent, usize) =
        match bincode::decode_from_slice(event.as_ref(), bincode::config::standard()) {
            Ok((msg, size)) => (msg, size),
            Err(e) => {
                bail!("Unable to decode broadcast message: {}", e);
            }
        };
    Ok(msg)
}
