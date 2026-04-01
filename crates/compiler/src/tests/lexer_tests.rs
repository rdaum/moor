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

use crate::{SyntaxKind, lex};

fn kinds(source: &str) -> Vec<SyntaxKind> {
    lex(source).into_iter().map(|token| token.kind).collect()
}

#[test]
fn keyword_prefix_identifiers_stay_identifiers() {
    assert_eq!(
        kinds("in0 for1 if_var begin end"),
        vec![
            SyntaxKind::Ident,
            SyntaxKind::Whitespace,
            SyntaxKind::Ident,
            SyntaxKind::Whitespace,
            SyntaxKind::Ident,
            SyntaxKind::Whitespace,
            SyntaxKind::Ident,
            SyntaxKind::Whitespace,
            SyntaxKind::Ident,
            SyntaxKind::Eof,
        ]
    );
}

#[test]
fn begin_and_end_are_not_reserved_keywords() {
    assert_eq!(
        kinds("begin = end;"),
        vec![
            SyntaxKind::Ident,
            SyntaxKind::Whitespace,
            SyntaxKind::Eq,
            SyntaxKind::Whitespace,
            SyntaxKind::Ident,
            SyntaxKind::Semi,
            SyntaxKind::Eof,
        ]
    );
}

#[test]
fn preserves_comment_and_newline_trivia() {
    assert_eq!(
        kinds("// hi\n/* lo */\nfoo"),
        vec![
            SyntaxKind::LineComment,
            SyntaxKind::Newline,
            SyntaxKind::BlockComment,
            SyntaxKind::Newline,
            SyntaxKind::Ident,
            SyntaxKind::Eof,
        ]
    );
}

#[test]
fn malformed_string_becomes_error_token_without_panicking() {
    assert_eq!(
        kinds("\"unterminated"),
        vec![SyntaxKind::Error, SyntaxKind::Eof]
    );
}

#[test]
fn malformed_block_comment_becomes_error_token() {
    assert_eq!(kinds("/* nope"), vec![SyntaxKind::Error, SyntaxKind::Eof]);
}

#[test]
fn malformed_binary_literal_becomes_error_token() {
    assert_eq!(
        kinds("b\"%%%\""),
        vec![SyntaxKind::Error, SyntaxKind::Eof]
    );
}

#[test]
fn longest_match_operators_are_lexed_correctly() {
    assert_eq!(
        kinds(">>> >> >= == => -> .."),
        vec![
            SyntaxKind::LShr,
            SyntaxKind::Whitespace,
            SyntaxKind::Shr,
            SyntaxKind::Whitespace,
            SyntaxKind::GtEq,
            SyntaxKind::Whitespace,
            SyntaxKind::EqEq,
            SyntaxKind::Whitespace,
            SyntaxKind::FatArrow,
            SyntaxKind::Whitespace,
            SyntaxKind::Arrow,
            SyntaxKind::Whitespace,
            SyntaxKind::DotDot,
            SyntaxKind::Eof,
        ]
    );
}

#[test]
fn signed_numbers_only_attach_when_an_expression_can_start() {
    assert_eq!(
        kinds("a-2; a = -2; (-2); .5; a+.5;"),
        vec![
            SyntaxKind::Ident,
            SyntaxKind::Minus,
            SyntaxKind::IntLit,
            SyntaxKind::Semi,
            SyntaxKind::Whitespace,
            SyntaxKind::Ident,
            SyntaxKind::Whitespace,
            SyntaxKind::Eq,
            SyntaxKind::Whitespace,
            SyntaxKind::IntLit,
            SyntaxKind::Semi,
            SyntaxKind::Whitespace,
            SyntaxKind::LParen,
            SyntaxKind::IntLit,
            SyntaxKind::RParen,
            SyntaxKind::Semi,
            SyntaxKind::Whitespace,
            SyntaxKind::FloatLit,
            SyntaxKind::Semi,
            SyntaxKind::Whitespace,
            SyntaxKind::Ident,
            SyntaxKind::Plus,
            SyntaxKind::FloatLit,
            SyntaxKind::Semi,
            SyntaxKind::Eof,
        ]
    );
}

#[test]
fn recognizes_type_constants_error_literals_symbols_and_objects() {
    assert_eq!(
        kinds("TYPE_INT E_PERM 'foo #0 #-1 #abcdef-0123456789 #anon_abcdef-0123456789"),
        vec![
            SyntaxKind::TypeConstant,
            SyntaxKind::Whitespace,
            SyntaxKind::ErrorLit,
            SyntaxKind::Whitespace,
            SyntaxKind::SymbolLit,
            SyntaxKind::Whitespace,
            SyntaxKind::ObjectLit,
            SyntaxKind::Whitespace,
            SyntaxKind::ObjectLit,
            SyntaxKind::Whitespace,
            SyntaxKind::ObjectLit,
            SyntaxKind::Whitespace,
            SyntaxKind::ObjectLit,
            SyntaxKind::Eof,
        ]
    );
}
