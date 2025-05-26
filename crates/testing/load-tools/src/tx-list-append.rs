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

//! A utility to exercise a jepsen/elle `list-append` workload against the moor daemon.
//! Connects num-concurrent-users to the daemon in parallel, and then executes `num-workload-executions`
//! of random read or appends to `num-props` random properties (which are lists of integers).
//! The results are written to a file in the EDN format that `elle-cli` can consume.
//! See: https://github.com/ligurio/elle-cli

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#[cfg_attr(coverage_nightly, coverage(off))]
mod setup;

use crate::setup::{
    broadcast_handle, create_user_session, initialization_session, listen_responses,
};
use clap::Parser;
use clap_derive::Parser;
use edn_format::{Keyword, Value};
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use moor_common::model::ObjectRef;
use moor_var::{List, Obj, Sequence, Symbol, Var, v_int, v_list};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_async_client::{make_host_token, start_host_session};
use rpc_common::DaemonToClientReply::TaskSubmitted;
use rpc_common::client_args::RpcClientArgs;
use rpc_common::{
    AuthToken, ClientToken, HostClientToDaemonMessage, HostType, ReplyResult, load_keypair,
};
use setup::ExecutionContext;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tmq::request;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, info};
use uuid::Uuid;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[command(flatten)]
    client_args: RpcClientArgs,

    #[arg(
        long,
        value_name = "num-users",
        help = "Number of concurrent fake users to generate load",
        default_value = "8"
    )]
    num_users: usize,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,

    #[arg(
        long,
        value_name = "num-props",
        help = "Number of properties to use in the workload",
        default_value = "5"
    )]
    num_props: usize,

    #[arg(
        long,
        value_name = "num-concurrent-workloads",
        help = "Number of concurrent workloads to run",
        default_value = "20"
    )]
    num_concurrent_workloads: usize,

    #[arg(
        long,
        value_name = "num-workload-executions",
        help = "Number of executions per workload",
        default_value = "20"
    )]
    num_workload_iterations: usize,

    #[arg(
        long,
        value_name = "output-file",
        help = "File to write the workload to",
        default_value = "workload.edn"
    )]
    output_file: PathBuf,
}

// Script for creating the set of properties we want to use
const LIST_APPEND_INITIALIZATION_SCRIPT: &str = r#"
for i in [1..num_props]
    let prop = "prop_" + tostr(i);
    try
        add_property(player, prop, {}, {player, "rw"});
    except e (ANY)
        player.(prop) = {};
    endtry
    player:tell("Added property " + prop);
endfor
try
    add_verb(player, {player, "rx", "write_workload"}, {"this", "none", "this"});
    add_verb(player, {player, "rx", "read_workload"}, {"this", "none", "this"});
except e (ANY)
endtry
player:tell("Added verbs: " + tostr(player) + ":" + toliteral(verbs(player)));
suspend(1);
return 1;
"#;

/// Verb code for writing to the properties. Returns the pre-write common and the written common
const LIST_APPEND_WRITE_WORKLOAD_VERB: &str = r#"
append_props = args[1];
let read_log = {};
let write_log = {};
for i in [1..length(append_props)]
    let append_prop_num = append_props[i];
    let append_prop = "prop_" + tostr(append_prop_num);

    let read_values = player.(append_prop);
    read_log = {@read_log, {append_prop_num, read_values}};

    let num_random_values = random(50);
    let write_values = {};
    for r in [1..num_random_values]
        let random_value = random(1000);
        if (!(random_value in write_values) && !(random_value in read_values))
            write_values = setadd(write_values, random_value);
        endif
    endfor
    player.(append_prop) = {@read_values, @write_values};
    write_log = {@write_log, {append_prop_num, write_values}};
endfor
return {read_log, write_log};
"#;

/// Verb code for a read workload. Just reads from random properties and returns the common
const LIST_APPEND_READ_WORKLOAD_VERB: &str = r#"
read_props = args[1];
let read_log = {};
for i in [1..length(read_props)]
    let read_prop_num = read_props[i];
    let read_prop = "prop_" + tostr(read_prop_num);
    let read_values = player.(read_prop);
    read_log = {@read_log, {read_prop_num, read_values}};
endfor
return {read_log};
"#;

#[derive(Debug, Clone)]
enum WorkItem {
    Append(usize, Vec<(usize, Vec<i64>)>),
    Read(usize, Vec<(usize, Vec<i64>)>),
    ReadEnd(usize, Vec<(usize, Vec<i64>)>),
    WriteEnd(usize, Vec<(usize, Vec<i64>)>),
}

fn process_reads(read_log: &List) -> Vec<(usize, Vec<i64>)> {
    let mut reads = vec![];
    for prop_entry in read_log.iter() {
        let Some(l) = prop_entry.as_list() else {
            panic!("Unexpected read log entry: {:?}", prop_entry);
        };
        let prop_entry: Vec<_> = l.iter().collect();

        // first item should be the prop num, second should be the readvalues
        let (prop, values) = {
            let Some(prop_num) = prop_entry[0].as_integer() else {
                panic!("Unexpected prop num value: {:?}", prop_entry[0]);
            };

            let Some(values) = prop_entry[1].as_list() else {
                panic!(
                    "Unexpected prop common for prop_num {}: {:?}",
                    prop_num, prop_entry[1]
                );
            };

            let values = values.iter().map(|v| {
                if let Some(i) = v.as_integer() {
                    i
                } else {
                    panic!("Unexpected prop value: {:?}", v);
                }
            });
            (prop_num, values.collect::<Vec<_>>())
        };
        reads.push((prop as usize, values));
    }
    reads
}

fn process_writes(write_log: &List) -> Vec<(usize, Vec<i64>)> {
    let mut appends = vec![];
    for prop_entry in write_log.iter() {
        let Some(l) = prop_entry.as_list() else {
            panic!("Unexpected write log entry: {:?}", prop_entry);
        };
        let prop_entry: Vec<_> = l.iter().collect();
        // first item should be the prop num, second should be the written common
        let (prop, values) = {
            let Some(prop_num) = prop_entry[0].as_integer() else {
                panic!("Unexpected prop num value: {:?}", prop_entry[0]);
            };

            let Some(values) = prop_entry[1].as_list() else {
                panic!(
                    "Unexpected prop common for prop_num {}: {:?}",
                    prop_num, prop_entry[1]
                );
            };

            let values = values.iter().map(|v| {
                if let Some(i) = v.as_integer() {
                    i
                } else {
                    panic!("Unexpected prop value: {:?}", v);
                }
            });
            (prop_num, values.collect::<Vec<_>>())
        };
        appends.push((prop as usize, values));
    }
    appends
}

#[allow(clippy::too_many_arguments)]
async fn workload(
    args: Args,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    process_id: usize,
    connection_oid: Obj,
    auth_token: AuthToken,
    client_token: ClientToken,
    client_id: Uuid,
    task_results: Arc<Mutex<HashMap<usize, Result<Var, eyre::Report>>>>,
) -> Result<Vec<(Instant, WorkItem)>, eyre::Error> {
    debug!(
        "Workload process {} starting, performing {} iterations across {} properties ",
        process_id, args.num_workload_iterations, args.num_props
    );
    let rpc_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(rpc_address.as_str())
        .expect("Unable to bind RPC server for connection");
    let mut rpc_client = RpcSendClient::new(rpc_request_sock);
    let mut workload = vec![];
    for _ in 0..args.num_workload_iterations {
        // Pick a random sized set of random props
        let num_props = rand::random::<usize>() % args.num_props;
        if num_props == 0 {
            continue;
        }
        let mut prop_keys = vec![];
        for i in 0..num_props {
            prop_keys.push(v_int((i + 1) as i64));
        }

        // Are we doing a read or a write workload?
        let is_read = rand::random::<bool>();

        let response = rpc_client
            .make_client_rpc_call(
                client_id,
                HostClientToDaemonMessage::InvokeVerb(
                    client_token.clone(),
                    auth_token.clone(),
                    ObjectRef::Id(connection_oid),
                    if is_read {
                        Symbol::mk("read_workload")
                    } else {
                        Symbol::mk("write_workload")
                    },
                    vec![v_list(&prop_keys)],
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
        // Spin waiting for results to show up in the task_results map
        let result = loop {
            {
                let mut tasks = task_results.lock().await;
                if let Some(results) = tasks.remove(&task_id) {
                    break results;
                }
            }
            sleep(std::time::Duration::from_millis(100)).await;

            if start_time.elapsed().as_secs() > 5 {
                panic!("Timed out waiting for task results");
            }
        }
        .expect("Task results not found");

        // For our workload this should be a list, and if it isn't there's something messed up!
        let Some(result) = result.as_list() else {
            panic!("Unexpected result: {:?}", result);
        };

        if is_read {
            let read_log = result.index(0).unwrap();
            let Some(read_log) = read_log.as_list() else {
                panic!("Unexpected read log type: {:?}", read_log);
            };

            let reads = process_reads(read_log);
            if reads.is_empty() {
                continue;
            }
            workload.push((Instant::now(), WorkItem::Read(process_id, reads.clone())));
            workload.push((Instant::now(), WorkItem::ReadEnd(process_id, reads)));
        } else {
            let write_log = result.index(1).unwrap();
            let Some(write_log) = write_log.as_list() else {
                panic!("Unexpected write log type: {:?}", write_log);
            };
            // let reads = process_reads(process_id, read_log);
            let appends = process_writes(write_log);
            if appends.is_empty() {
                continue;
            }
            workload.push((
                Instant::now(),
                WorkItem::Append(process_id, appends.clone()),
            ));
            workload.push((Instant::now(), WorkItem::WriteEnd(process_id, appends)));
        }
    }
    Ok(workload)
}

async fn list_append_workload(
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

    info!("Initializing workload session (creating properties & verbs)");
    let num_props_script = format!(
        "let num_props = {};{}",
        args.num_props, LIST_APPEND_INITIALIZATION_SCRIPT
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
                Symbol::mk("write_workload"),
                LIST_APPEND_WRITE_WORKLOAD_VERB.to_string(),
            ),
            (
                Symbol::mk("read_workload"),
                LIST_APPEND_READ_WORKLOAD_VERB.to_string(),
            ),
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

    info!("Starting {} workloads", args.num_concurrent_workloads);
    let mut workload_futures = FuturesUnordered::new();
    for i in 0..args.num_concurrent_workloads {
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

    info!(
        "Waiting for {} workloads to complete...",
        workload_futures.len()
    );

    let mut workload_results = vec![];
    while let Some(h) = workload_futures.next().await {
        let result = h.unwrap();
        workload_results.extend_from_slice(&result);
    }

    // Now sort the entire workload by the instant (first element of the tuple) they were
    // performed.
    workload_results.sort_by(|a, b| a.0.cmp(&b.0));

    info!(
        "Workloads performed. {} execution records",
        workload_results.len()
    );

    let mut output_document = String::new();

    // Generate EDN from the workloads, and emit in a form that elle can consume
    for (i, workload) in workload_results.iter().enumerate() {
        let mut map = BTreeMap::new();
        match &workload.1 {
            // {:index 1, :type :invoke, :process 2, :value [[ :append 4 2] [ :append 5 5] ]}
            WorkItem::Append(process, appends) => {
                if appends.is_empty() {
                    continue;
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("process")),
                    Value::Integer(*process as i64),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("index")),
                    Value::Integer(i as i64),
                );
                let mut append_ops = vec![];
                for (property, values) in appends {
                    for value in values {
                        append_ops.push(Value::Vector(vec![
                            Value::Keyword(Keyword::from_name("append")),
                            Value::Integer(*property as i64),
                            Value::Integer(*value),
                        ]));
                    }
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("value")),
                    Value::Vector(append_ops),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("type")),
                    Value::Keyword(Keyword::from_name("invoke")),
                );
            }
            //{:index 2, :type :invoke, :process 0, :value [[ :r 2 nil] [ :r 3 nil] [ :append
            WorkItem::Read(process, reads) => {
                if reads.is_empty() {
                    continue;
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("process")),
                    Value::Integer(*process as i64),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("index")),
                    Value::Integer(i as i64),
                );
                let mut read_ops = vec![];
                for (property, values) in reads {
                    read_ops.push(Value::Vector(vec![
                        Value::Keyword(Keyword::from_name("r")),
                        Value::Integer(*property as i64),
                        Value::Vector(values.iter().map(|v| Value::Integer(*v)).collect()),
                    ]));
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("value")),
                    Value::Vector(read_ops),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("type")),
                    Value::Keyword(Keyword::from_name("invoke")),
                );
            }
            //{:index 4, :type :ok, :process 2, :value [[ :append 4 2] [ :append 5 5] ]}
            WorkItem::WriteEnd(process, appends) => {
                map.insert(
                    Value::Keyword(Keyword::from_name("index")),
                    Value::Integer(i as i64),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("process")),
                    Value::Integer(*process as i64),
                );
                let mut append_ops = vec![];
                for (property, values) in appends {
                    for value in values {
                        append_ops.push(Value::Vector(vec![
                            Value::Keyword(Keyword::from_name("append")),
                            Value::Integer(*property as i64),
                            Value::Integer(*value),
                        ]));
                    }
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("value")),
                    Value::Vector(append_ops),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("type")),
                    Value::Keyword(Keyword::from_name("ok")),
                );
            }
            WorkItem::ReadEnd(process, reads) => {
                map.insert(
                    Value::Keyword(Keyword::from_name("index")),
                    Value::Integer(i as i64),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("process")),
                    Value::Integer(*process as i64),
                );
                let mut read_ops = vec![];
                for (property, values) in reads {
                    read_ops.push(Value::Vector(vec![
                        Value::Keyword(Keyword::from_name("r")),
                        Value::Integer(*property as i64),
                        Value::Vector(values.iter().map(|v| Value::Integer(*v)).collect()),
                    ]));
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("value")),
                    Value::Vector(read_ops),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("type")),
                    Value::Keyword(Keyword::from_name("ok")),
                );
            }
        }
        let edn_value = Value::Map(map);
        output_document.push_str(&format!("{}\n", edn_format::emit_str(&edn_value)));
    }
    std::fs::write(&args.output_file, output_document)?;
    info!("Workload written to {}", args.output_file.display());

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

    let (private, _public) =
        load_keypair(&args.client_args.public_key, &args.client_args.private_key)
            .expect("Unable to load keypair from public and private key files");
    let host_token = make_host_token(&private, HostType::TCP);

    let (listeners, _ljh) = setup::noop_listeners_loop().await;

    let _rpc_client = start_host_session(
        &host_token,
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
    list_append_workload(args, exec_context).await?;

    kill_switch.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}
