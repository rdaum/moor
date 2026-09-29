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

use std::{cell::OnceCell, path::PathBuf};

use crate::{
    CompileOptions, ObjDefParseError, ObjFileContext, ObjPropDef, ObjPropOverride, ObjVerbDef,
    ObjectDefinition, Program, compile, objdef::offset_compile_error,
};
use base64::Engine;
use moor_common::{
    model::{
        ArgSpec, CompileContext, CompileError, ObjFlag, ParseErrorDetails, PrepSpec, PropFlag,
        PropPerms, VerbArgsSpec, VerbFlag,
    },
    util::BitEnum,
    util::unquote_str,
};
use moor_var::{
    AnonymousObjid, ErrorCode, List, NOTHING, Obj, Symbol, UuObjid, Var,
    program::{ProgramType, names::Name, opcode::Op},
    v_binary, v_bool, v_err, v_error, v_float, v_flyweight, v_int, v_list, v_map, v_none, v_obj,
    v_str, v_sym,
};

type LambdaCaptureFrames = Vec<Vec<(Symbol, Var)>>;

#[derive(Clone, Copy)]
enum Stopper {
    Char(char),
    Keyword(&'static str),
}

pub(crate) fn parse_literal_value(
    literal_str: &str,
    context: &mut ObjFileContext,
) -> Result<Var, ObjDefParseError> {
    let mut parser = LiteralParser::new(literal_str, context);
    parser.skip_trivia();
    let value = parser.parse_literal()?;
    parser.skip_trivia();
    if !parser.is_eof() {
        return Err(parser.parse_error("unexpected trailing input"));
    }
    Ok(value)
}

pub(crate) fn parse_persistent_literal(
    text: &str,
    options: &CompileOptions,
) -> Result<Var, ObjDefParseError> {
    let mut context = ObjFileContext::new();
    let mut parser = LiteralParser::new(text, &mut context);
    parser.persistence_options = Some(options.clone());
    parser.skip_trivia();
    let value = parser.parse_literal()?;
    parser.skip_trivia();
    if !parser.is_eof() {
        return Err(parser.parse_error("unexpected trailing input"));
    }
    Ok(value)
}

pub(crate) fn compile_object_definitions(
    objdef: &str,
    options: &CompileOptions,
    context: &mut ObjFileContext,
) -> Result<Vec<ObjectDefinition>, ObjDefParseError> {
    let mut parser = LiteralParser::new(objdef, context);
    parser.parse_objects_file(options)
}

pub(crate) fn resolve_include_path(
    context: &ObjFileContext,
    rel_path: &str,
) -> Result<PathBuf, ObjDefParseError> {
    let base = context.base_path().ok_or_else(|| {
        ObjDefParseError::IncludeError(
            rel_path.to_string(),
            "include! macros require a file-based compilation context".to_string(),
        )
    })?;
    let resolved = base.join(rel_path);
    let root = context.root_path().unwrap_or(base);
    if let (Ok(canonical_root), Ok(canonical_resolved)) =
        (root.canonicalize(), resolved.canonicalize())
        && !canonical_resolved.starts_with(&canonical_root)
    {
        return Err(ObjDefParseError::IncludeError(
            rel_path.to_string(),
            "path escapes the source directory".to_string(),
        ));
    }
    Ok(resolved)
}

struct LiteralParser<'a> {
    source: &'a str,
    pos: usize,
    context: &'a mut ObjFileContext,
    depth: usize,
    line_starts: OnceCell<Vec<usize>>,
    persistence_options: Option<CompileOptions>,
}

impl<'a> LiteralParser<'a> {
    fn new(source: &'a str, context: &'a mut ObjFileContext) -> Self {
        Self {
            source,
            pos: 0,
            context,
            depth: 0,
            line_starts: OnceCell::new(),
            persistence_options: None,
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn remaining(&self) -> &'a str {
        &self.source[self.pos..]
    }

    fn line_col(&self, pos: usize) -> (usize, usize) {
        let line_starts = self.line_starts.get_or_init(|| {
            let mut starts = vec![0];
            starts.extend(
                self.source
                    .bytes()
                    .enumerate()
                    .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
            );
            starts
        });
        let pos = pos.min(self.source.len());
        let line_index = line_starts.partition_point(|&start| start <= pos) - 1;
        let line_start = line_starts[line_index];
        let col = self.source[line_start..pos].chars().count() + 1;
        (line_index + 1, col)
    }

    fn parse_error(&self, message: &str) -> ObjDefParseError {
        let line_col = self.line_col(self.pos);
        let context_line = self
            .source
            .lines()
            .nth(line_col.0.saturating_sub(1))
            .unwrap_or("")
            .to_string();
        ObjDefParseError::ParseError(CompileError::ParseError {
            error_position: CompileContext::new(line_col),
            end_line_col: Some(line_col),
            context: context_line,
            message: message.to_string(),
            details: Box::new(ParseErrorDetails::default()),
        })
    }

    fn skip_trivia(&mut self) {
        loop {
            let before = self.pos;

            while let Some(ch) = self.peek_char() {
                if ch.is_whitespace() {
                    self.bump_char();
                } else {
                    break;
                }
            }

            if self.remaining().starts_with("//") {
                while let Some(ch) = self.bump_char() {
                    if ch == '\n' {
                        break;
                    }
                }
            } else if self.remaining().starts_with("/*") {
                self.pos += 2;
                while !self.is_eof() && !self.remaining().starts_with("*/") {
                    self.bump_char();
                }
                if self.remaining().starts_with("*/") {
                    self.pos += 2;
                }
            }

            if self.pos == before {
                break;
            }
        }
    }

    fn skip_horizontal_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch == ' ' || ch == '\t' {
                self.bump_char();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn eat_char(&mut self, ch: char) -> bool {
        if self.peek_char() == Some(ch) {
            self.bump_char();
            true
        } else {
            false
        }
    }

    fn expect_char(&mut self, ch: char, message: &str) -> Result<(), ObjDefParseError> {
        if self.eat_char(ch) {
            Ok(())
        } else {
            Err(self.parse_error(message))
        }
    }

    fn starts_with_keyword(&self, keyword: &str) -> bool {
        let remaining = self.remaining();
        if !remaining
            .get(..keyword.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
        {
            return false;
        }
        let next = remaining[keyword.len()..].chars().next();
        !matches!(next, Some(c) if c == '_' || c.is_ascii_alphanumeric())
    }

    fn eat_keyword(&mut self, keyword: &str) -> bool {
        if self.starts_with_keyword(keyword) {
            self.pos += keyword.len();
            true
        } else {
            false
        }
    }

    fn parse_ident(&mut self) -> Result<&'a str, ObjDefParseError> {
        let start = self.pos;
        let Some(first) = self.peek_char() else {
            return Err(self.parse_error("expected identifier"));
        };
        if !(first == '_' || first.is_ascii_alphabetic()) {
            return Err(self.parse_error("expected identifier"));
        }
        self.bump_char();
        while let Some(ch) = self.peek_char() {
            if ch == '_' || ch.is_ascii_alphanumeric() {
                self.bump_char();
            } else {
                break;
            }
        }
        Ok(&self.source[start..self.pos])
    }

    fn parse_propchars(&mut self) -> Result<&'a str, ObjDefParseError> {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == '_' || ch.is_ascii_alphanumeric() {
                self.bump_char();
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.parse_error("expected property name"));
        }
        Ok(&self.source[start..self.pos])
    }

    fn parse_bool_value(&mut self) -> Result<bool, ObjDefParseError> {
        let ident = self.parse_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(self.parse_error("expected boolean literal")),
        }
    }

    fn parse_object_attr_value(&mut self) -> Result<Obj, ObjDefParseError> {
        let value = self.parse_literal()?;
        let Some(obj) = value.as_object() else {
            return Err(ObjDefParseError::BadAttributeType(value.type_code()));
        };
        Ok(obj)
    }

    fn parse_string_attr_value(&mut self) -> Result<String, ObjDefParseError> {
        let value = self.parse_literal()?;
        let Some(name) = value.as_string() else {
            return Err(ObjDefParseError::BadAttributeType(value.type_code()));
        };
        Ok(name.to_string())
    }

    fn parse_quoted(&mut self) -> Result<&'a str, ObjDefParseError> {
        let start = self.pos;
        self.expect_char('"', "expected string literal")?;
        let mut escaped = false;
        while let Some(ch) = self.bump_char() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                return Ok(&self.source[start..self.pos]);
            }
        }
        Err(self.parse_error("unterminated string literal"))
    }

    fn parse_string_value(&mut self) -> Result<String, ObjDefParseError> {
        let literal = self.parse_quoted()?;
        unquote_str(literal).map_err(|e| {
            ObjDefParseError::VerbCompileError(
                CompileError::StringLexError(CompileContext::new(self.line_col(self.pos)), e),
                String::new(),
            )
        })
    }

    fn parse_binary_value(&mut self) -> Result<Var, ObjDefParseError> {
        if !self.remaining().starts_with("b\"") {
            return Err(self.parse_error("expected binary literal"));
        }
        let start = self.pos;
        self.pos += 1;
        let literal = self.parse_quoted()?;
        let full = &self.source[start..self.pos];
        let _ = literal;
        let base64_content = full
            .strip_prefix("b\"")
            .and_then(|s| s.strip_suffix('"'))
            .ok_or_else(|| self.parse_error("invalid binary literal"))?;
        let decoded = base64::engine::general_purpose::URL_SAFE
            .decode(base64_content)
            .map_err(|e| {
                ObjDefParseError::VerbCompileError(
                    CompileError::StringLexError(
                        CompileContext::new(self.line_col(start)),
                        format!("invalid binary literal '{full}': invalid base64: {e}"),
                    ),
                    String::new(),
                )
            })?;
        Ok(v_binary(decoded))
    }

    fn parse_object_value(&mut self) -> Result<Var, ObjDefParseError> {
        let start = self.pos;
        self.expect_char('#', "expected object literal")?;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                self.bump_char();
            } else {
                break;
            }
        }
        let literal = &self.source[start..self.pos];
        let ostr = &literal[1..];
        let obj = if let Some(anon_part) = ostr.strip_prefix("anon_") {
            if anon_part.len() == 17 && anon_part.chars().nth(6) == Some('-') {
                let uuid = UuObjid::from_uuid_string(anon_part)
                    .map_err(|_| ObjDefParseError::InvalidObjectId(literal.to_string()))?;
                Obj::mk_anonymous(AnonymousObjid(uuid.0))
            } else {
                return Err(ObjDefParseError::InvalidObjectId(literal.to_string()));
            }
        } else if ostr.len() == 17 && ostr.chars().nth(6) == Some('-') {
            let uuid = UuObjid::from_uuid_string(ostr)
                .map_err(|_| ObjDefParseError::InvalidObjectId(literal.to_string()))?;
            Obj::mk_uuobjid(uuid)
        } else {
            Obj::try_from(literal)
                .map_err(|_| ObjDefParseError::InvalidObjectId(literal.to_string()))?
        };
        Ok(v_obj(obj))
    }

    fn parse_number_value(&mut self) -> Result<Var, ObjDefParseError> {
        let start = self.pos;
        self.eat_char('+');
        if !self.remaining().starts_with('+') {
            let _ = self.eat_char('-');
        }
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() || matches!(ch, '_' | '.' | 'e' | 'E' | '+' | '-') {
                self.bump_char();
            } else {
                break;
            }
        }
        let text = &self.source[start..self.pos];
        let normalized = text.replace('_', "");
        if normalized.contains('.') || normalized.contains('e') || normalized.contains('E') {
            let value = normalized.parse::<f64>().map_err(|e| {
                ObjDefParseError::VerbCompileError(
                    CompileError::StringLexError(
                        CompileContext::new(self.line_col(start)),
                        format!("Failed to parse '{text}' to f64: {e}"),
                    ),
                    String::new(),
                )
            })?;
            // MOO floats are always real numbers; out-of-range literals are errors.
            if !value.is_finite() {
                return Err(ObjDefParseError::VerbCompileError(
                    CompileError::StringLexError(
                        CompileContext::new(self.line_col(start)),
                        format!("float literal '{text}' is out of range"),
                    ),
                    String::new(),
                ));
            }
            Ok(v_float(value))
        } else {
            let value = normalized.parse::<i64>().map_err(|e| {
                ObjDefParseError::VerbCompileError(
                    CompileError::StringLexError(
                        CompileContext::new(self.line_col(start)),
                        format!("Failed to parse '{text}' to i64: {e}"),
                    ),
                    String::new(),
                )
            })?;
            Ok(v_int(value))
        }
    }

    fn parse_literal_list(&mut self) -> Result<Var, ObjDefParseError> {
        self.expect_char('{', "expected list literal")?;
        self.skip_trivia();
        let mut values = Vec::new();
        if self.eat_char('}') {
            return Ok(v_list(&values));
        }
        loop {
            values.push(self.parse_literal()?);
            self.skip_trivia();
            if self.eat_char(',') {
                self.skip_trivia();
                continue;
            }
            self.expect_char('}', "expected '}' to close list literal")?;
            break;
        }
        Ok(v_list(&values))
    }

    fn parse_literal_map(&mut self) -> Result<Var, ObjDefParseError> {
        self.expect_char('[', "expected map literal")?;
        self.skip_trivia();
        let mut entries = Vec::new();
        if self.eat_char(']') {
            return Ok(v_map(&entries));
        }
        loop {
            let key = self.parse_literal()?;
            self.skip_trivia();
            if !self.remaining().starts_with("->") {
                return Err(self.parse_error("expected '->' in map literal"));
            }
            self.pos += 2;
            self.skip_trivia();
            let value = self.parse_literal()?;
            entries.push((key, value));
            self.skip_trivia();
            if self.eat_char(',') {
                self.skip_trivia();
                continue;
            }
            self.expect_char(']', "expected ']' to close map literal")?;
            break;
        }
        Ok(v_map(&entries))
    }

    fn parse_literal_flyweight(&mut self) -> Result<Var, ObjDefParseError> {
        self.expect_char('<', "expected flyweight literal")?;
        self.skip_trivia();
        let delegate = self.parse_literal()?;
        let Some(delegate) = delegate.as_object() else {
            return Err(ObjDefParseError::BadAttributeType(delegate.type_code()));
        };
        let mut slots = Vec::new();
        let mut contents = List::mk_list(&[]);
        self.skip_trivia();

        while self.eat_char(',') {
            self.skip_trivia();
            if self.peek_char() == Some('.') {
                self.bump_char();
                let slot_start = self.pos;
                let quoted = self.peek_char() == Some('"');
                let slot_name = if quoted {
                    Symbol::mk(&self.parse_string_value()?)
                } else {
                    Symbol::mk(self.parse_ident()?)
                };
                if !quoted
                    && (slot_name == Symbol::mk("delegate") || slot_name == Symbol::mk("slots"))
                {
                    return Err(ObjDefParseError::VerbCompileError(
                        CompileError::BadSlotName(
                            CompileContext::new(self.line_col(slot_start)),
                            slot_name.to_string(),
                        ),
                        String::new(),
                    ));
                }
                self.skip_trivia();
                self.expect_char('=', "expected '=' after flyweight slot name")?;
                self.skip_trivia();
                let value = self.parse_literal()?;
                slots.push((slot_name, value));
                self.skip_trivia();
                continue;
            }

            if self.peek_char() != Some('{') {
                return Err(self.parse_error("expected flyweight slot or contents"));
            }

            let list = self.parse_literal_list()?;
            let Some(list) = list.as_list() else {
                return Err(self.parse_error("expected flyweight contents list"));
            };
            contents = list.clone();
            self.skip_trivia();
            break;
        }

        self.expect_char('>', "expected '>' to close flyweight literal")?;
        Ok(v_flyweight(delegate, &slots, contents))
    }

    /// `lambda-params = '{' [ param { ',' param } ] '}'`
    fn parse_lambda_params(&mut self) -> Result<(), ObjDefParseError> {
        self.expect_char('{', "expected '{' to start lambda params")?;
        self.skip_trivia();
        if self.eat_char('}') {
            return Ok(());
        }

        loop {
            if self.eat_char('?') {
                self.parse_optional_lambda_param()?
            } else if self.eat_char('@') {
                self.parse_rest_lambda_param()?
            } else {
                self.parse_required_lambda_param()?
            }
            self.skip_trivia();
            if self.eat_char(',') {
                self.skip_trivia();
                continue;
            }
            self.expect_char('}', "expected '}' to close lambda params")?;
            break;
        }

        Ok(())
    }

    fn parse_optional_lambda_param(&mut self) -> Result<(), ObjDefParseError> {
        self.parse_ident()?;
        self.skip_trivia();
        if self.eat_char('=') {
            self.skip_trivia();
            self.consume_expression_slice(&[Stopper::Char(','), Stopper::Char('}')])?;
        }
        Ok(())
    }

    fn parse_rest_lambda_param(&mut self) -> Result<(), ObjDefParseError> {
        self.parse_ident()?;
        Ok(())
    }

    fn parse_required_lambda_param(&mut self) -> Result<(), ObjDefParseError> {
        self.parse_ident()?;
        Ok(())
    }

    /// `lambda-captured-env = '[' '{' name ':' literal { ',' name ':' literal } '}' { ',' ... } ']'`
    fn parse_lambda_captured_env(&mut self) -> Result<LambdaCaptureFrames, ObjDefParseError> {
        let mut frames = Vec::new();
        self.skip_trivia();
        self.expect_char('[', "expected '[' after captured")?;
        self.skip_trivia();
        if self.eat_char(']') {
            return Ok(frames);
        }
        loop {
            self.expect_char('{', "expected '{' to start captured frame")?;
            self.skip_trivia();
            let mut frame = Vec::new();
            if !self.eat_char('}') {
                loop {
                    let name = Symbol::mk(self.parse_ident()?);
                    self.skip_trivia();
                    self.expect_char(':', "expected ':' in captured variable entry")?;
                    self.skip_trivia();
                    frame.push((name, self.parse_literal()?));
                    self.skip_trivia();
                    if self.eat_char(',') {
                        self.skip_trivia();
                        continue;
                    }
                    self.expect_char('}', "expected '}' to close captured frame")?;
                    break;
                }
            }
            frames.push(frame);
            self.skip_trivia();
            if self.eat_char(',') {
                self.skip_trivia();
                continue;
            }
            self.expect_char(']', "expected ']' to close captured env")?;
            break;
        }
        Ok(frames)
    }

    fn compile_lambda_value(
        &self,
        lambda_source: &str,
        frames: &LambdaCaptureFrames,
        self_symbol: Option<Symbol>,
    ) -> Result<
        (
            moor_var::program::opcode::ScatterArgs,
            Program,
            Option<Name>,
        ),
        ObjDefParseError,
    > {
        let lambda_source_with_constants = if self.persistence_options.is_some() {
            lambda_source.to_owned()
        } else {
            self.resolve_lambda_constants(lambda_source)
        };
        let mut source = String::new();
        let mut seen = std::collections::HashSet::new();
        for (depth, frame) in frames.iter().enumerate() {
            if depth != 0 {
                source.push_str("begin\n");
            }
            for (symbol, _) in frame {
                if !seen.insert((depth, *symbol)) {
                    return Err(self.parse_error("duplicate captured binding"));
                }
                if depth != 0 {
                    source.push_str("let ");
                }
                source.push_str(symbol.as_str());
                source.push_str(" = None;\n");
            }
        }
        if let Some(symbol) = self_symbol
            && !frames.iter().flatten().any(|(name, _)| *name == symbol)
        {
            source.push_str(symbol.as_str());
            source.push_str(" = None;\n");
        }
        source.push_str(&format!("return {lambda_source_with_constants};"));
        for _ in 1..frames.len() {
            source.push_str("\nend");
        }
        let program = compile(
            &source,
            self.persistence_options.clone().unwrap_or_default(),
        )
        .map_err(|e| ObjDefParseError::VerbCompileError(e, lambda_source.to_string()))?;

        let Some((scatter_offset, program_offset, self_var)) =
            program.main_vector().iter().find_map(|op| match op {
                Op::MakeLambda {
                    scatter_offset,
                    program_offset,
                    self_var,
                    ..
                } => Some((*scatter_offset, *program_offset, *self_var)),
                _ => None,
            })
        else {
            return Err(self.parse_error("compiled lambda did not contain a lambda value"));
        };

        Ok((
            program.scatter_table(scatter_offset).clone(),
            program.lambda_program(program_offset).clone(),
            self_var,
        ))
    }

    fn resolve_lambda_constants(&self, source: &str) -> String {
        let mut resolved = String::with_capacity(source.len());
        let mut copied_through = 0usize;

        for token in crate::lexer::lex(source) {
            if token.kind != crate::SyntaxKind::Ident {
                continue;
            }
            let symbol = Symbol::mk(&source[token.span.clone()]);
            let Some(value) = self.context.constants().get(&symbol) else {
                continue;
            };

            resolved.push_str(&source[copied_through..token.span.start]);
            resolved.push_str(&crate::unparse::to_literal(value));
            copied_through = token.span.end;
        }

        resolved.push_str(&source[copied_through..]);
        resolved
    }

    fn bind_lambda_captures(
        &self,
        frames: LambdaCaptureFrames,
        body: &Program,
    ) -> Result<Vec<Vec<Var>>, ObjDefParseError> {
        let mut captured_env: Vec<Vec<Var>> = vec![Vec::new(); frames.len()];
        let mut assigned = std::collections::HashSet::new();
        let names = body.var_names();

        for (source_depth, frame) in frames.into_iter().enumerate() {
            for (symbol, value) in frame {
                let mut candidates = names
                    .names()
                    .into_iter()
                    .filter(|name| names.ident_for_name(name) == Some(symbol))
                    .collect::<Vec<_>>();
                candidates.sort_unstable();

                let name = candidates
                    .iter()
                    .find(|name| name.1 as usize == source_depth)
                    .copied()
                    .or_else(|| {
                        if candidates.len() == 1 {
                            candidates.first().copied()
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| {
                        self.parse_error(&format!(
                            "captured variable '{}' does not identify one lambda variable",
                            symbol.as_arc_str()
                        ))
                    })?;

                let scope_depth = name.1 as usize;
                let var_offset = name.0 as usize;
                if captured_env.len() <= scope_depth {
                    captured_env.resize_with(scope_depth + 1, Vec::new);
                }
                let frame = &mut captured_env[scope_depth];
                if frame.len() <= var_offset {
                    frame.resize(var_offset + 1, v_none());
                }
                if !assigned.insert(name) {
                    return Err(self.parse_error(&format!(
                        "captured variable '{}' is specified more than once",
                        symbol.as_arc_str()
                    )));
                }
                frame[var_offset] = value;
            }
        }

        Ok(captured_env)
    }

    fn finish_lambda_value(&mut self, lambda_source: &str) -> Result<Var, ObjDefParseError> {
        let mut capture_frames = Vec::new();
        let mut self_symbol = None;
        self.skip_trivia();
        if self.eat_keyword("with") {
            self.skip_trivia();
            if self.eat_keyword("captured") {
                capture_frames = self.parse_lambda_captured_env()?;
                self.skip_trivia();
            }
            if self.eat_keyword("self") {
                self.skip_trivia();
                self_symbol = Some(Symbol::mk(self.parse_ident()?));
            }
        }
        let (params, body_program, compiled_self_var) =
            self.compile_lambda_value(lambda_source, &capture_frames, self_symbol)?;
        let self_var = if let Some(symbol) = self_symbol {
            let names = body_program.var_names();
            let mut candidates = names
                .names()
                .into_iter()
                .filter(|name| names.ident_for_name(name) == Some(symbol));
            let name = candidates
                .next()
                .ok_or_else(|| self.parse_error("unknown lambda self binding"))?;
            if candidates.next().is_some() {
                return Err(self.parse_error("ambiguous lambda self binding"));
            }
            Some(name)
        } else {
            compiled_self_var
        };
        let captured_env = self.bind_lambda_captures(capture_frames, &body_program)?;
        Ok(Var::mk_lambda(params, body_program, captured_env, self_var))
    }

    /// `lambda-literal = lambda-params '=>' expr ['with captured ...]`
    fn parse_literal_lambda(&mut self) -> Result<Var, ObjDefParseError> {
        let lambda_start = self.pos;
        self.parse_lambda_params()?;
        self.skip_trivia();
        if !self.remaining().starts_with("=>") {
            return Err(self.parse_error("expected '=>' after lambda params"));
        }
        self.pos += 2;
        self.skip_trivia();

        self.consume_expression_slice(&[
            Stopper::Keyword("with"),
            Stopper::Char(','),
            Stopper::Char(']'),
            Stopper::Char('}'),
            Stopper::Char(';'),
        ])?;
        let lambda_source = self.source[lambda_start..self.pos].trim().to_string();
        self.finish_lambda_value(&lambda_source)
    }

    fn parse_literal_fn_lambda(&mut self) -> Result<Var, ObjDefParseError> {
        let lambda_start = self.pos;
        let tokens = crate::lexer::lex(self.remaining());
        let mut depth = 0usize;

        for token in tokens {
            match token.kind {
                crate::SyntaxKind::FnKw => depth += 1,
                crate::SyntaxKind::EndFnKw => {
                    if depth == 0 {
                        return Err(self.parse_error("unexpected endfn in lambda literal"));
                    }
                    depth -= 1;
                    if depth == 0 {
                        self.pos = lambda_start + token.span.end;
                        let lambda_source = self.source[lambda_start..self.pos].to_string();
                        return self.finish_lambda_value(&lambda_source);
                    }
                }
                _ => {}
            }
        }

        Err(self.parse_error("expected endfn to close lambda literal"))
    }

    fn brace_starts_lambda(&self) -> bool {
        if self.peek_char() != Some('{') {
            return false;
        }
        let mut pos = self.pos;
        let mut depth = 0usize;
        let bytes = self.source.as_bytes();
        let mut in_string = false;
        let mut escaped = false;
        while pos < self.source.len() {
            let ch = bytes[pos] as char;
            if in_string {
                pos += 1;
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }
            if ch == '"' {
                in_string = true;
                pos += 1;
                continue;
            }
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    pos += 1;
                    while pos < self.source.len() {
                        let ch = self.source[pos..].chars().next().unwrap();
                        if ch.is_whitespace() {
                            pos += ch.len_utf8();
                            continue;
                        }
                        if self.source[pos..].starts_with("//") {
                            while pos < self.source.len() && !self.source[pos..].starts_with('\n') {
                                pos += self.source[pos..].chars().next().unwrap().len_utf8();
                            }
                            continue;
                        }
                        if self.source[pos..].starts_with("/*") {
                            pos += 2;
                            while pos < self.source.len() && !self.source[pos..].starts_with("*/") {
                                pos += self.source[pos..].chars().next().unwrap().len_utf8();
                            }
                            if pos < self.source.len() {
                                pos += 2;
                            }
                            continue;
                        }
                        break;
                    }
                    return self.source[pos..].starts_with("=>");
                }
            }
            pos += 1;
        }
        false
    }

    fn consume_expression_slice(
        &mut self,
        stoppers: &[Stopper],
    ) -> Result<&'a str, ObjDefParseError> {
        let start = self.pos;
        let mut paren = 0usize;
        let mut bracket = 0usize;
        let mut brace = 0usize;
        let mut angle = 0usize;
        let mut in_string = false;
        let mut escaped = false;

        while !self.is_eof() {
            if !in_string && paren == 0 && bracket == 0 && brace == 0 && angle == 0 {
                for stopper in stoppers {
                    match stopper {
                        Stopper::Char(ch) if self.peek_char() == Some(*ch) => {
                            return Ok(&self.source[start..self.pos]);
                        }
                        Stopper::Keyword(keyword) if self.starts_with_keyword(keyword) => {
                            return Ok(&self.source[start..self.pos]);
                        }
                        _ => {}
                    }
                }
            }

            let ch = self
                .bump_char()
                .ok_or_else(|| self.parse_error("unexpected end of input"))?;
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '(' => paren += 1,
                ')' => {
                    if paren == 0 {
                        self.pos -= 1;
                        return Ok(&self.source[start..self.pos]);
                    }
                    paren -= 1;
                }
                '[' => bracket += 1,
                ']' => {
                    if bracket == 0 {
                        self.pos -= 1;
                        return Ok(&self.source[start..self.pos]);
                    }
                    bracket -= 1;
                }
                '{' => brace += 1,
                '}' => {
                    if brace == 0 {
                        self.pos -= 1;
                        return Ok(&self.source[start..self.pos]);
                    }
                    brace -= 1;
                }
                '<' => angle += 1,
                '>' => {
                    angle = angle.saturating_sub(1);
                }
                _ => {}
            }
        }

        Ok(&self.source[start..self.pos])
    }

    fn parse_property_name(&mut self) -> Result<Symbol, ObjDefParseError> {
        self.skip_trivia();
        if self.peek_char() == Some('"') {
            return Ok(Symbol::mk(self.parse_string_value()?.as_str()));
        }
        Ok(Symbol::mk(self.parse_propchars()?.trim()))
    }

    fn parse_prop_flags_value(&mut self) -> Result<BitEnum<PropFlag>, ObjDefParseError> {
        let flags = self.parse_string_value()?;
        PropFlag::parse_str(&flags).ok_or(ObjDefParseError::BadPropFlags(flags))
    }

    fn parse_verb_flags_value(&mut self) -> Result<BitEnum<VerbFlag>, ObjDefParseError> {
        let flags = self.parse_string_value()?;
        VerbFlag::parse_str(&flags).ok_or(ObjDefParseError::BadVerbFlags(flags))
    }

    fn parse_propinfo(&mut self) -> Result<PropPerms, ObjDefParseError> {
        self.skip_trivia();
        self.expect_char('(', "expected '(' to start property permissions")?;
        self.skip_trivia();
        if !self.eat_keyword("owner") {
            return Err(self.parse_error("expected owner attribute in property info"));
        }
        self.skip_trivia();
        self.expect_char(':', "expected ':' after owner")?;
        self.skip_trivia();
        let owner = self.parse_object_attr_value()?;
        self.skip_trivia();
        self.expect_char(',', "expected ',' between owner and flags")?;
        self.skip_trivia();
        if !self.eat_keyword("flags") {
            return Err(self.parse_error("expected flags attribute in property info"));
        }
        self.skip_trivia();
        self.expect_char(':', "expected ':' after flags")?;
        self.skip_trivia();
        let flags = self.parse_prop_flags_value()?;
        self.skip_trivia();
        self.expect_char(')', "expected ')' to close property info")?;
        Ok(PropPerms::new(owner, flags))
    }

    fn parse_top_level_constant_decl(&mut self) -> Result<(), ObjDefParseError> {
        self.skip_trivia();
        let name = Symbol::mk(self.parse_ident()?);
        self.skip_trivia();
        self.expect_char('=', "expected '=' after constant name")?;
        self.skip_trivia();
        let value = self.parse_literal()?;
        self.skip_trivia();
        let _ = self.eat_char(';');
        if let Some(existing) = self.context.constants().get(&name) {
            return Err(ObjDefParseError::DuplicateConstant(
                name.to_string(),
                format!("{existing:?}"),
            ));
        }
        for (existing_name, existing_val) in self.context.constants().iter() {
            if *existing_val == value {
                return Err(ObjDefParseError::DuplicateConstant(
                    format!("{name} = {value:?}"),
                    format!("conflicts with {existing_name} = {existing_val:?}"),
                ));
            }
        }
        self.context.add_constant(name, value);
        Ok(())
    }

    fn parse_object_definition(
        &mut self,
        options: &CompileOptions,
    ) -> Result<ObjectDefinition, ObjDefParseError> {
        self.skip_trivia();
        let oid = self.parse_object_attr_value()?;
        let mut objdef = ObjectDefinition {
            declared_attributes: Some(Default::default()),
            oid,
            name: String::new(),
            parent: NOTHING,
            owner: NOTHING,
            location: NOTHING,
            flags: Default::default(),
            metadata: self.parse_optional_metadata_map()?,
            verbs: Vec::new(),
            property_definitions: Vec::new(),
            property_overrides: Vec::new(),
        };

        loop {
            self.skip_trivia();
            if self.is_eof() {
                return Err(self.parse_error("unexpected end of input inside object definition"));
            }
            if self.eat_keyword("endobject") {
                return Ok(objdef);
            }
            if self.eat_keyword("method") {
                objdef.verbs.push(self.parse_method_decl(options)?);
                continue;
            }
            if self.eat_keyword("verb") {
                objdef.verbs.push(self.parse_verb_decl(options)?);
                continue;
            }
            if self.eat_keyword("property") {
                objdef.property_definitions.push(self.parse_property_def()?);
                continue;
            }
            if self.eat_keyword("override") {
                objdef.property_overrides.push(self.parse_prop_override()?);
                continue;
            }

            let attr = self.parse_ident()?.to_ascii_lowercase();
            self.skip_trivia();
            self.expect_char(':', "expected ':' after object attribute")?;
            self.skip_trivia();
            if !objdef
                .declared_attributes
                .as_mut()
                .unwrap()
                .insert(attr.clone())
            {
                return Err(self.parse_error("duplicate object attribute"));
            }
            match attr.as_str() {
                "parent" => objdef.parent = self.parse_object_attr_value()?,
                "name" => objdef.name = self.parse_string_attr_value()?,
                "owner" => objdef.owner = self.parse_object_attr_value()?,
                "location" => objdef.location = self.parse_object_attr_value()?,
                "wizard" => {
                    if self.parse_bool_value()? {
                        objdef.flags.set(ObjFlag::Wizard);
                    }
                }
                "programmer" => {
                    if self.parse_bool_value()? {
                        objdef.flags.set(ObjFlag::Programmer);
                    }
                }
                "player" => {
                    if self.parse_bool_value()? {
                        objdef.flags.set(ObjFlag::User);
                    }
                }
                "fertile" => {
                    if self.parse_bool_value()? {
                        objdef.flags.set(ObjFlag::Fertile);
                    }
                }
                "readable" => {
                    if self.parse_bool_value()? {
                        objdef.flags.set(ObjFlag::Read);
                    }
                }
                "writeable" => {
                    if self.parse_bool_value()? {
                        objdef.flags.set(ObjFlag::Write);
                    }
                }
                _ => return Err(self.parse_error("unexpected object attribute")),
            }
        }
    }

    fn parse_property_def(&mut self) -> Result<ObjPropDef, ObjDefParseError> {
        let name = self.parse_property_name()?;
        self.skip_trivia();
        let perms = self.parse_propinfo()?;
        self.skip_trivia();
        let metadata = self.parse_optional_metadata_map()?;
        self.skip_trivia();
        let value = if self.eat_char('=') {
            self.skip_trivia();
            Some(self.parse_literal()?)
        } else {
            None
        };
        self.skip_trivia();
        let clear_value = value.is_none() && self.eat_keyword("clear");
        self.skip_trivia();
        let _ = self.eat_char(';');
        Ok(ObjPropDef {
            clear_value,
            name,
            perms,
            value,
            metadata,
        })
    }

    fn parse_prop_override(&mut self) -> Result<ObjPropOverride, ObjDefParseError> {
        let name = self.parse_property_name()?;
        self.skip_trivia();
        let perms_update = if self.peek_char() == Some('(') {
            Some(self.parse_propinfo()?)
        } else {
            None
        };
        self.skip_trivia();
        let metadata = self.parse_optional_metadata_map()?;
        self.skip_trivia();
        let value = if self.eat_char('=') {
            self.skip_trivia();
            Some(self.parse_literal()?)
        } else {
            None
        };
        self.skip_trivia();
        let clear_value = value.is_none() && self.eat_keyword("clear");
        self.skip_trivia();
        let _ = self.eat_char(';');
        Ok(ObjPropOverride {
            clear_value,
            name,
            perms_update,
            value,
            metadata,
        })
    }

    fn parse_optional_metadata_map(&mut self) -> Result<Vec<(Symbol, Var)>, ObjDefParseError> {
        self.skip_horizontal_whitespace();
        if self.peek_char() != Some('[') {
            return Ok(Vec::new());
        }

        self.bump_char();
        self.skip_trivia();
        let mut entries = Vec::new();
        if self.eat_char(']') {
            return Ok(entries);
        }

        loop {
            let key = self.parse_metadata_key()?;
            self.skip_trivia();
            if !self.remaining().starts_with("->") {
                return Err(self.parse_error("expected '->' in metadata map"));
            }
            self.pos += 2;
            self.skip_trivia();
            let value = self.parse_literal()?;
            entries.push((key, value));

            self.skip_trivia();
            if self.eat_char(',') {
                self.skip_trivia();
                if self.eat_char(']') {
                    break;
                }
                continue;
            }
            self.expect_char(']', "expected ']' to close metadata map")?;
            break;
        }

        Ok(entries)
    }

    fn parse_metadata_key(&mut self) -> Result<Symbol, ObjDefParseError> {
        self.skip_trivia();
        match self.peek_char() {
            Some('"') => {
                let key = self.parse_string_value()?;
                Ok(Symbol::mk(&key))
            }
            Some('\'') => {
                self.bump_char();
                Ok(Symbol::mk(self.parse_ident()?))
            }
            Some(_) => Ok(Symbol::mk(self.parse_ident()?)),
            None => Err(self.parse_error("expected metadata key")),
        }
    }

    fn parse_verb_body_until_end_keyword(
        &mut self,
        end_keyword: &str,
        missing_msg: &str,
    ) -> Result<(String, usize), ObjDefParseError> {
        let start = self.pos;
        let mut pos = self.pos;
        let mut in_string = false;
        let mut escaped = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        let mut line_start = self.pos == 0 || self.source[..self.pos].ends_with('\n');
        let end_kw_len = end_keyword.len();

        while pos < self.source.len() {
            if !in_string && !in_line_comment && !in_block_comment && line_start {
                let mut probe = pos;
                while probe < self.source.len() {
                    let ch = self.source[probe..].chars().next().unwrap();
                    if ch == ' ' || ch == '\t' {
                        probe += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                if self.source[probe..].len() >= end_kw_len
                    && self.source[probe..][..end_kw_len].eq_ignore_ascii_case(end_keyword)
                {
                    let next = self.source[probe + end_kw_len..].chars().next();
                    if !matches!(next, Some(c) if c == '_' || c.is_ascii_alphanumeric()) {
                        let body = self.source[start..pos].to_string();
                        let start_line = self.line_col(start).0;
                        self.pos = probe + end_kw_len;
                        return Ok((body, start_line));
                    }
                }
            }

            let ch = self.source[pos..].chars().next().unwrap();
            let len = ch.len_utf8();

            if in_string {
                pos += len;
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                line_start = ch == '\n';
                continue;
            }
            if in_line_comment {
                pos += len;
                if ch == '\n' {
                    in_line_comment = false;
                    line_start = true;
                } else {
                    line_start = false;
                }
                continue;
            }
            if in_block_comment {
                if self.source[pos..].starts_with("*/") {
                    pos += 2;
                    in_block_comment = false;
                    line_start = false;
                } else {
                    pos += len;
                    line_start = ch == '\n';
                }
                continue;
            }

            if self.source[pos..].starts_with("//") {
                pos += 2;
                in_line_comment = true;
                line_start = false;
                continue;
            }
            if self.source[pos..].starts_with("/*") {
                pos += 2;
                in_block_comment = true;
                line_start = false;
                continue;
            }
            if ch == '"' {
                in_string = true;
                escaped = false;
                pos += len;
                line_start = false;
                continue;
            }

            pos += len;
            line_start = ch == '\n';
        }

        Err(self.parse_error(missing_msg))
    }

    fn parse_verb_body_until_endverb(&mut self) -> Result<(String, usize), ObjDefParseError> {
        self.parse_verb_body_until_end_keyword("endverb", "missing endverb")
    }

    fn parse_verb_names(
        &mut self,
        allow_omitted_before_argspec: bool,
    ) -> Result<Vec<Symbol>, ObjDefParseError> {
        self.skip_trivia();
        let (line, column) = self.line_col(self.pos);
        if allow_omitted_before_argspec && self.peek_char() == Some('(') {
            return Err(ObjDefParseError::EmptyVerbName { line, column });
        }
        if self.peek_char() != Some('"') {
            return Ok(vec![Symbol::mk(self.parse_propchars()?.trim())]);
        }

        let names = self.parse_string_value()?;
        let names = names
            .split_whitespace()
            .map(|name| Symbol::mk(name.trim()))
            .collect::<Vec<_>>();
        if names.is_empty() {
            return Err(ObjDefParseError::EmptyVerbName { line, column });
        }
        Ok(names)
    }

    fn parse_verb_decl(
        &mut self,
        compile_options: &CompileOptions,
    ) -> Result<ObjVerbDef, ObjDefParseError> {
        let names = self.parse_verb_names(true)?;

        self.skip_trivia();
        self.expect_char('(', "expected '(' after verb name")?;
        let args_start = self.pos;
        while !self.is_eof() && self.peek_char() != Some(')') {
            self.bump_char();
        }
        self.expect_char(')', "expected ')' after verb argspec")?;
        let args_text = self.source[args_start..self.pos - 1].trim();
        let parts = args_text.split_whitespace().collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err(ObjDefParseError::BadVerbArgspec(args_text.to_string()));
        }
        let dobj = ArgSpec::from_string(parts[0])
            .ok_or_else(|| ObjDefParseError::BadVerbArgspec(args_text.to_string()))?;
        let prep = PrepSpec::parse(parts[1])
            .ok_or_else(|| ObjDefParseError::BadVerbArgspec(args_text.to_string()))?;
        let iobj = ArgSpec::from_string(parts[2])
            .ok_or_else(|| ObjDefParseError::BadVerbArgspec(args_text.to_string()))?;

        self.skip_trivia();
        if !self.eat_keyword("owner") {
            return Err(self.parse_error("expected owner attribute after verb argspec"));
        }
        self.skip_trivia();
        self.expect_char(':', "expected ':' after owner")?;
        self.skip_trivia();
        let owner = self.parse_object_attr_value()?;

        self.skip_trivia();
        if !self.eat_keyword("flags") {
            return Err(self.parse_error("expected flags attribute after verb owner"));
        }
        self.skip_trivia();
        self.expect_char(':', "expected ':' after flags")?;
        self.skip_trivia();
        let flags = self.parse_verb_flags_value()?;
        let metadata = self.parse_optional_metadata_map()?;

        let (_, source_column) = self.line_col(self.pos);
        let (statements_text, verb_start_line) = self.parse_verb_body_until_endverb()?;
        let program = compile(statements_text.as_str(), compile_options.clone()).map_err(|e| {
            ObjDefParseError::VerbCompileError(
                offset_compile_error(e, verb_start_line.saturating_sub(1)),
                statements_text.clone(),
            )
        })?;

        Ok(ObjVerbDef {
            source: Some(crate::objdef::ObjVerbSource {
                text: statements_text,
                line: verb_start_line,
                column: source_column,
            }),
            names,
            argspec: VerbArgsSpec { dobj, prep, iobj },
            owner,
            flags,
            program: ProgramType::MooR(program),
            metadata,
        })
    }

    fn parse_method_decl(
        &mut self,
        compile_options: &CompileOptions,
    ) -> Result<ObjVerbDef, ObjDefParseError> {
        let names = self.parse_verb_names(false)?;

        self.skip_trivia();
        if !self.eat_keyword("owner") {
            return Err(self.parse_error("expected owner attribute after method name"));
        }
        self.skip_trivia();
        self.expect_char(':', "expected ':' after owner")?;
        self.skip_trivia();
        let owner = self.parse_object_attr_value()?;

        let flags = {
            let saved_pos = self.pos;
            self.skip_horizontal_whitespace();
            if self.eat_keyword("flags") {
                self.skip_horizontal_whitespace();
                if self.eat_char(':') {
                    self.skip_horizontal_whitespace();
                    let flags = self.parse_verb_flags_value()?;
                    if !flags.contains(VerbFlag::Exec) {
                        return Err(self.parse_error("method requires exec (x) flag"));
                    }
                    flags
                } else {
                    self.pos = saved_pos;
                    VerbFlag::rxd()
                }
            } else {
                VerbFlag::rxd()
            }
        };
        let metadata = self.parse_optional_metadata_map()?;

        let (_, source_column) = self.line_col(self.pos);
        let (statements_text, verb_start_line) =
            self.parse_verb_body_until_end_keyword("endmethod", "missing endmethod")?;
        let program = compile(statements_text.as_str(), compile_options.clone()).map_err(|e| {
            ObjDefParseError::VerbCompileError(
                offset_compile_error(e, verb_start_line.saturating_sub(1)),
                statements_text.clone(),
            )
        })?;

        Ok(ObjVerbDef {
            source: Some(crate::objdef::ObjVerbSource {
                text: statements_text,
                line: verb_start_line,
                column: source_column,
            }),
            names,
            argspec: VerbArgsSpec::this_none_this(),
            owner,
            flags,
            program: ProgramType::MooR(program),
            metadata,
        })
    }

    fn parse_objects_file(
        &mut self,
        options: &CompileOptions,
    ) -> Result<Vec<ObjectDefinition>, ObjDefParseError> {
        let mut objects = Vec::new();
        self.skip_trivia();
        while !self.is_eof() {
            if self.eat_keyword("define") {
                self.skip_trivia();
                self.parse_top_level_constant_decl()?;
            } else if self.eat_keyword("object") {
                self.skip_trivia();
                objects.push(self.parse_object_definition(options)?);
            } else {
                return Err(self.parse_error("expected object or define declaration"));
            }
            self.skip_trivia();
        }
        Ok(objects)
    }

    /// `literal = string | object | symbol | map | list | flyweight | lambda | number | binary | boolean | include | constant`
    fn parse_literal(&mut self) -> Result<Var, ObjDefParseError> {
        self.skip_trivia();
        // Only container literals recurse; count their entry so scalar leaves
        // and map keys do not inflate the nesting depth.
        let is_container = matches!(self.peek_char(), Some('[' | '{' | '<'))
            || self.starts_with_keyword("fn")
            || self.remaining().starts_with("e\"")
            || self
                .remaining()
                .get(..2)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("e_"));
        if is_container {
            self.depth += 1;
            if self.depth > crate::objdef::MAX_LITERAL_NESTING {
                self.depth -= 1;
                return Err(ObjDefParseError::LiteralNestingTooDeep {
                    max_depth: crate::objdef::MAX_LITERAL_NESTING,
                });
            }
        }
        let result = self.parse_literal_inner();
        if is_container {
            self.depth -= 1;
        }
        result
    }

    fn parse_literal_inner(&mut self) -> Result<Var, ObjDefParseError> {
        let Some(ch) = self.peek_char() else {
            return Err(self.parse_error("expected literal"));
        };
        match ch {
            '"' => Ok(v_str(&self.parse_string_value()?)),
            '#' => self.parse_object_value(),
            '\'' => {
                self.bump_char();
                if self.peek_char() == Some('"') {
                    return Ok(v_sym(&self.parse_string_value()?));
                }
                let name = self.parse_ident()?;
                Ok(v_sym(name))
            }
            '[' => self.parse_literal_map(),
            '{' => {
                if self.brace_starts_lambda() {
                    self.parse_literal_lambda()
                } else {
                    self.parse_literal_list()
                }
            }
            '<' => self.parse_literal_flyweight(),
            '+' | '-' => self.parse_number_value(),
            c if c.is_ascii_digit() => self.parse_number_value(),
            _ => {
                if self.remaining().starts_with("f\"") {
                    let start = self.pos;
                    self.bump_char();
                    self.parse_quoted()?;
                    return crate::persistent::parse_float(&self.source[start..self.pos])
                        .map(v_float)
                        .map_err(|error| self.parse_error(&error));
                }
                if self.remaining().starts_with("e\"") {
                    self.bump_char();
                    let code = ErrorCode::ErrCustom(Symbol::mk(&self.parse_string_value()?));
                    return self.parse_error_value(code);
                }
                if self.remaining().starts_with("b\"") {
                    return self.parse_binary_value();
                }
                if self.starts_with_keyword("fn") {
                    return self.parse_literal_fn_lambda();
                }

                let start = self.pos;
                let ident = self.parse_ident()?;
                if let Some(value) = self.parse_keyword_literal(ident, start)? {
                    return Ok(value);
                }

                let sym = Symbol::mk(ident);
                let Some(value) = self.context.constants().get(&sym) else {
                    return Err(ObjDefParseError::ConstantNotFound(sym.to_string()));
                };
                Ok(value.clone())
            }
        }
    }

    fn parse_keyword_literal(
        &mut self,
        ident: &str,
        start: usize,
    ) -> Result<Option<Var>, ObjDefParseError> {
        if ident.eq_ignore_ascii_case("none") {
            return Ok(Some(v_none()));
        }
        if ident.eq_ignore_ascii_case("true") {
            return Ok(Some(v_bool(true)));
        }
        if ident.eq_ignore_ascii_case("false") {
            return Ok(Some(v_bool(false)));
        }
        if ident.eq_ignore_ascii_case("include") && self.eat_char('!') {
            return self.parse_include_text();
        }
        if ident.eq_ignore_ascii_case("include_bin") && self.eat_char('!') {
            return self.parse_include_binary();
        }
        if self.source[start..self.pos].len() >= 2
            && self.source[start..self.pos][..2].eq_ignore_ascii_case("e_")
        {
            let Some(error) = ErrorCode::parse_str(&self.source[start..self.pos]) else {
                return Err(self.parse_error("invalid error value"));
            };
            return self.parse_error_value(error).map(Some);
        }
        Ok(None)
    }

    fn parse_error_value(&mut self, error: ErrorCode) -> Result<Var, ObjDefParseError> {
        self.skip_trivia();
        if !self.eat_char('(') {
            return Ok(v_err(error));
        }

        self.skip_trivia();
        let message = if self.eat_keyword("None") {
            None
        } else {
            Some(self.parse_string_value()?)
        };
        self.skip_trivia();
        let value = if self.eat_char(',') {
            Some(self.parse_literal()?)
        } else {
            None
        };
        self.skip_trivia();
        self.expect_char(')', "expected ')' after error literal")?;
        Ok(v_error(moor_var::Error::new(error, message, value)))
    }

    fn parse_include_text(&mut self) -> Result<Option<Var>, ObjDefParseError> {
        if self.persistence_options.is_some() {
            return Err(self.parse_error("include macros are disabled in persistent literals"));
        }
        self.skip_trivia();
        self.expect_char('(', "expected '(' after include!")?;
        self.skip_trivia();
        let rel_path = self.parse_string_value()?;
        self.skip_trivia();
        self.expect_char(')', "expected ')' after include! path")?;
        let resolved = resolve_include_path(self.context, &rel_path)?;
        let contents = std::fs::read_to_string(&resolved).map_err(|e| {
            ObjDefParseError::IncludeError(resolved.display().to_string(), e.to_string())
        })?;
        Ok(Some(v_str(&contents)))
    }

    fn parse_include_binary(&mut self) -> Result<Option<Var>, ObjDefParseError> {
        if self.persistence_options.is_some() {
            return Err(self.parse_error("include macros are disabled in persistent literals"));
        }
        self.skip_trivia();
        self.expect_char('(', "expected '(' after include_bin!")?;
        self.skip_trivia();
        let rel_path = self.parse_string_value()?;
        self.skip_trivia();
        self.expect_char(')', "expected ')' after include_bin! path")?;
        let resolved = resolve_include_path(self.context, &rel_path)?;
        let bytes = std::fs::read(&resolved).map_err(|e| {
            ObjDefParseError::IncludeError(resolved.display().to_string(), e.to_string())
        })?;
        Ok(Some(v_binary(bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_builds_and_reuses_line_index() {
        let source = "first\nαβ\nlast\n";
        let mut context = ObjFileContext::new();
        let parser = LiteralParser::new(source, &mut context);

        assert!(parser.line_starts.get().is_none());
        assert_eq!(parser.line_col(0), (1, 1));
        assert_eq!(parser.line_col(5), (1, 6));
        assert_eq!(parser.line_col(6), (2, 1));
        assert_eq!(parser.line_col(10), (2, 3));
        assert_eq!(parser.line_col(11), (3, 1));
        assert_eq!(parser.line_col(source.len()), (4, 1));
        assert_eq!(parser.line_col(source.len() + 10), (4, 1));

        let line_starts = parser.line_starts.get().unwrap().as_ptr();
        assert_eq!(parser.line_col(8), (2, 2));
        assert_eq!(parser.line_starts.get().unwrap().as_ptr(), line_starts);
    }
}
