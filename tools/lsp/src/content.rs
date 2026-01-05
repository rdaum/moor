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
//! - mooR server (verb/property retrieval via RPC)
//!
//! ## moor:// URI Scheme
//!
//! Format: `moor://host/object/<objref>/<type>/<name>`
//!
//! Where:
//! - `host` - Server hostname (currently ignored, uses connected client)
//! - `objref` - Object reference (see below)
//! - `type` - Either `verb` or `property`
//! - `name` - Verb or property name
//!
//! ### Object Reference Formats
//!
//! - `#123` - Direct object ID
//! - `$player` - System property (resolves #0.player)
//! - `$player.location` - Dotted reference (resolve $player, then get .location)
//! - `$player.location.contents` - Arbitrary property chains supported
//! - `player` or `me` - Current logged-in user
//!
//! ### Examples
//!
//! - `moor://localhost/object/#0/verb/do_login_command`
//! - `moor://localhost/object/$room/property/description`
//! - `moor://localhost/object/player/verb/tell`
//! - `moor://localhost/object/me/property/name`

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use moor_common::model::ObjectRef;
use moor_var::Obj;
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
    /// URI format: `moor://host/object/<objref>/<type>/<name>`
    ///
    /// Examples:
    /// - `moor://localhost/object/#0/verb/do_login_command`
    /// - `moor://localhost/object/#2/property/description`
    ///
    /// The host is currently ignored (uses the connected client).
    async fn get_moor_content(&self, uri: &Url) -> Result<String, ContentError> {
        let Some(moor_client) = &self.moor_client else {
            return Err(ContentError::RpcError(
                "Not connected to mooR server".to_string(),
            ));
        };

        // Parse the URI path: /object/<objref>/<type>/<name>
        let path = uri.path();
        let parts: Vec<&str> = path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        // Expected: ["object", "<objref>", "<type>", "<name>"]
        if parts.len() != 4 || parts[0] != "object" {
            return Err(ContentError::NotFound(format!(
                "Invalid moor URI format. Expected moor://host/object/<objref>/<type>/<name>, got: {}",
                uri
            )));
        }

        let obj_ref_str = parts[1];
        let entity_type = parts[2];
        let entity_name = parts[3];

        // Parse object reference - supports #123, $player, $player.location
        let parsed_ref = parse_object_ref_extended(obj_ref_str).ok_or_else(|| {
            ContentError::NotFound(format!("Invalid object reference: {}", obj_ref_str))
        })?;

        let mut client = moor_client.write().await;

        // Resolve the object reference to an Obj
        let obj = resolve_object_ref(&mut client, &parsed_ref).await?;
        let object_ref = ObjectRef::Id(obj);

        match entity_type {
            "verb" => {
                // Get verb code from server
                let verb_code = client.get_verb(&object_ref, entity_name).await.map_err(|e| {
                    ContentError::RpcError(format!("Failed to get verb '{}': {}", entity_name, e))
                })?;

                // Join lines with newlines
                Ok(verb_code.code.join("\n"))
            }
            "property" => {
                // Get property value from server
                let value = client
                    .get_property(&object_ref, entity_name)
                    .await
                    .map_err(|e| {
                        ContentError::RpcError(format!(
                            "Failed to get property '{}': {}",
                            entity_name, e
                        ))
                    })?;

                // Format property value as MOO literal
                Ok(format_var_as_moo(&value))
            }
            _ => Err(ContentError::NotFound(format!(
                "Unknown entity type: {}. Expected 'verb' or 'property'.",
                entity_type
            ))),
        }
    }

    /// Check if content exists at the given URI without fetching it.
    #[allow(dead_code)]
    pub async fn exists(&self, uri: &Url) -> bool {
        // Check open documents
        {
            let docs = self.documents.read().await;
            if docs.contains_key(uri) {
                return true;
            }
        }

        // For file scheme, check if file exists
        if uri.scheme() == "file"
            && let Ok(path) = uri.to_file_path()
        {
            return path.exists();
        }

        // For other schemes, we'd need to make a request
        // For now, return false (could be enhanced with HEAD requests)
        false
    }
}

/// Parsed object reference - either a direct ID or a symbolic reference.
#[derive(Debug, Clone)]
pub enum ParsedObjectRef {
    /// Direct object ID like #123
    Id(Obj),
    /// System property reference like $player (resolves to #0.player)
    SysProp(String),
    /// Dotted reference like $player.location.contents (arbitrary chain)
    /// First element is the base sysprop, rest are property chain
    Dotted(String, Vec<String>),
    /// Current player reference (player, me)
    CurrentPlayer,
}

/// Parse an object reference string.
///
/// Supports:
/// - `#123` or `123` - Direct object ID
/// - `$name` - System property reference (resolves to #0.name)
/// - `$obj.prop.chain` - Dotted reference with arbitrary depth
/// - `player` or `me` - Current logged-in player
fn parse_object_ref_extended(s: &str) -> Option<ParsedObjectRef> {
    let s = s.trim();

    // Check for current player keywords first
    if s.eq_ignore_ascii_case("player") || s.eq_ignore_ascii_case("me") {
        return Some(ParsedObjectRef::CurrentPlayer);
    }

    if let Some(stripped) = s.strip_prefix('#') {
        // Direct object ID: #123
        stripped.parse::<i32>().ok().map(|id| ParsedObjectRef::Id(Obj::mk_id(id)))
    } else if let Some(rest) = s.strip_prefix('$') {
        // System property or dotted reference
        let parts: Vec<&str> = rest.split('.').collect();
        match parts.as_slice() {
            [] | [""] => None,
            [name] if !name.is_empty() => {
                // Simple sysprop: $player
                Some(ParsedObjectRef::SysProp(name.to_string()))
            }
            [base, props @ ..] if !base.is_empty() && props.iter().all(|p| !p.is_empty()) => {
                // Dotted: $player.location or $player.location.contents
                Some(ParsedObjectRef::Dotted(
                    base.to_string(),
                    props.iter().map(|s| s.to_string()).collect(),
                ))
            }
            _ => None,
        }
    } else {
        // Try parsing as raw number
        s.parse::<i32>().ok().map(|id| ParsedObjectRef::Id(Obj::mk_id(id)))
    }
}

/// Resolve a parsed object reference to an actual Obj.
///
/// - `ParsedObjectRef::Id(obj)` - returns the object directly
/// - `ParsedObjectRef::SysProp(name)` - resolves #0.name to get the object
/// - `ParsedObjectRef::Dotted(base, chain)` - resolves $base, then follows property chain
/// - `ParsedObjectRef::CurrentPlayer` - returns the current logged-in player
async fn resolve_object_ref(
    client: &mut MoorClient,
    parsed_ref: &ParsedObjectRef,
) -> Result<Obj, ContentError> {
    match parsed_ref {
        ParsedObjectRef::Id(obj) => Ok(*obj),
        ParsedObjectRef::CurrentPlayer => {
            // Get the current logged-in player from the client
            client.player().copied().ok_or_else(|| {
                ContentError::NotFound(
                    "Not logged in - 'player'/'me' requires authentication".to_string(),
                )
            })
        }
        ParsedObjectRef::SysProp(name) => {
            // Get #0.name (system object property)
            let sys_obj = ObjectRef::Id(Obj::mk_id(0));
            let value = client.get_property(&sys_obj, name).await.map_err(|e| {
                ContentError::RpcError(format!("Failed to resolve ${}: {}", name, e))
            })?;

            // Extract Obj from the value
            extract_obj_from_var(&value).ok_or_else(|| {
                ContentError::NotFound(format!(
                    "${} does not resolve to an object (got {:?})",
                    name, value
                ))
            })
        }
        ParsedObjectRef::Dotted(base_name, prop_chain) => {
            // First resolve $base_name (inline the SysProp logic to avoid recursion)
            let sys_obj = ObjectRef::Id(Obj::mk_id(0));
            let base_value = client.get_property(&sys_obj, base_name).await.map_err(|e| {
                ContentError::RpcError(format!("Failed to resolve ${}: {}", base_name, e))
            })?;
            let mut current_obj = extract_obj_from_var(&base_value).ok_or_else(|| {
                ContentError::NotFound(format!(
                    "${} does not resolve to an object (got {:?})",
                    base_name, base_value
                ))
            })?;

            // Build path string for error messages
            let mut path = format!("${}", base_name);

            // Follow the property chain
            for prop_name in prop_chain {
                path.push('.');
                path.push_str(prop_name);

                let obj_ref = ObjectRef::Id(current_obj);
                let value = client.get_property(&obj_ref, prop_name).await.map_err(|e| {
                    ContentError::RpcError(format!("Failed to get {}: {}", path, e))
                })?;

                current_obj = extract_obj_from_var(&value).ok_or_else(|| {
                    ContentError::NotFound(format!(
                        "{} does not resolve to an object (got {:?})",
                        path, value
                    ))
                })?;
            }

            Ok(current_obj)
        }
    }
}

/// Extract an Obj from a Var, if it contains one.
fn extract_obj_from_var(var: &moor_var::Var) -> Option<Obj> {
    use moor_var::Variant;
    match var.variant() {
        Variant::Obj(o) => Some(o),
        _ => None,
    }
}

/// Format a Var as a MOO literal for display.
fn format_var_as_moo(var: &moor_var::Var) -> String {
    use moor_var::Variant;

    match var.variant() {
        Variant::None => "0".to_string(), // MOO represents none as 0 in some contexts
        Variant::Bool(b) => {
            if b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Variant::Int(i) => i.to_string(),
        Variant::Float(f) => format!("{}", f),
        Variant::Str(s) => format!("{:?}", s.as_str()), // Quoted string
        Variant::Obj(o) => format!("{}", o),            // #123 format
        Variant::Err(e) => format!("{}", e),
        Variant::List(list) => {
            let items: Vec<String> = list.iter().map(|v| format_var_as_moo(&v)).collect();
            format!("{{{}}}", items.join(", "))
        }
        Variant::Map(map) => {
            let items: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{} -> {}", format_var_as_moo(&k), format_var_as_moo(&v)))
                .collect();
            format!("[{}]", items.join(", "))
        }
        Variant::Sym(s) => format!("'{}", s.as_string()),
        Variant::Binary(b) => format!("~{:?}~", b.as_bytes()),
        Variant::Flyweight(fw) => {
            let delegate = fw.delegate();
            let slots: Vec<String> = fw
                .slots()
                .iter()
                .map(|(k, v)| format!("{}: {}", k.as_string(), format_var_as_moo(v)))
                .collect();
            if slots.is_empty() {
                format!("<{}>", delegate)
            } else {
                format!("<{} | {}>", delegate, slots.join(", "))
            }
        }
        Variant::Lambda(_) => "<lambda>".to_string(),
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
