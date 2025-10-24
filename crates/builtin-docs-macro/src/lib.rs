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
/// This ensures only actually-registered builtins are documented.
#[proc_macro]
pub fn generate_builtin_docs(_input: TokenStream) -> TokenStream {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");

    let builtins_dir = PathBuf::from(&manifest_dir)
        .join("src/vm/builtins");

    let mut docs_map: HashMap<String, Vec<String>> = HashMap::new();

    // Find all bf_*.rs files
    let entries = fs::read_dir(&builtins_dir)
        .expect("Failed to read builtins directory");

    for entry in entries {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if filename.starts_with("bf_") && filename.ends_with(".rs") {
                extract_docs_from_file(&path, &mut docs_map);
            }
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
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read file {:?}: {}", path, e));

    let ast = syn::parse_file(&content)
        .unwrap_or_else(|e| panic!("Failed to parse file {:?}: {}", path, e));

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
/// Looks for patterns like: builtins[offset_for_builtin("abs")] = Box::new(bf_abs);
fn extract_registrations(
    stmts: &[Stmt],
    fn_docs: &HashMap<String, Vec<String>>,
    docs_map: &mut HashMap<String, Vec<String>>,
) {
    for stmt in stmts {
        if let Stmt::Expr(Expr::Assign(assign), _) = stmt {
            if let Expr::Index(ExprIndex { expr, index, .. }) = &*assign.left {
                if let Expr::Path(path) = &**expr {
                    if path.path.segments.last().map(|s| s.ident.to_string()) == Some("builtins".to_string()) {
                        if let Some(builtin_name) = extract_builtin_name_from_index(index) {
                            if let Some(bf_fn_name) = extract_bf_fn_name(&assign.right) {
                                if let Some(docs) = fn_docs.get(&bf_fn_name) {
                                    docs_map.insert(builtin_name, docs.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn extract_builtin_name_from_index(index: &Expr) -> Option<String> {
    // Parse: offset_for_builtin("name")
    if let Expr::Call(call) = index {
        if let Expr::Path(path) = &*call.func {
            if path.path.segments.last().map(|s| s.ident.to_string()) == Some("offset_for_builtin".to_string()) {
                if let Some(Expr::Lit(lit)) = call.args.first() {
                    if let syn::Lit::Str(s) = &lit.lit {
                        return Some(s.value());
                    }
                }
            }
        }
    }
    None
}

fn extract_bf_fn_name(expr: &Expr) -> Option<String> {
    // Parse: Box::new(bf_name)
    if let Expr::Call(call) = expr {
        if let Expr::Path(path) = &*call.func {
            // Check if it's Box::new
            if path.path.segments.len() == 2
                && path.path.segments[0].ident == "Box"
                && path.path.segments[1].ident == "new" {
                // Get the argument to Box::new
                if let Some(Expr::Path(fn_path)) = call.args.first() {
                    if let Some(segment) = fn_path.path.segments.last() {
                        return Some(segment.ident.to_string());
                    }
                }
            }
        }
    }
    None
}

fn extract_doc_comments(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("doc") {
                if let Ok(meta) = attr.meta.require_name_value() {
                    if let syn::Expr::Lit(expr_lit) = &meta.value {
                        if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                            return Some(lit_str.value());
                        }
                    }
                }
            }
            None
        })
        .collect()
}
