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
//! Provides document symbols, diagnostics, and workspace scanning.
//!
//! # Current Features (Offline Mode)
//!
//! - Document symbols (objects, verbs, properties)
//! - Parse error diagnostics
//! - Workspace scanning for .moo files
//!
//! # Usage
//!
//! TCP mode (for IDE integration):
//! ```bash
//! moor-lsp --port 8888 --workspace /path/to/moo/files
//! ```
//!
//! Stdio mode (for editor plugins):
//! ```bash
//! moor-lsp --stdio --workspace /path/to/moo/files
//! ```
//!
//! # Future: Live Server Mode
//!
//! Future versions will support connecting to a running mooR server for:
//! - Sysprop resolution ($name lookups)
//! - Live object/verb/property validation
//! - Code completion from live database
//!
//! This will use the mooR RPC interface (ZMQ-based, not telnet).

mod backend;
mod diagnostics;
mod symbols;
mod workspace;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use clap_derive::Parser;
use eyre::Result;
use tokio::net::TcpListener;
use tower_lsp::{LspService, Server};
use tracing::{info, warn};

use backend::MooLanguageServer;

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

    let workspace = args.workspace.unwrap_or_else(|| PathBuf::from("."));

    if let Some(port) = args.port {
        // TCP mode
        run_tcp_server(port, workspace).await
    } else {
        // Stdio mode
        run_stdio_server(workspace).await
    }
}

/// Run LSP server over stdio
async fn run_stdio_server(workspace: PathBuf) -> Result<()> {
    info!("Starting MOO LSP server on stdio");
    info!("Workspace: {}", workspace.display());

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| MooLanguageServer::new(client, workspace));

    Server::new(stdin, stdout, socket).serve(service).await;

    Ok(())
}

/// Run LSP server over TCP, accepting one client at a time
async fn run_tcp_server(port: u16, workspace: PathBuf) -> Result<()> {
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
        let client_active_clone = Arc::clone(&client_active);

        let (read, write) = tokio::io::split(stream);

        let (service, socket) =
            LspService::new(|client| MooLanguageServer::new(client, workspace));

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
