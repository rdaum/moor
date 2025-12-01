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

//! mooR MCP Host
//!
//! Model Context Protocol server that enables AI assistants to interact with
//! mooR MOO virtual worlds. Run as an MCP server to allow Claude and other
//! AI assistants to explore, query, and modify a running MOO.
//!
//! # Usage
//!
//! ```bash
//! moor-mcp-host --rpc-address tcp://localhost:7899 --events-address tcp://localhost:7898
//! ```
//!
//! Or with authentication:
//!
//! ```bash
//! moor-mcp-host --rpc-address tcp://localhost:7899 --username wizard --password secret
//! ```

mod mcp_server;
mod mcp_types;
mod moor_client;
mod prompts;
mod resources;
mod tools;

use clap::Parser;
use clap_derive::Parser;
use eyre::Result;
use figment::{
    Figment,
    providers::{Format, Serialized, Yaml},
};
use mcp_server::McpServer;
use moor_client::{MoorClient, MoorClientConfig};
use rpc_common::client_args::RpcClientArgs;
use serde_derive::{Deserialize, Serialize};
use tracing::{info, warn};

/// mooR MCP Host - AI assistant interface for MOO virtual worlds
#[derive(Parser, Debug, Serialize, Deserialize)]
#[command(name = "moor-mcp-host")]
#[command(about = "Model Context Protocol server for mooR MOO virtual worlds")]
#[command(version)]
struct Args {
    #[command(flatten)]
    client_args: RpcClientArgs,

    /// Username to authenticate with (optional - can also be done via MCP)
    #[arg(long)]
    username: Option<String>,

    /// Password to authenticate with (optional - can also be done via MCP)
    #[arg(long)]
    password: Option<String>,

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
    let curve_keys = match setup_curve_auth(&args.client_args).await {
        Ok(keys) => keys,
        Err(e) => {
            warn!(
                "Failed to setup CURVE auth (will try without encryption): {}",
                e
            );
            None
        }
    };

    // Create the mooR client
    let config = MoorClientConfig {
        rpc_address: args.client_args.rpc_address.clone(),
        events_address: args.client_args.events_address.clone(),
        curve_keys,
    };

    let client = MoorClient::new(config)?;

    // Create MCP server
    let mut server = McpServer::new(client);

    // If credentials were provided, set them for use after connection
    if let (Some(username), Some(password)) = (&args.username, &args.password) {
        info!("Credentials configured for {}", username);
        server.set_credentials(username.clone(), password.clone());
    }

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

/// Setup CURVE authentication
async fn setup_curve_auth(args: &RpcClientArgs) -> Result<Option<(String, String, String)>> {
    // Check if we need CURVE auth (TCP endpoints typically do)
    if !args.rpc_address.starts_with("tcp://") {
        return Ok(None);
    }

    // Try to use the enrollment client to get keys
    match rpc_async_client::enrollment_client::setup_curve_auth(
        &args.rpc_address,
        &args.enrollment_address,
        args.enrollment_token_file.as_deref(),
        "mcp-host",
        &args.data_dir,
    ) {
        Ok(keys) => Ok(keys),
        Err(e) => {
            warn!("CURVE auth setup failed: {}", e);
            Ok(None)
        }
    }
}
