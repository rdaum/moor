// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
use rpc_async_client::worker_loop;
use rpc_common::client_args::RpcClientArgs;
use std::{
    net::SocketAddr,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
};
use tokio::{
    io::AsyncWriteExt,
    net::TcpListener,
    select,
    signal::unix::{SignalKind, signal},
};
use tracing::{debug, error, info};
use uuid::Uuid;

// TODO: timeouts, and generally more error handling
use once_cell::sync::Lazy;

static VERSION_STRING: Lazy<String> = Lazy::new(|| {
    format!(
        "{} (commit: {})",
        env!("CARGO_PKG_VERSION"),
        moor_common::build::short_commit()
    )
});

#[derive(Parser, Debug)]
#[command(version = VERSION_STRING.as_str())]
struct Args {
    #[command(flatten)]
    client_args: RpcClientArgs,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,

    #[arg(
        long,
        value_name = "health-check-port",
        help = "Port for health check endpoint (responds with OK)",
        default_value = "9999"
    )]
    health_check_port: u16,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install()?;
    let args: Args = Args::parse();

    moor_common::tracing::init_tracing(args.debug).expect("Unable to configure logging");

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

    // Setup CURVE encryption if using TCP endpoint
    let curve_keys = match rpc_async_client::enrollment_client::setup_curve_auth(
        &args.client_args.rpc_address,
        &args.client_args.enrollment_address,
        args.client_args.enrollment_token_file.as_deref(),
        "curl-worker",
        &args.client_args.data_dir,
    ) {
        Ok(keys) => keys,
        Err(e) => {
            error!("Failed to setup CURVE authentication: {}", e);
            std::process::exit(1);
        }
    };

    // Generate a worker ID (or use enrolled UUID if we have one)
    let my_id = uuid::Uuid::new_v4();

    // Create atomic for tracking daemon pings (for health checks)
    let last_daemon_ping = Arc::new(AtomicU64::new(0));

    // Start health check server
    let health_check_addr = format!("0.0.0.0:{}", args.health_check_port);
    info!("Starting health check endpoint on {}", health_check_addr);
    let health_kill_switch = kill_switch.clone();
    let health_ping_tracker = last_daemon_ping.clone();
    tokio::spawn(async move {
        let health_sockaddr = match health_check_addr.parse::<SocketAddr>() {
            Ok(addr) => addr,
            Err(e) => {
                error!(
                    "Failed to parse health check address {}: {}",
                    health_check_addr, e
                );
                return;
            }
        };

        let listener = match TcpListener::bind(health_sockaddr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Unable to bind health check listener: {}", e);
                return;
            }
        };

        loop {
            if health_kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            match listener.accept().await {
                Ok((mut socket, addr)) => {
                    debug!("Health check probe from {}", addr);

                    // Check if we've received a daemon ping recently
                    let last_ping = health_ping_tracker.load(std::sync::atomic::Ordering::Relaxed);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    // Report healthy if: no ping yet (last_ping == 0, still starting up) OR ping within last 30s
                    let response: &[u8] = if last_ping == 0 || now - last_ping < 30 {
                        b"OK\n"
                    } else {
                        b"UNHEALTHY\n"
                    };

                    let _ = socket.write_all(response).await;
                }
                Err(e) => {
                    debug!("Health check accept error: {}", e);
                }
            }
        }
    });

    let worker_response_rpc_addr = args.client_args.workers_response_address.clone();
    let worker_request_rpc_addr = args.client_args.workers_request_address.clone();
    let worker_type = Symbol::mk("curl");
    let ks = kill_switch.clone();
    let perform_func = Arc::new(perform_http_request);
    let worker_loop_thread = tokio::spawn(async move {
        if let Err(e) = worker_loop(
            &ks,
            my_id,
            &worker_response_rpc_addr,
            &worker_request_rpc_addr,
            worker_type,
            perform_func,
            curve_keys,
            Some(last_daemon_ping),
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
    _request_id: Uuid,
    _worker_type: Symbol,
    _perms: Obj,
    arguments: Vec<Var>,
    timeout: Option<std::time::Duration>,
) -> Result<Var, WorkerError> {
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
                WorkerError::RequestError(format!("Failed to build client with timeout: {e}"))
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
                .map_err(|e| WorkerError::RequestError(format!("Invalid header key: {e}")))?;
            let value = reqwest::header::HeaderValue::from_str(value)
                .map_err(|e| WorkerError::RequestError(format!("Invalid header value: {e}")))?;
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
        method = method.as_arc_str().as_str(),
        url = url.as_str(),
        "HTTP request"
    );
    let response = match method.as_arc_str().to_lowercase().as_str() {
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
                WorkerError::RequestError(format!("Failed to send GET request: {e}"))
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
                WorkerError::RequestError(format!("Failed to send POST request: {e}"))
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
                WorkerError::RequestError(format!("Failed to send PUT request: {e}"))
            })?
        }
        _ => {
            return Err(WorkerError::RequestError(format!(
                "Unsupported HTTP method ({})",
                method.as_arc_str()
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
        .map_err(|e| WorkerError::RequestError(format!("Failed to read response body: {e}")))?;

    let body = v_str(body.as_str());

    Ok(v_list(&[status_code, headers, body]))
}
