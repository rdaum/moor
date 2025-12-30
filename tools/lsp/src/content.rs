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

//! Content accessor for fetching MOO source from multiple sources.
//!
//! Supports:
//! - Open documents (in-memory)
//! - Filesystem
//! - HTTP/HTTPS URLs
//! - mooR server (verb retrieval via RPC)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::lsp_types::Url;

use crate::client::MoorClient;

/// Error type for content retrieval.
#[derive(Debug)]
pub enum ContentError {
    /// The URI scheme is not supported.
    UnsupportedScheme(String),
    /// File not found or not readable.
    FileNotFound(PathBuf),
    /// Network error fetching URL.
    NetworkError(String),
    /// RPC error communicating with mooR server.
    RpcError(String),
    /// The content is not available.
    NotFound(String),
}

impl std::fmt::Display for ContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentError::UnsupportedScheme(s) => write!(f, "Unsupported URI scheme: {}", s),
            ContentError::FileNotFound(p) => write!(f, "File not found: {}", p.display()),
            ContentError::NetworkError(e) => write!(f, "Network error: {}", e),
            ContentError::RpcError(e) => write!(f, "RPC error: {}", e),
            ContentError::NotFound(e) => write!(f, "Content not found: {}", e),
        }
    }
}

impl std::error::Error for ContentError {}

/// Content accessor that retrieves MOO source from various sources.
pub struct ContentAccessor {
    /// In-memory documents (open files in editor).
    documents: Arc<RwLock<HashMap<Url, String>>>,
    /// Optional mooR client for server-connected features.
    moor_client: Option<Arc<RwLock<MoorClient>>>,
}

impl ContentAccessor {
    /// Create a new content accessor.
    pub fn new(
        documents: Arc<RwLock<HashMap<Url, String>>>,
        moor_client: Option<Arc<RwLock<MoorClient>>>,
    ) -> Self {
        Self {
            documents,
            moor_client,
        }
    }

    /// Get content for a URI.
    ///
    /// Priority:
    /// 1. Open documents (in-memory)
    /// 2. Filesystem (file:// scheme)
    /// 3. HTTP/HTTPS URLs
    /// 4. mooR server (moor:// scheme, e.g., moor://object/verb)
    pub async fn get_content(&self, uri: &Url) -> Result<String, ContentError> {
        // Check open documents first
        {
            let docs = self.documents.read().await;
            if let Some(content) = docs.get(uri) {
                return Ok(content.clone());
            }
        }

        // Handle based on scheme
        match uri.scheme() {
            "file" => self.get_file_content(uri).await,
            "http" | "https" => self.get_url_content(uri).await,
            "moor" => self.get_moor_content(uri).await,
            scheme => Err(ContentError::UnsupportedScheme(scheme.to_string())),
        }
    }

    /// Get content from a file.
    async fn get_file_content(&self, uri: &Url) -> Result<String, ContentError> {
        let path = uri
            .to_file_path()
            .map_err(|_| ContentError::NotFound(format!("Invalid file path: {}", uri)))?;

        tokio::fs::read_to_string(&path)
            .await
            .map_err(|_| ContentError::FileNotFound(path))
    }

    /// Get content from an HTTP/HTTPS URL.
    async fn get_url_content(&self, uri: &Url) -> Result<String, ContentError> {
        // Use a simple HTTP client
        // Note: This is a minimal implementation. For production, consider
        // using reqwest or similar with proper timeout/retry handling.
        let response = reqwest::get(uri.as_str())
            .await
            .map_err(|e| ContentError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ContentError::NetworkError(format!(
                "HTTP {} for {}",
                response.status(),
                uri
            )));
        }

        response
            .text()
            .await
            .map_err(|e| ContentError::NetworkError(e.to_string()))
    }

    /// Get content from mooR server.
    ///
    /// URI format: moor://object/verb
    /// Example: moor://#123/do_something or moor://$player/look
    ///
    /// Note: This is a placeholder. The actual implementation requires:
    /// 1. Resolving object reference (e.g., $player -> Obj)
    /// 2. Getting verb program via MoorIntrospection::get_verb
    /// 3. Decompiling program to source via program_to_tree + unparse
    async fn get_moor_content(&self, uri: &Url) -> Result<String, ContentError> {
        let Some(_moor_client) = &self.moor_client else {
            return Err(ContentError::RpcError(
                "Not connected to mooR server".to_string(),
            ));
        };

        // Parse the URI: moor://object/verb
        let path = uri.path();
        let parts: Vec<&str> = path.trim_start_matches('/').splitn(2, '/').collect();

        if parts.len() != 2 {
            return Err(ContentError::NotFound(format!(
                "Invalid moor URI format. Expected moor://object/verb, got: {}",
                uri
            )));
        }

        let _object_ref = parts[0];
        let _verb_name = parts[1];

        // TODO: Implement full flow:
        // 1. Resolve object_ref to Obj (via object name registry or parse #123)
        // 2. Call moor_client.get_verb(obj, verb_name) -> (VerbDef, ProgramType)
        // 3. Extract Program from ProgramType
        // 4. Call moor_compiler::program_to_tree(&program) -> Parse
        // 5. Call moor_compiler::unparse(&parse, false, true) -> Vec<String>
        // 6. Join lines and return

        Err(ContentError::RpcError(
            "moor:// scheme not yet fully implemented".to_string(),
        ))
    }

    /// Check if content exists at the given URI without fetching it.
    pub async fn exists(&self, uri: &Url) -> bool {
        // Check open documents
        {
            let docs = self.documents.read().await;
            if docs.contains_key(uri) {
                return true;
            }
        }

        // For file scheme, check if file exists
        if uri.scheme() == "file" {
            if let Ok(path) = uri.to_file_path() {
                return path.exists();
            }
        }

        // For other schemes, we'd need to make a request
        // For now, return false (could be enhanced with HEAD requests)
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_open_document() {
        let mut docs = HashMap::new();
        let uri = Url::parse("file:///test.moo").unwrap();
        docs.insert(uri.clone(), "x = 1;".to_string());

        let accessor = ContentAccessor::new(Arc::new(RwLock::new(docs)), None);

        let content = accessor.get_content(&uri).await.unwrap();
        assert_eq!(content, "x = 1;");
    }

    #[tokio::test]
    async fn test_unsupported_scheme() {
        let docs = HashMap::new();
        let accessor = ContentAccessor::new(Arc::new(RwLock::new(docs)), None);

        let uri = Url::parse("ftp://example.com/file.moo").unwrap();
        let result = accessor.get_content(&uri).await;

        assert!(matches!(result, Err(ContentError::UnsupportedScheme(_))));
    }

    #[tokio::test]
    async fn test_moor_uri_without_client() {
        let docs = HashMap::new();
        let accessor = ContentAccessor::new(Arc::new(RwLock::new(docs)), None);

        let uri = Url::parse("moor://#123/look").unwrap();
        let result = accessor.get_content(&uri).await;

        assert!(matches!(result, Err(ContentError::RpcError(_))));
    }
}
