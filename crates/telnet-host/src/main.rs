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
    sync::{Arc, atomic::AtomicBool},
};
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
};
use tracing::{error, info};

mod connection;
mod connection_codec;
mod listen;

#[derive(Parser, Debug, Serialize, Deserialize)]
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

    // Check if we need CURVE encryption (only for TCP endpoints, not IPC)
    let use_curve = args.client_args.rpc_address.starts_with("tcp://");

    // Enroll with daemon and load CURVE keys only if using TCP
    let curve_keys = if use_curve {
        info!("TCP endpoint detected - enrolling with daemon and loading CURVE keys");

        let enrollment_token = std::env::var("MOOR_ENROLLMENT_TOKEN").ok();
        let (daemon_public_key, _service_uuid) =
            match rpc_async_client::enrollment_client::ensure_enrolled(
                &args.client_args.enrollment_address,
                enrollment_token.as_deref(),
                args.client_args.enrollment_token_file.as_deref(),
                "telnet-host",
                &args.client_args.data_dir,
            ) {
                Ok(enrollment) => enrollment,
                Err(e) => {
                    error!("Failed to enroll with daemon: {}", e);
                    std::process::exit(1);
                }
            };

        let keypair = match rpc_async_client::curve_keys::load_or_generate_keypair(
            &args.client_args.data_dir,
            "telnet-host",
        ) {
            Ok(keypair) => keypair,
            Err(e) => {
                error!("Unable to load CURVE keypair: {}", e);
                std::process::exit(1);
            }
        };

        // Create curve_keys tuple for per-connection RPC sockets
        Some((keypair.secret, keypair.public, daemon_public_key))
    } else {
        info!("IPC endpoint detected - CURVE encryption disabled");
        None
    };

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

    let (rpc_client, host_id) = match start_host_session(
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
