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

use std::ops::Range;

use crate::{SyntaxKind, Token};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Range<usize>,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Range<usize>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

#[derive(Debug)]
pub struct TokenCursor<'a> {
    source: &'a str,
    tokens: &'a [Token],
    pos: usize,
    errors: Vec<ParseError>,
}

impl<'a> TokenCursor<'a> {
    pub fn new(source: &'a str, tokens: &'a [Token]) -> Self {
        Self {
            source,
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    pub fn source(&self) -> &'a str {
        self.source
    }

    pub fn current(&self) -> &'a Token {
        self.nth_non_trivia(0)
    }

    pub fn current_kind(&self) -> SyntaxKind {
        self.current().kind
    }

    pub fn current_text(&self) -> &'a str {
        let token = self.current();
        &self.source[token.span.clone()]
    }

    pub fn current_span(&self) -> Range<usize> {
        self.current().span.clone()
    }

    pub fn raw_index(&self) -> usize {
        self.pos
    }

    pub fn current_raw_index(&self) -> usize {
        let mut idx = self.pos;
        loop {
            let token = self
                .tokens
                .get(idx)
                .or_else(|| self.tokens.last())
                .expect("lexer must always provide EOF token");
            if !token.kind.is_trivia() {
                return idx;
            }
            if token.kind == SyntaxKind::Eof {
                return idx;
            }
            idx += 1;
        }
    }

    pub fn nth_kind(&self, n: usize) -> SyntaxKind {
        self.nth_non_trivia(n).kind
    }

    pub fn at(&self, kind: SyntaxKind) -> bool {
        self.current_kind() == kind
    }

    pub fn bump(&mut self) -> &'a Token {
        self.bump_trivia();
        let token = self.raw_current();
        if token.kind != SyntaxKind::Eof {
            self.pos += 1;
        }
        token
    }

    pub fn bump_if(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.bump();
            return true;
        }
        false
    }

    pub fn expect(&mut self, kind: SyntaxKind, message: impl Into<String>) -> bool {
        if self.bump_if(kind) {
            return true;
        }
        self.push_error(message);
        false
    }

    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    pub fn into_errors(self) -> Vec<ParseError> {
        self.errors
    }

    pub fn push_error(&mut self, message: impl Into<String>) {
        self.errors
            .push(ParseError::new(message, self.current_span()));
    }

    pub fn bump_trivia(&mut self) {
        while self.raw_current().kind.is_trivia() {
            self.pos += 1;
        }
    }

    fn raw_current(&self) -> &'a Token {
        self.tokens
            .get(self.pos)
            .or_else(|| self.tokens.last())
            .expect("lexer must always provide EOF token")
    }

    fn nth_non_trivia(&self, n: usize) -> &'a Token {
        let mut idx = self.pos;
        let mut seen = 0usize;
        loop {
            let token = self
                .tokens
                .get(idx)
                .or_else(|| self.tokens.last())
                .expect("lexer must always provide EOF token");
            if !token.kind.is_trivia() {
                if seen == n {
                    return token;
                }
                seen += 1;
            }
            if token.kind == SyntaxKind::Eof {
                return token;
            }
            idx += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TokenCursor;
    use crate::{SyntaxKind, lex};

    #[test]
    fn cursor_skips_trivia_for_current_token() {
        let source = " \nfoo";
        let tokens = lex(source);
        let cursor = TokenCursor::new(source, &tokens);
        assert_eq!(cursor.current_kind(), SyntaxKind::Ident);
        assert_eq!(cursor.current_text(), "foo");
    }

    #[test]
    fn cursor_bump_and_lookahead_ignore_trivia() {
        let source = "foo  +  bar";
        let tokens = lex(source);
        let mut cursor = TokenCursor::new(source, &tokens);
        assert_eq!(cursor.current_kind(), SyntaxKind::Ident);
        assert_eq!(cursor.nth_kind(1), SyntaxKind::Plus);
        cursor.bump();
        assert_eq!(cursor.current_kind(), SyntaxKind::Plus);
        cursor.bump();
        assert_eq!(cursor.current_kind(), SyntaxKind::Ident);
    }

    #[test]
    fn cursor_expect_records_errors_at_current_token() {
        let source = "foo";
        let tokens = lex(source);
        let mut cursor = TokenCursor::new(source, &tokens);
        assert!(!cursor.expect(SyntaxKind::IfKw, "expected if"));
        assert_eq!(cursor.errors().len(), 1);
        assert_eq!(cursor.errors()[0].span, 0..3);
    }
}
