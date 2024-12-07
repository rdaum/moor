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

#![allow(clippy::too_many_arguments)]

use crate::listen::Listeners;
use clap::Parser;
use clap_derive::Parser;
use moor_values::SYSTEM_OBJECT;
use rpc_async_client::{make_host_token, proces_hosts_events, start_host_session};
use rpc_common::client_args::RpcClientArgs;
use rpc_common::{load_keypair, HostType};
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;

mod connection;
mod listen;

#[derive(Parser, Debug)]
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
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install()?;
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

    let mut hup_signal =
        signal(SignalKind::hangup()).expect("Unable to register HUP signal handler");
    let mut stop_signal =
        signal(SignalKind::interrupt()).expect("Unable to register STOP signal handler");

    let kill_switch = Arc::new(AtomicBool::new(false));

    // Parse the telnet address and port.
    let listen_addr = format!("{}:{}", args.telnet_address, args.telnet_port);
    let telnet_sockaddr = listen_addr.parse::<SocketAddr>().unwrap();

    let zmq_ctx = tmq::Context::new();

    let (mut listeners_server, listeners_channel, listeners) = Listeners::new(
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        args.client_args.events_address.clone(),
        kill_switch.clone(),
    );
    let listeners_thread = tokio::spawn(async move {
        listeners_server.run(listeners_channel).await;
    });

    listeners
        .add_listener(&SYSTEM_OBJECT, telnet_sockaddr)
        .await
        .expect("Unable to start default listener");

    let keypair = load_keypair(&args.client_args.public_key, &args.client_args.private_key)
        .expect("Unable to load keypair from public and private key files");
    let host_token = make_host_token(&keypair, HostType::TCP);

    let rpc_client = start_host_session(
        host_token.clone(),
        zmq_ctx.clone(),
        args.client_args.rpc_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
    )
    .await
    .expect("Unable to establish initial host session");

    let host_listen_loop = proces_hosts_events(
        rpc_client,
        host_token,
        zmq_ctx.clone(),
        args.client_args.events_address.clone(),
        args.telnet_address.clone(),
        kill_switch.clone(),
        listeners.clone(),
        HostType::TCP,
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
