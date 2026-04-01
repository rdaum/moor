// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use rowan::{Checkpoint, GreenNode, GreenNodeBuilder};

use crate::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MooLanguage {}

impl rowan::Language for MooLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        if raw.0 <= SyntaxKind::ConstantDecl as u16 {
            // SAFETY: SyntaxKind is repr(u16) and the range is checked above.
            unsafe { std::mem::transmute::<u16, SyntaxKind>(raw.0) }
        } else {
            SyntaxKind::Error
        }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

pub type SyntaxNode = rowan::SyntaxNode<MooLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<MooLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<MooLanguage>;

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(value: SyntaxKind) -> Self {
        rowan::SyntaxKind(value as u16)
    }
}

pub struct CstBuilder {
    inner: GreenNodeBuilder<'static>,
}

impl Default for CstBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CstBuilder {
    pub fn new() -> Self {
        Self {
            inner: GreenNodeBuilder::new(),
        }
    }

    pub fn start_node(&mut self, kind: SyntaxKind) {
        self.inner.start_node(kind.into());
    }

    pub fn checkpoint(&self) -> Checkpoint {
        self.inner.checkpoint()
    }

    pub fn start_node_at(&mut self, checkpoint: Checkpoint, kind: SyntaxKind) {
        self.inner.start_node_at(checkpoint, kind.into());
    }

    pub fn token(&mut self, kind: SyntaxKind, text: &str) {
        self.inner.token(kind.into(), text);
    }

    pub fn finish_node(&mut self) {
        self.inner.finish_node();
    }

    pub fn finish(self) -> GreenNode {
        self.inner.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{CstBuilder, SyntaxNode};
    use crate::SyntaxKind;

    #[test]
    fn cst_builder_creates_typed_root() {
        let mut builder = CstBuilder::new();
        builder.start_node(SyntaxKind::Program);
        builder.token(SyntaxKind::Ident, "foo");
        builder.finish_node();
        let green = builder.finish();
        let root = SyntaxNode::new_root(green);
        assert_eq!(root.kind(), SyntaxKind::Program);
        let child = root.first_child_or_token().unwrap();
        assert_eq!(child.kind(), SyntaxKind::Ident);
    }
}
