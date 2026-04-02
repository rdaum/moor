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

use moor_common::util::unquote_str;
use moor_var::v_binary;

use crate::{CompileOptions, parse_program_frontend, unparse::to_literal};

#[test]
fn string_unquote_standard_escapes() {
    assert_eq!(unquote_str(r#""foo""#).unwrap(), "foo");
    assert_eq!(unquote_str(r#""foo\"bar""#).unwrap(), r#"foo"bar"#);
    assert_eq!(unquote_str(r#""foo\\bar""#).unwrap(), r"foo\bar");
    assert_eq!(unquote_str(r#""hello\nworld""#).unwrap(), "hello\nworld");
    assert_eq!(unquote_str(r#""hello\tworld""#).unwrap(), "hello\tworld");
    assert_eq!(unquote_str(r#""hello\rworld""#).unwrap(), "hello\rworld");
    assert_eq!(unquote_str(r#""hello\0world""#).unwrap(), "hello\0world");
    assert_eq!(unquote_str(r#""hello\'world""#).unwrap(), "hello'world");
}

#[test]
fn string_unquote_hex_and_unicode_escapes() {
    assert_eq!(unquote_str(r#""A is \x41""#).unwrap(), "A is A");
    assert_eq!(unquote_str(r#""\x48\x65\x6C\x6C\x6F""#).unwrap(), "Hello");
    assert_eq!(unquote_str(r#""\x00\xFF""#).unwrap(), "\0\u{FF}");
    assert_eq!(unquote_str(r#""\x4a\x4A""#).unwrap(), "JJ");

    assert_eq!(unquote_str(r#""Hello \u0041""#).unwrap(), "Hello A");
    assert_eq!(
        unquote_str(r#""\u0048\u0065\u006C\u006C\u006F""#).unwrap(),
        "Hello"
    );
    assert_eq!(unquote_str(r#""Smile: \u263A""#).unwrap(), "Smile: ☺");
}

#[test]
fn string_unquote_reports_malformed_escapes() {
    assert!(unquote_str(r#""\x""#).is_err());
    assert!(unquote_str(r#""\x4""#).is_err());
    assert!(unquote_str(r#""\xGG""#).is_err());
    assert!(unquote_str(r#""\x4G""#).is_err());
    assert!(unquote_str(r#""\u""#).is_err());
    assert!(unquote_str(r#""\u123""#).is_err());
    assert!(unquote_str(r#""\uGGGG""#).is_err());
    assert!(unquote_str(r#""\u123G""#).is_err());
}

#[test]
fn string_unquote_preserves_backward_compatibility() {
    assert_eq!(unquote_str(r#""foo\bbar""#).unwrap(), "foobbar");
    assert_eq!(unquote_str(r#""foo\fbar""#).unwrap(), "foofbar");
    assert_eq!(unquote_str(r#""foo\vbar""#).unwrap(), "foovbar");
    assert_eq!(unquote_str(r#""foo\zbar""#).unwrap(), "foozbar");
    assert_eq!(unquote_str(r#""foo\""#).unwrap(), "foo");
}

#[test]
fn parses_binary_literals_through_frontend() {
    let parsed =
        parse_program_frontend(r#"return b"SGVsbG8gV29ybGQ=";"#, CompileOptions::default())
            .unwrap();
    let stmt = &parsed.stmts[0].node;
    if let crate::ast::StmtNode::Expr(crate::ast::Expr::Return(Some(expr))) = stmt
        && let crate::ast::Expr::Value(val) = expr.as_ref()
        && let Some(binary) = val.as_binary()
    {
        assert_eq!(binary.as_bytes(), b"Hello World");
    } else {
        panic!("expected binary literal return, got {stmt:?}");
    }
}

#[test]
fn parses_empty_binary_literals_through_frontend() {
    let parsed = parse_program_frontend(r#"return b"";"#, CompileOptions::default()).unwrap();
    let stmt = &parsed.stmts[0].node;
    if let crate::ast::StmtNode::Expr(crate::ast::Expr::Return(Some(expr))) = stmt
        && let crate::ast::Expr::Value(val) = expr.as_ref()
        && let Some(binary) = val.as_binary()
    {
        assert_eq!(binary.as_bytes(), b"");
    } else {
        panic!("expected empty binary literal return, got {stmt:?}");
    }
}

#[test]
fn rejects_invalid_binary_literal_base64() {
    let result = parse_program_frontend(r#"return b"SGVsbG8gV29ybGQ";"#, CompileOptions::default());
    assert!(result.is_err());
    let error = result.unwrap_err().to_string();
    assert!(error.contains("invalid base64") || error.contains("binary literal"));
}

#[test]
fn binary_literal_roundtrips_through_literal_formatter() {
    let original_data = b"Hello, World! This is binary data.";
    let binary_var = v_binary(original_data.to_vec());
    let literal_str = to_literal(&binary_var);
    let program = format!("return {literal_str};");
    let parsed = parse_program_frontend(&program, CompileOptions::default()).unwrap();

    if let crate::ast::StmtNode::Expr(crate::ast::Expr::Return(Some(expr))) = &parsed.stmts[0].node
        && let crate::ast::Expr::Value(val) = expr.as_ref()
        && let Some(binary) = val.as_binary()
    {
        assert_eq!(binary.as_bytes(), original_data);
    } else {
        panic!("expected roundtripped binary literal");
    }
}
