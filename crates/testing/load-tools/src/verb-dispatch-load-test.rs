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
    listen_responses, wait_for_task_completion,
};
use clap::Parser;
use clap_derive::Parser;
use futures::{StreamExt, stream::FuturesUnordered};
use moor_common::model::ObjectRef;
use moor_schema::rpc as moor_rpc;
use moor_var::{Obj, Symbol, Var, v_int};
use planus::ReadAsRoot;
use rpc_async_client::{rpc_client::RpcClient, start_host_session};
use rpc_common::{
    AuthToken, ClientToken, client_args::RpcClientArgs, mk_invoke_verb_msg,
    mk_request_performance_counters_msg,
};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Notify};
use tracing::info;
use uuid::Uuid;

type TaskResults = Arc<Mutex<HashMap<usize, (Result<Var, eyre::Report>, Arc<Notify>)>>>;

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
        default_value = "7000"
    )]
    num_verb_iterations: usize,

    #[arg(
        long,
        help = "How many times the top-level verb should be called.",
        default_value = "200"
    )]
    num_verb_invocations: usize,

    #[arg(long, help = "CSV output file for benchmark data")]
    output_file: Option<PathBuf>,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,

    #[arg(
        long,
        help = "Swamp mode: immediately run at maximum concurrency with all requests in parallel to stress test the server",
        default_value = "false"
    )]
    swamp_mode: bool,

    #[arg(
        long,
        help = "Duration in seconds to run swamp mode (continuously sending requests)",
        default_value = "30"
    )]
    swamp_duration_seconds: u64,
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
async fn continuous_workload(
    args: Args,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    _process_id: usize,
    connection_oid: Obj,
    auth_token: AuthToken,
    client_token: ClientToken,
    client_id: Uuid,
    task_results: TaskResults,
    stop_time: Instant,
) -> Result<(Duration, usize), eyre::Error> {
    // Create managed RPC client with connection pooling and cancellation safety
    let rpc_client = RpcClient::new_with_defaults(
        std::sync::Arc::new(zmq_ctx.clone()),
        rpc_address.clone(),
        None, // No CURVE encryption for load testing
    );
    let start_time = Instant::now();
    let mut request_count = 0;

    while Instant::now() < stop_time {
        let num_iterations = v_int(args.num_verb_iterations as i64);
        let invoke_msg = mk_invoke_verb_msg(
            &client_token,
            &auth_token,
            &ObjectRef::Id(connection_oid),
            &Symbol::mk("invoke_load_test"),
            vec![&num_iterations],
        )
        .expect("Failed to create invoke_verb message");

        let reply_bytes = rpc_client
            .make_client_rpc_call(client_id, invoke_msg)
            .await
            .expect("Unable to send call request to RPC server");

        let reply =
            moor_rpc::ReplyResultRef::read_as_root(&reply_bytes).expect("Failed to parse reply");

        let task_id = match reply.result().expect("Failed to get reply result") {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success.reply().expect("Failed to get daemon reply");
                match daemon_reply
                    .reply()
                    .expect("Failed to get daemon reply union")
                {
                    moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted) => {
                        task_submitted.task_id().expect("Failed to get task_id")
                    }
                    other => panic!("Unexpected client result in call: {other:?}"),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure.error().expect("Failed to get error");
                let error_msg = error
                    .message()
                    .expect("Failed to get error message")
                    .unwrap_or("Unknown error");
                panic!("RPC failure in call: {error_msg}");
            }
            other => panic!("Unexpected reply result: {other:?}"),
        };

        let results = wait_for_task_completion(
            task_id as usize,
            task_results.clone(),
            Duration::from_secs(10),
        )
        .await
        .expect("Failed to get task completion")
        .expect("Task results not found");

        let Some(result) = results.as_integer() else {
            panic!("Unexpected task result: {results:?}");
        };
        if result != 1 {
            panic!("Load test failed");
        }

        request_count += 1;
    }

    Ok((start_time.elapsed(), request_count))
}

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
    task_results: TaskResults,
) -> Result<Duration, eyre::Error> {
    // Create managed RPC client with connection pooling and cancellation safety
    let rpc_client = RpcClient::new_with_defaults(
        std::sync::Arc::new(zmq_ctx.clone()),
        rpc_address.clone(),
        None, // No CURVE encryption for load testing
    );
    let start_time = Instant::now();
    for _ in 0..args.num_verb_invocations {
        let num_iterations = v_int(args.num_verb_iterations as i64);
        let invoke_msg = mk_invoke_verb_msg(
            &client_token,
            &auth_token,
            &ObjectRef::Id(connection_oid),
            &Symbol::mk("invoke_load_test"),
            vec![&num_iterations],
        )
        .expect("Failed to create invoke_verb message");

        let reply_bytes = rpc_client
            .make_client_rpc_call(client_id, invoke_msg)
            .await
            .expect("Unable to send call request to RPC server");

        let reply =
            moor_rpc::ReplyResultRef::read_as_root(&reply_bytes).expect("Failed to parse reply");

        let task_id = match reply.result().expect("Failed to get reply result") {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success.reply().expect("Failed to get daemon reply");
                match daemon_reply
                    .reply()
                    .expect("Failed to get daemon reply union")
                {
                    moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted) => {
                        task_submitted.task_id().expect("Failed to get task_id")
                    }
                    other => panic!("Unexpected client result in call: {other:?}"),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure.error().expect("Failed to get error");
                let error_msg = error
                    .message()
                    .expect("Failed to get error message")
                    .unwrap_or("Unknown error");
                panic!("RPC failure in call: {error_msg}");
            }
            other => panic!("Unexpected reply result: {other:?}"),
        };

        let results = wait_for_task_completion(
            task_id as usize,
            task_results.clone(),
            Duration::from_secs(10),
        )
        .await
        .expect("Failed to get task completion")
        .expect("Task results not found");

        let Some(result) = results.as_integer() else {
            panic!("Unexpected task result: {results:?}");
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
    host_id: Uuid,
) -> Result<HashMap<Symbol, HashMap<Symbol, (isize, isize)>>, eyre::Error> {
    // Create managed RPC client with connection pooling and cancellation safety
    let rpc_client = RpcClient::new_with_defaults(
        std::sync::Arc::new(zmq_ctx.clone()),
        rpc_address.clone(),
        None, // No CURVE encryption for load testing
    );

    let request_msg = mk_request_performance_counters_msg();
    let reply_bytes = rpc_client
        .make_host_rpc_call(host_id, request_msg)
        .await
        .expect("Unable to send call request to RPC server");

    let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)?;

    let counters = match reply.result()? {
        moor_rpc::ReplyResultUnionRef::HostSuccess(host_success) => {
            let daemon_reply = host_success.reply()?;
            match daemon_reply.reply()? {
                moor_rpc::DaemonToHostReplyUnionRef::DaemonToHostPerfCounters(perf_counters) => {
                    perf_counters.counters()?
                }
                other => panic!("Unexpected daemon reply: {other:?}"),
            }
        }
        other => panic!("Unexpected response from daemon: {other:?}"),
    };

    // Build a map of maps for the counters.
    let mut counters_map = HashMap::new();
    for category in counters {
        let category = category?;
        let category_symbol = Symbol::mk(category.category()?.value()?);
        let mut category_map = HashMap::new();

        for counter in category.counters()? {
            let counter = counter?;
            let counter_name = Symbol::mk(counter.name()?.value()?);
            let count = counter.count()? as isize;
            let total = counter.total_cumulative_ns()? as isize;
            category_map.insert(counter_name, (count, total));
        }

        counters_map.insert(category_symbol, category_map);
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

async fn swamp_mode_workload(
    args: &Args,
    ExecutionContext {
        zmq_ctx,
        kill_switch,
    }: ExecutionContext,
    host_id: Uuid,
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

    info!("Initializing swamp mode workload session");
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

    let task_results = Arc::new(Mutex::new(HashMap::new()));
    listen_responses(
        client_id,
        events_sub,
        kill_switch.clone(),
        task_results.clone(),
    )
    .await;

    info!(
        "Starting swamp mode - running {} concurrent threads for {} seconds",
        args.max_concurrent_workload, args.swamp_duration_seconds
    );

    let start_time = Instant::now();
    let duration = Duration::from_secs(args.swamp_duration_seconds);
    let before_counters = request_counters(
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        host_id,
    )
    .await?;

    // Create continuous workload tasks that run for the specified duration
    let mut all_tasks = FuturesUnordered::new();
    let stop_time = start_time + duration;

    for i in 0..args.max_concurrent_workload {
        let zmq_ctx = zmq_ctx.clone();
        let auth_token = auth_token.clone();
        let client_token = client_token.clone();
        let rpc_address = args.client_args.rpc_address.clone();
        let args = args.clone();
        let task_results = task_results.clone();

        all_tasks.push(continuous_workload(
            args,
            zmq_ctx,
            rpc_address,
            i,
            connection_oid,
            auth_token,
            client_token,
            client_id,
            task_results,
            stop_time,
        ));
    }

    // Wait for all tasks to complete
    let mut times = vec![];
    let mut total_requests = 0;
    while let Some(result) = all_tasks.next().await {
        let (time, requests) = result?;
        times.push(time);
        total_requests += requests;
    }

    let after_counters = request_counters(
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        host_id,
    )
    .await?;

    let processed_counters = process_counters(before_counters, after_counters);
    let cumulative_time = times.iter().fold(Duration::new(0, 0), |acc, x| acc + *x);
    let total_time = start_time.elapsed();
    let total_verb_calls = total_requests * args.num_verb_iterations + total_requests;

    let result = Results {
        concurrency: args.max_concurrent_workload,
        total_invocations: total_requests,
        total_time,
        cumulative_time,
        total_verb_calls,
        per_verb_call: Duration::from_secs_f64(
            cumulative_time.as_secs_f64() / total_verb_calls as f64,
        ),
        counters: processed_counters,
    };

    info!(
        "Swamp mode completed: {} concurrent threads, {} total requests, Total Time: {:?}, Cumulative: {:?}, Per Verb: {:?}",
        result.concurrency,
        result.total_invocations,
        result.total_time,
        result.cumulative_time,
        result.per_verb_call
    );

    Ok(vec![result])
}

async fn load_test_workload(
    args: &Args,
    ExecutionContext {
        zmq_ctx,
        kill_switch,
    }: ExecutionContext,
    host_id: Uuid,
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
            host_id,
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
            host_id,
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

    moor_common::tracing::init_tracing(false).unwrap_or_else(|e| {
        eprintln!("Unable to configure logging: {e}");
        std::process::exit(1);
    });

    let zmq_ctx = tmq::Context::new();
    let kill_switch = Arc::new(AtomicBool::new(false));

    let (listeners, _ljh) = setup::noop_listeners_loop().await;

    let host_id = Uuid::new_v4();
    let rpc_address = args.client_args.rpc_address.clone();
    let (_rpc_client, host_id) = start_host_session(
        host_id,
        zmq_ctx.clone(),
        rpc_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
        None, // No CURVE encryption for load testing
    )
    .await
    .expect("Unable to establish initial host session");

    let exec_context = ExecutionContext {
        zmq_ctx: zmq_ctx.clone(),
        kill_switch: kill_switch.clone(),
    };

    let results = if args.swamp_mode {
        swamp_mode_workload(&args, exec_context, host_id).await?
    } else {
        load_test_workload(&args, exec_context, host_id).await?
    };

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
            header.push(format!("{x}-avg_μs"));
            header.push(format!("{x}-total_μs"));
            header.push(format!("{x}-count"));
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
