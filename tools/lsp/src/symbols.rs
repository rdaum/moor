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

use moor_compiler::{CompileOptions, ObjFileContext, compile_object_definitions};
use tower_lsp::lsp_types::{DocumentSymbol, Range, SymbolKind};

/// Extract document symbols from MOO source code.
#[allow(deprecated)] // DocumentSymbol::deprecated field is deprecated but required
pub fn extract_symbols(source: &str) -> Vec<DocumentSymbol> {
    let options = CompileOptions::default();
    let mut context = ObjFileContext::default();

    let Ok(definitions) = compile_object_definitions(source, &options, &mut context) else {
        // Return empty on parse error (diagnostics will show error)
        return Vec::new();
    };

    definitions
        .into_iter()
        .map(|def| {
            let verb_children: Vec<DocumentSymbol> = def
                .verbs
                .iter()
                .map(|verb| {
                    let names: Vec<_> = verb.names.iter().map(|s| s.to_string()).collect();
                    DocumentSymbol {
                        name: names.join(","),
                        detail: Some(format!("{:?}", verb.argspec)),
                        kind: SymbolKind::METHOD,
                        tags: None,
                        deprecated: None,
                        range: Range::default(), // TODO: Add span tracking
                        selection_range: Range::default(),
                        children: None,
                    }
                })
                .collect();

            let prop_children: Vec<DocumentSymbol> = def
                .property_definitions
                .iter()
                .map(|prop| DocumentSymbol {
                    name: prop.name.to_string(),
                    detail: None,
                    kind: SymbolKind::FIELD,
                    tags: None,
                    deprecated: None,
                    range: Range::default(),
                    selection_range: Range::default(),
                    children: None,
                })
                .collect();

            let mut children = verb_children;
            children.extend(prop_children);

            DocumentSymbol {
                name: def.name.clone(),
                detail: Some(format!("parent: {}", def.parent)),
                kind: SymbolKind::CLASS,
                tags: None,
                deprecated: None,
                range: Range::default(),
                selection_range: Range::default(),
                children: if children.is_empty() {
                    None
                } else {
                    Some(children)
                },
            }
        })
        .collect()
}
