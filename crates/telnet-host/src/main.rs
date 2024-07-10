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

use std::net::SocketAddr;

use clap::Parser;
use clap_derive::Parser;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;

mod telnet;

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        value_name = "telnet-address",
        help = "Telnet server listen address",
        default_value = "0.0.0.0:8080"
    )]
    telnet_address: String,

    #[arg(
        long,
        value_name = "rpc-address",
        help = "RPC socket address",
        default_value = "ipc:///tmp/moor_rpc.sock"
    )]
    rpc_address: String,

    #[arg(
        long,
        value_name = "events-address",
        help = "Events socket address",
        default_value = "ipc:///tmp/moor_events.sock"
    )]
    events_address: String,

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

    let telnet_sockaddr = args.telnet_address.parse::<SocketAddr>().unwrap();
    let listen_loop = telnet::telnet_listen_loop(
        telnet_sockaddr,
        args.rpc_address.as_str(),
        args.events_address.as_str(),
    );

    info!("Host started, listening @ {}...", args.telnet_address);
    select! {
        msg = listen_loop => {
            msg?;
            info!("ZMQ client loop exited, stopping...");
        }
        _ = hup_signal.recv() => {
            info!("HUP received, stopping...");
        },
        _ = stop_signal.recv() => {
            info!("STOP received, stopping...");
        }
    }
    info!("Done.");

    Ok(())
}
