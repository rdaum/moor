#[cfg(feature = "ssr")]
use rpc_async_client::rpc_client::RpcSendClient;
#[cfg(feature = "ssr")]
use rpc_common::ClientToken;
#[cfg(feature = "ssr")]
use std::net::SocketAddr;

#[cfg(feature = "ssr")]
use moor_var::Obj;

use std::sync::Arc;
#[cfg(feature = "ssr")]
use tokio::sync::Mutex;
#[cfg(feature = "ssr")]
use uuid::Uuid;

pub mod app;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}

#[cfg(feature = "ssr")]
#[derive(Clone)]
pub struct Context {
    pub zmq_ctx: tmq::Context,
    pub rpc_address: String,
    pub events_address: String,
    pub listen_address: SocketAddr,
}

#[cfg(feature = "ssr")]
pub struct ClientSession {
    context: Context,
    player: Obj,
    client_id: Uuid,
    rpc_send_client: Arc<Mutex<RpcSendClient>>,
    client_token: ClientToken,
}

#[cfg(feature = "ssr")]
impl Context {}

#[cfg(feature = "ssr")]
pub async fn establish_client_connection(
    context: &Context,
) -> Result<(Uuid, RpcSendClient, ClientToken), eyre::Report> {
    use eyre::bail;
    use rpc_common::HostClientToDaemonMessage::ConnectionEstablish;
    use rpc_common::{DaemonToClientReply, ReplyResult};
    use tmq::request;
    use tracing::info;

    let zmq_ctx = context.zmq_ctx.clone();
    let rcp_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(context.rpc_address.as_str())
        .expect("Unable to bind RPC server for connection");

    let client_id = Uuid::new_v4();
    let mut rpc_client = RpcSendClient::new(rcp_request_sock);

    let client_token = match rpc_client
        .make_client_rpc_call(
            client_id,
            ConnectionEstablish(context.listen_address.to_string()),
        )
        .await
    {
        Ok(ReplyResult::ClientSuccess(DaemonToClientReply::NewConnection(client_token, objid))) => {
            info!("Connection established, connection ID: {}", objid);
            client_token
        }
        Ok(ReplyResult::Failure(f)) => {
            bail!("RPC failure in connection establishment: {}", f);
        }
        Ok(ReplyResult::ClientSuccess(r)) => {
            bail!("Unexpected response from RPC server");
        }
        Err(e) => {
            bail!("Unable to establish connection: {}", e);
        }
        Ok(ReplyResult::HostSuccess(hs)) => {
            bail!("Unexpected response from RPC server: {:?}", hs);
        }
    };

    Ok((client_id, rpc_client, client_token))
}
