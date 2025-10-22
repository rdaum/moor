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

    #[arg(
        long,
        value_name = "enrollment-address",
        help = "Enrollment server address for host registration",
        default_value = "tcp://localhost:7900"
    )]
    pub enrollment_address: String,

    #[arg(
        long,
        value_name = "data-dir",
        help = "Directory for host identity and CURVE keys",
        default_value = "./.moor-host-data"
    )]
    pub data_dir: PathBuf,

    #[arg(
        long,
        value_name = "enrollment-token-file",
        help = "Path to enrollment token file (if not specified, checks MOOR_ENROLLMENT_TOKEN env var)"
    )]
    pub enrollment_token_file: Option<PathBuf>,
}
