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

#![allow(clippy::too_many_arguments)]

use crate::listen::Listeners;
use clap::Parser;
use clap_derive::Parser;
use colored::control;
use figment::{
    Figment,
    providers::{Format, Serialized, Yaml},
};
use moor_var::SYSTEM_OBJECT;
use rpc_async_client::{process_hosts_events, start_host_session};
use rpc_common::{HostType, client_args::RpcClientArgs};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
};
use tokio::{
    net::TcpListener,
    select,
    signal::unix::{SignalKind, signal},
};
use tracing::{debug, error, info};
use uuid::Uuid;

mod connection;
mod connection_codec;
mod djot_formatter;
mod listen;
mod moo_highlighter;

use once_cell::sync::Lazy;

static VERSION_STRING: Lazy<String> = Lazy::new(|| {
    format!(
        "{} (commit: {})",
        env!("CARGO_PKG_VERSION"),
        moor_common::build::short_commit()
    )
});

#[derive(Parser, Debug, Serialize, Deserialize)]
#[command(version = VERSION_STRING.as_str())]
struct Args {
    #[command(flatten)]
    client_args: RpcClientArgs,

    #[arg(
        long,
        value_name = "telnet-address",
        help = "Listen address for the default telnet connections listener",
        default_value = "0.0.0.0"
    )]
    telnet_address: String,

    #[arg(
        long,
        value_name = "telnet-port",
        help = "Listen port for the default telnet connections listener",
        default_value = "8888"
    )]
    telnet_port: u16,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    debug: bool,

    #[arg(long, help = "Yaml config file to use, overrides values in CLI args")]
    config_file: Option<String>,

    #[arg(
        long,
        value_name = "health-check-port",
        help = "Port for HTTP-style health check endpoint (responds with OK)",
        default_value = "9888"
    )]
    health_check_port: u16,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install()?;
    let cli_args = Args::parse();
    let config_file = cli_args.config_file.clone();
    let mut args_figment = Figment::new().merge(Serialized::defaults(cli_args));
    if let Some(config_file) = config_file {
        args_figment = args_figment.merge(Yaml::file(config_file));
    }
    let args = args_figment.extract::<Args>().unwrap();

    moor_common::tracing::init_tracing(args.debug).unwrap_or_else(|e| {
        eprintln!("Unable to configure logging: {e}");
        std::process::exit(1);
    });
    control::set_override(true);

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

    // Parse the telnet address and port.
    let listen_addr = format!("{}:{}", args.telnet_address, args.telnet_port);
    let telnet_sockaddr = match listen_addr.parse::<SocketAddr>() {
        Ok(addr) => addr,
        Err(e) => {
            error!(
                "Failed to parse telnet socket address {}: {}",
                listen_addr, e
            );
            std::process::exit(1);
        }
    };

    let zmq_ctx = tmq::Context::new();

    // Setup CURVE encryption if using TCP endpoint
    let curve_keys = match rpc_async_client::enrollment_client::setup_curve_auth(
        &args.client_args.rpc_address,
        &args.client_args.enrollment_address,
        args.client_args.enrollment_token_file.as_deref(),
        "telnet-host",
        &args.client_args.data_dir,
    ) {
        Ok(keys) => keys,
        Err(e) => {
            error!("Failed to setup CURVE authentication: {}", e);
            std::process::exit(1);
        }
    };

    let host_id = Uuid::new_v4();
    let last_daemon_ping = Arc::new(AtomicU64::new(0));

    let (mut listeners_server, listeners_channel, listeners) = Listeners::new(
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        args.client_args.events_address.clone(),
        kill_switch.clone(),
        curve_keys.clone(),
    );

    let listeners_thread = tokio::spawn(async move {
        listeners_server.run(listeners_channel).await;
    });

    listeners
        .add_listener(&SYSTEM_OBJECT, telnet_sockaddr)
        .await
        .unwrap_or_else(|e| {
            error!("Unable to start default listener: {}", e);
            std::process::exit(1);
        });

    // Start health check server
    let health_check_addr = format!("{}:{}", args.telnet_address, args.health_check_port);
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

                    let _ = tokio::io::AsyncWriteExt::write_all(&mut socket, response).await;
                }
                Err(e) => {
                    debug!("Health check accept error: {}", e);
                }
            }
        }
    });

    info!("Starting host session...");

    let (rpc_client, host_id) = match start_host_session(
        host_id,
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
        curve_keys.clone(),
    )
    .await
    {
        Ok((client, id)) => (client, id),
        Err(e) => {
            error!("Unable to establish initial host session: {}", e);
            std::process::exit(1);
        }
    };

    let host_listen_loop = process_hosts_events(
        rpc_client,
        host_id,
        zmq_ctx.clone(),
        args.client_args.events_address.clone(),
        args.telnet_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
        HostType::TCP,
        curve_keys,
        Some(last_daemon_ping),
    );
    select! {
        _ = host_listen_loop => {
            info!("Host events loop exited.");
        },
        _ = listeners_thread => {
            info!("Listener set exited.");
        }
        _ = hup_signal.recv() => {
            info!("HUP received, stopping...");
            kill_switch.store(true, std::sync::atomic::Ordering::SeqCst);
        },
        _ = stop_signal.recv() => {
            info!("STOP received, stopping...");
            kill_switch.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    info!("Done.");

    Ok(())
}
