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

//! LSP server state and configuration.

use std::path::PathBuf;
use std::sync::Arc;

use crate::connection::ConnectionManager;

/// Configuration for the LSP server.
#[derive(Debug, Clone)]
pub struct LspConfig {
    /// TCP port to listen on.
    pub port: u16,
    /// Workspace directory containing .moo files.
    pub workspace: PathBuf,
}

/// Shared state for the LSP server.
pub struct LspState {
    /// Configuration.
    pub config: LspConfig,
    /// Connection manager for mooR RPC.
    pub connections: Arc<tokio::sync::Mutex<ConnectionManager>>,
}

impl LspState {
    pub fn new(config: LspConfig, connections: Arc<tokio::sync::Mutex<ConnectionManager>>) -> Self {
        Self { config, connections }
    }
}
