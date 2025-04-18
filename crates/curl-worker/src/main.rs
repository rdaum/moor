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

use clap::Parser;
use clap_derive::Parser;
use moor_common::tasks::WorkerError;
use moor_var::{Sequence, Symbol, Var, Variant, v_int, v_list, v_list_iter, v_str};
use reqwest::Url;
use rpc_async_client::pubsub_client::workers_events_recv;
use rpc_async_client::{WorkerRpcSendClient, attach_worker, make_worker_token};
use rpc_common::client_args::RpcClientArgs;
use rpc_common::{
    DaemonToWorkerMessage, WORKER_BROADCAST_TOPIC, WorkerToDaemonMessage, WorkerToken, load_keypair,
};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tmq::request;
use tokio::select;
use tokio::signal::unix::{SignalKind, signal};
use tracing::info;
use tracing_subscriber::fmt::format::FmtSpan;
use uuid::Uuid;

// TODO: timeouts, and generally more error handling
// TODO: almost everything in here is generic across any worker and could be moved to a common
//   library, only really perform_http_request is specific to this worker.

#[derive(Parser, Debug)]
struct Args {
    #[command(flatten)]
    client_args: RpcClientArgs,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install()?;
    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_file(true)
        .with_target(false)
        .with_line_number(true)
        .with_thread_names(true)
        .with_span_events(FmtSpan::NONE)
        .with_max_level(if args.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    let mut hup_signal =
        signal(SignalKind::hangup()).expect("Unable to register HUP signal handler");
    let mut stop_signal =
        signal(SignalKind::interrupt()).expect("Unable to register STOP signal handler");

    let kill_switch = Arc::new(AtomicBool::new(false));

    let (private_key, _public_key) =
        load_keypair(&args.client_args.public_key, &args.client_args.private_key)
            .expect("Unable to load keypair from public and private key files");
    let my_id = Uuid::new_v4();
    let worker_token = make_worker_token(&private_key, my_id);

    let zmq_ctx = tmq::Context::new();

    // First attempt to connect to the daemon and "attach" ourselves.
    let _rpc_client = attach_worker(
        &worker_token,
        Symbol::mk("curl"),
        my_id,
        zmq_ctx.clone(),
        args.client_args.workers_response_address.clone(),
    )
    .await
    .expect("Unable to attach to daemon");

    // Now make the pub-sub client to the daemon and listen.
    let sub = tmq::subscribe(&zmq_ctx)
        .connect(&args.client_args.workers_request_address)
        .expect("Unable to connect host worker events subscriber ");
    let mut sub = sub
        .subscribe(WORKER_BROADCAST_TOPIC)
        .expect("Unable to subscribe to topic");
    loop {
        select! {
            _ = hup_signal.recv() => {
                info!("Received HUP signal, reloading configuration is not supported yet");
                break;
            },
            _ = stop_signal.recv() => {
                info!("Received STOP signal, shutting down...");
                kill_switch.store(true, std::sync::atomic::Ordering::Relaxed);
                break;
            },
            event = workers_events_recv(&mut sub) => {
                if let Ok(event) = event {
                    let addr = args.client_args.workers_response_address.clone();
                    let ctx = zmq_ctx.clone();
                    let worker_token = worker_token.clone();
                    tokio::spawn(process(event, ctx, addr, my_id, worker_token, kill_switch.clone()));
                }
            }
        }
    }
    info!("Done");
    Ok(())
}

async fn process(
    event: DaemonToWorkerMessage,
    zmq_ctx: tmq::Context,
    rpc_address: String,
    my_id: Uuid,
    worker_token: WorkerToken,
    kill_switch: Arc<AtomicBool>,
) {
    let rpc_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(&rpc_address)
        .expect("Unable to bind RPC server for connection");
    let mut rpc_client = WorkerRpcSendClient::new(rpc_request_sock);

    match event {
        DaemonToWorkerMessage::PingWorker(_, worker_id) => {
            if worker_id == my_id {
                info!("Received ping from daemon");
                rpc_client
                    .make_worker_rpc_call(
                        &worker_token,
                        my_id,
                        WorkerToDaemonMessage::Pong(worker_token.clone()),
                    )
                    .await
                    .expect("Unable to send pong to daemon");
            }
        }
        DaemonToWorkerMessage::WorkerRequest {
            worker_id,
            token: _,
            id: request_id,
            perms: _,
            request,
        } => {
            if worker_id != my_id {
                return;
            }

            // Make an outbound HTTP request w/ request
            let result = perform_http_request(request).await;
            match result {
                Ok(r) => {
                    rpc_client
                        .make_worker_rpc_call(
                            &worker_token,
                            my_id,
                            WorkerToDaemonMessage::RequestResult(
                                worker_token.clone(),
                                request_id,
                                r,
                            ),
                        )
                        .await
                        .expect("Unable to send response to daemon");
                }
                Err(e) => {
                    info!("Error performing request: {}", e);
                    rpc_client
                        .make_worker_rpc_call(
                            &worker_token,
                            my_id,
                            WorkerToDaemonMessage::RequestError(
                                worker_token.clone(),
                                request_id,
                                e,
                            ),
                        )
                        .await
                        .expect("Unable to send error response to daemon");
                }
            }
        }
        DaemonToWorkerMessage::PleaseDie(token, _) => {
            if token == worker_token {
                info!("Received please die from daemon");
                kill_switch.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}

async fn perform_http_request(arguments: Vec<Var>) -> Result<Vec<Var>, WorkerError> {
    if arguments.len() < 2 {
        return Err(WorkerError::RequestError(
            "At least two arguments are required".to_string(),
        ));
    }
    // args: method (symbol or string), URL, and then headers then optionally body.
    let client = reqwest::Client::new();
    let method = arguments[0].as_symbol().map_err(|_| {
        WorkerError::RequestError("First argument must be a symbol or string".to_string())
    })?;

    let Variant::Str(url) = arguments[1].variant() else {
        return Err(WorkerError::RequestError(
            "Second argument must be a string".to_string(),
        ));
    };

    let Ok(url) = Url::parse(url.as_str()) else {
        return Err(WorkerError::RequestError("Invalid URL".to_string()));
    };

    let headers = if arguments.len() > 3 {
        // List of String, String
        let Variant::List(headers) = arguments[3].variant() else {
            return Err(WorkerError::RequestError(
                "Headers must be a list".to_string(),
            ));
        };

        let mut headers_map = reqwest::header::HeaderMap::new();
        for header_pair in headers.iter() {
            let Variant::List(pair) = header_pair.variant() else {
                return Err(WorkerError::RequestError(
                    "Header pair must be a list".to_string(),
                ));
            };

            if pair.len() != 2 {
                return Err(WorkerError::RequestError(
                    "Header pair must have exactly two elements".to_string(),
                ));
            }

            let Variant::Str(key) = pair[0].variant() else {
                return Err(WorkerError::RequestError(
                    "Header key must be a string".to_string(),
                ));
            };

            let Variant::Str(value) = pair[1].variant() else {
                return Err(WorkerError::RequestError(
                    "Header value must be a string".to_string(),
                ));
            };

            let key = reqwest::header::HeaderName::from_str(key.as_str())
                .map_err(|e| WorkerError::RequestError(format!("Invalid header key: {}", e)))?;
            let value = reqwest::header::HeaderValue::from_str(value.as_str())
                .map_err(|e| WorkerError::RequestError(format!("Invalid header value: {}", e)))?;
            headers_map.insert(key, value);
        }
        Some(headers_map)
    } else {
        None
    };

    let body = if arguments.len() > 2 {
        match arguments[2].variant() {
            Variant::Str(body) => Some(body.as_str().to_string()),
            Variant::List(list) => {
                let mut body = String::new();
                for item in list.iter() {
                    match item.variant() {
                        Variant::Str(s) => body.push_str(s.as_str()),
                        _ => {
                            return Err(WorkerError::RequestError(
                                "List items must be strings".to_string(),
                            ));
                        }
                    }
                }
                Some(body)
            }
            _ => {
                return Err(WorkerError::RequestError(
                    "Body must be a string or list".to_string(),
                ));
            }
        }
    } else {
        None
    };

    info!(
        "Performing HTTP request: method={}, url={}, body={:?}, headers={:?}",
        method, url, body, headers
    );
    let response = match method.as_str().to_lowercase().as_str() {
        "get" => {
            let client = client.get(url);
            let client = if let Some(headers) = headers {
                client.headers(headers)
            } else {
                client
            };
            let client = if let Some(body) = body {
                client.body(body)
            } else {
                client
            };
            client.send().await.map_err(|e| {
                WorkerError::RequestError(format!("Failed to send GET request: {}", e))
            })?
        }
        "post" => {
            let client = client.post(url);
            let client = if let Some(headers) = headers {
                client.headers(headers)
            } else {
                client
            };
            let client = if let Some(body) = body {
                client.body(body)
            } else {
                client
            };
            client.send().await.map_err(|e| {
                WorkerError::RequestError(format!("Failed to send POST request: {}", e))
            })?
        }
        "put" => {
            let client = client.put(url);
            let client = if let Some(headers) = headers {
                client.headers(headers)
            } else {
                client
            };
            let client = if let Some(body) = body {
                client.body(body)
            } else {
                client
            };
            client.send().await.map_err(|e| {
                WorkerError::RequestError(format!("Failed to send PUT request: {}", e))
            })?
        }
        _ => {
            return Err(WorkerError::RequestError(format!(
                "Unsupported HTTP method ({})",
                method.as_str()
            )));
        }
    };
    let status = response.status();
    let status_code = status.as_u16();
    let status_code = v_int(status_code as i64);
    let headers = response.headers().clone();
    let headers = headers
        .iter()
        .map(|(k, v)| v_list(&[v_str(k.as_str()), v_str(v.to_str().unwrap_or(""))]));
    let headers = v_list_iter(headers);
    let body = response
        .text()
        .await
        .map_err(|e| WorkerError::RequestError(format!("Failed to read response body: {}", e)))?;

    let body = v_str(body.as_str());

    Ok(vec![status_code, headers, body])
}
