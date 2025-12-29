# mdmoot Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build mdmoot, a markdown-based MOO testing framework that treats test files as living specification documents with multi-implementation comparison support.

**Architecture:** New `moor-mdmoot` crate alongside existing `moor-moot`. Core components: spec parser (markdown + frontmatter + tables), execution engine with handler abstraction, CLI binary, and web server. Reuses `MootRunner` trait from moot 1.0 for implementation handlers.

**Tech Stack:** Rust, pulldown-cmark (markdown), serde/toml (config), clap (CLI), axum (web server), pest (table/block parsing)

---

## Phase 1: Core Parser Foundation

### Task 1: Create mdmoot crate skeleton

**Files:**
- Create: `crates/testing/mdmoot/Cargo.toml`
- Create: `crates/testing/mdmoot/src/lib.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "moor-mdmoot"
version = "0.1.0"
authors.workspace = true
categories.workspace = true
edition.workspace = true
keywords.workspace = true
license.workspace = true
readme.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Markdown-based MOO testing framework with multi-implementation comparison"

[dependencies]
# Internal Dependencies
moor-moot = { path = "../moot" }
moor-var = { path = "../../var" }

# Markdown & Parsing
pulldown-cmark = "0.12"

# Config & Serialization
serde = { workspace = true, features = ["derive"] }
toml = "0.8"

# Error Handling & Logging
eyre.workspace = true
thiserror.workspace = true
tracing.workspace = true

[dev-dependencies]
pretty_assertions.workspace = true
```

**Step 2: Create lib.rs skeleton**

```rust
//! mdmoot: Markdown-based MOO testing framework
//!
//! Treats test files as living specification documents with support for:
//! - REPL-style code blocks
//! - Decision and Script tables
//! - Multi-implementation comparison
//! - Named bindings with hierarchical scoping

pub mod ast;
pub mod config;
pub mod parser;

pub use ast::*;
pub use config::Config;
pub use parser::parse_spec;
```

**Step 3: Add to workspace**

Edit `Cargo.toml` (root) to add `crates/testing/mdmoot` to workspace members.

**Step 4: Verify it compiles**

Run: `cargo check -p moor-mdmoot`
Expected: Compilation errors for missing modules (expected at this stage)

**Step 5: Commit**

```bash
git add crates/testing/mdmoot/ Cargo.toml
git commit -m "feat(mdmoot): create crate skeleton"
```

---

### Task 2: Define AST types

**Files:**
- Create: `crates/testing/mdmoot/src/ast.rs`

**Step 1: Write the AST types**

```rust
//! Abstract Syntax Tree for mdmoot spec files

use std::collections::HashMap;

/// A parsed spec file
#[derive(Debug, Clone)]
pub struct Spec {
    /// Frontmatter metadata
    pub frontmatter: Frontmatter,
    /// Document sections in order
    pub sections: Vec<Section>,
}

/// Frontmatter from YAML header
#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    /// Tags for filtering test runs
    pub tags: Vec<String>,
    /// Implementations to compare (overrides config)
    pub compare: Vec<String>,
}

/// A section of the document (corresponds to markdown headings)
#[derive(Debug, Clone)]
pub struct Section {
    /// Heading level (1-6)
    pub level: u8,
    /// Section title
    pub title: String,
    /// Whether this section is isolated
    pub isolated: bool,
    /// Content blocks in this section
    pub blocks: Vec<Block>,
}

/// A content block within a section
#[derive(Debug, Clone)]
pub enum Block {
    /// REPL-style moot block
    Moot(MootBlock),
    /// Setup/binding block
    Moo(MooBlock),
    /// Decision table (independent rows)
    DecisionTable(Table),
    /// Script table (sequential rows)
    ScriptTable(Table),
    /// Prose (ignored for execution)
    Prose(String),
}

/// A REPL-style moot block
#[derive(Debug, Clone)]
pub struct MootBlock {
    /// Player context switches and commands
    pub items: Vec<MootItem>,
}

/// An item within a moot block
#[derive(Debug, Clone)]
pub enum MootItem {
    /// Switch player context (@wizard, @programmer)
    PlayerSwitch(String),
    /// Expression to evaluate with expected result
    Eval {
        expr: String,
        expected: Expected,
        line_no: usize,
    },
    /// Comment line
    Comment(String),
}

/// Expected result with optional divergence annotations
#[derive(Debug, Clone)]
pub struct Expected {
    /// Primary expected value
    pub value: String,
    /// Implementation-specific divergences: impl_name -> expected_value
    pub divergences: HashMap<String, String>,
    /// Golden annotation if present
    pub golden: Option<GoldenAnnotation>,
}

/// Golden capture annotation
#[derive(Debug, Clone)]
pub struct GoldenAnnotation {
    pub impl_name: String,
    pub date: String,
}

/// A setup/binding block
#[derive(Debug, Clone)]
pub struct MooBlock {
    /// Player context
    pub player: Option<String>,
    /// Binding name (if any)
    pub bind: Option<String>,
    /// Whether this resets state
    pub reset: bool,
    /// The MOO code
    pub code: String,
}

/// A table (Decision or Script)
#[derive(Debug, Clone)]
pub struct Table {
    /// Table title (from first row for Script tables)
    pub title: Option<String>,
    /// Column definitions
    pub columns: Vec<Column>,
    /// Data rows
    pub rows: Vec<Row>,
}

/// A table column
#[derive(Debug, Clone)]
pub struct Column {
    /// Column header text
    pub header: String,
    /// Column type
    pub kind: ColumnKind,
}

/// Types of table columns
#[derive(Debug, Clone)]
pub enum ColumnKind {
    /// Input value (placeholder `_`)
    Input,
    /// Binding column (name: expr)
    Binding { name: String, expr: Option<String> },
    /// Output column (name?)
    Output { template: String },
    /// Comment column (#name)
    Comment,
}

/// A table row
#[derive(Debug, Clone)]
pub struct Row {
    /// Cell values
    pub cells: Vec<Cell>,
    /// Line number for error reporting
    pub line_no: usize,
}

/// A table cell
#[derive(Debug, Clone)]
pub struct Cell {
    /// Raw cell content
    pub content: String,
    /// Whether the cell is empty
    pub empty: bool,
    /// Expected value with divergences (for output cells)
    pub expected: Option<Expected>,
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p moor-mdmoot`
Expected: PASS (AST is standalone)

**Step 3: Commit**

```bash
git add crates/testing/mdmoot/src/ast.rs
git commit -m "feat(mdmoot): define AST types for spec files"
```

---

### Task 3: Define config types

**Files:**
- Create: `crates/testing/mdmoot/src/config.rs`

**Step 1: Write config types**

```rust
//! Configuration for mdmoot

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level configuration (mdmoot.toml)
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Project settings
    pub project: ProjectConfig,
    /// Implementation handlers
    #[serde(default)]
    pub implementations: HashMap<String, ImplementationConfig>,
    /// Web server settings
    #[serde(default)]
    pub server: ServerConfig,
}

/// Project-level configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    /// Root directory for spec files
    pub root: PathBuf,
    /// Default implementation to run tests against
    #[serde(default = "default_impl")]
    pub default_impl: String,
}

fn default_impl() -> String {
    "moor".to_string()
}

/// Configuration for a single implementation
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "handler")]
pub enum ImplementationConfig {
    /// In-process moor execution
    #[serde(rename = "in-process")]
    InProcess,
    /// Telnet connection to external server
    #[serde(rename = "telnet")]
    Telnet {
        host: String,
        port: u16,
    },
}

/// Web server configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServerConfig {
    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    8080
}

impl Config {
    /// Load config from file
    pub fn load(path: &std::path::Path) -> eyre::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Find config file by walking up from current directory
    pub fn find_and_load() -> eyre::Result<(PathBuf, Self)> {
        let mut dir = std::env::current_dir()?;
        loop {
            let config_path = dir.join("mdmoot.toml");
            if config_path.exists() {
                let config = Self::load(&config_path)?;
                return Ok((config_path, config));
            }
            if !dir.pop() {
                eyre::bail!("No mdmoot.toml found in current directory or parents");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let toml = r#"
[project]
root = "specs/"
default_impl = "moor"

[implementations.moor]
handler = "in-process"

[implementations.lambdamoo]
handler = "telnet"
host = "localhost"
port = 7777

[server]
port = 8080
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.project.root, PathBuf::from("specs/"));
        assert_eq!(config.project.default_impl, "moor");
        assert!(matches!(
            config.implementations.get("moor"),
            Some(ImplementationConfig::InProcess)
        ));
    }
}
```

**Step 2: Run the test**

Run: `cargo test -p moor-mdmoot config::tests`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/testing/mdmoot/src/config.rs
git commit -m "feat(mdmoot): add config types with toml parsing"
```

---

### Task 4: Implement frontmatter parser

**Files:**
- Create: `crates/testing/mdmoot/src/parser.rs`
- Create: `crates/testing/mdmoot/src/parser/frontmatter.rs`

**Step 1: Create parser module**

```rust
//! Spec file parser

mod frontmatter;

use crate::ast::*;
use eyre::Result;

pub use frontmatter::parse_frontmatter;

/// Parse a spec file from markdown content
pub fn parse_spec(content: &str) -> Result<Spec> {
    let (frontmatter, body) = parse_frontmatter(content)?;

    // TODO: Parse markdown body
    let sections = vec![];

    Ok(Spec {
        frontmatter,
        sections,
    })
}
```

**Step 2: Implement frontmatter parser**

```rust
//! Frontmatter parsing (YAML between --- delimiters)

use crate::ast::Frontmatter;
use eyre::{eyre, Result};
use serde::Deserialize;

/// Raw frontmatter for deserialization
#[derive(Debug, Deserialize, Default)]
struct RawFrontmatter {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    compare: Vec<String>,
}

/// Parse YAML frontmatter from the start of a document
/// Returns (frontmatter, remaining_content)
pub fn parse_frontmatter(content: &str) -> Result<(Frontmatter, &str)> {
    let content = content.trim_start();

    if !content.starts_with("---") {
        return Ok((Frontmatter::default(), content));
    }

    // Find the closing ---
    let after_open = &content[3..];
    let close_pos = after_open
        .find("\n---")
        .ok_or_else(|| eyre!("Unclosed frontmatter: missing closing ---"))?;

    let yaml_content = &after_open[..close_pos];
    let body_start = 3 + close_pos + 4; // "---" + yaml + "\n---"
    let body = content[body_start..].trim_start_matches('\n');

    let raw: RawFrontmatter = serde_yaml::from_str(yaml_content.trim())
        .map_err(|e| eyre!("Invalid frontmatter YAML: {}", e))?;

    Ok((
        Frontmatter {
            tags: raw.tags,
            compare: raw.compare,
        },
        body,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_frontmatter() {
        let (fm, body) = parse_frontmatter("# Hello\n\nSome content").unwrap();
        assert!(fm.tags.is_empty());
        assert!(fm.compare.is_empty());
        assert_eq!(body, "# Hello\n\nSome content");
    }

    #[test]
    fn test_with_frontmatter() {
        let content = r#"---
tags: [strings, core]
compare: [moor, lambdamoo]
---

# Test Document
"#;
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.tags, vec!["strings", "core"]);
        assert_eq!(fm.compare, vec!["moor", "lambdamoo"]);
        assert!(body.starts_with("# Test Document"));
    }

    #[test]
    fn test_empty_frontmatter() {
        let content = "---\n---\n# Hello";
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert!(fm.tags.is_empty());
        assert_eq!(body, "# Hello");
    }
}
```

**Step 3: Add serde_yaml dependency**

Edit `crates/testing/mdmoot/Cargo.toml`:
```toml
serde_yaml = "0.9"
```

**Step 4: Run tests**

Run: `cargo test -p moor-mdmoot parser::frontmatter::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/testing/mdmoot/src/parser.rs crates/testing/mdmoot/src/parser/
git commit -m "feat(mdmoot): implement frontmatter parser"
```

---

### Task 5: Implement markdown structure parser

**Files:**
- Create: `crates/testing/mdmoot/src/parser/markdown.rs`
- Modify: `crates/testing/mdmoot/src/parser.rs`

**Step 1: Create markdown parser**

```rust
//! Markdown structure parsing using pulldown-cmark

use crate::ast::*;
use eyre::{eyre, Result};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd, CodeBlockKind};

/// Parse markdown content into sections
pub fn parse_markdown(content: &str) -> Result<Vec<Section>> {
    let parser = Parser::new(content);
    let mut sections: Vec<Section> = vec![];
    let mut current_section: Option<Section> = None;
    let mut in_heading = false;
    let mut heading_text = String::new();
    let mut heading_level = 1u8;
    let mut in_code_block = false;
    let mut code_block_lang = String::new();
    let mut code_block_content = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                // Save current section if any
                if let Some(section) = current_section.take() {
                    sections.push(section);
                }
                in_heading = true;
                heading_level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                let (title, isolated) = parse_heading_modifiers(&heading_text);
                current_section = Some(Section {
                    level: heading_level,
                    title,
                    isolated,
                    blocks: vec![],
                });
            }
            Event::Text(text) if in_heading => {
                heading_text.push_str(&text);
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                code_block_content.clear();
            }
            Event::Text(text) if in_code_block => {
                code_block_content.push_str(&text);
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                if let Some(ref mut section) = current_section {
                    if let Some(block) = parse_code_block(&code_block_lang, &code_block_content)? {
                        section.blocks.push(block);
                    }
                }
            }
            Event::Text(text) => {
                // Prose content
                if let Some(ref mut section) = current_section {
                    section.blocks.push(Block::Prose(text.to_string()));
                }
            }
            _ => {}
        }
    }

    // Don't forget the last section
    if let Some(section) = current_section {
        sections.push(section);
    }

    Ok(sections)
}

/// Parse heading modifiers like (isolate)
fn parse_heading_modifiers(heading: &str) -> (String, bool) {
    let heading = heading.trim();
    if heading.ends_with("(isolate)") {
        let title = heading.trim_end_matches("(isolate)").trim().to_string();
        (title, true)
    } else {
        (heading.to_string(), false)
    }
}

/// Parse a code block into an AST Block
fn parse_code_block(lang: &str, content: &str) -> Result<Option<Block>> {
    let lang_parts: Vec<&str> = lang.split_whitespace().collect();
    let base_lang = lang_parts.first().map(|s| *s).unwrap_or("");

    match base_lang {
        "moot" => {
            let block = parse_moot_block(content)?;
            Ok(Some(Block::Moot(block)))
        }
        "moo" => {
            let block = parse_moo_block(lang, content)?;
            Ok(Some(Block::Moo(block)))
        }
        _ => Ok(None), // Ignore unknown code blocks
    }
}

/// Parse a moot REPL block
fn parse_moot_block(content: &str) -> Result<MootBlock> {
    let mut items = vec![];
    let mut current_expr = String::new();
    let mut expr_line_no = 0;

    for (line_no, line) in content.lines().enumerate() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if line.starts_with('#') {
            // Flush any pending expression
            if !current_expr.is_empty() {
                // This shouldn't happen - expression without expected value
                eyre::bail!("Expression without expected value at line {}", expr_line_no);
            }
            items.push(MootItem::Comment(line[1..].trim().to_string()));
        } else if line.starts_with('@') {
            // Player switch
            if !current_expr.is_empty() {
                eyre::bail!("Expression without expected value at line {}", expr_line_no);
            }
            items.push(MootItem::PlayerSwitch(line[1..].to_string()));
        } else if line.starts_with('>') {
            // Flush any pending expression
            if !current_expr.is_empty() {
                eyre::bail!("Expression without expected value at line {}", expr_line_no);
            }
            current_expr = line[1..].trim().to_string();
            expr_line_no = line_no + 1;
        } else if !current_expr.is_empty() {
            // This is the expected value
            let expected = parse_expected(line)?;
            items.push(MootItem::Eval {
                expr: std::mem::take(&mut current_expr),
                expected,
                line_no: expr_line_no,
            });
        }
    }

    if !current_expr.is_empty() {
        eyre::bail!("Expression without expected value at end of block");
    }

    Ok(MootBlock { items })
}

/// Parse a moo setup block
fn parse_moo_block(lang: &str, content: &str) -> Result<MooBlock> {
    let mut player = None;
    let mut bind = None;
    let mut reset = false;

    // Parse directives from lang string: "moo @wizard bind: fixtures"
    let parts: Vec<&str> = lang.split_whitespace().collect();
    for part in &parts[1..] {
        if part.starts_with('@') {
            player = Some(part[1..].to_string());
        } else if *part == "reset" {
            reset = true;
        } else if part.starts_with("bind:") {
            bind = Some(part[5..].to_string());
        }
    }

    Ok(MooBlock {
        player,
        bind,
        reset,
        code: content.to_string(),
    })
}

/// Parse expected value with divergence annotations
fn parse_expected(line: &str) -> Result<Expected> {
    // Format: value  # !impl1: val1  !impl2: val2  <!-- golden:impl:date -->
    let mut divergences = std::collections::HashMap::new();
    let mut golden = None;

    let (value_part, rest) = if let Some(hash_pos) = line.find('#') {
        (line[..hash_pos].trim(), Some(&line[hash_pos + 1..]))
    } else if let Some(comment_pos) = line.find("<!--") {
        (line[..comment_pos].trim(), Some(&line[comment_pos..]))
    } else {
        (line.trim(), None)
    };

    if let Some(rest) = rest {
        // Parse divergence annotations: !impl: value
        for part in rest.split('!').skip(1) {
            let part = part.trim();
            if let Some(colon_pos) = part.find(':') {
                let impl_name = part[..colon_pos].trim();
                let impl_value = part[colon_pos + 1..].trim();
                // Stop at next ! or <!-- or end
                let impl_value = impl_value
                    .split('!')
                    .next()
                    .unwrap_or(impl_value)
                    .split("<!--")
                    .next()
                    .unwrap_or(impl_value)
                    .trim();
                divergences.insert(impl_name.to_string(), impl_value.to_string());
            }
        }

        // Parse golden annotation: <!-- golden:impl:date -->
        if let Some(golden_start) = rest.find("<!-- golden:") {
            let after = &rest[golden_start + 12..];
            if let Some(end) = after.find("-->") {
                let golden_content = &after[..end];
                let parts: Vec<&str> = golden_content.split(':').collect();
                if parts.len() >= 2 {
                    golden = Some(GoldenAnnotation {
                        impl_name: parts[0].to_string(),
                        date: parts[1].to_string(),
                    });
                }
            }
        }
    }

    Ok(Expected {
        value: value_part.to_string(),
        divergences,
        golden,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading_modifiers() {
        let (title, isolated) = parse_heading_modifiers("Edge Cases (isolate)");
        assert_eq!(title, "Edge Cases");
        assert!(isolated);

        let (title, isolated) = parse_heading_modifiers("Normal Section");
        assert_eq!(title, "Normal Section");
        assert!(!isolated);
    }

    #[test]
    fn test_parse_expected_simple() {
        let expected = parse_expected("42").unwrap();
        assert_eq!(expected.value, "42");
        assert!(expected.divergences.is_empty());
    }

    #[test]
    fn test_parse_expected_with_divergences() {
        let expected = parse_expected("inf  # !lambdamoo: E_INVARG  !toaststunt: E_INVARG").unwrap();
        assert_eq!(expected.value, "inf");
        assert_eq!(expected.divergences.get("lambdamoo"), Some(&"E_INVARG".to_string()));
        assert_eq!(expected.divergences.get("toaststunt"), Some(&"E_INVARG".to_string()));
    }

    #[test]
    fn test_parse_expected_with_golden() {
        let expected = parse_expected("\"moor 0.1\"  <!-- golden:moor:2025-12-29 -->").unwrap();
        assert_eq!(expected.value, "\"moor 0.1\"");
        let golden = expected.golden.unwrap();
        assert_eq!(golden.impl_name, "moor");
        assert_eq!(golden.date, "2025-12-29");
    }

    #[test]
    fn test_parse_moot_block() {
        let content = r#"@wizard
> x = create($nothing);
#1
> return x.owner;
$wizard_player
"#;
        let block = parse_moot_block(content).unwrap();
        assert_eq!(block.items.len(), 3);
        assert!(matches!(&block.items[0], MootItem::PlayerSwitch(p) if p == "wizard"));
    }

    #[test]
    fn test_parse_moo_block() {
        let block = parse_moo_block("moo @wizard bind: fixtures reset", "$x = 1;").unwrap();
        assert_eq!(block.player, Some("wizard".to_string()));
        assert_eq!(block.bind, Some("fixtures".to_string()));
        assert!(block.reset);
        assert_eq!(block.code, "$x = 1;");
    }
}
```

**Step 2: Update parser.rs to use markdown parser**

```rust
//! Spec file parser

mod frontmatter;
mod markdown;

use crate::ast::*;
use eyre::Result;

pub use frontmatter::parse_frontmatter;
pub use markdown::parse_markdown;

/// Parse a spec file from markdown content
pub fn parse_spec(content: &str) -> Result<Spec> {
    let (frontmatter, body) = parse_frontmatter(content)?;
    let sections = parse_markdown(body)?;

    Ok(Spec {
        frontmatter,
        sections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_spec() {
        let content = r#"---
tags: [test]
---

# Test Spec

## Setup

```moo @wizard bind: fixtures
$x = create($nothing);
```

## Tests

```moot
@wizard
> return $x.owner;
$wizard_player
```
"#;
        let spec = parse_spec(content).unwrap();
        assert_eq!(spec.frontmatter.tags, vec!["test"]);
        assert_eq!(spec.sections.len(), 2);
        assert_eq!(spec.sections[0].title, "Setup");
        assert_eq!(spec.sections[1].title, "Tests");
    }
}
```

**Step 3: Run tests**

Run: `cargo test -p moor-mdmoot`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/testing/mdmoot/src/parser/
git commit -m "feat(mdmoot): implement markdown structure parser"
```

---

### Task 6: Implement table parser

**Files:**
- Create: `crates/testing/mdmoot/src/parser/table.rs`
- Modify: `crates/testing/mdmoot/src/parser/markdown.rs`

**Step 1: Create table parser**

```rust
//! Table parsing for Decision and Script tables

use crate::ast::*;
use eyre::{eyre, Result};

/// Parse a markdown table into AST
pub fn parse_table(lines: &[&str], is_script: bool) -> Result<Table> {
    if lines.len() < 2 {
        eyre::bail!("Table must have at least header and separator rows");
    }

    let header_cells = parse_table_row(lines[0])?;
    let columns = parse_column_headers(&header_cells, is_script)?;

    // Skip separator row (line 1)
    let mut rows = vec![];
    for (i, line) in lines[2..].iter().enumerate() {
        let cells = parse_table_row(line)?;
        let row = parse_data_row(&cells, &columns, i + 3)?;
        rows.push(row);
    }

    // Extract title for script tables
    let title = if is_script {
        columns.first().and_then(|c| {
            if c.header.starts_with("Script:") {
                Some(c.header.trim_start_matches("Script:").trim().to_string())
            } else {
                None
            }
        })
    } else {
        None
    };

    Ok(Table {
        title,
        columns,
        rows,
    })
}

/// Parse a single table row into cells
fn parse_table_row(line: &str) -> Result<Vec<String>> {
    let line = line.trim();
    if !line.starts_with('|') || !line.ends_with('|') {
        eyre::bail!("Invalid table row: must start and end with |");
    }

    let inner = &line[1..line.len() - 1];
    Ok(inner.split('|').map(|s| s.trim().to_string()).collect())
}

/// Parse column headers
fn parse_column_headers(cells: &[String], is_script: bool) -> Result<Vec<Column>> {
    let mut columns = vec![];

    for (i, cell) in cells.iter().enumerate() {
        let kind = parse_column_kind(cell, i == 0 && is_script)?;
        columns.push(Column {
            header: cell.clone(),
            kind,
        });
    }

    Ok(columns)
}

/// Determine column kind from header
fn parse_column_kind(header: &str, is_script_first: bool) -> Result<ColumnKind> {
    let header = header.trim();

    // Comment column
    if header.starts_with('#') {
        return Ok(ColumnKind::Comment);
    }

    // Output column (ends with ?)
    if header.ends_with('?') {
        let template = header.trim_end_matches('?').trim().to_string();
        return Ok(ColumnKind::Output { template });
    }

    // Binding column (contains :)
    if header.contains(':') && !is_script_first {
        let parts: Vec<&str> = header.splitn(2, ':').collect();
        let name = parts[0].trim().to_string();
        let expr = if parts.len() > 1 && !parts[1].trim().is_empty() {
            Some(parts[1].trim().to_string())
        } else {
            None
        };
        return Ok(ColumnKind::Binding { name, expr });
    }

    // Script table first column with title
    if is_script_first && header.starts_with("Script:") {
        return Ok(ColumnKind::Comment); // Title row, not actually a column
    }

    // Input column (plain _ or value)
    Ok(ColumnKind::Input)
}

/// Parse a data row
fn parse_data_row(cells: &[String], columns: &[Column], line_no: usize) -> Result<Row> {
    let mut parsed_cells = vec![];

    for (i, cell) in cells.iter().enumerate() {
        let column = columns.get(i);
        let is_output = matches!(column.map(|c| &c.kind), Some(ColumnKind::Output { .. }));

        let parsed = if is_output && !cell.is_empty() {
            Cell {
                content: cell.clone(),
                empty: false,
                expected: Some(super::markdown::parse_expected(cell)?),
            }
        } else {
            Cell {
                content: cell.clone(),
                empty: cell.is_empty(),
                expected: None,
            }
        };

        parsed_cells.push(parsed);
    }

    Ok(Row {
        cells: parsed_cells,
        line_no,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_table_row() {
        let cells = parse_table_row("| a | b | c |").unwrap();
        assert_eq!(cells, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_column_kind_output() {
        let kind = parse_column_kind("length(_)?", false).unwrap();
        assert!(matches!(kind, ColumnKind::Output { template } if template == "length(_)"));
    }

    #[test]
    fn test_parse_column_kind_binding() {
        let kind = parse_column_kind("obj: create($nothing)", false).unwrap();
        assert!(matches!(
            kind,
            ColumnKind::Binding { name, expr }
            if name == "obj" && expr == Some("create($nothing)".to_string())
        ));
    }

    #[test]
    fn test_parse_column_kind_comment() {
        let kind = parse_column_kind("#notes", false).unwrap();
        assert!(matches!(kind, ColumnKind::Comment));
    }

    #[test]
    fn test_parse_decision_table() {
        let lines = vec![
            "| _ | length(_)? |",
            "|---|------------|",
            "| `\"foo\"` | `3` |",
            "| `\"\"` | `0` |",
        ];
        let table = parse_table(&lines, false).unwrap();
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.rows.len(), 2);
    }
}
```

**Step 2: Integrate table parser into markdown parser**

Update `markdown.rs` to detect and parse tables:

```rust
// Add to imports
mod table;
pub use table::parse_table;

// Add table detection in parse_markdown function - this requires refactoring
// to collect consecutive table lines and parse them together
```

**Step 3: Run tests**

Run: `cargo test -p moor-mdmoot parser::table::tests`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/testing/mdmoot/src/parser/table.rs
git commit -m "feat(mdmoot): implement table parser for Decision and Script tables"
```

---

## Phase 2: Execution Engine

### Task 7: Define handler trait

**Files:**
- Create: `crates/testing/mdmoot/src/handler.rs`

**Step 1: Write handler trait**

```rust
//! Implementation handlers for executing MOO code

use eyre::Result;

/// Result of evaluating a MOO expression
#[derive(Debug, Clone)]
pub struct EvalResult {
    /// The result value as a string
    pub value: String,
    /// Whether the evaluation succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Handler for a MOO implementation
pub trait Handler: Send + Sync {
    /// Name of this implementation
    fn name(&self) -> &str;

    /// Initialize the handler (connect, setup, etc.)
    fn init(&mut self) -> Result<()>;

    /// Switch to a player context
    fn switch_player(&mut self, player: &str) -> Result<()>;

    /// Evaluate a MOO expression
    fn eval(&mut self, expr: &str) -> Result<EvalResult>;

    /// Execute a command (as opposed to eval)
    fn command(&mut self, cmd: &str) -> Result<EvalResult>;

    /// Get a binding value by name
    fn get_binding(&self, name: &str) -> Option<String>;

    /// Set a binding value
    fn set_binding(&mut self, name: &str, value: String);

    /// Clear all bindings (for reset)
    fn clear_bindings(&mut self);

    /// Shutdown the handler
    fn shutdown(&mut self) -> Result<()>;
}

/// Registry of available handlers
pub struct HandlerRegistry {
    handlers: std::collections::HashMap<String, Box<dyn Handler>>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, handler: Box<dyn Handler>) {
        let name = handler.name().to_string();
        self.handlers.insert(name, handler);
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Box<dyn Handler>> {
        self.handlers.get_mut(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 2: Update lib.rs**

```rust
pub mod handler;
pub use handler::{Handler, HandlerRegistry, EvalResult};
```

**Step 3: Verify it compiles**

Run: `cargo check -p moor-mdmoot`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/testing/mdmoot/src/handler.rs
git commit -m "feat(mdmoot): define Handler trait for implementation abstraction"
```

---

### Task 8: Implement test executor

**Files:**
- Create: `crates/testing/mdmoot/src/executor.rs`

**Step 1: Write executor**

```rust
//! Test execution engine

use crate::ast::*;
use crate::handler::{Handler, EvalResult};
use eyre::Result;
use std::collections::HashMap;

/// Result of running a spec
#[derive(Debug)]
pub struct SpecResult {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub results: Vec<BlockResult>,
}

/// Result of running a single block
#[derive(Debug)]
pub struct BlockResult {
    pub block_type: String,
    pub section: String,
    pub passed: bool,
    pub details: Vec<AssertionResult>,
}

/// Result of a single assertion
#[derive(Debug)]
pub struct AssertionResult {
    pub line_no: usize,
    pub expr: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
    pub impl_name: String,
}

/// Executor for running specs
pub struct Executor<'a> {
    handler: &'a mut dyn Handler,
    impl_name: String,
    scopes: Vec<HashMap<String, String>>,
}

impl<'a> Executor<'a> {
    pub fn new(handler: &'a mut dyn Handler) -> Self {
        Self {
            impl_name: handler.name().to_string(),
            handler,
            scopes: vec![HashMap::new()],
        }
    }

    /// Run a spec and return results
    pub fn run(&mut self, spec: &Spec) -> Result<SpecResult> {
        let mut results = vec![];
        let mut passed = 0;
        let mut failed = 0;

        for section in &spec.sections {
            let section_results = self.run_section(section)?;
            for result in section_results {
                if result.passed {
                    passed += 1;
                } else {
                    failed += 1;
                }
                results.push(result);
            }
        }

        Ok(SpecResult {
            passed,
            failed,
            skipped: 0,
            results,
        })
    }

    fn run_section(&mut self, section: &Section) -> Result<Vec<BlockResult>> {
        // Push scope for this section
        if section.isolated {
            self.handler.clear_bindings();
        }
        self.scopes.push(HashMap::new());

        let mut results = vec![];

        for block in &section.blocks {
            if let Some(result) = self.run_block(block, &section.title)? {
                results.push(result);
            }
        }

        // Pop scope
        self.scopes.pop();

        Ok(results)
    }

    fn run_block(&mut self, block: &Block, section_title: &str) -> Result<Option<BlockResult>> {
        match block {
            Block::Moot(moot) => self.run_moot_block(moot, section_title),
            Block::Moo(moo) => {
                self.run_moo_block(moo)?;
                Ok(None)
            }
            Block::DecisionTable(table) => self.run_decision_table(table, section_title),
            Block::ScriptTable(table) => self.run_script_table(table, section_title),
            Block::Prose(_) => Ok(None),
        }
    }

    fn run_moot_block(&mut self, block: &MootBlock, section: &str) -> Result<Option<BlockResult>> {
        let mut details = vec![];
        let mut all_passed = true;

        for item in &block.items {
            match item {
                MootItem::PlayerSwitch(player) => {
                    self.handler.switch_player(player)?;
                }
                MootItem::Eval { expr, expected, line_no } => {
                    let result = self.handler.eval(expr)?;
                    let expected_value = self.get_expected_for_impl(expected);
                    let passed = result.value == expected_value;

                    if !passed {
                        all_passed = false;
                    }

                    details.push(AssertionResult {
                        line_no: *line_no,
                        expr: expr.clone(),
                        expected: expected_value,
                        actual: result.value,
                        passed,
                        impl_name: self.impl_name.clone(),
                    });
                }
                MootItem::Comment(_) => {}
            }
        }

        Ok(Some(BlockResult {
            block_type: "moot".to_string(),
            section: section.to_string(),
            passed: all_passed,
            details,
        }))
    }

    fn run_moo_block(&mut self, block: &MooBlock) -> Result<()> {
        if block.reset {
            self.handler.clear_bindings();
            self.scopes.clear();
            self.scopes.push(HashMap::new());
        }

        if let Some(player) = &block.player {
            self.handler.switch_player(player)?;
        }

        // Execute the code
        let result = self.handler.eval(&block.code)?;

        // If there's a binding, store the result
        if let Some(bind_name) = &block.bind {
            if let Some(scope) = self.scopes.last_mut() {
                scope.insert(bind_name.clone(), result.value);
            }
        }

        Ok(())
    }

    fn run_decision_table(&mut self, table: &Table, section: &str) -> Result<Option<BlockResult>> {
        let mut details = vec![];
        let mut all_passed = true;

        for row in &table.rows {
            // Each row is independent - don't carry state
            let row_result = self.run_table_row(table, row, false)?;
            for assertion in row_result {
                if !assertion.passed {
                    all_passed = false;
                }
                details.push(assertion);
            }
        }

        Ok(Some(BlockResult {
            block_type: "decision_table".to_string(),
            section: section.to_string(),
            passed: all_passed,
            details,
        }))
    }

    fn run_script_table(&mut self, table: &Table, section: &str) -> Result<Option<BlockResult>> {
        let mut details = vec![];
        let mut all_passed = true;
        let mut prev_bindings: HashMap<String, String> = HashMap::new();

        for row in &table.rows {
            // Script tables carry state - pass previous bindings
            let row_result = self.run_table_row_with_carry(table, row, &mut prev_bindings)?;
            for assertion in row_result {
                if !assertion.passed {
                    all_passed = false;
                }
                details.push(assertion);
            }
        }

        Ok(Some(BlockResult {
            block_type: "script_table".to_string(),
            section: section.to_string(),
            passed: all_passed,
            details,
        }))
    }

    fn run_table_row(&mut self, table: &Table, row: &Row, _carry: bool) -> Result<Vec<AssertionResult>> {
        let mut results = vec![];
        let mut bindings: HashMap<String, String> = HashMap::new();

        // First pass: execute binding columns
        for (i, col) in table.columns.iter().enumerate() {
            if let ColumnKind::Binding { name, expr } = &col.kind {
                let cell = row.cells.get(i);
                let code = if let Some(cell) = cell {
                    if cell.empty {
                        expr.clone().unwrap_or_default()
                    } else {
                        cell.content.clone()
                    }
                } else {
                    expr.clone().unwrap_or_default()
                };

                if !code.is_empty() {
                    let result = self.handler.eval(&code)?;
                    bindings.insert(name.clone(), result.value);
                }
            }
        }

        // Second pass: execute output columns
        for (i, col) in table.columns.iter().enumerate() {
            if let ColumnKind::Output { template } = &col.kind {
                let cell = row.cells.get(i);
                if let Some(cell) = cell {
                    if let Some(expected) = &cell.expected {
                        // Substitute bindings in template
                        let code = self.substitute_bindings(template, &bindings);
                        let result = self.handler.eval(&code)?;
                        let expected_value = self.get_expected_for_impl(expected);
                        let passed = result.value == expected_value;

                        results.push(AssertionResult {
                            line_no: row.line_no,
                            expr: code,
                            expected: expected_value,
                            actual: result.value,
                            passed,
                            impl_name: self.impl_name.clone(),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    fn run_table_row_with_carry(
        &mut self,
        table: &Table,
        row: &Row,
        prev_bindings: &mut HashMap<String, String>,
    ) -> Result<Vec<AssertionResult>> {
        let mut results = vec![];

        // First pass: execute binding columns (use prev if empty)
        for (i, col) in table.columns.iter().enumerate() {
            if let ColumnKind::Binding { name, expr } = &col.kind {
                let cell = row.cells.get(i);
                let should_execute = cell.map(|c| !c.empty).unwrap_or(false);

                if should_execute {
                    let code = cell.unwrap().content.clone();
                    let code = if code.is_empty() {
                        expr.clone().unwrap_or_default()
                    } else {
                        code
                    };

                    if !code.is_empty() {
                        let result = self.handler.eval(&code)?;
                        prev_bindings.insert(name.clone(), result.value);
                    }
                }
                // If empty, prev_bindings already has the value
            }
        }

        // Second pass: execute output columns
        for (i, col) in table.columns.iter().enumerate() {
            if let ColumnKind::Output { template } = &col.kind {
                let cell = row.cells.get(i);
                if let Some(cell) = cell {
                    if let Some(expected) = &cell.expected {
                        let code = self.substitute_bindings(template, prev_bindings);
                        let result = self.handler.eval(&code)?;
                        let expected_value = self.get_expected_for_impl(expected);
                        let passed = result.value == expected_value;

                        results.push(AssertionResult {
                            line_no: row.line_no,
                            expr: code,
                            expected: expected_value,
                            actual: result.value,
                            passed,
                            impl_name: self.impl_name.clone(),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    fn get_expected_for_impl(&self, expected: &Expected) -> String {
        expected
            .divergences
            .get(&self.impl_name)
            .cloned()
            .unwrap_or_else(|| expected.value.clone())
    }

    fn substitute_bindings(&self, template: &str, bindings: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (name, value) in bindings {
            result = result.replace(name, value);
        }
        result
    }
}
```

**Step 2: Update lib.rs**

```rust
pub mod executor;
pub use executor::{Executor, SpecResult, BlockResult, AssertionResult};
```

**Step 3: Verify it compiles**

Run: `cargo check -p moor-mdmoot`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/testing/mdmoot/src/executor.rs
git commit -m "feat(mdmoot): implement test executor with table support"
```

---

## Phase 3: CLI

### Task 9: Create CLI binary

**Files:**
- Create: `crates/testing/mdmoot/src/bin/mdmoot.rs`
- Modify: `crates/testing/mdmoot/Cargo.toml`

**Step 1: Add CLI dependencies to Cargo.toml**

```toml
# Add to [dependencies]
clap = { version = "4", features = ["derive"] }

[[bin]]
name = "mdmoot"
path = "src/bin/mdmoot.rs"
```

**Step 2: Create CLI binary**

```rust
//! mdmoot CLI - Markdown-based MOO testing

use clap::{Parser, Subcommand};
use eyre::Result;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mdmoot")]
#[command(about = "Markdown-based MOO testing framework")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run tests
    Test {
        /// Spec file or directory to test
        path: Option<PathBuf>,

        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Implementations to compare (comma-separated)
        #[arg(long)]
        compare: Option<String>,

        /// Output format
        #[arg(long, default_value = "summary")]
        format: String,

        /// Output file (for html format)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Capture golden outputs
    Golden {
        /// Spec file or directory
        path: Option<PathBuf>,

        /// Implementation to capture from
        #[arg(long)]
        r#impl: Option<String>,
    },

    /// Start web server
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "8080")]
        port: u16,
    },

    /// Interactive REPL
    Repl {
        /// Implementations to compare
        #[arg(long)]
        compare: Option<String>,
    },

    /// Validate spec syntax
    Check {
        /// Path to check
        path: Option<PathBuf>,
    },

    /// Migrate from moot 1.0
    Migrate {
        /// Path to migrate
        path: Option<PathBuf>,

        /// Preview only, don't write files
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Test { path, tags, compare, format, output } => {
            cmd_test(path, tags, compare, format, output)
        }
        Commands::Golden { path, r#impl } => {
            cmd_golden(path, r#impl)
        }
        Commands::Serve { port } => {
            cmd_serve(port)
        }
        Commands::Repl { compare } => {
            cmd_repl(compare)
        }
        Commands::Check { path } => {
            cmd_check(path)
        }
        Commands::Migrate { path, dry_run } => {
            cmd_migrate(path, dry_run)
        }
    }
}

fn cmd_test(
    path: Option<PathBuf>,
    tags: Option<String>,
    compare: Option<String>,
    format: String,
    output: Option<PathBuf>,
) -> Result<()> {
    println!("Running tests...");
    println!("  Path: {:?}", path);
    println!("  Tags: {:?}", tags);
    println!("  Compare: {:?}", compare);
    println!("  Format: {}", format);
    println!("  Output: {:?}", output);

    // TODO: Implement
    Ok(())
}

fn cmd_golden(path: Option<PathBuf>, impl_name: Option<String>) -> Result<()> {
    println!("Capturing golden outputs...");
    println!("  Path: {:?}", path);
    println!("  Impl: {:?}", impl_name);

    // TODO: Implement
    Ok(())
}

fn cmd_serve(port: u16) -> Result<()> {
    println!("Starting web server on port {}...", port);

    // TODO: Implement
    Ok(())
}

fn cmd_repl(compare: Option<String>) -> Result<()> {
    println!("Starting REPL...");
    println!("  Compare: {:?}", compare);

    // TODO: Implement
    Ok(())
}

fn cmd_check(path: Option<PathBuf>) -> Result<()> {
    use moor_mdmoot::parse_spec;
    use std::fs;

    let path = path.unwrap_or_else(|| PathBuf::from("."));

    let specs = if path.is_file() {
        vec![path]
    } else {
        find_specs(&path)?
    };

    let mut errors = 0;
    for spec_path in &specs {
        let content = fs::read_to_string(spec_path)?;
        match parse_spec(&content) {
            Ok(_) => println!("✓ {}", spec_path.display()),
            Err(e) => {
                println!("✗ {}: {}", spec_path.display(), e);
                errors += 1;
            }
        }
    }

    if errors > 0 {
        println!("\n{} error(s) found", errors);
        std::process::exit(1);
    } else {
        println!("\nAll {} specs valid", specs.len());
    }

    Ok(())
}

fn cmd_migrate(path: Option<PathBuf>, dry_run: bool) -> Result<()> {
    println!("Migrating from moot 1.0...");
    println!("  Path: {:?}", path);
    println!("  Dry run: {}", dry_run);

    // TODO: Implement
    Ok(())
}

fn find_specs(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut specs = vec![];

    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry?;
        if entry.path().extension().map(|e| e == "md").unwrap_or(false) {
            if entry.path().to_string_lossy().ends_with(".spec.md") {
                specs.push(entry.path().to_path_buf());
            }
        }
    }

    Ok(specs)
}
```

**Step 3: Add walkdir dependency**

```toml
walkdir = "2"
```

**Step 4: Verify it builds**

Run: `cargo build -p moor-mdmoot --bin mdmoot`
Expected: PASS

**Step 5: Test help output**

Run: `cargo run -p moor-mdmoot --bin mdmoot -- --help`
Expected: Shows CLI help

**Step 6: Commit**

```bash
git add crates/testing/mdmoot/
git commit -m "feat(mdmoot): add CLI binary with subcommands"
```

---

## Phase 4: Migration Tool

### Task 10: Implement moot 1.0 migrator

**Files:**
- Create: `crates/testing/mdmoot/src/migrate.rs`

**Step 1: Write migrator**

```rust
//! Migration from moot 1.0 to mdmoot

use eyre::Result;
use std::path::Path;

/// Migrate a .moot file to .spec.md format
pub fn migrate_file(moot_content: &str, source_path: &Path) -> Result<String> {
    let mut output = String::new();

    // Add frontmatter with inferred tags
    let tags = infer_tags(source_path);
    output.push_str("---\n");
    if !tags.is_empty() {
        output.push_str(&format!("tags: [{}]\n", tags.join(", ")));
    }
    output.push_str("---\n\n");

    // Add title from filename
    let title = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Test");
    output.push_str(&format!("# {}\n\n", title_case(title)));

    // Parse and convert blocks
    let blocks = parse_moot_blocks(moot_content);
    let grouped = group_similar_blocks(&blocks);

    for group in grouped {
        output.push_str(&convert_group_to_mdmoot(&group)?);
        output.push_str("\n");
    }

    Ok(output)
}

/// Infer tags from file path
fn infer_tags(path: &Path) -> Vec<String> {
    let mut tags = vec![];

    // Use parent directory names as tags
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            if let Some(s) = name.to_str() {
                if s != "moot" && s != "testsuite" && s != "tests" {
                    tags.push(s.to_string());
                }
            }
        }
    }

    // Remove the filename from tags
    tags.pop();

    tags
}

fn title_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone)]
enum MootBlock {
    Comment(String),
    PlayerSwitch(String),
    Eval { expr: String, expected: String },
    Command { cmd: String, expected: String },
    EvalBg { expr: String },
}

fn parse_moot_blocks(content: &str) -> Vec<MootBlock> {
    let mut blocks = vec![];
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if line.starts_with("//") {
            blocks.push(MootBlock::Comment(line[2..].trim().to_string()));
        } else if line.starts_with('@') {
            blocks.push(MootBlock::PlayerSwitch(line[1..].to_string()));
        } else if line.starts_with(';') {
            // Eval - collect continuation lines and expected
            let mut expr = line[1..].trim().to_string();

            // Collect continuation lines (starting with >)
            while lines.peek().map(|l| l.trim().starts_with('>')).unwrap_or(false) {
                let cont = lines.next().unwrap().trim();
                expr.push('\n');
                expr.push_str(&cont[1..].trim());
            }

            // Collect expected output
            let mut expected = String::new();
            while let Some(next) = lines.peek() {
                let next = next.trim();
                if next.is_empty() || next.starts_with(';') || next.starts_with('%')
                    || next.starts_with('@') || next.starts_with('&') || next.starts_with("//") {
                    break;
                }
                if !expected.is_empty() {
                    expected.push('\n');
                }
                let exp_line = lines.next().unwrap().trim();
                // Handle < prefix
                if exp_line.starts_with('<') {
                    expected.push_str(exp_line[1..].trim());
                } else {
                    expected.push_str(exp_line);
                }
            }

            blocks.push(MootBlock::Eval { expr, expected });
        } else if line.starts_with('%') {
            // Command
            let mut cmd = line[1..].trim().to_string();

            // Collect continuation lines
            while lines.peek().map(|l| l.trim().starts_with('>')).unwrap_or(false) {
                let cont = lines.next().unwrap().trim();
                cmd.push('\n');
                cmd.push_str(&cont[1..].trim());
            }

            // Collect expected output (same as eval)
            let mut expected = String::new();
            while let Some(next) = lines.peek() {
                let next = next.trim();
                if next.is_empty() || next.starts_with(';') || next.starts_with('%')
                    || next.starts_with('@') || next.starts_with('&') || next.starts_with("//") {
                    break;
                }
                if !expected.is_empty() {
                    expected.push('\n');
                }
                expected.push_str(lines.next().unwrap().trim());
            }

            blocks.push(MootBlock::Command { cmd, expected });
        } else if line.starts_with('&') {
            blocks.push(MootBlock::EvalBg { expr: line[1..].trim().to_string() });
        }
    }

    blocks
}

#[derive(Debug)]
enum BlockGroup {
    /// Sequential eval/command blocks as REPL
    Repl(Vec<MootBlock>),
    /// Similar pattern blocks as table
    Table {
        pattern: TablePattern,
        rows: Vec<(String, String)>, // (input, expected)
    },
    /// Comments become prose
    Prose(String),
}

#[derive(Debug, Clone)]
enum TablePattern {
    TypeOf,
    Conversion(String), // tostr, toint, etc.
    BinaryOp(String),   // +, -, *, /, etc.
    Builtin(String),    // length, index, etc.
    Generic,
}

fn group_similar_blocks(blocks: &[MootBlock]) -> Vec<BlockGroup> {
    let mut groups = vec![];
    let mut current_repl: Vec<MootBlock> = vec![];

    for block in blocks {
        match block {
            MootBlock::Comment(c) => {
                if !current_repl.is_empty() {
                    groups.push(BlockGroup::Repl(std::mem::take(&mut current_repl)));
                }
                groups.push(BlockGroup::Prose(c.clone()));
            }
            _ => {
                current_repl.push(block.clone());
            }
        }
    }

    if !current_repl.is_empty() {
        groups.push(BlockGroup::Repl(current_repl));
    }

    // TODO: Detect tabular patterns and convert to Table groups
    groups
}

fn convert_group_to_mdmoot(group: &BlockGroup) -> Result<String> {
    match group {
        BlockGroup::Repl(blocks) => {
            let mut output = String::new();
            output.push_str("```moot\n");

            for block in blocks {
                match block {
                    MootBlock::PlayerSwitch(player) => {
                        output.push_str(&format!("@{}\n", player));
                    }
                    MootBlock::Eval { expr, expected } => {
                        for line in expr.lines() {
                            output.push_str(&format!("> {}\n", line));
                        }
                        output.push_str(&format!("{}\n", expected));
                    }
                    MootBlock::Command { cmd, expected } => {
                        output.push_str(&format!("> {}\n", cmd));
                        output.push_str(&format!("{}\n", expected));
                    }
                    MootBlock::EvalBg { expr } => {
                        output.push_str(&format!("> {} # background\n", expr));
                    }
                    MootBlock::Comment(_) => {} // Handled separately
                }
            }

            output.push_str("```\n");
            Ok(output)
        }
        BlockGroup::Table { pattern: _, rows } => {
            let mut output = String::new();
            output.push_str("| input | expected |\n");
            output.push_str("|-------|----------|\n");
            for (input, expected) in rows {
                output.push_str(&format!("| `{}` | `{}` |\n", input, expected));
            }
            Ok(output)
        }
        BlockGroup::Prose(text) => {
            Ok(format!("{}\n", text))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_eval() {
        let content = "; return 1 + 1;\n2";
        let blocks = parse_moot_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], MootBlock::Eval { expr, expected }
            if expr == "return 1 + 1;" && expected == "2"));
    }

    #[test]
    fn test_parse_player_switch() {
        let content = "@wizard\n; return 1;\n1";
        let blocks = parse_moot_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], MootBlock::PlayerSwitch(p) if p == "wizard"));
    }

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("basic_arithmetic"), "Basic Arithmetic");
        assert_eq!(title_case("test_create"), "Test Create");
    }
}
```

**Step 2: Wire up CLI migrate command**

Update `cmd_migrate` in the CLI to use the migrator.

**Step 3: Run tests**

Run: `cargo test -p moor-mdmoot migrate::tests`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/testing/mdmoot/src/migrate.rs
git commit -m "feat(mdmoot): implement moot 1.0 migration tool"
```

---

## Phase 5: Integration

### Task 11: Create in-process moor handler

**Files:**
- Create: `crates/testing/mdmoot/src/handlers/mod.rs`
- Create: `crates/testing/mdmoot/src/handlers/moor.rs`

This task requires integrating with the existing moor kernel and will depend on the in-memory MoorClient work.

### Task 12: End-to-end test with sample spec

Create a sample .spec.md file and verify the full pipeline works.

### Task 13: Web server implementation

Implement the axum-based web server for viewing, editing, and running specs.

---

## Notes

- **Phase 1** establishes the parsing foundation - can be done independently
- **Phase 2** builds the execution engine - requires handlers to be stubbed
- **Phase 3** creates the CLI shell - mostly scaffolding initially
- **Phase 4** migration can be tested with existing .moot files
- **Phase 5** brings everything together with real moor integration

Dependencies:
- moor-moot (for MootRunner trait if reusing)
- moor-var (for value types)
- pulldown-cmark (markdown)
- serde + toml + serde_yaml (config)
- clap (CLI)
- axum (web server - Phase 5)
- walkdir (file discovery)
