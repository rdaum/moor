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

//! mooR LSP Server
//!
//! Language Server Protocol implementation for MOO language support.
//! Connects to a running mooR server for live features.
//!
//! # Features
//!
//! - Document symbols (objects, verbs, properties)
//! - Parse error diagnostics
//! - Workspace scanning for .moo files
//! - Server-connected operations (via RPC)
//!
//! # Usage
//!
//! TCP mode (for IDE integration):
//! ```bash
//! moor-lsp --port 8888 --workspace /path/to/moo/files \
//!     --rpc-address tcp://127.0.0.1:7899 \
//!     --username wizard --password wizard
//! ```
//!
//! Stdio mode (for editor plugins):
//! ```bash
//! moor-lsp --stdio --workspace /path/to/moo/files \
//!     --rpc-address tcp://127.0.0.1:7899 \
//!     --username wizard --password wizard
//! ```
//!
//! Environment variables can also be used:
//! - MOOR_RPC_ADDRESS
//! - MOOR_USERNAME
//! - MOOR_PASSWORD

mod backend;
mod client;
mod completion;
mod content;
mod definition;
mod diagnostics;
mod hover;
mod objects;
mod parsing;
mod references;
mod symbols;
mod sync;
mod workspace;
mod workspace_index;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;
use clap_derive::Parser;
use eyre::Result;
use tokio::net::TcpListener;
use tower_lsp::{LspService, Server};
use tracing::{info, warn};

use backend::MooLanguageServer;
use client::{MoorClient, MoorClientConfig};

/// mooR LSP Server - Language Server Protocol for MOO
#[derive(Parser, Debug)]
#[command(name = "moor-lsp")]
#[command(about = "Language Server Protocol implementation for MOO language")]
#[command(version)]
struct Args {
    /// TCP port to listen on (default: use stdio)
    #[arg(long)]
    port: Option<u16>,

    /// Use stdio for communication (default if --port not specified)
    #[arg(long)]
    stdio: bool,

    /// Workspace directory containing .moo files
    #[arg(long)]
    workspace: Option<PathBuf>,

    /// Enable debug logging
    #[arg(long, default_value = "false")]
    debug: bool,

    // RPC connection options
    /// mooR daemon RPC address
    #[arg(long, env = "MOOR_RPC_ADDRESS", default_value = "tcp://127.0.0.1:7899")]
    rpc_address: String,

    /// mooR daemon events address (for pub/sub)
    #[arg(
        long,
        env = "MOOR_EVENTS_ADDRESS",
        default_value = "tcp://127.0.0.1:7898"
    )]
    events_address: String,

    /// Enrollment server address for CURVE key exchange
    #[arg(
        long,
        env = "MOOR_ENROLLMENT_ADDRESS",
        default_value = "tcp://localhost:7900"
    )]
    enrollment_address: String,

    /// Directory for storing host identity and CURVE keys
    #[arg(long, default_value = "./.moor-lsp-data")]
    data_dir: PathBuf,

    /// Path to enrollment token file
    #[arg(long, env = "MOOR_ENROLLMENT_TOKEN_FILE")]
    enrollment_token_file: Option<PathBuf>,

    /// Username for mooR authentication
    #[arg(long, env = "MOOR_USERNAME")]
    username: Option<String>,

    /// Password for mooR authentication
    #[arg(long, env = "MOOR_PASSWORD")]
    password: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // Load .env file if present
    if let Err(e) = dotenvy::dotenv() {
        // Not an error if .env doesn't exist
        if !matches!(e, dotenvy::Error::Io(_)) {
            warn!("Error loading .env file: {}", e);
        }
    }

    let args = Args::parse();

    // Setup logging to stderr
    setup_logging(args.debug)?;

    let workspace = args.workspace.clone().unwrap_or_else(|| PathBuf::from("."));

    // Setup CURVE authentication if using TCP endpoints
    let curve_keys = setup_curve_auth(&args);

    // Create mooR client config
    let client_config = MoorClientConfig {
        rpc_address: args.rpc_address.clone(),
        events_address: args.events_address.clone(),
        curve_keys,
    };

    // Connect to mooR daemon if credentials provided
    let moor_client = if let (Some(username), Some(password)) = (&args.username, &args.password) {
        match create_moor_client(&client_config, username, password).await {
            Ok(client) => {
                info!("Connected to mooR daemon at {}", args.rpc_address);
                Some(Arc::new(tokio::sync::RwLock::new(client)))
            }
            Err(e) => {
                warn!(
                    "Failed to connect to mooR daemon: {}. Running in offline mode.",
                    e
                );
                None
            }
        }
    } else {
        info!("No credentials provided. Running in offline mode.");
        None
    };

    if let Some(port) = args.port {
        // TCP mode
        run_tcp_server(port, workspace, moor_client).await
    } else {
        // Stdio mode
        run_stdio_server(workspace, moor_client).await
    }
}

/// Create and connect a mooR client
async fn create_moor_client(
    config: &MoorClientConfig,
    username: &str,
    password: &str,
) -> Result<MoorClient> {
    let mut client = MoorClient::new(config.clone())?;
    client.connect().await?;
    client.login(username, password).await?;
    Ok(client)
}

/// Run LSP server over stdio
async fn run_stdio_server(
    workspace: PathBuf,
    moor_client: Option<Arc<tokio::sync::RwLock<MoorClient>>>,
) -> Result<()> {
    info!("Starting MOO LSP server on stdio");
    info!("Workspace: {}", workspace.display());

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) =
        LspService::new(|client| MooLanguageServer::new(client, workspace, moor_client));

    Server::new(stdin, stdout, socket).serve(service).await;

    Ok(())
}

/// Run LSP server over TCP, accepting one client at a time
async fn run_tcp_server(
    port: u16,
    workspace: PathBuf,
    moor_client: Option<Arc<tokio::sync::RwLock<MoorClient>>>,
) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("MOO LSP server listening on {}", addr);
    info!("Workspace: {}", workspace.display());

    let client_active = Arc::new(AtomicBool::new(false));

    loop {
        let (stream, client_addr) = listener.accept().await?;

        // Single-client enforcement
        if client_active.swap(true, Ordering::SeqCst) {
            warn!(
                "Rejecting connection from {}: another client is active",
                client_addr
            );
            drop(stream);
            continue;
        }

        info!("LSP client connected from {}", client_addr);

        let workspace = workspace.clone();
        let moor_client = moor_client.clone();
        let client_active_clone = Arc::clone(&client_active);

        let (read, write) = tokio::io::split(stream);

        let (service, socket) =
            LspService::new(|client| MooLanguageServer::new(client, workspace, moor_client));

        // Run the LSP server for this client
        Server::new(read, write, socket).serve(service).await;

        // Mark client as disconnected
        client_active_clone.store(false, Ordering::SeqCst);
        info!("LSP client from {} disconnected", client_addr);
    }
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

/// Setup CURVE authentication for secure daemon communication
fn setup_curve_auth(args: &Args) -> Option<(String, String, String)> {
    moor_client::setup_curve_auth(
        &args.rpc_address,
        &args.enrollment_address,
        args.enrollment_token_file.as_deref(),
        "moor-lsp",
        &args.data_dir,
    )
}
