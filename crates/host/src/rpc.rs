/// RPC related functions, for talking to/from the RPC daemon over ZMQ.
use anyhow::bail;
use futures_util::StreamExt;
use tmq::request_reply::RequestSender;
use tmq::subscribe::Subscribe;
use tmq::Multipart;
use tracing::{error, trace};
use uuid::Uuid;

use rpc_common::{ConnectionEvent, RpcRequest, RpcResult};

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

    trace!(?event, "Received narrative");

    let (msg, _msg_size): (ConnectionEvent, usize) =
        match bincode::decode_from_slice(event.as_ref(), bincode::config::standard()) {
            Ok((msg, size)) => (msg, size),
            Err(e) => {
                bail!("Unable to decode narrative message: {}", e);
            }
        };
    Ok(msg)
}

/// Call the ZMQ RPC (REQ/REPLY) endpoint with a `ClientRequest`, and receive a `ServerResponse`.
/// The `RequestSender` is consumed in the process, and a new one is returned.
pub async fn make_rpc_call(
    client_id: Uuid,
    rcp_request_sock: RequestSender,
    rpc_msg: RpcRequest,
) -> Result<(RpcResult, RequestSender), anyhow::Error> {
    let rpc_msg_payload = bincode::encode_to_vec(&rpc_msg, bincode::config::standard())
        .expect("Unable to encode connection establish request");
    let message = Multipart::from(vec![client_id.as_bytes().to_vec(), rpc_msg_payload]);
    let rpc_reply_sock = match rcp_request_sock.send(message).await {
        Ok(rpc_reply_sock) => rpc_reply_sock,
        Err(e) => {
            error!(
                "Unable to send connection establish request to RPC server: {}",
                e
            );
            bail!(e);
        }
    };
    let (msg, recv_sock) = match rpc_reply_sock.recv().await {
        Ok((msg, recv_sock)) => (msg, recv_sock),
        Err(e) => {
            error!(
                "Unable to receive connection establish reply from RPC server: {}",
                e
            );
            bail!(e);
        }
    };

    match bincode::decode_from_slice(&msg[0], bincode::config::standard()) {
        Ok((msg, _)) => {
            Ok((msg, recv_sock))
        }
        Err(e) => {
            error!("Unable to decode RPC response: {}", e);
            bail!(e);
        }
    }
}
