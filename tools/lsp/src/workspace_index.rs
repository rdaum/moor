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

//! Workspace-wide symbol index for MOO source files.
//!
//! This module maintains an index of all symbols across the workspace, enabling:
//! - Workspace symbol search (`workspace/symbol` LSP request)
//! - Go-to-Definition (finding where objects, verbs, properties are defined)
//! - Find-References (finding all locations where a symbol is used)

use std::collections::HashMap;
use std::path::PathBuf;

use moor_compiler::{CompileOptions, ObjFileContext, compile_object_definitions};
use moor_var::Obj;
use tower_lsp::lsp_types::{Location, Range, SymbolInformation, SymbolKind, Url};

/// A symbol indexed from a MOO source file.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IndexedSymbol {
    /// The symbol name (object name, verb name, or property name).
    pub name: String,
    /// The kind of symbol (CLASS for objects, METHOD for verbs, FIELD for properties).
    pub kind: SymbolKind,
    /// The location where this symbol is defined.
    pub location: Location,
    /// The container name (parent object name for verbs/properties).
    pub container_name: Option<String>,
}

/// Workspace-wide symbol index.
///
/// Maintains indexes for:
/// - File path -> list of symbols defined in that file
/// - Object ID -> file path where object is defined
/// - Symbol name -> list of locations where symbol appears
#[allow(dead_code)]
pub struct WorkspaceIndex {
    /// Symbols indexed by file path.
    symbols_by_file: HashMap<PathBuf, Vec<IndexedSymbol>>,
    /// Object ID to file path mapping (for go-to-definition).
    object_files: HashMap<Obj, PathBuf>,
    /// Symbol name to list of locations (for find-references).
    symbol_locations: HashMap<String, Vec<Location>>,
}

#[allow(dead_code)]
impl WorkspaceIndex {
    /// Create an empty workspace index.
    pub fn new() -> Self {
        Self {
            symbols_by_file: HashMap::new(),
            object_files: HashMap::new(),
            symbol_locations: HashMap::new(),
        }
    }

    /// Index a file and add its symbols to the workspace index.
    ///
    /// Parses the file content and extracts all object, verb, and property definitions.
    /// Updates all internal indexes accordingly.
    pub fn index_file(&mut self, path: PathBuf, content: &str) {
        // Remove any existing entries for this file first
        self.remove_file(&path);

        let options = CompileOptions::default();
        let mut context = ObjFileContext::default();

        let Ok(definitions) = compile_object_definitions(content, &options, &mut context) else {
            // Skip files that don't parse (diagnostics will show errors)
            return;
        };

        let uri = match Url::from_file_path(&path) {
            Ok(uri) => uri,
            Err(_) => return,
        };

        let mut symbols = Vec::new();

        for def in definitions {
            // TODO: Add span tracking to compiler for accurate ranges
            let range = Range::default();
            let location = Location::new(uri.clone(), range);

            // Index the object itself
            let object_symbol = IndexedSymbol {
                name: def.name.clone(),
                kind: SymbolKind::CLASS,
                location: location.clone(),
                container_name: None,
            };
            symbols.push(object_symbol);

            // Track object ID -> file path
            self.object_files.insert(def.oid, path.clone());

            // Track symbol name -> locations
            self.symbol_locations
                .entry(def.name.clone())
                .or_default()
                .push(location.clone());

            // Index verbs
            for verb in &def.verbs {
                let names: Vec<_> = verb.names.iter().map(|s| s.to_string()).collect();
                let verb_name = names.join(",");

                let verb_location = Location::new(uri.clone(), Range::default());

                let verb_symbol = IndexedSymbol {
                    name: verb_name.clone(),
                    kind: SymbolKind::METHOD,
                    location: verb_location.clone(),
                    container_name: Some(def.name.clone()),
                };
                symbols.push(verb_symbol);

                // Track each verb name separately for lookup
                for name in &names {
                    self.symbol_locations
                        .entry(name.clone())
                        .or_default()
                        .push(verb_location.clone());
                }
            }

            // Index properties
            for prop in &def.property_definitions {
                let prop_name = prop.name.to_string();
                let prop_location = Location::new(uri.clone(), Range::default());

                let prop_symbol = IndexedSymbol {
                    name: prop_name.clone(),
                    kind: SymbolKind::FIELD,
                    location: prop_location.clone(),
                    container_name: Some(def.name.clone()),
                };
                symbols.push(prop_symbol);

                self.symbol_locations
                    .entry(prop_name)
                    .or_default()
                    .push(prop_location);
            }
        }

        self.symbols_by_file.insert(path, symbols);
    }

    /// Remove a file from the index.
    ///
    /// Removes all symbols associated with the file from all internal indexes.
    pub fn remove_file(&mut self, path: &PathBuf) {
        // Remove symbols for this file
        if let Some(symbols) = self.symbols_by_file.remove(path) {
            // Clean up object_files entries pointing to this file
            self.object_files.retain(|_, p| p != path);

            // Clean up symbol_locations entries from this file
            let uri = Url::from_file_path(path).ok();
            if let Some(uri) = uri {
                for symbol in symbols {
                    if let Some(locations) = self.symbol_locations.get_mut(&symbol.name) {
                        locations.retain(|loc| loc.uri != uri);
                        if locations.is_empty() {
                            self.symbol_locations.remove(&symbol.name);
                        }
                    }
                }
            }
        }
    }

    /// Search for symbols matching the query.
    ///
    /// Performs a case-insensitive substring match against symbol names.
    /// Returns `SymbolInformation` for LSP compatibility.
    #[allow(deprecated)] // SymbolInformation::deprecated field is deprecated but required
    pub fn search(&self, query: &str) -> Vec<SymbolInformation> {
        let query_lower = query.to_lowercase();

        self.symbols_by_file
            .values()
            .flatten()
            .filter(|symbol| symbol.name.to_lowercase().contains(&query_lower))
            .map(|symbol| SymbolInformation {
                name: symbol.name.clone(),
                kind: symbol.kind,
                tags: None,
                deprecated: None,
                location: symbol.location.clone(),
                container_name: symbol.container_name.clone(),
            })
            .collect()
    }

    /// Get the file path where an object is defined.
    ///
    /// Returns `None` if the object is not found in the index.
    pub fn file_for_object(&self, obj: &Obj) -> Option<PathBuf> {
        self.object_files.get(obj).cloned()
    }

    /// Get all locations where a symbol is defined.
    ///
    /// Useful for find-references functionality.
    #[allow(dead_code)]
    pub fn locations_for_symbol(&self, name: &str) -> Option<&Vec<Location>> {
        self.symbol_locations.get(name)
    }

    /// Get all symbols in a specific file.
    #[allow(dead_code)]
    pub fn symbols_in_file(&self, path: &PathBuf) -> Option<&Vec<IndexedSymbol>> {
        self.symbols_by_file.get(path)
    }

    /// Get the number of indexed files.
    #[allow(dead_code)]
    pub fn file_count(&self) -> usize {
        self.symbols_by_file.len()
    }

    /// Get the total number of indexed symbols.
    #[allow(dead_code)]
    pub fn symbol_count(&self) -> usize {
        self.symbols_by_file.values().map(|v| v.len()).sum()
    }
}

impl Default for WorkspaceIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_index() {
        let index = WorkspaceIndex::new();
        assert_eq!(index.file_count(), 0);
        assert_eq!(index.symbol_count(), 0);
        assert!(index.search("anything").is_empty());
    }

    #[test]
    fn test_index_simple_object() {
        let mut index = WorkspaceIndex::new();
        let content = r#"
object #1
    parent: #0
    name: "Simple Object"
    location: #0
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true
endobject
"#;
        let path = PathBuf::from("/test/simple.moo");
        index.index_file(path.clone(), content);

        assert_eq!(index.file_count(), 1);
        assert!(index.symbol_count() >= 1);

        // Search for the object by name
        let results = index.search("Simple");
        assert!(!results.is_empty());
        assert!(results.iter().any(|s| s.kind == SymbolKind::CLASS));
    }

    #[test]
    fn test_index_object_with_verbs_and_props() {
        let mut index = WorkspaceIndex::new();
        let content = r#"
object #42
    parent: #1
    name: "My Object"
    location: #0
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true

    property my_prop (owner: #1, flags: "r") = 42;

    verb "greet" (this none this) owner: #1 flags: "rxd"
        return "Hello!";
    endverb
endobject
"#;
        let path = PathBuf::from("/test/my_object.moo");
        index.index_file(path.clone(), content);

        // Should have 3 symbols: object, verb, property
        assert!(index.symbol_count() >= 3);

        // Search for object
        let results = index.search("My Object");
        assert!(results.iter().any(|s| s.kind == SymbolKind::CLASS));

        // Search for verb
        let results = index.search("greet");
        assert!(results.iter().any(|s| s.kind == SymbolKind::METHOD));

        // Search for property
        let results = index.search("my_prop");
        assert!(results.iter().any(|s| s.kind == SymbolKind::FIELD));
    }

    #[test]
    fn test_search_case_insensitive() {
        let mut index = WorkspaceIndex::new();
        let content = r#"
object #1
    parent: #0
    name: "MixedCaseObject"
    location: #0
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true
endobject
"#;
        let path = PathBuf::from("/test/mixed.moo");
        index.index_file(path, content);

        // All these should match
        assert!(!index.search("MixedCaseObject").is_empty());
        assert!(!index.search("mixedcaseobject").is_empty());
        assert!(!index.search("MIXEDCASEOBJECT").is_empty());
        assert!(!index.search("mixed").is_empty());
        assert!(!index.search("Case").is_empty());
    }

    #[test]
    fn test_remove_file() {
        let mut index = WorkspaceIndex::new();
        let content = r#"
object #1
    parent: #0
    name: "To Remove"
    location: #0
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true
endobject
"#;
        let path = PathBuf::from("/test/remove.moo");
        index.index_file(path.clone(), content);

        assert_eq!(index.file_count(), 1);
        assert!(!index.search("To Remove").is_empty());

        index.remove_file(&path);

        assert_eq!(index.file_count(), 0);
        assert!(index.search("To Remove").is_empty());
    }

    #[test]
    fn test_file_for_object() {
        let mut index = WorkspaceIndex::new();
        let content = r#"
object #42
    parent: #0
    name: "Indexed Object"
    location: #0
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true
endobject
"#;
        let path = PathBuf::from("/test/indexed.moo");
        index.index_file(path.clone(), content);

        let obj = Obj::mk_id(42);
        let found_path = index.file_for_object(&obj);
        assert_eq!(found_path, Some(path));

        // Non-existent object
        let other_obj = Obj::mk_id(999);
        assert!(index.file_for_object(&other_obj).is_none());
    }

    #[test]
    fn test_invalid_file_not_indexed() {
        let mut index = WorkspaceIndex::new();
        let invalid_content = "this is not valid MOO syntax at all";
        let path = PathBuf::from("/test/invalid.moo");
        index.index_file(path, invalid_content);

        // Should not crash, and should have no symbols
        assert_eq!(index.file_count(), 0);
        assert_eq!(index.symbol_count(), 0);
    }

    #[test]
    fn test_reindex_file() {
        let mut index = WorkspaceIndex::new();
        let path = PathBuf::from("/test/reindex.moo");

        // Initial content
        let content1 = r#"
object #1
    parent: #0
    name: "First Version"
    location: #0
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true
endobject
"#;
        index.index_file(path.clone(), content1);
        assert!(!index.search("First Version").is_empty());
        assert!(index.search("Second Version").is_empty());

        // Updated content
        let content2 = r#"
object #1
    parent: #0
    name: "Second Version"
    location: #0
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true
endobject
"#;
        index.index_file(path, content2);
        assert!(index.search("First Version").is_empty());
        assert!(!index.search("Second Version").is_empty());
    }
}
