// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use syn::{Attribute, Expr, ExprIndex, Item, ItemFn, Stmt};

/// Extracts documentation from builtin functions by:
/// 1. Finding all `register_bf_*` functions in bf_*.rs files
/// 2. Parsing which `bf_*` functions they register
/// 3. Extracting doc comments only from registered functions
///
/// This ensures only actually-registered builtins are documented.
#[proc_macro]
pub fn generate_builtin_docs(_input: TokenStream) -> TokenStream {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let builtins_dir = PathBuf::from(&manifest_dir).join("src/vm/builtins");

    let mut docs_map: HashMap<String, Vec<String>> = HashMap::new();

    // Find all bf_*.rs files
    let entries = fs::read_dir(&builtins_dir).expect("Failed to read builtins directory");

    for entry in entries {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if let Some(filename) = path.file_name().and_then(|n| n.to_str())
            && filename.starts_with("bf_")
            && filename.ends_with(".rs")
        {
            extract_docs_from_file(&path, &mut docs_map);
        }
    }

    // Generate the static HashMap code
    let entries = docs_map.iter().map(|(name, lines)| {
        let doc_lines = lines.iter().map(|line| {
            quote! { #line.to_string() }
        });
        quote! {
            m.insert(#name, vec![#(#doc_lines),*]);
        }
    });

    let expanded = quote! {
        lazy_static::lazy_static! {
            pub static ref BUILTIN_DOCS: std::collections::HashMap<&'static str, Vec<String>> = {
                let mut m = std::collections::HashMap::new();
                #(#entries)*
                m
            };
        }
    };

    TokenStream::from(expanded)
}

fn extract_docs_from_file(path: &PathBuf, docs_map: &mut HashMap<String, Vec<String>>) {
    let content =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("Failed to read file {path:?}: {e}"));

    let ast =
        syn::parse_file(&content).unwrap_or_else(|e| panic!("Failed to parse file {path:?}: {e}"));

    // Build a map of all bf_* functions and their doc comments
    let mut fn_docs: HashMap<String, Vec<String>> = HashMap::new();
    for item in &ast.items {
        if let Item::Fn(ItemFn { attrs, sig, .. }) = item {
            let fn_name = sig.ident.to_string();
            if fn_name.starts_with("bf_") {
                let doc_lines = extract_doc_comments(attrs);
                if !doc_lines.is_empty() {
                    fn_docs.insert(fn_name, doc_lines);
                }
            }
        }
    }

    // Find register_bf_* functions and extract docs for registered builtins only
    for item in &ast.items {
        if let Item::Fn(ItemFn { sig, block, .. }) = item {
            let fn_name = sig.ident.to_string();

            if fn_name.starts_with("register_bf_") {
                extract_registrations(&block.stmts, &fn_docs, docs_map);
            }
        }
    }
}

/// Extracts builtin registrations from a register_bf_* function body.
/// Looks for patterns like: builtins[offset_for_builtin("abs")] = bf_abs;
fn extract_registrations(
    stmts: &[Stmt],
    fn_docs: &HashMap<String, Vec<String>>,
    docs_map: &mut HashMap<String, Vec<String>>,
) {
    for stmt in stmts {
        let Stmt::Expr(Expr::Assign(assign), _) = stmt else {
            continue;
        };

        let Expr::Index(ExprIndex { expr, index, .. }) = &*assign.left else {
            continue;
        };

        let Expr::Path(path) = &**expr else {
            continue;
        };

        let Some(last_segment) = path.path.segments.last() else {
            continue;
        };

        if last_segment.ident != "builtins" {
            continue;
        }

        let Some(builtin_name) = extract_builtin_name_from_index(index) else {
            continue;
        };

        let Some(bf_fn_name) = extract_bf_fn_name(&assign.right) else {
            continue;
        };

        let Some(docs) = fn_docs.get(&bf_fn_name) else {
            continue;
        };

        docs_map.insert(builtin_name, docs.clone());
    }
}

fn extract_builtin_name_from_index(index: &Expr) -> Option<String> {
    // Parse: offset_for_builtin("name")
    let Expr::Call(call) = index else {
        return None;
    };

    let Expr::Path(path) = &*call.func else {
        return None;
    };

    let last_segment = path.path.segments.last()?;

    if last_segment.ident != "offset_for_builtin" {
        return None;
    }

    let Some(Expr::Lit(lit)) = call.args.first() else {
        return None;
    };

    let syn::Lit::Str(s) = &lit.lit else {
        return None;
    };

    Some(s.value())
}

fn extract_bf_fn_name(expr: &Expr) -> Option<String> {
    // Parse direct function pointer: bf_name
    let Expr::Path(path) = expr else {
        return None;
    };

    path.path.segments.last().map(|s| s.ident.to_string())
}

fn extract_doc_comments(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }

            let Ok(meta) = attr.meta.require_name_value() else {
                return None;
            };

            let syn::Expr::Lit(expr_lit) = &meta.value else {
                return None;
            };

            let syn::Lit::Str(lit_str) = &expr_lit.lit else {
                return None;
            };

            Some(lit_str.value())
        })
        .collect()
}
