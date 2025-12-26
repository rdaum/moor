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

//! TCP-based LSP server with single-client enforcement.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use eyre::Result;
use tokio::net::TcpListener;
use tower_lsp::{LspService, Server};
use tracing::{info, warn};

use crate::connection::ConnectionManager;
use crate::lsp::backend::MooLanguageServer;
use crate::lsp::state::{LspConfig, LspState};

/// LSP server that listens on TCP and handles one client at a time.
pub struct LspServer {
    state: Arc<LspState>,
    client_active: Arc<AtomicBool>,
}

impl LspServer {
    pub fn new(config: LspConfig, connections: Arc<tokio::sync::Mutex<ConnectionManager>>) -> Self {
        let state = Arc::new(LspState::new(config, connections));
        Self {
            state,
            client_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Run the LSP server, accepting one client at a time.
    pub async fn run(&self) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.state.config.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("LSP server listening on {}", addr);

        loop {
            let (stream, client_addr) = listener.accept().await?;

            // Single-client enforcement
            if self.client_active.swap(true, Ordering::SeqCst) {
                warn!(
                    "Rejecting connection from {}: another client is active",
                    client_addr
                );
                drop(stream);
                continue;
            }

            info!("LSP client connected from {}", client_addr);

            let state = Arc::clone(&self.state);
            let client_active = Arc::clone(&self.client_active);

            let (read, write) = tokio::io::split(stream);

            let (service, socket) = LspService::new(|client| MooLanguageServer::new(client, state));

            // Run the LSP server for this client
            Server::new(read, write, socket).serve(service).await;

            // Mark client as disconnected
            client_active.store(false, Ordering::SeqCst);
            info!("LSP client from {} disconnected", client_addr);
        }
    }
}
