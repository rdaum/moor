# MOO LSP Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extend mcp-host with LSP support for AI-assisted MOO development

**Architecture:** TCP-based LSP server sharing mooR connection with existing MCP server. Parses `.moo` files, maps them to database objects via cowbell conventions, provides symbols/diagnostics/completion.

**Tech Stack:** tower-lsp, lsp-types, tokio, moor-compiler (objdef parsing)

---

## Task 1: Add LSP Dependencies

**Files:**
- Modify: `Cargo.toml` (workspace)
- Modify: `crates/mcp-host/Cargo.toml`

**Step 1: Add lsp-types and tower-lsp to workspace**

Add to workspace `Cargo.toml` under `[workspace.dependencies]`:

```toml
lsp-types = "0.97"
tower-lsp = "0.20"
```

**Step 2: Add dependencies to mcp-host**

Add to `crates/mcp-host/Cargo.toml`:

```toml
lsp-types.workspace = true
tower-lsp.workspace = true
```

**Step 3: Verify build**

Run: `cargo build -p moor-mcp-host`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add Cargo.toml crates/mcp-host/Cargo.toml
git commit -m "$(cat <<'EOF'
deps: add tower-lsp and lsp-types for LSP support

Adds Language Server Protocol dependencies to mcp-host for
future LSP server implementation.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Add CLI Arguments for LSP

**Files:**
- Modify: `crates/mcp-host/src/main.rs`

**Step 1: Add --lsp-port and --lsp-workspace arguments**

In the `Args` struct, add after the existing fields:

```rust
    /// TCP port for LSP server (enables LSP mode when set)
    #[arg(long)]
    lsp_port: Option<u16>,

    /// Workspace directory for LSP file scanning (required if --lsp-port is set)
    #[arg(long)]
    lsp_workspace: Option<PathBuf>,
```

Add `use std::path::PathBuf;` at the top.

**Step 2: Add validation in main()**

After `args` is extracted, add validation:

```rust
    // Validate LSP arguments
    if args.lsp_port.is_some() && args.lsp_workspace.is_none() {
        eprintln!("Error: --lsp-workspace is required when --lsp-port is specified");
        std::process::exit(1);
    }
    if let Some(ref workspace) = args.lsp_workspace {
        if !workspace.exists() {
            eprintln!("Error: LSP workspace directory does not exist: {}", workspace.display());
            std::process::exit(1);
        }
    }
```

**Step 3: Add logging for LSP config**

After the existing info! logs, add:

```rust
    if let Some(port) = args.lsp_port {
        info!("LSP port: {}", port);
        info!("LSP workspace: {}", args.lsp_workspace.as_ref().unwrap().display());
    }
```

**Step 4: Verify build and --help**

Run: `cargo build -p moor-mcp-host`
Run: `cargo run -p moor-mcp-host -- --help`
Expected: Shows --lsp-port and --lsp-workspace options

**Step 5: Commit**

```bash
git add crates/mcp-host/src/main.rs
git commit -m "$(cat <<'EOF'
feat(mcp-host): add --lsp-port and --lsp-workspace CLI args

Adds command-line arguments to enable LSP server mode:
- --lsp-port: TCP port for LSP connections
- --lsp-workspace: Directory to scan for .moo files

Validates that workspace exists and is required when port is set.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Create LSP Module Structure

**Files:**
- Create: `crates/mcp-host/src/lsp/mod.rs`
- Create: `crates/mcp-host/src/lsp/server.rs`
- Create: `crates/mcp-host/src/lsp/state.rs`
- Modify: `crates/mcp-host/src/main.rs`

**Step 1: Create lsp/mod.rs**

```rust
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

//! LSP server implementation for MOO language support.

mod server;
mod state;

pub use server::LspServer;
pub use state::LspConfig;
```

**Step 2: Create lsp/state.rs**

```rust
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
    pub connections: Arc<ConnectionManager>,
}

impl LspState {
    pub fn new(config: LspConfig, connections: Arc<ConnectionManager>) -> Self {
        Self { config, connections }
    }
}
```

**Step 3: Create lsp/server.rs (skeleton)**

```rust
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

use crate::lsp::state::{LspConfig, LspState};
use crate::connection::ConnectionManager;

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
```

**Step 4: Add lsp module to main.rs**

Add after the other module declarations:

```rust
mod lsp;
```

**Step 5: Verify build**

Run: `cargo build -p moor-mcp-host`
Expected: Build succeeds

**Step 6: Commit**

```bash
git add crates/mcp-host/src/lsp/
git add crates/mcp-host/src/main.rs
git commit -m "$(cat <<'EOF'
feat(mcp-host): add LSP module structure

Creates lsp/ module with:
- mod.rs: Module exports
- state.rs: LspConfig and LspState types
- server.rs: LspServer skeleton with TCP listener

The server currently accepts connections but doesn't implement
the protocol yet.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Update ConnectionManager for Sharing

**Files:**
- Modify: `crates/mcp-host/src/main.rs`
- Modify: `crates/mcp-host/src/mcp_server.rs`

**Step 1: Wrap ConnectionManager in Arc**

In `main.rs`, change the connection manager creation and usage:

```rust
use std::sync::Arc;

// ... in main()

    // Create the connection manager (wrapped in Arc for sharing)
    let connection_config = ConnectionConfig {
        client_config,
        programmer_credentials,
        wizard_credentials,
    };
    let connections = Arc::new(ConnectionManager::new(connection_config));

    // Create MCP server with Arc clone
    let mut server = McpServer::new(Arc::clone(&connections));
```

**Step 2: Update McpServer to accept Arc<ConnectionManager>**

In `mcp_server.rs`, update the struct and constructor:

```rust
use std::sync::Arc;

pub struct McpServer {
    connections: Arc<ConnectionManager>,
    // ... other fields
}

impl McpServer {
    pub fn new(connections: Arc<ConnectionManager>) -> Self {
        Self {
            connections,
            // ... other fields
        }
    }
}
```

Update all usages of `self.connections` to work with Arc (should be transparent for method calls).

**Step 3: Verify build and tests**

Run: `cargo build -p moor-mcp-host`
Run: `cargo test -p moor-mcp-host`
Expected: Both succeed

**Step 4: Commit**

```bash
git add crates/mcp-host/src/main.rs crates/mcp-host/src/mcp_server.rs
git commit -m "$(cat <<'EOF'
refactor(mcp-host): wrap ConnectionManager in Arc for sharing

Allows both MCP and LSP servers to share the same connection manager
for communicating with the mooR daemon.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Start LSP Server from main()

**Files:**
- Modify: `crates/mcp-host/src/main.rs`

**Step 1: Add LSP server spawning**

In `main()`, after creating the MCP server but before running it:

```rust
    // Start LSP server if configured
    let lsp_handle = if let (Some(port), Some(workspace)) = (args.lsp_port, args.lsp_workspace) {
        let lsp_config = lsp::LspConfig { port, workspace };
        let lsp_server = lsp::LspServer::new(lsp_config, Arc::clone(&connections));
        Some(tokio::spawn(async move {
            if let Err(e) = lsp_server.run().await {
                tracing::error!("LSP server error: {}", e);
            }
        }))
    } else {
        None
    };
```

**Step 2: Handle LSP server shutdown**

After `server.run_stdio().await?;`, add:

```rust
    // Stop LSP server if running
    if let Some(handle) = lsp_handle {
        handle.abort();
    }
```

**Step 3: Verify build**

Run: `cargo build -p moor-mcp-host`
Expected: Build succeeds

**Step 4: Test LSP server starts**

Run (in separate terminal): `cargo run -p moor-mcp-host -- --rpc-address tcp://localhost:7899 --lsp-port 8888 --lsp-workspace /tmp`
Expected: Logs show "LSP server listening on 127.0.0.1:8888"

Test connection: `nc localhost 8888` (should connect and immediately close with warning)

**Step 5: Commit**

```bash
git add crates/mcp-host/src/main.rs
git commit -m "$(cat <<'EOF'
feat(mcp-host): spawn LSP server when --lsp-port is set

Starts the LSP TCP listener in a background task when configured.
The MCP and LSP servers run concurrently, sharing the connection
manager.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Implement LSP Protocol Handler with tower-lsp

**Files:**
- Modify: `crates/mcp-host/src/lsp/server.rs`
- Create: `crates/mcp-host/src/lsp/backend.rs`
- Modify: `crates/mcp-host/src/lsp/mod.rs`

**Step 1: Create backend.rs with LanguageServer implementation**

```rust
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

//! LSP backend implementing the LanguageServer trait.

use std::sync::Arc;

use lsp_types::{
    InitializeParams, InitializeResult, InitializedParams, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind,
};
use tower_lsp::jsonrpc::Result;
use tower_lsp::{Client, LanguageServer};

use crate::lsp::state::LspState;

/// LSP backend that handles protocol messages.
pub struct MooLanguageServer {
    client: Client,
    state: Arc<LspState>,
}

impl MooLanguageServer {
    pub fn new(client: Client, state: Arc<LspState>) -> Self {
        Self { client, state }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for MooLanguageServer {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                // More capabilities will be added in future tasks
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(lsp_types::MessageType::INFO, "MOO LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
```

**Step 2: Update server.rs to use tower-lsp**

Replace the contents of server.rs:

```rust
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
use std::sync::atomic::{AtomicBool, Ordering};

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
    pub fn new(config: LspConfig, connections: Arc<ConnectionManager>) -> Self {
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
                warn!("Rejecting connection from {}: another client is active", client_addr);
                drop(stream);
                continue;
            }

            info!("LSP client connected from {}", client_addr);

            let state = Arc::clone(&self.state);
            let client_active = Arc::clone(&self.client_active);

            let (read, write) = tokio::io::split(stream);

            let (service, socket) = LspService::new(|client| {
                MooLanguageServer::new(client, state)
            });

            // Run the LSP server for this client
            Server::new(read, write, socket).serve(service).await;

            // Mark client as disconnected
            client_active.store(false, Ordering::SeqCst);
            info!("LSP client from {} disconnected", client_addr);
        }
    }
}
```

**Step 3: Update mod.rs to export backend**

```rust
mod backend;
mod server;
mod state;

pub use server::LspServer;
pub use state::LspConfig;
```

**Step 4: Verify build**

Run: `cargo build -p moor-mcp-host`
Expected: Build succeeds

**Step 5: Test with LSP client**

Start server: `cargo run -p moor-mcp-host -- --rpc-address tcp://localhost:7899 --lsp-port 8888 --lsp-workspace /tmp`

Test with a simple LSP initialize request via netcat or a test script.

**Step 6: Commit**

```bash
git add crates/mcp-host/src/lsp/
git commit -m "$(cat <<'EOF'
feat(mcp-host): implement LSP protocol handler with tower-lsp

Adds MooLanguageServer backend implementing tower-lsp LanguageServer trait:
- initialize: Returns basic server capabilities
- initialized: Logs startup message
- shutdown: Clean shutdown handling

Server enforces single-client connections using atomic flag.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Add Document Symbol Support

**Files:**
- Create: `crates/mcp-host/src/lsp/symbols.rs`
- Modify: `crates/mcp-host/src/lsp/backend.rs`
- Modify: `crates/mcp-host/src/lsp/mod.rs`

**Step 1: Create symbols.rs for parsing and symbol extraction**

```rust
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

//! Symbol extraction from MOO source files.

use lsp_types::{DocumentSymbol, Position, Range, SymbolKind};
use moor_compiler::{compile_object_definitions, ObjFileContext};

/// Extract document symbols from MOO source code.
pub fn extract_symbols(source: &str) -> Vec<DocumentSymbol> {
    let context = ObjFileContext::default();

    match compile_object_definitions(source, context) {
        Ok(definitions) => {
            definitions
                .into_iter()
                .map(|def| {
                    let name = def.name.clone();

                    // Collect verb symbols as children
                    let verb_children: Vec<DocumentSymbol> = def.verbs.iter().map(|verb| {
                        DocumentSymbol {
                            name: verb.name.clone(),
                            detail: Some(format!("{:?}", verb.argspec)),
                            kind: SymbolKind::METHOD,
                            tags: None,
                            deprecated: None,
                            range: Range::default(), // TODO: Add span tracking
                            selection_range: Range::default(),
                            children: None,
                        }
                    }).collect();

                    // Collect property symbols as children
                    let prop_children: Vec<DocumentSymbol> = def.props.iter().map(|prop| {
                        DocumentSymbol {
                            name: prop.name.clone(),
                            detail: None,
                            kind: SymbolKind::FIELD,
                            tags: None,
                            deprecated: None,
                            range: Range::default(),
                            selection_range: Range::default(),
                            children: None,
                        }
                    }).collect();

                    let mut children = verb_children;
                    children.extend(prop_children);

                    DocumentSymbol {
                        name,
                        detail: def.parent.map(|p| format!("parent: {}", p)),
                        kind: SymbolKind::CLASS,
                        tags: None,
                        deprecated: None,
                        range: Range::default(),
                        selection_range: Range::default(),
                        children: if children.is_empty() { None } else { Some(children) },
                    }
                })
                .collect()
        }
        Err(_) => Vec::new(), // Return empty on parse error (diagnostics will show error)
    }
}
```

**Step 2: Update backend.rs to implement document_symbol**

Add to imports:

```rust
use lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse,
    // ... existing imports
};
```

Add to ServerCapabilities in initialize:

```rust
document_symbol_provider: Some(lsp_types::OneOf::Left(true)),
```

Add method to LanguageServer impl:

```rust
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        // Read file content
        let path = uri.to_file_path().map_err(|_| {
            tower_lsp::jsonrpc::Error::invalid_params("Invalid file URI")
        })?;

        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            tower_lsp::jsonrpc::Error::internal_error()
        })?;

        let symbols = crate::lsp::symbols::extract_symbols(&content);
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }
```

**Step 3: Update mod.rs**

```rust
mod backend;
mod server;
mod state;
mod symbols;

pub use server::LspServer;
pub use state::LspConfig;
```

**Step 4: Verify build**

Run: `cargo build -p moor-mcp-host`
Expected: Build succeeds

**Step 5: Test with a .moo file**

Create test file and verify symbols are extracted correctly.

**Step 6: Commit**

```bash
git add crates/mcp-host/src/lsp/
git commit -m "$(cat <<'EOF'
feat(mcp-host): add document symbol support for MOO files

Implements textDocument/documentSymbol for MOO language:
- Objects mapped to Class symbols
- Verbs mapped to Method symbols
- Properties mapped to Field symbols

Uses moor_compiler::compile_object_definitions for parsing.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Add Diagnostics for Parse Errors

**Files:**
- Create: `crates/mcp-host/src/lsp/diagnostics.rs`
- Modify: `crates/mcp-host/src/lsp/backend.rs`
- Modify: `crates/mcp-host/src/lsp/mod.rs`

**Step 1: Create diagnostics.rs**

```rust
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

//! Diagnostics generation from MOO compilation errors.

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use moor_common::model::CompileError;

/// Convert a byte offset to LSP Position.
fn byte_to_position(source: &str, byte_offset: usize) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;

    for (i, c) in source.char_indices() {
        if i >= byte_offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    Position { line, character: col }
}

/// Convert CompileError to LSP Diagnostic.
pub fn compile_error_to_diagnostic(source: &str, error: &CompileError) -> Diagnostic {
    let (start, end) = error.details.span;

    let range = Range {
        start: byte_to_position(source, start),
        end: byte_to_position(source, end),
    };

    let mut message = error.message.clone();
    if !error.details.expected_tokens.is_empty() {
        message.push_str(&format!(
            "\nExpected: {}",
            error.details.expected_tokens.join(", ")
        ));
    }

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("moo".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Parse source and return diagnostics for any errors.
pub fn get_diagnostics(source: &str) -> Vec<Diagnostic> {
    use moor_compiler::{compile_object_definitions, ObjFileContext};

    let context = ObjFileContext::default();

    match compile_object_definitions(source, context) {
        Ok(_) => Vec::new(),
        Err(errors) => {
            errors
                .iter()
                .map(|e| compile_error_to_diagnostic(source, e))
                .collect()
        }
    }
}
```

**Step 2: Update backend.rs to publish diagnostics on file open/change**

Add to imports:

```rust
use lsp_types::{
    DidOpenTextDocumentParams, DidChangeTextDocumentParams, PublishDiagnosticsParams,
    // ... existing imports
};
use std::collections::HashMap;
use tokio::sync::RwLock;
```

Add document storage to MooLanguageServer:

```rust
pub struct MooLanguageServer {
    client: Client,
    state: Arc<LspState>,
    documents: Arc<RwLock<HashMap<lsp_types::Url, String>>>,
}

impl MooLanguageServer {
    pub fn new(client: Client, state: Arc<LspState>) -> Self {
        Self {
            client,
            state,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn publish_diagnostics(&self, uri: lsp_types::Url, content: &str) {
        let diagnostics = crate::lsp::diagnostics::get_diagnostics(content);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}
```

Add handlers:

```rust
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;

        self.documents.write().await.insert(uri.clone(), content.clone());
        self.publish_diagnostics(uri, &content).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            let content = change.text;
            self.documents.write().await.insert(uri.clone(), content.clone());
            self.publish_diagnostics(uri, &content).await;
        }
    }
```

**Step 3: Update mod.rs**

```rust
mod backend;
mod diagnostics;
mod server;
mod state;
mod symbols;

pub use server::LspServer;
pub use state::LspConfig;
```

**Step 4: Verify build**

Run: `cargo build -p moor-mcp-host`
Expected: Build succeeds

**Step 5: Test diagnostics**

Open a .moo file with syntax errors, verify diagnostics are published.

**Step 6: Commit**

```bash
git add crates/mcp-host/src/lsp/
git commit -m "$(cat <<'EOF'
feat(mcp-host): add diagnostic publishing for MOO parse errors

Implements textDocument/didOpen and didChange handlers that:
- Store document content for subsequent operations
- Parse MOO source using moor_compiler
- Convert CompileError to LSP Diagnostic with proper positions
- Push diagnostics to client via publishDiagnostics

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: Add Workspace File Scanning

**Files:**
- Create: `crates/mcp-host/src/lsp/workspace.rs`
- Modify: `crates/mcp-host/src/lsp/backend.rs`
- Modify: `crates/mcp-host/src/lsp/mod.rs`

**Step 1: Create workspace.rs**

```rust
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

//! Workspace scanning and file-to-object mapping.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::debug;

/// Scan workspace for .moo files.
pub async fn scan_workspace(workspace: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    scan_directory(workspace, &mut files).await;
    files
}

async fn scan_directory(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(mut entries) = fs::read_dir(dir).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_dir() {
            Box::pin(scan_directory(&path, files)).await;
        } else if path.extension().map_or(false, |ext| ext == "moo") {
            debug!("Found MOO file: {}", path.display());
            files.push(path);
        }
    }
}

/// File-to-object mapping information.
#[derive(Debug, Clone)]
pub struct FileMapping {
    /// Path to the .moo file.
    pub file_path: PathBuf,
    /// Object name declared in file (from `object NAME` line).
    pub object_name: Option<String>,
    /// import_export_id if declared.
    pub import_export_id: Option<String>,
    /// import_export_hierarchy if declared.
    pub import_export_hierarchy: Option<Vec<String>>,
}

/// Parse a file to extract mapping information.
pub async fn extract_mapping(file_path: &Path) -> Option<FileMapping> {
    let content = fs::read_to_string(file_path).await.ok()?;

    use moor_compiler::{compile_object_definitions, ObjFileContext};
    let context = ObjFileContext::default();

    let definitions = compile_object_definitions(&content, context).ok()?;
    let def = definitions.into_iter().next()?;

    Some(FileMapping {
        file_path: file_path.to_path_buf(),
        object_name: Some(def.name),
        import_export_id: None, // TODO: Extract from properties
        import_export_hierarchy: None,
    })
}
```

**Step 2: Update backend.rs to scan workspace on initialized**

Update initialized handler:

```rust
    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(lsp_types::MessageType::INFO, "MOO LSP server initialized")
            .await;

        // Scan workspace for .moo files
        let files = crate::lsp::workspace::scan_workspace(&self.state.config.workspace).await;

        self.client
            .log_message(
                lsp_types::MessageType::INFO,
                format!("Found {} .moo files in workspace", files.len()),
            )
            .await;

        // Parse each file and publish initial diagnostics
        for file in files {
            if let Ok(content) = tokio::fs::read_to_string(&file).await {
                let uri = lsp_types::Url::from_file_path(&file).unwrap();
                self.documents.write().await.insert(uri.clone(), content.clone());
                self.publish_diagnostics(uri, &content).await;
            }
        }
    }
```

**Step 3: Update mod.rs**

```rust
mod backend;
mod diagnostics;
mod server;
mod state;
mod symbols;
mod workspace;

pub use server::LspServer;
pub use state::LspConfig;
```

**Step 4: Verify build**

Run: `cargo build -p moor-mcp-host`
Expected: Build succeeds

**Step 5: Test workspace scanning**

Point --lsp-workspace at cowbell/src and verify files are discovered.

**Step 6: Commit**

```bash
git add crates/mcp-host/src/lsp/
git commit -m "$(cat <<'EOF'
feat(mcp-host): add workspace scanning for .moo files

On LSP initialized:
- Recursively scans --lsp-workspace for .moo files
- Parses each file to extract object definitions
- Publishes initial diagnostics for all files

Prepares for file-to-object mapping implementation.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Add workspace/symbol Support

**Files:**
- Modify: `crates/mcp-host/src/lsp/backend.rs`
- Modify: `crates/mcp-host/src/lsp/symbols.rs`

**Step 1: Add workspace symbol extraction to symbols.rs**

```rust
use lsp_types::{SymbolInformation, Location};

/// Extract workspace symbols from all cached documents.
pub fn extract_workspace_symbols(
    documents: &HashMap<lsp_types::Url, String>,
    query: &str,
) -> Vec<SymbolInformation> {
    let query_lower = query.to_lowercase();
    let mut symbols = Vec::new();

    for (uri, content) in documents {
        let doc_symbols = extract_symbols(content);
        flatten_symbols(&doc_symbols, uri, &query_lower, &mut symbols);
    }

    symbols
}

fn flatten_symbols(
    doc_symbols: &[DocumentSymbol],
    uri: &lsp_types::Url,
    query: &str,
    out: &mut Vec<SymbolInformation>,
) {
    for sym in doc_symbols {
        if query.is_empty() || sym.name.to_lowercase().contains(query) {
            #[allow(deprecated)]
            out.push(SymbolInformation {
                name: sym.name.clone(),
                kind: sym.kind,
                tags: sym.tags.clone(),
                deprecated: sym.deprecated,
                location: Location {
                    uri: uri.clone(),
                    range: sym.range,
                },
                container_name: None,
            });
        }

        if let Some(children) = &sym.children {
            flatten_symbols(children, uri, query, out);
        }
    }
}
```

**Step 2: Update backend.rs to implement symbol**

Add to ServerCapabilities in initialize:

```rust
workspace_symbol_provider: Some(lsp_types::OneOf::Left(true)),
```

Add handler:

```rust
    async fn symbol(
        &self,
        params: lsp_types::WorkspaceSymbolParams,
    ) -> Result<Option<Vec<lsp_types::SymbolInformation>>> {
        let documents = self.documents.read().await;
        let symbols = crate::lsp::symbols::extract_workspace_symbols(&documents, &params.query);
        Ok(Some(symbols))
    }
```

**Step 3: Verify build**

Run: `cargo build -p moor-mcp-host`
Expected: Build succeeds

**Step 4: Test workspace symbols**

Verify searching for symbols across workspace returns results.

**Step 5: Commit**

```bash
git add crates/mcp-host/src/lsp/
git commit -m "$(cat <<'EOF'
feat(mcp-host): add workspace/symbol support

Implements workspace-wide symbol search:
- Flattens document symbols into SymbolInformation
- Filters by query string (case-insensitive)
- Returns matching symbols with file locations

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Future Tasks (Phase 2)

The following tasks build on the foundation above:

### Task 11: Add Hover Support
- Show object/verb/property documentation on hover
- Query mooR database for runtime information

### Task 12: Add Go-to-Definition
- Resolve `$name:verb` references to source locations
- Follow property object references

### Task 13: Add Completion Support
- `$` prefix → object names from #0
- `$obj:` → verbs on object
- `$obj.` → properties on object
- Builtin function suggestions

### Task 14: Add File-Object Mapping with constants.moo
- Parse constants.moo for symbolic names
- Extract import_export_id from object properties
- Build bidirectional mapping

### Task 15: Add Drift Detection
- Compare file content with DB state
- Emit warning diagnostics for drifted files
- Provide moo/resolveConflict custom method

---

## Verification Checkpoint

After completing Tasks 1-10, verify:

```bash
# Build succeeds
cargo build -p moor-mcp-host

# Help shows new options
cargo run -p moor-mcp-host -- --help | grep -E "lsp-(port|workspace)"

# LSP server starts (requires mooR daemon running)
cargo run -p moor-mcp-host -- \
    --rpc-address tcp://localhost:7899 \
    --username programmer --password secret \
    --lsp-port 8888 --lsp-workspace /path/to/cowbell/src
```

Test with Serena or a minimal LSP client:
1. Initialize handshake completes
2. documentSymbol returns objects/verbs/properties
3. Parse errors show as diagnostics
4. workspace/symbol searches across files
