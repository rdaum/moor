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

use eyre::{anyhow, bail};
use moor_values::model::ObjectRef;
use moor_values::tasks::VerbProgramError;
use moor_values::{Obj, Symbol, Var, SYSTEM_OBJECT};
use rpc_async_client::pubsub_client::{broadcast_recv, events_recv};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_async_client::{ListenersClient, ListenersMessage};
use rpc_common::HostClientToDaemonMessage::ConnectionEstablish;
use rpc_common::{
    AuthToken, ClientEvent, ClientToken, ClientsBroadcastEvent, DaemonToClientReply,
    HostClientToDaemonMessage, HostType, ReplyResult, VerbProgramResponse, CLIENT_BROADCAST_TOPIC,
};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tmq::subscribe::Subscribe;
use tmq::{request, subscribe};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};
use uuid::Uuid;

pub async fn noop_listeners_loop() -> (ListenersClient, JoinHandle<()>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let t = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                ListenersMessage::AddListener(_, _) => {}
                ListenersMessage::RemoveListener(_) => {}
                ListenersMessage::GetListeners(r) => {
                    let _ = r.send(vec![]);
                }
            }
        }
    });

    (ListenersClient::new(tx), t)
}

pub async fn broadcast_handle(
    zmq_ctx: tmq::Context,
    rpc_address: String,
    mut broadcast_sub: Subscribe,
    client_id: Uuid,
    client_token: ClientToken,
    connection_oid: Obj,
    kill_switch: Arc<AtomicBool>,
) {
    let rpc_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(rpc_address.as_str())
        .expect("Unable to bind RPC server for connection");

    let mut rpc_client = RpcSendClient::new(rpc_request_sock);
    // Process ping-pongs on the broadcast topic.
    tokio::spawn(async move {
        loop {
            if kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            if let Ok(event) = broadcast_recv(&mut broadcast_sub).await {
                match event {
                    ClientsBroadcastEvent::PingPong(_) => {
                        let _ = rpc_client
                            .make_client_rpc_call(
                                client_id,
                                HostClientToDaemonMessage::ClientPong(
                                    client_token.clone(),
                                    SystemTime::now(),
                                    connection_oid.clone(),
                                    HostType::TCP,
                                    SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0),
                                ),
                            )
                            .await;
                    }
                }
            }
        }
    });
}

pub async fn create_user_session(
    zmq_ctx: tmq::Context,
    rpc_address: String,
    events_address: String,
) -> Result<
    (
        Obj,
        AuthToken,
        ClientToken,
        Uuid,
        RpcSendClient,
        Subscribe,
        Subscribe,
    ),
    eyre::Error,
> {
    let rpc_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(rpc_address.as_str())
        .expect("Unable to bind RPC server for connection");

    // And let the RPC server know we're here, and it should start sending events on the
    // narrative subscription.
    debug!(rpc_address, "Contacting RPC server to establish connection");
    let mut rpc_client = RpcSendClient::new(rpc_request_sock);
    let client_id = uuid::Uuid::new_v4();
    let peer_addr = format!("{}.test", Uuid::new_v4());
    let (client_token, connection_oid) = match rpc_client
        .make_client_rpc_call(client_id, ConnectionEstablish(peer_addr.to_string()))
        .await
    {
        Ok(ReplyResult::ClientSuccess(DaemonToClientReply::NewConnection(token, objid))) => {
            (token, objid)
        }
        Ok(ReplyResult::Failure(f)) => {
            bail!("RPC failure in connection establishment: {}", f);
        }
        Ok(_) => {
            bail!("Unexpected response from RPC server");
        }
        Err(e) => {
            bail!("Unable to establish connection: {}", e);
        }
    };
    debug!(client_id = ?client_id, connection = ?connection_oid, "Connection established");

    let events_sub = subscribe(&zmq_ctx)
        .connect(events_address.as_str())
        .expect("Unable to connect narrative subscriber ");
    let events_sub = events_sub
        .subscribe(&client_id.as_bytes()[..])
        .expect("Unable to subscribe to narrative messages for client connection");
    let broadcast_sub = subscribe(&zmq_ctx)
        .connect(events_address.as_str())
        .expect("Unable to connect broadcast subscriber ");
    let broadcast_sub = broadcast_sub
        .subscribe(CLIENT_BROADCAST_TOPIC)
        .expect("Unable to subscribe to broadcast messages for client connection");

    info!(
        "Subscribed on pubsub events socket for {:?}, socket addr {}",
        client_id, events_address
    );

    // Now "connect wizard"
    let response = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::LoginCommand(
                client_token.clone(),
                SYSTEM_OBJECT,
                vec!["connect".to_string(), "wizard".to_string()],
                false,
            ),
        )
        .await
        .expect("Unable to send login request to RPC server");
    let (connection_oid, auth_token) = if let ReplyResult::ClientSuccess(
        DaemonToClientReply::LoginResult(Some((auth_token, _connect_type, player))),
    ) = response
    {
        (player.clone(), auth_token.clone())
    } else {
        panic!("Unexpected response from RPC server");
    };

    Ok((
        connection_oid,
        auth_token,
        client_token,
        client_id,
        rpc_client,
        events_sub,
        broadcast_sub,
    ))
}

pub async fn compile(
    rpc_client: &mut RpcSendClient,
    client_id: Uuid,
    oid: Obj,
    auth_token: AuthToken,
    client_token: ClientToken,
    verb_name: Symbol,
    verb_contents: Vec<String>,
) {
    let response = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Verbs(
                client_token.clone(),
                auth_token.clone(),
                ObjectRef::Id(oid.clone()),
            ),
        )
        .await
        .expect("Unable to send verbs request to RPC server");
    match response {
        ReplyResult::ClientSuccess(DaemonToClientReply::Verbs(verbs)) => {
            info!("Got verbs: {:?}", verbs);
        }
        _ => {
            panic!("RPC failure in verbs");
        }
    }

    let response = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Program(
                client_token.clone(),
                auth_token.clone(),
                ObjectRef::Id(oid.clone()),
                verb_name,
                verb_contents,
            ),
        )
        .await
        .expect("Unable to send program request to RPC server");

    match response {
        ReplyResult::ClientSuccess(DaemonToClientReply::ProgramResponse(
            VerbProgramResponse::Success(_, _),
        )) => {
            info!("Programmed {}:{} successfully", oid, verb_name);
        }
        ReplyResult::ClientSuccess(DaemonToClientReply::ProgramResponse(
            VerbProgramResponse::Failure(e),
        )) => match e {
            VerbProgramError::NoVerbToProgram => {
                panic!("No verb to program");
            }
            VerbProgramError::CompilationError(e) => {
                error!("Compilation error in {}:{}", oid, verb_name);
                for e in e {
                    error!("{}", e);
                }
                panic!("Compilation error");
            }
            VerbProgramError::DatabaseError => {
                panic!("Database error");
            }
        },
        _ => {
            panic!("RPC failure in program");
        }
    }
}

pub async fn initialization_session(
    connection_oid: Obj,
    auth_token: AuthToken,
    client_token: ClientToken,
    client_id: Uuid,
    mut rpc_client: RpcSendClient,
    initialization_script: &str,
    verbs: &[(Symbol, String)],
) -> Result<(), eyre::Error> {
    let response = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::Eval(
                client_token.clone(),
                auth_token.clone(),
                initialization_script.to_string(),
            ),
        )
        .await
        .expect("Unable to send eval request to RPC server");

    match response {
        ReplyResult::HostSuccess(hs) => {
            info!("Evaluated successfully: {:?}", hs);
        }
        ReplyResult::ClientSuccess(cs) => {
            info!("Evaluated successfully: {:?}", cs);
        }
        ReplyResult::Failure(f) => {
            panic!("RPC failure in eval: {}", f);
        }
    }

    info!("Initialization script executed successfully");

    for (verb_name, verb_code) in verbs {
        info!("Compiling {} verb", verb_name);
        compile(
            &mut rpc_client,
            client_id,
            connection_oid.clone(),
            auth_token.clone(),
            client_token.clone(),
            verb_name.clone(),
            verb_code.split('\n').map(|s| s.to_string()).collect(),
        )
        .await;

        info!("Compiled {} verb", verb_name);
    }

    info!("Initialization session complete");

    Ok(())
}

pub struct ExecutionContext {
    pub zmq_ctx: tmq::Context,
    pub kill_switch: Arc<std::sync::atomic::AtomicBool>,
}

pub async fn listen_responses(
    client_id: Uuid,
    mut events_sub: Subscribe,
    ks: Arc<AtomicBool>,
    event_listen_task_results: Arc<Mutex<HashMap<usize, Result<Var, eyre::Error>>>>,
) {
    tokio::spawn(async move {
        let start_time = Instant::now();
        info!("Waiting for events...");
        loop {
            if ks.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let msg = events_recv(client_id, &mut events_sub).await;
            match msg {
                Ok(ClientEvent::TaskSuccess(tid, v)) => {
                    let mut tasks = event_listen_task_results.lock().await;
                    tasks.insert(tid, Ok(v));
                }
                Ok(ClientEvent::TaskError(tid, e)) => {
                    let mut tasks = event_listen_task_results.lock().await;
                    tasks.insert(tid, Err(anyhow!("Task error: {:?}", e)));
                }
                Ok(_) => {}
                Err(e) => {
                    panic!("Error in event recv: {}", e);
                }
            }
        }
        let seconds_since_start = start_time.elapsed().as_secs();
        if seconds_since_start % 5 == 0 {
            let tasks = event_listen_task_results.lock().await;
            info!(
                "Event listener running for {} seconds with {} tasks",
                seconds_since_start,
                tasks.len()
            );
        }
    });
}
