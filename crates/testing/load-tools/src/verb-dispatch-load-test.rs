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

//! Measures concurrent verb executions under load

mod setup;

use crate::setup::{
    broadcast_handle, create_user_session, initialization_session, listen_responses,
    ExecutionContext,
};
use clap::Parser;
use clap_derive::Parser;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use moor_values::model::ObjectRef;
use moor_values::{v_int, Obj, Symbol, Var};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_async_client::{make_host_token, start_host_session};
use rpc_common::client_args::RpcClientArgs;
use rpc_common::DaemonToClientReply::TaskSubmitted;
use rpc_common::{
    load_keypair, AuthToken, ClientToken, HostClientToDaemonMessage, HostType, ReplyResult,
};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tmq::request;
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[command(flatten)]
    client_args: RpcClientArgs,

    #[arg(
        long,
        value_name = "num-concurrent-workloads",
        help = "Number of concurrent fake users to generate load",
        default_value = "50"
    )]
    num_concurrent_workloads: usize,

    #[arg(
        long,
        value_name = "num-objects",
        help = "Number of objects to create for the workload",
        default_value = "20"
    )]
    num_objects: usize,

    #[arg(
        long,
        value_name = "num-verb-invocations",
        help = "How many times to invoke the top-level verb which then calls the load verb for each object",
        default_value = "500"
    )]
    num_verb_invocations: usize,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,
}

/// Create N objects as children of the player, these wil be used to invoke the verb which is
/// being run in the load test.
const LOAD_TEST_INITIALIZATION_SCRIPT: &str = r#"
notify(player, "Initializing load test objects: " + toliteral(num_objects));
try
    add_property(player, "test_objects", {}, {player, "rw"});
    notify(player, "Added test_objects property");
    for i in [1..num_objects]
        let obj = create(player);
        player.test_objects = {@player.test_objects, obj};
        notify(player, "Created object: " + toliteral(obj));
    endfor
    add_verb(player, {player, "rx", "invoke_load_test"}, {"this", "none", "this"});
    add_verb(player, {player, "rx", "load_test"}, {"this", "none", "this"});
except e (ANY)
    notify(player, "Already initialized");
endtry
notify(player, "Initialized load test objects");
"#;

const LOAD_TEST_INVOKE_VERB: &str = r#"
let num_verb_invocations = args[1];
for i in [1..num_verb_invocations]
    for obj in (player.test_objects)
        obj:load_test();
    endfor
endfor
"#;

const LOAD_TEST_VERB: &str = r#"
return 1;
"#;

#[allow(clippy::too_many_arguments)]
async fn workload(
    args: Args,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    _process_id: usize,
    connection_oid: Obj,
    auth_token: AuthToken,
    client_token: ClientToken,
    client_id: Uuid,
    task_results: Arc<Mutex<HashMap<usize, Result<Var, eyre::Report>>>>,
) -> Result<Duration, eyre::Error> {
    let rpc_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(rpc_address.as_str())
        .expect("Unable to bind RPC server for connection");
    let mut rpc_client = RpcSendClient::new(rpc_request_sock);
    let start_time = Instant::now();
    for _ in 0..args.num_verb_invocations {
        let response = rpc_client
            .make_client_rpc_call(
                client_id,
                HostClientToDaemonMessage::InvokeVerb(
                    client_token.clone(),
                    auth_token.clone(),
                    ObjectRef::Id(connection_oid.clone()),
                    Symbol::mk("invoke_load_test"),
                    vec![v_int(args.num_verb_invocations as i64)],
                ),
            )
            .await
            .expect("Unable to send call request to RPC server");

        let task_id = match response {
            ReplyResult::HostSuccess(hs) => {
                panic!("Unexpected host message: {:?}", hs);
            }
            ReplyResult::ClientSuccess(TaskSubmitted(submitted_task_id)) => submitted_task_id,
            ReplyResult::ClientSuccess(e) => {
                panic!("Unexpected client result in call: {:?}", e);
            }
            ReplyResult::Failure(e) => {
                panic!("RPC failure in call: {}", e);
            }
        };

        let start_time = Instant::now();
        loop {
            {
                let mut tasks = task_results.lock().await;
                if let Some(results) = tasks.remove(&task_id) {
                    break results;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));

            if start_time.elapsed().as_secs() > 5 {
                panic!("Timed out waiting for task results");
            }
        }
        .expect("Task results not found");
    }

    Ok(start_time.elapsed())
}

async fn load_test_workload(
    args: Args,
    ExecutionContext {
        zmq_ctx,
        kill_switch,
    }: ExecutionContext,
) -> Result<(), eyre::Error> {
    let (
        connection_oid,
        auth_token,
        client_token,
        client_id,
        rpc_client,
        events_sub,
        broadcast_sub,
    ) = create_user_session(
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        args.client_args.events_address.clone(),
    )
    .await?;

    {
        let kill_switch = kill_switch.clone();
        let zmq_ctx = zmq_ctx.clone();
        let rpc_address = args.client_args.rpc_address.clone();
        let client_token = client_token.clone();
        let connection_oid = connection_oid.clone();
        tokio::spawn(async move {
            broadcast_handle(
                zmq_ctx,
                rpc_address,
                broadcast_sub,
                client_id,
                client_token,
                connection_oid,
                kill_switch,
            )
            .await;
        });
    }

    info!("Initializing load-test workload session (creating properties & verbs)");
    let num_props_script = format!(
        "let num_objects = {};{}",
        args.num_objects, LOAD_TEST_INITIALIZATION_SCRIPT
    );

    initialization_session(
        connection_oid.clone(),
        auth_token.clone(),
        client_token.clone(),
        client_id,
        rpc_client,
        &num_props_script,
        &[
            (
                Symbol::mk("invoke_load_test"),
                LOAD_TEST_INVOKE_VERB.to_string(),
            ),
            (Symbol::mk("load_test"), LOAD_TEST_VERB.to_string()),
        ],
    )
    .await?;

    info!("Load-test workload session initialized, starting load test");

    let task_results = Arc::new(Mutex::new(HashMap::new()));
    listen_responses(
        client_id,
        events_sub,
        kill_switch.clone(),
        task_results.clone(),
    )
    .await;

    let start_time = std::time::Instant::now();
    // Spawn N = num_users threads that call the invoke_load_test verb
    info!(
        "Starting {} concurrent workloads",
        args.num_concurrent_workloads
    );
    let mut workload_futures = FuturesUnordered::new();
    for i in 0..args.num_concurrent_workloads {
        let zmq_ctx = zmq_ctx.clone();
        let connection_oid = connection_oid.clone();
        let auth_token = auth_token.clone();
        let client_token = client_token.clone();
        let rpc_address = args.client_args.rpc_address.clone();
        let args = args.clone();
        let task_results = task_results.clone();
        workload_futures.push(workload(
            args,
            zmq_ctx,
            rpc_address,
            i,
            connection_oid,
            auth_token,
            client_token,
            client_id,
            task_results,
        ));
    }

    info!(
        "Waiting for {} workloads to complete...",
        workload_futures.len()
    );

    let mut times = vec![];
    while let Some(h) = workload_futures.next().await {
        times.push(h.expect("Workload failed"));
    }

    let sum_time = times.iter().fold(Duration::new(0, 0), |acc, x| acc + *x);
    info!(
        "All workloads completed in {:?}s total vs {}s cumulative",
        start_time.elapsed().as_secs(),
        sum_time.as_secs_f64()
    );
    let total_verb_invocations =
        args.num_verb_invocations * args.num_concurrent_workloads * args.num_objects;
    info!(
        "Verb invocations completed : {}/s cumulative {}/s concurrent. Across {} objects, {} invocations",
        total_verb_invocations as f64 / sum_time.as_secs_f64(),
        total_verb_invocations as f64 / start_time.elapsed().as_secs_f64(),
        args.num_objects,
        total_verb_invocations
    );
    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install().expect("Unable to install color_eyre");
    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_max_level(if args.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    let zmq_ctx = tmq::Context::new();
    let kill_switch = Arc::new(AtomicBool::new(false));

    let keypair = load_keypair(&args.client_args.public_key, &args.client_args.private_key)
        .expect("Unable to load keypair from public and private key files");
    let host_token = make_host_token(&keypair, HostType::TCP);

    let (listeners, _ljh) = setup::noop_listeners_loop().await;

    let _rpc_client = start_host_session(
        host_token.clone(),
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
    )
    .await
    .expect("Unable to establish initial host session");

    let exec_context = ExecutionContext {
        zmq_ctx,
        kill_switch: kill_switch.clone(),
    };

    load_test_workload(args, exec_context).await?;

    kill_switch.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}
