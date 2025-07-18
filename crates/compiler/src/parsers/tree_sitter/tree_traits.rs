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

//! Generic tree traversal traits that allow both PEST/CST and tree-sitter
//! to share common parsing logic.

/// Generic trait for tree nodes that can be traversed during parsing.
/// This provides a common interface for both PEST CST nodes and tree-sitter nodes.
pub trait TreeNode {
    /// Get the kind/type name of this node
    fn kind_name(&self) -> &str;

    /// Get the number of children this node has
    fn child_count(&self) -> usize;

    /// Get a child by index
    fn child(&self, index: usize) -> Option<Self>
    where
        Self: Sized;

    /// Get a child by field name (for tree-sitter style named fields)
    fn child_by_field_name(&self, name: &str) -> Option<Self>
    where
        Self: Sized;

    /// Get the text content of this node (if it's a terminal)
    fn text(&self) -> Option<&str>;

    /// Get the start position (line, column) of this node
    fn start_position(&self) -> (usize, usize);

    /// Get the end position (line, column) of this node
    fn end_position(&self) -> (usize, usize);

    /// Get the start byte offset of this node
    fn start_byte(&self) -> usize;

    /// Get the end byte offset of this node
    fn end_byte(&self) -> usize;
}

// Implementation for tree-sitter nodes
#[cfg(feature = "tree-sitter-parser")]
impl<'a> TreeNode for tree_sitter::Node<'a> {
    fn kind_name(&self) -> &str {
        self.kind()
    }

    fn child_count(&self) -> usize {
        self.child_count()
    }

    fn child(&self, index: usize) -> Option<Self> {
        self.child(index)
    }

    fn child_by_field_name(&self, name: &str) -> Option<Self> {
        self.child_by_field_name(name)
    }

    fn text(&self) -> Option<&str> {
        // Tree-sitter nodes don't directly contain text
        // The text must be extracted from the source using byte ranges
        None
    }

    fn start_position(&self) -> (usize, usize) {
        let pos = self.start_position();
        (pos.row, pos.column)
    }

    fn end_position(&self) -> (usize, usize) {
        let pos = self.end_position();
        (pos.row, pos.column)
    }

    fn start_byte(&self) -> usize {
        self.start_byte()
    }

    fn end_byte(&self) -> usize {
        self.end_byte()
    }
}
