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
