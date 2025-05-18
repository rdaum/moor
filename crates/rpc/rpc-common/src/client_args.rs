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

use clap_derive::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Common command line / config arguments for hosts / clients
#[derive(Clone, Parser, Debug, Serialize, Deserialize)] // requires `derive` feature
pub struct RpcClientArgs {
    #[arg(
        long,
        value_name = "rpc-address",
        help = "RPC ZMQ req-reply socket address",
        default_value = "ipc:///tmp/moor_rpc.sock"
    )]
    pub rpc_address: String,

    #[arg(
        long,
        value_name = "events-address",
        help = "Events ZMQ pub-sub address",
        default_value = "ipc:///tmp/moor_events.sock"
    )]
    pub events_address: String,

    #[arg(
        long,
        value_name = "public_key",
        help = "file containing the PEM encoded public key (shared with the daemon), used for authenticating client & host connections",
        default_value = "moor-verifying-key.pem"
    )]
    pub public_key: PathBuf,

    #[arg(
        long,
        value_name = "private_key",
        help = "file containing an openssh generated ed25519 format private key (shared with the daemon), used for authenticating client & host connections",
        default_value = "moor-signing-key.pem"
    )]
    pub private_key: PathBuf,

    #[arg(
        long,
        value_name = "workers-dispatch-address",
        help = "Workers server ZMQ pub-sub address for receiving dispatch requests",
        default_value = "ipc:///tmp/moor_workers_response.sock"
    )]
    pub workers_response_address: String,

    #[arg(
        long,
        value_name = "workers-address",
        help = "Workers server ZMQ RPC for sending dispatch responses",
        default_value = "ipc:///tmp/moor_workers_request.sock"
    )]
    pub workers_request_address: String,
}
