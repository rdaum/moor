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
use moor_var::{Obj, Sequence, Symbol, Var, Variant, v_int, v_list, v_list_iter, v_str};
use reqwest::Url;
use rpc_async_client::{make_worker_token, worker_loop};
use rpc_common::client_args::RpcClientArgs;
use rpc_common::{WorkerToken, load_keypair};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::select;
use tokio::signal::unix::{SignalKind, signal};
use tracing::{error, info};
use tracing_subscriber::fmt::format::FmtSpan;
use uuid::Uuid;

// TODO: timeouts, and generally more error handling
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

    let mut hup_signal = match signal(SignalKind::hangup()) {
        Ok(signal) => signal,
        Err(e) => {
            error!("Unable to register HUP signal handler: {}", e);
            std::process::exit(1);
        }
    };
    let mut stop_signal = match signal(SignalKind::interrupt()) {
        Ok(signal) => signal,
        Err(e) => {
            error!("Unable to register STOP signal handler: {}", e);
            std::process::exit(1);
        }
    };

    let kill_switch = Arc::new(AtomicBool::new(false));

    let (private_key, _public_key) =
        match load_keypair(&args.client_args.public_key, &args.client_args.private_key) {
            Ok(keypair) => keypair,
            Err(e) => {
                error!(
                    "Unable to load keypair from public and private key files: {}",
                    e
                );
                std::process::exit(1);
            }
        };
    let my_id = Uuid::new_v4();
    let worker_token = make_worker_token(&private_key, my_id);

    let worker_response_rpc_addr = args.client_args.workers_response_address.clone();
    let worker_request_rpc_addr = args.client_args.workers_request_address.clone();
    let worker_type = Symbol::mk("curl");
    let ks = kill_switch.clone();
    let perform_func = Arc::new(perform_http_request);
    let worker_loop_thread = tokio::spawn(async move {
        if let Err(e) = worker_loop(
            &ks,
            my_id,
            &worker_token,
            &worker_response_rpc_addr,
            &worker_request_rpc_addr,
            worker_type,
            perform_func,
        )
        .await
        {
            error!("Worker loop for {my_id} exited with error: {}", e);
            ks.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });

    select! {
        _ = hup_signal.recv() => {
            info!("Received HUP signal, reloading configuration is not supported yet");
        },
        _ = stop_signal.recv() => {
            info!("Received STOP signal, shutting down...");
            kill_switch.store(true, std::sync::atomic::Ordering::Relaxed);
        },
        _ = worker_loop_thread => {
            info!("Worker loop thread exited");
        }
    }
    info!("Done");
    Ok(())
}

async fn perform_http_request(
    _token: WorkerToken,
    _request_id: Uuid,
    _worker_type: Symbol,
    _perms: Obj,
    arguments: Vec<Var>,
    timeout: Option<std::time::Duration>,
) -> Result<Vec<Var>, WorkerError> {
    if arguments.len() < 2 {
        return Err(WorkerError::RequestError(
            "At least two arguments are required".to_string(),
        ));
    }
    // args: method (symbol or string), URL, and then headers then optionally body.
    let client = if let Some(timeout) = timeout {
        reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| {
                WorkerError::RequestError(format!("Failed to build client with timeout: {}", e))
            })?
    } else {
        reqwest::Client::new()
    };
    let method = arguments[0].as_symbol().map_err(|_| {
        WorkerError::RequestError("First argument must be a symbol or string".to_string())
    })?;

    let Some(url) = arguments[1].as_string() else {
        return Err(WorkerError::RequestError(
            "Second argument must be a string".to_string(),
        ));
    };

    let Ok(url) = Url::parse(url) else {
        return Err(WorkerError::RequestError("Invalid URL".to_string()));
    };

    let headers = if arguments.len() > 3 {
        // List of String, String
        let Some(headers) = arguments[3].as_list() else {
            return Err(WorkerError::RequestError(
                "Headers must be a list".to_string(),
            ));
        };

        let mut headers_map = reqwest::header::HeaderMap::new();
        for header_pair in headers.iter() {
            let Some(pair) = header_pair.as_list() else {
                return Err(WorkerError::RequestError(
                    "Header pair must be a list".to_string(),
                ));
            };

            if pair.len() != 2 {
                return Err(WorkerError::RequestError(
                    "Header pair must have exactly two elements".to_string(),
                ));
            }

            let Some(key) = pair[0].as_string() else {
                return Err(WorkerError::RequestError(
                    "Header key must be a string".to_string(),
                ));
            };

            let Some(value) = pair[1].as_string() else {
                return Err(WorkerError::RequestError(
                    "Header value must be a string".to_string(),
                ));
            };

            let key = reqwest::header::HeaderName::from_str(key)
                .map_err(|e| WorkerError::RequestError(format!("Invalid header key: {}", e)))?;
            let value = reqwest::header::HeaderValue::from_str(value)
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
