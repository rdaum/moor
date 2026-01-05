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

//! mooR MCP Host
//!
//! Model Context Protocol server that enables AI assistants to interact with
//! mooR MOO virtual worlds. Run as an MCP server to allow Claude and other
//! AI assistants to explore, query, and modify a running MOO.
//!
//! # Usage
//!
//! Basic usage with programmer credentials (recommended for most operations):
//!
//! ```bash
//! moor-mcp-host --rpc-address tcp://localhost:7899 --username programmer --password secret
//! ```
//!
//! With both programmer and wizard credentials (for operations requiring elevated privileges):
//!
//! ```bash
//! moor-mcp-host --rpc-address tcp://localhost:7899 \
//!     --username programmer --password secret \
//!     --wizard-username wizard --wizard-password wizard_secret
//! ```
//!
//! # Connection Model
//!
//! The MCP host supports two connection types:
//! - **Programmer**: Default connection used for most operations. Uses `--username`/`--password`.
//! - **Wizard**: Elevated privileges for operations like dump/load. Uses `--wizard-username`/`--wizard-password`.
//!
//! Connections are established lazily on first use. Most tools default to programmer mode
//! and accept an optional `wizard: true` parameter for elevated access. Some tools (objdef
//! operations) always require wizard privileges.

mod connection;
mod mcp_server;
mod mcp_types;
mod prompts;
mod resources;
mod tools;

use clap::Parser;
use clap_derive::Parser;
use connection::{ConnectionConfig, ConnectionManager, Credentials};
use eyre::Result;
use figment::{
    Figment,
    providers::{Format, Serialized, Yaml},
};
use mcp_server::McpServer;
use moor_client::MoorClientConfig;
use rpc_common::client_args::RpcClientArgs;
use serde_derive::{Deserialize, Serialize};
use tracing::info;

/// mooR MCP Host - AI assistant interface for MOO virtual worlds
#[derive(Parser, Debug, Serialize, Deserialize)]
#[command(name = "moor-mcp-host")]
#[command(about = "Model Context Protocol server for mooR MOO virtual worlds")]
#[command(version)]
struct Args {
    #[command(flatten)]
    client_args: RpcClientArgs,

    /// Username for default (programmer) connection
    #[arg(long)]
    username: Option<String>,

    /// Password for default (programmer) connection
    #[arg(long)]
    password: Option<String>,

    /// Username for wizard connection (elevated privileges)
    #[arg(long)]
    wizard_username: Option<String>,

    /// Password for wizard connection (elevated privileges)
    #[arg(long)]
    wizard_password: Option<String>,

    /// Enable debug logging (logs go to stderr to avoid interfering with MCP)
    #[arg(long, default_value = "false")]
    debug: bool,

    /// YAML config file to use (overrides CLI args)
    #[arg(long)]
    config_file: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // Parse arguments
    let cli_args = Args::parse();
    let config_file = cli_args.config_file.clone();
    let mut args_figment = Figment::new().merge(Serialized::defaults(cli_args));
    if let Some(config_file) = config_file {
        args_figment = args_figment.merge(Yaml::file(config_file));
    }
    let args: Args = args_figment.extract()?;

    // Setup logging to stderr (so it doesn't interfere with MCP on stdout)
    setup_logging(args.debug)?;

    info!("mooR MCP Host starting...");
    info!("RPC address: {}", args.client_args.rpc_address);
    info!("Events address: {}", args.client_args.events_address);

    // Setup CURVE authentication if needed
    let curve_keys = moor_client::setup_curve_auth(
        &args.client_args.rpc_address,
        &args.client_args.enrollment_address,
        args.client_args.enrollment_token_file.as_deref(),
        "mcp-host",
        &args.client_args.data_dir,
    );

    // Create the client config
    let client_config = MoorClientConfig {
        rpc_address: args.client_args.rpc_address.clone(),
        events_address: args.client_args.events_address.clone(),
        curve_keys,
    };

    // Build credentials from args
    let programmer_credentials = match (&args.username, &args.password) {
        (Some(username), Some(password)) => {
            info!("Programmer credentials configured for {}", username);
            Some(Credentials {
                username: username.clone(),
                password: password.clone(),
            })
        }
        _ => None,
    };

    let wizard_credentials = match (&args.wizard_username, &args.wizard_password) {
        (Some(username), Some(password)) => {
            info!("Wizard credentials configured for {}", username);
            Some(Credentials {
                username: username.clone(),
                password: password.clone(),
            })
        }
        _ => None,
    };

    // Create the connection manager
    let connection_config = ConnectionConfig {
        client_config,
        programmer_credentials,
        wizard_credentials,
    };
    let connections = ConnectionManager::new(connection_config);

    // Create MCP server
    let mut server = McpServer::new(connections);

    // Run the MCP server
    info!("MCP server ready, listening on stdio");
    server.run_stdio().await?;

    info!("MCP server shutting down");
    Ok(())
}

/// Setup logging to stderr
fn setup_logging(debug: bool) -> Result<()> {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(filter)
        .init();

    Ok(())
}

