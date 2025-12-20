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

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#[cfg_attr(coverage_nightly, coverage(off))]
use eyre::{anyhow, bail};
use moor_common::model::ObjectRef;
use moor_schema::{
    convert::{obj_from_ref, var_from_flatbuffer_ref},
    rpc as moor_rpc,
};
use moor_var::{Obj, SYSTEM_OBJECT, Symbol, Var};
use rpc_async_client::{
    ListenersClient, ListenersMessage,
    pubsub_client::{broadcast_recv, events_recv},
    rpc_client::RpcClient,
};
use rpc_common::{
    AuthToken, CLIENT_BROADCAST_TOPIC, ClientToken, auth_token_from_ref, mk_client_pong_msg,
    mk_connection_establish_msg, mk_eval_msg, mk_login_command_msg, mk_program_msg, mk_verbs_msg,
    read_reply_result,
};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, atomic::AtomicBool},
    time::{Instant, SystemTime},
};
use tmq::{subscribe, subscribe::Subscribe};
use tokio::{
    sync::{Mutex, Notify},
    task::JoinHandle,
};
use tracing::{debug, error, info};
use uuid::Uuid;

type TaskResults = Arc<Mutex<HashMap<usize, (Result<Var, eyre::Error>, Arc<Notify>)>>>;

pub async fn noop_listeners_loop() -> (ListenersClient, JoinHandle<()>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let t = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                ListenersMessage::AddListener(_, _, reply) => {
                    let _ = reply.send(Ok(()));
                }
                ListenersMessage::RemoveListener(_, reply) => {
                    let _ = reply.send(Ok(()));
                }
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
    // Create managed RPC client with connection pooling and cancellation safety
    let rpc_client = RpcClient::new_with_defaults(
        std::sync::Arc::new(zmq_ctx.clone()),
        rpc_address.clone(),
        None, // No CURVE encryption for load testing
    );
    // Process ping-pongs on the broadcast topic.
    tokio::spawn(async move {
        loop {
            if kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            if let Ok(event_msg) = broadcast_recv(&mut broadcast_sub).await {
                let event = event_msg.event().expect("Failed to parse broadcast event");
                match event.event().expect("Missing event union") {
                    moor_rpc::ClientsBroadcastEventUnionRef::ClientsBroadcastPingPong(_) => {
                        let timestamp = SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_nanos() as u64;
                        let pong_msg = mk_client_pong_msg(
                            &client_token,
                            timestamp,
                            &connection_oid,
                            moor_rpc::HostType::Tcp,
                            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0).to_string(),
                        );
                        let _ = rpc_client.make_client_rpc_call(client_id, pong_msg).await;
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
        RpcClient,
        Subscribe,
        Subscribe,
    ),
    eyre::Error,
> {
    // Create managed RPC client with connection pooling and cancellation safety
    let rpc_client = RpcClient::new_with_defaults(
        std::sync::Arc::new(zmq_ctx.clone()),
        rpc_address.clone(),
        None, // No CURVE encryption for load testing
    );

    // And let the RPC server know we're here, and it should start sending events on the
    // narrative subscription.
    debug!(rpc_address, "Contacting RPC server to establish connection");
    let client_id = uuid::Uuid::new_v4();
    let peer_addr = format!("{}.test", Uuid::new_v4());

    let establish_msg = mk_connection_establish_msg(peer_addr, 7777, 12345, None, None);

    let reply_bytes = match rpc_client
        .make_client_rpc_call(client_id, establish_msg)
        .await
    {
        Ok(bytes) => bytes,
        Err(e) => {
            bail!("Unable to establish connection: {}", e);
        }
    };

    let reply =
        read_reply_result(&reply_bytes).map_err(|e| anyhow!("Failed to parse reply: {}", e))?;

    let (client_token, connection_oid) = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::NewConnection(new_conn) => {
                    let client_token_ref = new_conn.client_token().expect("Missing client_token");
                    let client_token =
                        ClientToken(client_token_ref.token().expect("Missing token").to_string());
                    let objid_ref = new_conn.connection_obj().expect("Missing connection_obj");
                    let objid = obj_from_ref(objid_ref).expect("Failed to decode connection_obj");
                    (client_token, objid)
                }
                _ => {
                    bail!("Unexpected response from RPC server");
                }
            }
        }
        moor_rpc::ReplyResultUnionRef::Failure(failure) => {
            let error_ref = failure.error().expect("Missing error");
            bail!("RPC failure in connection establishment: {:?}", error_ref);
        }
        _ => {
            bail!("Unexpected response type from RPC server");
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
    let login_msg = mk_login_command_msg(
        &client_token,
        &SYSTEM_OBJECT,
        vec!["connect".to_string(), "wizard".to_string()],
        false,
    );

    let reply_bytes = rpc_client
        .make_client_rpc_call(client_id, login_msg)
        .await
        .expect("Unable to send login request to RPC server");

    let reply = read_reply_result(&reply_bytes).expect("Failed to parse login reply");

    let (connection_oid, auth_token) = match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::LoginResult(login_result) => {
                    if !login_result.success().expect("Missing success") {
                        panic!("Login failed");
                    }
                    let auth_token_ref = login_result
                        .auth_token()
                        .expect("Missing auth_token")
                        .expect("Auth token is None");
                    let auth_token =
                        auth_token_from_ref(auth_token_ref).expect("Failed to decode auth token");
                    let player_ref = login_result
                        .player()
                        .expect("Missing player")
                        .expect("Player is None");
                    let player = obj_from_ref(player_ref).expect("Failed to decode player");
                    (player, auth_token)
                }
                _ => {
                    panic!("Unexpected response from RPC server");
                }
            }
        }
        _ => {
            panic!("Unexpected response type from RPC server");
        }
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
    rpc_client: &mut RpcClient,
    client_id: Uuid,
    oid: Obj,
    auth_token: AuthToken,
    client_token: ClientToken,
    verb_name: Symbol,
    verb_contents: Vec<String>,
) {
    // Query verbs (optional - just for logging)
    let verbs_message = mk_verbs_msg(&auth_token, &ObjectRef::Id(oid), false);

    let reply_bytes = rpc_client
        .make_client_rpc_call(client_id, verbs_message)
        .await
        .expect("Unable to send verbs request to RPC server");

    let reply = read_reply_result(&reply_bytes).expect("Failed to parse verbs reply");
    if let moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) =
        reply.result().expect("Missing result")
    {
        let daemon_reply = client_success.reply().expect("Missing reply");
        if let moor_rpc::DaemonToClientReplyUnionRef::VerbsReply(verbs_reply) =
            daemon_reply.reply().expect("Missing reply union")
        {
            info!(
                "Got {} verbs",
                verbs_reply.verbs().expect("Missing verbs").len()
            );
        }
    }

    // Program the verb
    let program_msg = mk_program_msg(
        &client_token,
        &auth_token,
        &ObjectRef::Id(oid),
        &verb_name,
        verb_contents,
    );

    let reply_bytes = rpc_client
        .make_client_rpc_call(client_id, program_msg)
        .await
        .expect("Unable to send program request to RPC server");

    let reply = read_reply_result(&reply_bytes).expect("Failed to parse program reply");
    match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            let daemon_reply = client_success.reply().expect("Missing reply");
            match daemon_reply.reply().expect("Missing reply union") {
                moor_rpc::DaemonToClientReplyUnionRef::VerbProgramResponseReply(prog_resp) => {
                    let response = prog_resp.response().expect("Missing response");
                    match response.response().expect("Missing response union") {
                        moor_rpc::VerbProgramResponseUnionRef::VerbProgramSuccess(_) => {
                            info!("Programmed {}:{} successfully", oid, verb_name);
                        }
                        moor_rpc::VerbProgramResponseUnionRef::VerbProgramFailure(failure) => {
                            let error = failure.error().expect("Missing error");
                            error!("Compilation error in {}:{}: {:?}", oid, verb_name, error);
                        }
                    }
                }
                _ => {
                    panic!("Unexpected response from RPC server");
                }
            }
        }
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
    mut rpc_client: RpcClient,
    initialization_script: &str,
    verbs: &[(Symbol, String)],
) -> Result<(), eyre::Error> {
    let eval_message = mk_eval_msg(
        &client_token,
        &auth_token,
        initialization_script.to_string(),
    );

    let reply_bytes = rpc_client
        .make_client_rpc_call(client_id, eval_message)
        .await
        .expect("Unable to send eval request to RPC server");

    let reply = read_reply_result(&reply_bytes).expect("Failed to parse eval reply");
    match reply.result().expect("Missing result") {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(_) => {
            info!("Evaluated successfully");
        }
        moor_rpc::ReplyResultUnionRef::Failure(failure) => {
            let error_ref = failure.error().expect("Missing error");
            panic!("RPC failure in eval: {error_ref:?}");
        }
        _ => {
            panic!("Unexpected response type from RPC server");
        }
    }

    info!("Initialization script executed successfully");

    for (verb_name, verb_code) in verbs {
        info!("Compiling {} verb", verb_name);
        compile(
            &mut rpc_client,
            client_id,
            connection_oid,
            auth_token.clone(),
            client_token.clone(),
            *verb_name,
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

/// Wait for a specific task to complete using async notification instead of polling
pub async fn wait_for_task_completion(
    task_id: usize,
    task_results: TaskResults,
    timeout: std::time::Duration,
) -> Result<Result<Var, eyre::Error>, eyre::Error> {
    let notify = {
        let mut tasks = task_results.lock().await;
        if let Some((result, _notify)) = tasks.remove(&task_id) {
            // Task already completed
            return Ok(result);
        }
        // Task not completed yet, create a notifier for it
        let notify = Arc::new(Notify::new());
        tasks.insert(task_id, (Err(anyhow!("Task pending")), notify.clone()));
        notify
    };

    // Wait for notification with timeout
    let timeout_result = tokio::time::timeout(timeout, notify.notified()).await;

    match timeout_result {
        Ok(_) => {
            // Notification received, get the result
            let mut tasks = task_results.lock().await;
            if let Some((result, _)) = tasks.remove(&task_id) {
                Ok(result)
            } else {
                Err(anyhow!("Task result not found after notification"))
            }
        }
        Err(_) => {
            // Timeout occurred
            let mut tasks = task_results.lock().await;
            tasks.remove(&task_id); // Clean up
            Err(anyhow!("Timeout waiting for task {}", task_id))
        }
    }
}

pub async fn listen_responses(
    client_id: Uuid,
    mut events_sub: Subscribe,
    ks: Arc<AtomicBool>,
    event_listen_task_results: TaskResults,
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
                Ok(event_msg) => {
                    let event = event_msg.event().expect("Failed to parse event");
                    match event.event().expect("Missing event union") {
                        moor_rpc::ClientEventUnionRef::TaskSuccessEvent(task_success) => {
                            let tid = task_success.task_id().expect("Missing task_id") as usize;
                            let value_ref = task_success.result().expect("Missing result");
                            let v =
                                var_from_flatbuffer_ref(value_ref).expect("Failed to decode value");

                            let mut tasks = event_listen_task_results.lock().await;
                            if let Some((_, notify)) = tasks.get(&tid) {
                                let notify = notify.clone();
                                tasks.insert(tid, (Ok(v), notify.clone()));
                                notify.notify_one();
                            } else {
                                // Task not found in pending tasks, create new entry
                                tasks.insert(tid, (Ok(v), Arc::new(Notify::new())));
                            }
                        }
                        moor_rpc::ClientEventUnionRef::TaskErrorEvent(task_error) => {
                            let tid = task_error.task_id().expect("Missing task_id") as usize;
                            let error = task_error.error().expect("Missing error");

                            let mut tasks = event_listen_task_results.lock().await;
                            if let Some((_, notify)) = tasks.get(&tid) {
                                let notify = notify.clone();
                                tasks.insert(
                                    tid,
                                    (Err(anyhow!("Task error: {:?}", error)), notify.clone()),
                                );
                                notify.notify_one();
                            } else {
                                // Task not found in pending tasks, create new entry
                                tasks.insert(
                                    tid,
                                    (
                                        Err(anyhow!("Task error: {:?}", error)),
                                        Arc::new(Notify::new()),
                                    ),
                                );
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    panic!("Error in event recv: {e}");
                }
            }
        }
        let seconds_since_start = start_time.elapsed().as_secs();
        if seconds_since_start.is_multiple_of(5) {
            let tasks = event_listen_task_results.lock().await;
            info!(
                "Event listener running for {} seconds with {} tasks",
                seconds_since_start,
                tasks.len()
            );
        }
    });
}
