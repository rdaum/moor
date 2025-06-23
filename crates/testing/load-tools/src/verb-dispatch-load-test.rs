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

//! Measures concurrent verb executions under load

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#[cfg_attr(coverage_nightly, coverage(off))]
mod setup;

use crate::setup::{
    ExecutionContext, broadcast_handle, create_user_session, initialization_session,
    listen_responses,
};
use clap::Parser;
use clap_derive::Parser;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use moor_common::model::ObjectRef;
use moor_var::{Obj, Symbol, Var, v_int};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_async_client::{make_host_token, start_host_session};
use rpc_common::DaemonToClientReply::TaskSubmitted;
use rpc_common::client_args::RpcClientArgs;
use rpc_common::{
    AuthToken, ClientToken, DaemonToHostReply, HostClientToDaemonMessage, HostToDaemonMessage,
    HostToken, HostType, ReplyResult, load_keypair,
};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tmq::request;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::info;
use uuid::Uuid;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[command(flatten)]
    client_args: RpcClientArgs,

    #[arg(
        long,
        help = "Min number of concurrent fake users to generate load. Load tests will start at `min_concurrent_workload` and increase to `max_concurrent_workload`.",
        default_value = "1"
    )]
    min_concurrent_workload: usize,

    #[arg(
        long,
        help = "Max number of concurrent fake users to generate load.",
        default_value = "32"
    )]
    max_concurrent_workload: usize,

    #[arg(
        long,
        help = "Number of objects to create for the workload",
        default_value = "10"
    )]
    num_objects: usize,

    #[arg(
        long,
        help = "How many times the top-level verb should call the workload verb",
        default_value = "1000"
    )]
    num_verb_iterations: usize,

    #[arg(
        long,
        help = "How many times the top-level verb should be called.",
        default_value = "100"
    )]
    num_verb_invocations: usize,

    #[arg(long, help = "CSV output file for benchmark data")]
    output_file: Option<PathBuf>,

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
        object = create(player);
        player.test_objects = {@player.test_objects, object};
        notify(player, "Created object: " + toliteral(object));
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
    for object in (player.test_objects)
        if (object:load_test() != 1) 
            raise(E_INVARG, "Load test failed");
        endif
    endfor
endfor
return 1;
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
                    ObjectRef::Id(connection_oid),
                    Symbol::mk("invoke_load_test"),
                    vec![v_int(args.num_verb_iterations as i64)],
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

        let wait_time = Instant::now();
        let results = loop {
            {
                let mut tasks = task_results.lock().await;
                if let Some(results) = tasks.remove(&task_id) {
                    break results;
                }
            }
            sleep(Duration::from_millis(1)).await;

            if wait_time.elapsed().as_secs() > 10 {
                panic!("Timed out waiting for task results");
            }
        }
        .expect("Task results not found");

        let Some(result) = results.as_integer() else {
            panic!("Unexpected task result: {:?}", results);
        };
        if result != 1 {
            panic!("Load test failed");
        }
    }

    Ok(start_time.elapsed())
}

async fn request_counters(
    zmq_ctx: tmq::Context,
    rpc_address: String,
    host_token: &HostToken,
) -> Result<HashMap<Symbol, HashMap<Symbol, (isize, isize)>>, eyre::Error> {
    let rpc_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(rpc_address.as_str())
        .expect("Unable to bind RPC server for connection");
    let mut rpc_client = RpcSendClient::new(rpc_request_sock);
    let response = rpc_client
        .make_host_rpc_call(host_token, HostToDaemonMessage::RequestPerformanceCounters)
        .await
        .expect("Unable to send call request to RPC server");
    let ReplyResult::HostSuccess(DaemonToHostReply::PerfCounters(_, counters)) = response else {
        panic!("Unexpected response from daemon: {:?}", response);
    };

    // Build a map of maps for the counters.
    let mut counters_map = HashMap::new();
    for (category, counter_list) in counters {
        let mut category_map = HashMap::new();
        for (counter_name, count, total) in counter_list {
            category_map.insert(counter_name, (count, total));
        }
        counters_map.insert(category, category_map);
    }

    Ok(counters_map)
}

fn process_counters(
    before_counters: HashMap<Symbol, HashMap<Symbol, (isize, isize)>>,
    after_counters: HashMap<Symbol, HashMap<Symbol, (isize, isize)>>,
) -> BTreeMap<String, (f64, f64, isize)> {
    let mut results = BTreeMap::new();
    for (category, counters) in after_counters {
        let mut diff = HashMap::new();
        for (counter_name, (count, total)) in counters {
            let before_count = before_counters
                .get(&category)
                .and_then(|c| c.get(&counter_name))
                .map(|c| c.0)
                .unwrap_or(0);
            let before_total = before_counters
                .get(&category)
                .and_then(|c| c.get(&counter_name))
                .map(|c| c.1)
                .unwrap_or(0);
            diff.insert(counter_name, (count - before_count, total - before_total));
        }
        // Print the averages for each counter
        for (counter_name, (count, total)) in diff {
            let total = (total as f64) / 1000.0;
            let avg = total / (count as f64);
            results.insert(format!("{category}/{counter_name}"), (avg, total, count));
        }
    }
    results
}

async fn load_test_workload(
    args: &Args,
    ExecutionContext {
        zmq_ctx,
        kill_switch,
    }: ExecutionContext,
    host_token: &HostToken,
) -> Result<Vec<Results>, eyre::Error> {
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
        connection_oid,
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

    let mut results = vec![];

    // Do one throw-awy workload run to warm up the system.
    info!("Running warm-up workload run...");
    let warmup_start = Instant::now();
    for _ in 0..5 {
        workload(
            args.clone(),
            zmq_ctx.clone(),
            args.client_args.rpc_address.clone(),
            1,
            connection_oid,
            auth_token.clone(),
            client_token.clone(),
            client_id,
            task_results.clone(),
        )
        .await?;
    }
    info!(
        "Warm-up workload run completed in {:?}",
        warmup_start.elapsed()
    );

    // Cool down for a couple seconds before starting the actual load test.
    info!("Cooling down for 2 seconds before starting the load test...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut concurrency = args.min_concurrent_workload as f32;
    loop {
        if concurrency > args.max_concurrent_workload as f32 {
            break;
        }
        let num_concurrent_workload = concurrency as usize;
        let start_time = Instant::now();

        let before_counters = request_counters(
            zmq_ctx.clone(),
            args.client_args.rpc_address.clone(),
            host_token,
        )
        .await?;
        info!(
            "Starting {num_concurrent_workload} threads workloads, calling load test {} times, which does {} dispatch iterations...",
            args.num_verb_invocations, args.num_verb_iterations
        );
        let mut workload_futures = FuturesUnordered::new();
        for i in 0..num_concurrent_workload {
            let zmq_ctx = zmq_ctx.clone();
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

        let mut times = vec![];
        while let Some(h) = workload_futures.next().await {
            times.push(h.expect("Workload failed"));
        }

        let after_counters = request_counters(
            zmq_ctx.clone(),
            args.client_args.rpc_address.clone(),
            host_token,
        )
        .await?;

        let processed_counters = process_counters(before_counters, after_counters);

        let cumulative_time = times.iter().fold(Duration::new(0, 0), |acc, x| acc + *x);
        let total_time = start_time.elapsed();
        let total_invocations = args.num_verb_invocations * num_concurrent_workload;
        let total_verb_calls =
            (args.num_verb_invocations * args.num_verb_iterations * num_concurrent_workload)
                + total_invocations;
        let r = Results {
            concurrency: num_concurrent_workload,
            total_invocations,
            total_time,
            cumulative_time,
            total_verb_calls,
            per_verb_call: Duration::from_secs_f64(
                cumulative_time.as_secs_f64() / total_verb_calls as f64,
            ),
            counters: processed_counters,
        };
        info!(
            "@ Concurrency: {} w/ total invocations: {}, ({total_verb_calls} total verb calls): Total Time: {:?}, Cumulative: {:?}, Per Verb Dispatch: {:?} ",
            r.concurrency, r.total_invocations, r.total_time, r.cumulative_time, r.per_verb_call
        );
        results.push(r);

        // Scale up by 25% or 1, whichever is larger, so we don't get stuck on lower values.
        let mut next_concurrency = concurrency * 1.25;
        if next_concurrency as usize <= concurrency as usize {
            next_concurrency = concurrency + 1.0;
        }
        concurrency = next_concurrency;
    }
    Ok(results)
}

struct Results {
    /// How many concurrent threads there were.
    concurrency: usize,
    /// How many times the top-level verb was invoked
    total_invocations: usize,
    /// How many total verb calls that led to
    total_verb_calls: usize,
    /// The duration of the whole load test
    total_time: Duration,
    /// The cumulative time actually spent waiting for the daemon to respond
    cumulative_time: Duration,
    /// The time per verb dispatch
    per_verb_call: Duration,
    /// All system performance counters aggregated before and after the load run
    counters: BTreeMap<String, (f64, f64, isize)>,
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

    let (private, _public) =
        load_keypair(&args.client_args.public_key, &args.client_args.private_key)
            .expect("Unable to load keypair from public and private key files");
    let host_token = make_host_token(&private, HostType::TCP);

    let (listeners, _ljh) = setup::noop_listeners_loop().await;

    let rpc_address = args.client_args.rpc_address.clone();
    let _rpc_client = start_host_session(
        &host_token,
        zmq_ctx.clone(),
        rpc_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
    )
    .await
    .expect("Unable to establish initial host session");

    let exec_context = ExecutionContext {
        zmq_ctx: zmq_ctx.clone(),
        kill_switch: kill_switch.clone(),
    };

    let results = load_test_workload(&args, exec_context, &host_token).await?;

    if let Some(output_file) = args.output_file {
        let num_records = results.len();
        let mut writer =
            csv::Writer::from_path(&output_file).expect("Could not open benchmark output file");

        // Use first row of results to figure out the header.
        let first_row = results.first().expect("No results found");
        let mut header = vec![
            "concurrency".to_string(),
            "total_invocations".to_string(),
            "total_verb_calls".to_string(),
            "total_time_ns".to_string(),
            "per_dispatch_time_ns".to_string(),
        ];
        for x in first_row.counters.keys() {
            header.push(format!("{}-avg_μs", x));
            header.push(format!("{}-total_μs", x));
            header.push(format!("{}-count", x));
        }
        writer.write_record(header)?;
        for r in results {
            let mut base = vec![
                r.concurrency.to_string(),
                r.total_invocations.to_string(),
                r.total_verb_calls.to_string(),
                r.total_time.as_nanos().to_string(),
                r.per_verb_call.as_nanos().to_string(),
            ];
            for (_, (avg, total, count)) in r.counters {
                base.push(avg.to_string());
                base.push(total.to_string());
                base.push(count.to_string());
            }
            writer.write_record(base)?
        }
        info!("Wrote {num_records} to {}", output_file.display())
    }
    kill_switch.store(true, std::sync::atomic::Ordering::Relaxed);

    Ok(())
}
