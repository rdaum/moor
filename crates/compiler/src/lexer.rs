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

//! Handwritten lexer for MOO source.

use std::ops::Range;

use crate::syntax_kind::SyntaxKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub span: Range<usize>,
}

impl Token {
    fn new(kind: SyntaxKind, start: usize, end: usize) -> Self {
        Self {
            kind,
            span: start..end,
        }
    }
}

pub fn lex(source: &str) -> Vec<Token> {
    Lexer::new(source).lex()
}

struct Lexer<'a> {
    source: &'a str,
    pos: usize,
    tokens: Vec<Token>,
    prev_significant: Option<SyntaxKind>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            pos: 0,
            tokens: Vec::new(),
            prev_significant: None,
        }
    }

    fn lex(mut self) -> Vec<Token> {
        while let Some(ch) = self.peek() {
            let start = self.pos;
            let kind = match ch {
                ' ' | '\t' | '\u{00A0}' => {
                    self.consume_horizontal_whitespace();
                    SyntaxKind::Whitespace
                }
                '\n' | '\r' => {
                    self.consume_newline();
                    SyntaxKind::Newline
                }
                '/' if self.peek_next() == Some('/') => {
                    self.bump();
                    self.bump();
                    self.consume_line_comment_body();
                    SyntaxKind::LineComment
                }
                '/' if self.peek_next() == Some('*') => self.lex_block_comment(),
                '"' => self.lex_string(),
                'b' if self.peek_next() == Some('"') => self.lex_binary_literal(),
                '\'' => self.lex_symbol_or_apostrophe(),
                '#' => self.lex_object_or_hash(),
                '+' if self.can_start_signed_number() => self.lex_signed_number_or_operator(),
                '.' if self.can_start_leading_dot_float() => self.lex_number(),
                '0'..='9' => self.lex_number(),
                '(' => {
                    self.bump();
                    SyntaxKind::LParen
                }
                ')' => {
                    self.bump();
                    SyntaxKind::RParen
                }
                '{' => {
                    self.bump();
                    SyntaxKind::LBrace
                }
                '}' => {
                    self.bump();
                    SyntaxKind::RBrace
                }
                '[' => {
                    self.bump();
                    SyntaxKind::LBracket
                }
                ']' => {
                    self.bump();
                    SyntaxKind::RBracket
                }
                ';' => {
                    self.bump();
                    SyntaxKind::Semi
                }
                ',' => {
                    self.bump();
                    SyntaxKind::Comma
                }
                ':' => {
                    self.bump();
                    SyntaxKind::Colon
                }
                '@' => {
                    self.bump();
                    SyntaxKind::At
                }
                '$' => {
                    self.bump();
                    SyntaxKind::Dollar
                }
                '`' => {
                    self.bump();
                    SyntaxKind::Backtick
                }
                '?' => {
                    self.bump();
                    SyntaxKind::Question
                }
                '~' => {
                    self.bump();
                    SyntaxKind::Tilde
                }
                '^' if self.peek_next() == Some('.') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::CaretDot
                }
                '^' => {
                    self.bump();
                    SyntaxKind::Caret
                }
                '&' if self.peek_next() == Some('&') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::AmpAmp
                }
                '&' if self.peek_next() == Some('.') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::AmpDot
                }
                '|' if self.peek_next() == Some('|') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::PipePipe
                }
                '|' if self.peek_next() == Some('.') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::PipeDot
                }
                '|' => {
                    self.bump();
                    SyntaxKind::Pipe
                }
                '!' if self.peek_next() == Some('=') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::BangEq
                }
                '!' => {
                    self.bump();
                    SyntaxKind::Bang
                }
                '=' if self.peek_next() == Some('=') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::EqEq
                }
                '=' if self.peek_next() == Some('>') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::FatArrow
                }
                '=' => {
                    self.bump();
                    SyntaxKind::Eq
                }
                '-' if self.peek_next() == Some('>') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::Arrow
                }
                '-' => {
                    if self.can_start_signed_number() {
                        self.lex_signed_number_or_operator()
                    } else {
                        self.bump();
                        SyntaxKind::Minus
                    }
                }
                '+' => {
                    self.bump();
                    SyntaxKind::Plus
                }
                '*' => {
                    self.bump();
                    SyntaxKind::Star
                }
                '/' => {
                    self.bump();
                    SyntaxKind::Slash
                }
                '%' => {
                    self.bump();
                    SyntaxKind::Percent
                }
                '<' if self.peek_next() == Some('<') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::Shl
                }
                '<' if self.peek_next() == Some('=') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::LtEq
                }
                '<' => {
                    self.bump();
                    SyntaxKind::Lt
                }
                '>' if self.matches(">>>") => {
                    self.bump();
                    self.bump();
                    self.bump();
                    SyntaxKind::LShr
                }
                '>' if self.peek_next() == Some('>') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::Shr
                }
                '>' if self.peek_next() == Some('=') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::GtEq
                }
                '>' => {
                    self.bump();
                    SyntaxKind::Gt
                }
                '.' if self.peek_next() == Some('.') => {
                    self.bump();
                    self.bump();
                    SyntaxKind::DotDot
                }
                '.' => {
                    self.bump();
                    SyntaxKind::Dot
                }
                '_' | 'A'..='Z' | 'a'..='z' => self.lex_ident_like(),
                _ => {
                    self.bump();
                    SyntaxKind::Error
                }
            };
            self.push_token(kind, start, self.pos);
        }

        let eof = self.pos;
        self.tokens.push(Token::new(SyntaxKind::Eof, eof, eof));
        self.tokens
    }

    fn push_token(&mut self, kind: SyntaxKind, start: usize, end: usize) {
        self.tokens.push(Token::new(kind, start, end));
        if !kind.is_trivia() && kind != SyntaxKind::Error {
            self.prev_significant = Some(kind);
        }
    }

    fn lex_ident_like(&mut self) -> SyntaxKind {
        let start = self.pos;
        self.bump();
        while let Some(ch) = self.peek() {
            if is_ident_continue(ch) {
                self.bump();
            } else {
                break;
            }
        }

        let text = &self.source[start..self.pos];
        keyword_kind(text)
            .or_else(|| type_constant_kind(text))
            .or_else(|| error_literal_kind(text))
            .unwrap_or(SyntaxKind::Ident)
    }

    fn lex_string(&mut self) -> SyntaxKind {
        self.bump();

        while let Some(ch) = self.peek() {
            match ch {
                '"' => {
                    self.bump();
                    return SyntaxKind::StringLit;
                }
                '\\' => {
                    self.bump();
                    match self.peek() {
                        Some('b' | 't' | 'n' | 'f' | 'r' | '"' | '\\' | '\n' | '\r') => {
                            self.bump();
                            if self.peek_prev_was('\r') && self.peek() == Some('\n') {
                                self.bump();
                            }
                        }
                        Some('x') => {
                            self.bump();
                            if !self.consume_hex_digits(2) {
                                self.consume_string_error_tail();
                                return SyntaxKind::Error;
                            }
                        }
                        Some('u') => {
                            self.bump();
                            if !self.consume_hex_digits(4) {
                                self.consume_string_error_tail();
                                return SyntaxKind::Error;
                            }
                        }
                        Some('U') => {
                            self.bump();
                            if !self.consume_hex_digits(8) {
                                self.consume_string_error_tail();
                                return SyntaxKind::Error;
                            }
                        }
                        Some(_) | None => {
                            self.consume_string_error_tail();
                            return SyntaxKind::Error;
                        }
                    }
                }
                '\u{0000}'..='\u{001F}' => {
                    self.consume_string_error_tail();
                    return SyntaxKind::Error;
                }
                _ => {
                    self.bump();
                }
            }
        }

        SyntaxKind::Error
    }

    fn lex_binary_literal(&mut self) -> SyntaxKind {
        self.bump();
        self.bump();

        while let Some(ch) = self.peek() {
            match ch {
                '"' => {
                    self.bump();
                    return SyntaxKind::BinaryLit;
                }
                'A'..='Z' | 'a'..='z' | '0'..='9' | '+' | '/' | '=' | '_' | '-' => {
                    self.bump();
                }
                _ => {
                    self.bump();
                    while let Some(rest) = self.peek() {
                        if rest == '"' {
                            self.bump();
                            break;
                        }
                        if matches!(rest, '\n' | '\r') {
                            break;
                        }
                        self.bump();
                    }
                    return SyntaxKind::Error;
                }
            }
        }

        SyntaxKind::Error
    }

    fn lex_symbol_or_apostrophe(&mut self) -> SyntaxKind {
        let start = self.pos;
        self.bump();
        if let Some(ch) = self.peek()
            && is_ident_start(ch)
        {
            self.bump();
            while let Some(next) = self.peek() {
                if is_ident_continue(next) {
                    self.bump();
                } else {
                    break;
                }
            }
            return SyntaxKind::SymbolLit;
        }

        self.pos = start + '\''.len_utf8();
        SyntaxKind::Apostrophe
    }

    fn lex_object_or_hash(&mut self) -> SyntaxKind {
        let start = self.pos;
        self.bump();

        if self.matches("anon_") {
            self.pos += "anon_".len();
            if self.consume_uuid_body() {
                return SyntaxKind::ObjectLit;
            }
            self.pos = start + 1;
            return SyntaxKind::Hash;
        }

        if self.consume_uuid_body() {
            return SyntaxKind::ObjectLit;
        }

        let sign_pos = self.pos;
        if matches!(self.peek(), Some('+') | Some('-')) {
            self.bump();
        }
        if self.consume_number_digits() {
            return SyntaxKind::ObjectLit;
        }

        self.pos = sign_pos;
        SyntaxKind::Hash
    }

    fn consume_uuid_body(&mut self) -> bool {
        let start = self.pos;
        if !self.consume_hex_digits(6) {
            self.pos = start;
            return false;
        }
        if self.peek() != Some('-') {
            self.pos = start;
            return false;
        }
        self.bump();
        if !self.consume_hex_digits(10) {
            self.pos = start;
            return false;
        }
        true
    }

    fn lex_signed_number_or_operator(&mut self) -> SyntaxKind {
        if matches!(self.peek_next(), Some('0'..='9'))
            || (self.peek_next() == Some('.') && self.peek_nth(2).is_some_and(|ch| ch.is_ascii_digit()))
        {
            return self.lex_number();
        }

        let ch = self.bump().expect("sign must be present");
        match ch {
            '+' => SyntaxKind::Plus,
            '-' => SyntaxKind::Minus,
            _ => SyntaxKind::Error,
        }
    }

    fn lex_number(&mut self) -> SyntaxKind {
        let start = self.pos;

        if matches!(self.peek(), Some('+') | Some('-')) {
            self.bump();
        }

        let saw_digits = if self.peek() == Some('.') {
            self.bump();
            if !self.consume_number_digits() {
                self.pos = start + 1;
                return SyntaxKind::Dot;
            }
            true
        } else {
            let saw_digits = self.consume_number_digits();
            if !saw_digits {
                self.pos = start;
                self.bump();
                return SyntaxKind::Error;
            }

            if self.peek() == Some('.') && self.peek_next() != Some('.') {
                self.bump();
                self.consume_number_digits();
                return self.finish_number_kind(start);
            }
            saw_digits
        };

        if !saw_digits {
            return SyntaxKind::Error;
        }

        self.finish_number_kind(start)
    }

    fn finish_number_kind(&mut self, start: usize) -> SyntaxKind {
        if matches!(self.peek(), Some('e') | Some('E')) {
            let save = self.pos;
            self.bump();
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.bump();
            }
            if self.consume_number_digits() {
                return SyntaxKind::FloatLit;
            }
            self.pos = save;
        }

        let text = &self.source[start..self.pos];
        if text.ends_with('.') || text.contains('.') {
            SyntaxKind::FloatLit
        } else {
            SyntaxKind::IntLit
        }
    }

    fn lex_block_comment(&mut self) -> SyntaxKind {
        self.bump();
        self.bump();
        while let Some(ch) = self.peek() {
            if ch == '*' && self.peek_next() == Some('/') {
                self.bump();
                self.bump();
                return SyntaxKind::BlockComment;
            }
            self.bump();
        }
        SyntaxKind::Error
    }

    fn can_start_signed_number(&self) -> bool {
        match self.prev_significant {
            None => true,
            Some(kind) => !kind.can_end_expr(),
        }
    }

    fn can_start_leading_dot_float(&self) -> bool {
        self.can_start_signed_number() && self.peek_next().is_some_and(|ch| ch.is_ascii_digit())
    }

    fn consume_horizontal_whitespace(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\u{00A0}')) {
            self.bump();
        }
    }

    fn consume_newline(&mut self) {
        if self.peek() == Some('\r') {
            self.bump();
            if self.peek() == Some('\n') {
                self.bump();
            }
            return;
        }
        self.bump();
    }

    fn consume_line_comment_body(&mut self) {
        while let Some(ch) = self.peek() {
            if matches!(ch, '\n' | '\r') {
                break;
            }
            self.bump();
        }
    }

    fn consume_number_digits(&mut self) -> bool {
        let start = self.pos;
        let mut saw_digit = false;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                saw_digit = true;
                self.bump();
                continue;
            }
            if ch == '_' && self.peek_next().is_some_and(|next| next.is_ascii_digit()) {
                self.bump();
                continue;
            }
            break;
        }
        saw_digit && self.pos > start
    }

    fn consume_hex_digits(&mut self, count: usize) -> bool {
        for _ in 0..count {
            if self.peek().is_some_and(|ch| ch.is_ascii_hexdigit()) {
                self.bump();
            } else {
                return false;
            }
        }
        true
    }

    fn consume_string_error_tail(&mut self) {
        while let Some(ch) = self.peek() {
            if matches!(ch, '\n' | '\r') {
                break;
            }
            self.bump();
            if ch == '"' {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        self.peek_nth(1)
    }

    fn peek_nth(&self, n: usize) -> Option<char> {
        self.source[self.pos..].chars().nth(n)
    }

    fn peek_prev_was(&self, ch: char) -> bool {
        self.pos >= ch.len_utf8() && self.source[..self.pos].ends_with(ch)
    }

    fn matches(&self, text: &str) -> bool {
        self.source[self.pos..].starts_with(text)
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }
}

fn keyword_kind(text: &str) -> Option<SyntaxKind> {
    if text.eq_ignore_ascii_case("if") {
        return Some(SyntaxKind::IfKw);
    }
    if text.eq_ignore_ascii_case("else") {
        return Some(SyntaxKind::ElseKw);
    }
    if text.eq_ignore_ascii_case("elseif") {
        return Some(SyntaxKind::ElseIfKw);
    }
    if text.eq_ignore_ascii_case("endif") {
        return Some(SyntaxKind::EndIfKw);
    }
    if text.eq_ignore_ascii_case("for") {
        return Some(SyntaxKind::ForKw);
    }
    if text.eq_ignore_ascii_case("endfor") {
        return Some(SyntaxKind::EndForKw);
    }
    if text.eq_ignore_ascii_case("while") {
        return Some(SyntaxKind::WhileKw);
    }
    if text.eq_ignore_ascii_case("endwhile") {
        return Some(SyntaxKind::EndWhileKw);
    }
    if text.eq_ignore_ascii_case("fork") {
        return Some(SyntaxKind::ForkKw);
    }
    if text.eq_ignore_ascii_case("endfork") {
        return Some(SyntaxKind::EndForkKw);
    }
    if text.eq_ignore_ascii_case("in") {
        return Some(SyntaxKind::InKw);
    }
    if text.eq_ignore_ascii_case("return") {
        return Some(SyntaxKind::ReturnKw);
    }
    if text.eq_ignore_ascii_case("break") {
        return Some(SyntaxKind::BreakKw);
    }
    if text.eq_ignore_ascii_case("continue") {
        return Some(SyntaxKind::ContinueKw);
    }
    if text.eq_ignore_ascii_case("try") {
        return Some(SyntaxKind::TryKw);
    }
    if text.eq_ignore_ascii_case("except") {
        return Some(SyntaxKind::ExceptKw);
    }
    if text.eq_ignore_ascii_case("finally") {
        return Some(SyntaxKind::FinallyKw);
    }
    if text.eq_ignore_ascii_case("endtry") {
        return Some(SyntaxKind::EndTryKw);
    }
    if text.eq_ignore_ascii_case("fn") {
        return Some(SyntaxKind::FnKw);
    }
    if text.eq_ignore_ascii_case("endfn") {
        return Some(SyntaxKind::EndFnKw);
    }
    if text.eq_ignore_ascii_case("let") {
        return Some(SyntaxKind::LetKw);
    }
    if text.eq_ignore_ascii_case("const") {
        return Some(SyntaxKind::ConstKw);
    }
    if text.eq_ignore_ascii_case("global") {
        return Some(SyntaxKind::GlobalKw);
    }
    if text.eq_ignore_ascii_case("pass") {
        return Some(SyntaxKind::PassKw);
    }
    if text.eq_ignore_ascii_case("any") {
        return Some(SyntaxKind::AnyKw);
    }
    if text.eq_ignore_ascii_case("true") {
        return Some(SyntaxKind::TrueKw);
    }
    if text.eq_ignore_ascii_case("false") {
        return Some(SyntaxKind::FalseKw);
    }
    None
}

fn type_constant_kind(text: &str) -> Option<SyntaxKind> {
    let lower = text.to_ascii_lowercase();
    match lower.as_str() {
        "type_int" | "type_num" | "type_float" | "type_str" | "type_err" | "type_obj"
        | "type_list" | "type_map" | "type_bool" | "type_flyweight" | "type_binary"
        | "type_lambda" | "type_sym" => Some(SyntaxKind::TypeConstant),
        _ => None,
    }
}

fn error_literal_kind(text: &str) -> Option<SyntaxKind> {
    if text.len() > 2
        && text[..2].eq_ignore_ascii_case("e_")
        && text[2..].chars().all(is_ident_continue)
    {
        return Some(SyntaxKind::ErrorLit);
    }
    None
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}
