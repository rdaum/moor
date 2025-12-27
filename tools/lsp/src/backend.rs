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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability, InitializeParams,
    InitializeResult, InitializedParams, MessageType, OneOf, ServerCapabilities, SymbolInformation,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url, WorkspaceSymbolParams,
};
use tower_lsp::{Client, LanguageServer};

use crate::client::MoorClient;
use crate::completion;
use crate::definition;
use crate::diagnostics;
use crate::hover;
use crate::objects::ObjectNameRegistry;
use crate::symbols;
use crate::workspace;
use crate::workspace_index::WorkspaceIndex;

/// LSP backend that handles protocol messages.
pub struct MooLanguageServer {
    client: Client,
    workspace: PathBuf,
    /// In-memory document storage for open files.
    documents: Arc<RwLock<HashMap<Url, String>>>,
    /// Optional mooR daemon client for server-connected features.
    moor_client: Option<Arc<RwLock<MoorClient>>>,
    /// Object name registry ($name → Obj mapping from #0 properties).
    object_names: Arc<RwLock<ObjectNameRegistry>>,
    /// Workspace-wide symbol index for workspace/symbol requests.
    workspace_index: Arc<RwLock<WorkspaceIndex>>,
}

impl MooLanguageServer {
    pub fn new(
        client: Client,
        workspace: PathBuf,
        moor_client: Option<Arc<RwLock<MoorClient>>>,
    ) -> Self {
        Self {
            client,
            workspace,
            documents: Arc::new(RwLock::new(HashMap::new())),
            moor_client,
            object_names: Arc::new(RwLock::new(ObjectNameRegistry::new())),
            workspace_index: Arc::new(RwLock::new(WorkspaceIndex::new())),
        }
    }

    /// Load object names from the mooR server if connected.
    async fn load_object_names(&self) {
        let Some(moor_client) = &self.moor_client else {
            return;
        };

        match ObjectNameRegistry::load_from_server(moor_client).await {
            Ok(registry) => {
                let count = registry.len();
                *self.object_names.write().await = registry;
                self.client
                    .log_message(
                        MessageType::INFO,
                        format!("Loaded {} object names from #0", count),
                    )
                    .await;
            }
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("Failed to load object names from server: {}", e),
                    )
                    .await;
            }
        }
    }

    /// Resolve a symbolic name (without $) to an object ID.
    #[allow(dead_code)]
    pub async fn resolve_object_name(&self, name: &str) -> Option<moor_var::Obj> {
        self.object_names.read().await.resolve(name)
    }

    /// Publish diagnostics for a document.
    async fn publish_diagnostics(&self, uri: Url, content: &str) {
        let diags = diagnostics::get_diagnostics(content);
        self.client.publish_diagnostics(uri, diags, None).await;
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
                completion_provider: Some(CompletionOptions::default()),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "MOO LSP server initialized")
            .await;

        // Load object names from server if connected
        self.load_object_names().await;

        // Scan workspace for .moo files and publish initial diagnostics
        let files = workspace::scan_workspace(&self.workspace).await;

        self.client
            .log_message(
                MessageType::INFO,
                format!("Found {} .moo files in workspace", files.len()),
            )
            .await;

        // Parse each file and publish initial diagnostics
        for file in files {
            let Ok(content) = tokio::fs::read_to_string(&file).await else {
                continue;
            };
            let Ok(uri) = Url::from_file_path(&file) else {
                continue;
            };
            self.documents
                .write()
                .await
                .insert(uri.clone(), content.clone());

            // Index the file for workspace symbol search
            self.workspace_index
                .write()
                .await
                .index_file(file.clone(), &content);

            self.publish_diagnostics(uri, &content).await;
        }

        // Log index stats
        let index = self.workspace_index.read().await;
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "Indexed {} symbols across {} files",
                    index.symbol_count(),
                    index.file_count()
                ),
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;

        self.documents
            .write()
            .await
            .insert(uri.clone(), content.clone());

        // Update workspace index
        if let Ok(path) = uri.to_file_path() {
            self.workspace_index
                .write()
                .await
                .index_file(path, &content);
        }

        self.publish_diagnostics(uri, &content).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            let content = change.text;
            self.documents
                .write()
                .await
                .insert(uri.clone(), content.clone());

            // Update workspace index
            if let Ok(path) = uri.to_file_path() {
                self.workspace_index
                    .write()
                    .await
                    .index_file(path, &content);
            }

            self.publish_diagnostics(uri, &content).await;
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        // Try to get content from our document store first
        let content = {
            let docs = self.documents.read().await;
            docs.get(&uri).cloned()
        };

        let content = match content {
            Some(c) => c,
            None => {
                // Fall back to reading from disk
                let path = uri
                    .to_file_path()
                    .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid file URI"))?;

                tokio::fs::read_to_string(&path)
                    .await
                    .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?
            }
        };

        let symbols = symbols::extract_symbols(&content);
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let index = self.workspace_index.read().await;
        Ok(Some(index.search(&params.query)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Try to get content from our document store first
        let content = {
            let docs = self.documents.read().await;
            docs.get(&uri).cloned()
        };

        let content = match content {
            Some(c) => c,
            None => {
                // Fall back to reading from disk
                let Ok(path) = uri.to_file_path() else {
                    return Ok(None);
                };
                match tokio::fs::read_to_string(&path).await {
                    Ok(c) => c,
                    Err(_) => return Ok(None),
                }
            }
        };

        Ok(hover::get_hover(&content, position))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Try to get content from our document store first
        let content = {
            let docs = self.documents.read().await;
            docs.get(&uri).cloned()
        };

        let content = match content {
            Some(c) => c,
            None => {
                // Fall back to reading from disk
                let Ok(path) = uri.to_file_path() else {
                    return Ok(None);
                };
                match tokio::fs::read_to_string(&path).await {
                    Ok(c) => c,
                    Err(_) => return Ok(None),
                }
            }
        };

        // Get the object names and workspace index
        let object_names = self.object_names.read().await;
        let workspace_index = self.workspace_index.read().await;

        // Find the definition
        let location =
            definition::find_definition(&content, position, &workspace_index, &object_names);

        Ok(location.map(GotoDefinitionResponse::Scalar))
    }

    async fn completion(&self, _params: CompletionParams) -> Result<Option<CompletionResponse>> {
        // For now, return all builtin completions regardless of context.
        // Future enhancements could filter based on cursor position and context.
        let items = completion::get_builtin_completions();
        Ok(Some(CompletionResponse::Array(items)))
    }
}
