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

use std::sync::Arc;

use eyre::Result;
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::connection::ConnectionManager;
use crate::lsp::state::{LspConfig, LspState};

/// LSP server that listens on TCP and handles one client at a time.
pub struct LspServer {
    state: Arc<LspState>,
}

impl LspServer {
    pub fn new(config: LspConfig, connections: Arc<ConnectionManager>) -> Self {
        let state = Arc::new(LspState::new(config, connections));
        Self { state }
    }

    /// Run the LSP server, accepting one client at a time.
    pub async fn run(&self) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.state.config.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("LSP server listening on {}", addr);

        loop {
            let (stream, client_addr) = listener.accept().await?;
            info!("LSP client connected from {}", client_addr);

            // TODO: Handle client connection with tower-lsp
            // For now, just log and close
            warn!("LSP protocol handling not yet implemented, closing connection");
            drop(stream);
        }
    }
}
