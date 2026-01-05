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
    CodeAction, CodeActionKind, CodeActionOptions, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, CodeActionResponse, Command, CompletionOptions, CompletionParams,
    CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFormattingParams, DocumentSymbolParams,
    DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents,
    HoverParams, HoverProviderCapability, InitializeParams, InitializeResult, InitializedParams,
    Location, MarkupContent, MarkupKind, MessageType, OneOf, Position, PrepareRenameResponse,
    Range, ReferenceParams, RenameParams, ServerCapabilities, SymbolInformation,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url, WorkspaceEdit,
    WorkspaceSymbolParams,
};
use tower_lsp::{Client, LanguageServer};

use crate::client::MoorClient;
use crate::completion;
use crate::content::ContentAccessor;
use crate::definition;
use crate::diagnostics;
use crate::hover;
use crate::objects::ObjectNameRegistry;
use crate::references;
use crate::symbols;
use crate::sync;
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
    /// Content accessor for fetching from multiple sources.
    content_accessor: ContentAccessor,
}

impl MooLanguageServer {
    pub fn new(
        client: Client,
        workspace: PathBuf,
        moor_client: Option<Arc<RwLock<MoorClient>>>,
    ) -> Self {
        let documents = Arc::new(RwLock::new(HashMap::new()));
        let content_accessor =
            ContentAccessor::new(Arc::clone(&documents), moor_client.clone());

        Self {
            client,
            workspace,
            documents,
            moor_client,
            object_names: Arc::new(RwLock::new(ObjectNameRegistry::new())),
            workspace_index: Arc::new(RwLock::new(WorkspaceIndex::new())),
            content_accessor,
        }
    }

    /// Get content for a URI from any supported source.
    ///
    /// Sources checked in order:
    /// 1. Open documents (in-memory)
    /// 2. Filesystem (file:// scheme)
    /// 3. HTTP/HTTPS URLs
    /// 4. mooR server (moor:// scheme)
    async fn get_content(&self, uri: &Url) -> Option<String> {
        self.content_accessor.get_content(uri).await.ok()
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
        use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

        // Get compilation diagnostics using the shared context with constants
        let context = self.workspace_index.read().await.context().clone();
        let mut diags = diagnostics::get_diagnostics_with_context(content, &context);

        // If connected to server, also check for sync differences
        if let Some(moor_client) = &self.moor_client {
            let mut client = moor_client.write().await;
            let sync_infos = sync::check_sync_status(content, &mut client, &context).await;

            for info in sync_infos {
                let diagnostic = Diagnostic {
                    range: Range {
                        start: Position {
                            line: info.start_line,
                            character: 0,
                        },
                        end: Position {
                            line: info.end_line,
                            character: 0,
                        },
                    },
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: None,
                    code_description: None,
                    source: Some("moor-sync".to_string()),
                    message: format!(
                        "Object '{}' differs from database: {}",
                        info.obj_name, info.summary
                    ),
                    related_information: None,
                    tags: None,
                    data: None,
                };
                diags.push(diagnostic);
            }
        }

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
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ":".to_string(), // $obj:verb
                        ".".to_string(), // $obj.prop
                        "$".to_string(), // $name
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::SOURCE,
                            CodeActionKind::QUICKFIX,
                        ]),
                        ..Default::default()
                    },
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
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

        // Look for constants.moo and load it first
        // This populates the context with symbolic constants used by other files
        let constants_file = files.iter().find(|f| {
            f.file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("constants.moo"))
        });

        if let Some(constants_path) = constants_file
            && let Ok(content) = tokio::fs::read_to_string(constants_path).await
        {
            self.workspace_index
                .write()
                .await
                .load_constants(&content);
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("Loaded constants from {}", constants_path.display()),
                )
                .await;
        }

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

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        // Remove from document store to free memory
        self.documents.write().await.remove(&uri);

        // Note: We intentionally keep the file in the workspace index since
        // it's still part of the workspace (just not open in the editor).
        // This allows workspace symbol search to still find symbols in closed files.

        tracing::debug!("Document closed: {}", uri);
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

        // Get the line at the cursor position
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;
        let line = lines.get(line_idx).copied().unwrap_or("");

        // Check if we're hovering over a builtin function call
        if let Some(symbol) = references::symbol_at_position(&content, position.line, position.character)
            && symbol.kind == references::ReferenceKind::Builtin
            && let Some(hover_text) = hover::get_builtin_hover(&symbol.name)
        {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_text,
                }),
                range: Some(Range {
                    start: Position {
                        line: symbol.line,
                        character: symbol.start_col,
                    },
                    end: Position {
                        line: symbol.line,
                        character: symbol.end_col,
                    },
                }),
            }));
        }

        // If connected to a server, try server-based hover first for $obj:verb or $obj.prop
        if let Some(moor_client) = &self.moor_client {
            let object_names = self.object_names.read().await;
            let mut client = moor_client.write().await;

            if let Some(hover) =
                hover::get_hover_from_server(line, position, &mut client, &object_names).await
            {
                return Ok(Some(hover));
            }
        }

        // Fall back to file-based hover
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

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        // Get the document content
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

        // Find the symbol at the cursor position
        let Some(symbol) =
            references::symbol_at_position(&content, position.line, position.character)
        else {
            return Ok(None);
        };

        // Find all references to this symbol across the workspace
        let mut all_locations = Vec::new();

        // First, search in the current file
        let refs_in_current = references::find_references_to(&content, &symbol.name);
        for reference in refs_in_current {
            // Optionally filter by kind to match the same kind of reference
            all_locations.push(references::reference_to_location(&reference, &uri));
        }

        // Then search across all indexed files in the workspace
        let workspace_index = self.workspace_index.read().await;
        let documents = self.documents.read().await;

        // Get all files from the workspace index
        for (file_path, _symbols) in workspace_index.files() {
            // Skip the current file (already searched)
            let file_uri = match Url::from_file_path(file_path) {
                Ok(u) => u,
                Err(_) => continue,
            };
            if file_uri == uri {
                continue;
            }

            // Get the file content
            let file_content = if let Some(c) = documents.get(&file_uri) {
                c.clone()
            } else {
                match tokio::fs::read_to_string(file_path).await {
                    Ok(c) => c,
                    Err(_) => continue,
                }
            };

            // Find references in this file
            let refs = references::find_references_to(&file_content, &symbol.name);
            for reference in refs {
                all_locations.push(references::reference_to_location(&reference, &file_uri));
            }
        }

        if all_locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(all_locations))
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        // Try to get the current line content
        let line = {
            let docs = self.documents.read().await;
            if let Some(content) = docs.get(&uri) {
                let lines: Vec<&str> = content.lines().collect();
                lines.get(position.line as usize).map(|s| s.to_string())
            } else {
                None
            }
        };

        // If we have a line, check for completion context
        if let Some(line) = line {
            let context = completion::parse_completion_context(&line, position.character);

            match context {
                completion::CompletionContext::VerbCompletion {
                    object_name,
                    partial,
                } => {
                    if let Some(moor_client) = &self.moor_client {
                        let object_names = self.object_names.read().await;
                        if let Some(obj) = object_names.resolve(&object_name) {
                            let mut client = moor_client.write().await;
                            let items =
                                completion::get_verb_completions(&mut client, obj, &partial).await;
                            if !items.is_empty() {
                                return Ok(Some(CompletionResponse::Array(items)));
                            }
                        }
                    }
                }
                completion::CompletionContext::PropertyCompletion {
                    object_name,
                    partial,
                } => {
                    if let Some(moor_client) = &self.moor_client {
                        let object_names = self.object_names.read().await;
                        if let Some(obj) = object_names.resolve(&object_name) {
                            let mut client = moor_client.write().await;
                            let items =
                                completion::get_property_completions(&mut client, obj, &partial)
                                    .await;
                            if !items.is_empty() {
                                return Ok(Some(CompletionResponse::Array(items)));
                            }
                        }
                    }
                }
                completion::CompletionContext::None => {}
            }
        }

        // Fall back to builtin completions
        let items = completion::get_builtin_completions();
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        // Only provide sync actions if connected to a mooR server
        let Some(moor_client) = &self.moor_client else {
            return Ok(None);
        };

        let uri = params.text_document.uri.clone();

        // Get the document content
        let content = {
            let docs = self.documents.read().await;
            docs.get(&uri).cloned()
        };

        let Some(content) = content else {
            return Ok(None);
        };

        // Get context with constants for parsing
        let context = self.workspace_index.read().await.context().clone();

        // Parse the file to get object definitions
        let Some(object_defs) = sync::parse_object_definitions_with_context(&content, &context)
        else {
            return Ok(None);
        };

        // For now, provide sync actions for each object in the file
        let mut actions = Vec::new();

        // Acquire the lock once outside the loop to reduce contention
        let mut client = moor_client.write().await;

        for obj_def in &object_defs {
            // Try to resolve the object in the database
            let obj_id = obj_def.oid;

            // Compare with database
            let diff = sync::compare_object(obj_def, &mut client, obj_id).await;

            if diff.has_differences() {
                // Add "Upload to database" action
                let upload_action = CodeAction {
                    title: format!("Upload {} to database ({})", obj_def.name, diff.summary()),
                    kind: Some(CodeActionKind::SOURCE),
                    diagnostics: None,
                    edit: None,
                    command: Some(Command {
                        title: "Upload to database".to_string(),
                        command: "moor.uploadToDatabase".to_string(),
                        arguments: Some(vec![
                            serde_json::to_value(uri.to_string()).unwrap(),
                            serde_json::to_value(obj_id.id().0).unwrap(),
                        ]),
                    }),
                    is_preferred: None,
                    disabled: None,
                    data: None,
                };
                actions.push(CodeActionOrCommand::CodeAction(upload_action));

                // Add "Download from database" action
                let download_action = CodeAction {
                    title: format!("Download {} from database", obj_def.name),
                    kind: Some(CodeActionKind::SOURCE),
                    diagnostics: None,
                    edit: None,
                    command: Some(Command {
                        title: "Download from database".to_string(),
                        command: "moor.downloadFromDatabase".to_string(),
                        arguments: Some(vec![
                            serde_json::to_value(uri.to_string()).unwrap(),
                            serde_json::to_value(obj_id.id().0).unwrap(),
                        ]),
                    }),
                    is_preferred: None,
                    disabled: None,
                    data: None,
                };
                actions.push(CodeActionOrCommand::CodeAction(download_action));

                // Add "Show diff" action
                let show_diff_action = CodeAction {
                    title: format!("Show diff for {}", obj_def.name),
                    kind: Some(CodeActionKind::SOURCE),
                    diagnostics: None,
                    edit: None,
                    command: Some(Command {
                        title: "Show diff".to_string(),
                        command: "moor.showDiff".to_string(),
                        arguments: Some(vec![
                            serde_json::to_value(uri.to_string()).unwrap(),
                            serde_json::to_value(obj_id.id().0).unwrap(),
                        ]),
                    }),
                    is_preferred: None,
                    disabled: None,
                    data: None,
                };
                actions.push(CodeActionOrCommand::CodeAction(show_diff_action));
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        // Get the document content
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

        // Find the symbol at the cursor position
        let Some(symbol) =
            references::symbol_at_position(&content, position.line, position.character)
        else {
            return Ok(None);
        };

        // Collect all edits grouped by document URI
        let mut changes: std::collections::HashMap<Url, Vec<TextEdit>> =
            std::collections::HashMap::new();

        // First, find references in the current file
        let refs_in_current = references::find_references_to(&content, &symbol.name);
        for reference in refs_in_current {
            let range = tower_lsp::lsp_types::Range {
                start: tower_lsp::lsp_types::Position {
                    line: reference.line,
                    character: reference.start_col,
                },
                end: tower_lsp::lsp_types::Position {
                    line: reference.line,
                    character: reference.end_col,
                },
            };
            changes
                .entry(uri.clone())
                .or_default()
                .push(TextEdit {
                    range,
                    new_text: new_name.clone(),
                });
        }

        // Then search across all indexed files in the workspace
        let workspace_index = self.workspace_index.read().await;
        let documents = self.documents.read().await;

        for (file_path, _symbols) in workspace_index.files() {
            // Skip the current file (already searched)
            let file_uri = match Url::from_file_path(file_path) {
                Ok(u) => u,
                Err(_) => continue,
            };
            if file_uri == uri {
                continue;
            }

            // Get the file content
            let file_content = if let Some(c) = documents.get(&file_uri) {
                c.clone()
            } else {
                match tokio::fs::read_to_string(file_path).await {
                    Ok(c) => c,
                    Err(_) => continue,
                }
            };

            // Find references in this file
            let refs = references::find_references_to(&file_content, &symbol.name);
            for reference in refs {
                let range = tower_lsp::lsp_types::Range {
                    start: tower_lsp::lsp_types::Position {
                        line: reference.line,
                        character: reference.start_col,
                    },
                    end: tower_lsp::lsp_types::Position {
                        line: reference.line,
                        character: reference.end_col,
                    },
                };
                changes
                    .entry(file_uri.clone())
                    .or_default()
                    .push(TextEdit {
                        range,
                        new_text: new_name.clone(),
                    });
            }
        }

        if changes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            }))
        }
    }

    async fn prepare_rename(
        &self,
        params: tower_lsp::lsp_types::TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;

        // Get the document content
        let content = {
            let docs = self.documents.read().await;
            docs.get(&uri).cloned()
        };

        let content = match content {
            Some(c) => c,
            None => {
                let Ok(path) = uri.to_file_path() else {
                    return Ok(None);
                };
                match tokio::fs::read_to_string(&path).await {
                    Ok(c) => c,
                    Err(_) => return Ok(None),
                }
            }
        };

        // Find the symbol at the cursor position
        let Some(symbol) =
            references::symbol_at_position(&content, position.line, position.character)
        else {
            return Ok(None);
        };

        // Return the range and placeholder text for the rename
        let range = tower_lsp::lsp_types::Range {
            start: tower_lsp::lsp_types::Position {
                line: symbol.line,
                character: symbol.start_col,
            },
            end: tower_lsp::lsp_types::Position {
                line: symbol.line,
                character: symbol.end_col,
            },
        };

        Ok(Some(PrepareRenameResponse::Range(range)))
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;

        // Get content from any supported source (open doc, file, URL, mooR server)
        let Some(content) = self.get_content(&uri).await else {
            return Ok(None);
        };

        // Parse and unparse: unparse(parse(src))
        let parse_result =
            moor_compiler::parse::parse_program(&content, moor_compiler::CompileOptions::default());

        let parsed = match parse_result {
            Ok(p) => p,
            Err(_) => {
                // Can't format code that doesn't parse
                return Ok(None);
            }
        };

        // Unparse with indentation enabled
        let formatted_lines = match moor_compiler::unparse(&parsed, false, true) {
            Ok(lines) => lines,
            Err(_) => return Ok(None),
        };

        let formatted = formatted_lines.join("\n");

        // If nothing changed, return no edits
        if formatted == content {
            return Ok(Some(vec![]));
        }

        // Create a single edit that replaces the entire document
        let line_count = content.lines().count() as u32;
        let last_line_len = content.lines().last().map(|l| l.len()).unwrap_or(0) as u32;

        let edit = TextEdit {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: line_count,
                    character: last_line_len,
                },
            },
            new_text: formatted,
        };

        Ok(Some(vec![edit]))
    }
}
