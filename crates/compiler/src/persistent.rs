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

//! Versioned, fallible MOO source and literal persistence.
use crate::{CompileOptions, compile, program_to_tree, unparse};
use moor_var::{Var, Variant, program::ProgramType};
use std::fmt::Write;

/// Canonical literal grammar and escaping version.
pub const PERSISTENT_LITERAL_VERSION: u32 = 1;
/// Decompiled program source format version.
pub const PERSISTENT_SOURCE_VERSION: u32 = 1;
/// Bump when compiler semantics change incompatibly with stored source.
pub const PERSISTENT_COMPILER_PROFILE_VERSION: u32 = 1;

/// Complete interpretation profile. Version 1 supports exactly the native MOO profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceProfile {
    pub language: String,
    pub literal_version: u32,
    pub source_version: u32,
    pub compiler_profile_version: u32,
    pub options: CompileOptions,
}
impl Default for SourceProfile {
    fn default() -> Self {
        Self {
            language: "moo".into(),
            literal_version: PERSISTENT_LITERAL_VERSION,
            source_version: PERSISTENT_SOURCE_VERSION,
            compiler_profile_version: PERSISTENT_COMPILER_PROFILE_VERSION,
            options: CompileOptions {
                flyweight_type: true,
                bool_type: true,
                symbol_type: true,
                custom_errors: true,
                call_unsupported_builtins: false,
                legacy_type_constants: false,
            },
        }
    }
}
impl SourceProfile {
    /// Validate before decoding any row. Build identifiers are diagnostic, not compatibility keys.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self != &Self::default() {
            return Err(ProfileError {
                profile: self.clone(),
            });
        }
        Ok(())
    }
}
#[derive(Debug, thiserror::Error)]
#[error("Unsupported persistent source profile: {profile:?}")]
pub struct ProfileError {
    pub profile: SourceProfile,
}
#[derive(Debug, thiserror::Error)]
pub enum LiteralEncodeError {
    #[error(transparent)]
    Profile(#[from] ProfileError),
    #[error("Literal rendering failed: {0}")]
    Render(#[from] crate::decompile::DecompileError),
    #[error("Rendered literal failed validation: {0}")]
    Validation(#[from] LiteralDecodeError),
    #[error("Literal nesting exceeds {0}")]
    Nesting(usize),
    #[error("Literal output failed")]
    Output(#[from] std::fmt::Error),
}
#[derive(Debug, thiserror::Error)]
pub enum LiteralDecodeError {
    #[error(transparent)]
    Profile(#[from] ProfileError),
    #[error("Invalid literal: {0}")]
    Parse(#[from] crate::ObjDefParseError),
    #[error("Invalid source at byte {offset}: {message}")]
    Source { offset: usize, message: String },
}
#[derive(Debug, thiserror::Error)]
pub enum SourceCodecError {
    #[error(transparent)]
    Profile(#[from] ProfileError),
    #[error("Source rendering failed: {0}")]
    Render(#[from] crate::decompile::DecompileError),
    #[error("Source compilation failed: {0:?}")]
    Compile(moor_common::model::CompileError),
    #[error(transparent)]
    Literal(#[from] LiteralDecodeError),
    #[error("Invalid source constant: {0}")]
    Constant(#[from] LiteralEncodeError),
    #[error("Source output failed")]
    Output(#[from] std::fmt::Error),
}

/// Stored source and the metadata needed to interpret it after a restart.
#[derive(Clone, Debug)]
pub struct PersistentProgram {
    pub profile: SourceProfile,
    pub originating_compiler: String,
    pub source: String,
}
impl PersistentProgram {
    pub fn encode(
        program: &ProgramType,
        profile: &SourceProfile,
    ) -> Result<Self, SourceCodecError> {
        let mut source = String::new();
        write_persistent_source(program, profile, &mut source)?;
        Ok(Self {
            profile: profile.clone(),
            originating_compiler: format!("moor-compiler/{}", env!("CARGO_PKG_VERSION")),
            source,
        })
    }
    pub fn decode(&self) -> Result<ProgramType, SourceCodecError> {
        read_persistent_source(&self.source, &self.profile)
    }
}

/// Encode and compile-check a literal before writing it to the caller's buffer.
pub fn write_persistent_literal(
    value: &Var,
    profile: &SourceProfile,
    out: &mut impl Write,
) -> Result<(), LiteralEncodeError> {
    profile.validate()?;
    check_depth(value, 0)?;
    let mut text = String::new();
    crate::unparse::write_literal(value, &mut text)?;
    read_persistent_literal(&text, profile)?;
    out.write_str(&text)?;
    Ok(())
}
/// Parse values without evaluation, external constants, object aliases, or include macros.
pub fn read_persistent_literal(
    text: &str,
    profile: &SourceProfile,
) -> Result<Var, LiteralDecodeError> {
    profile.validate()?;
    validate_source_text(text)?;
    Ok(crate::objdef_literal::parse_persistent_literal(
        text,
        &profile.options,
    )?)
}
/// Decompile into authoritative MOO text, and compile the result before accepting it.
pub fn write_persistent_source(
    program: &ProgramType,
    profile: &SourceProfile,
    out: &mut impl Write,
) -> Result<(), SourceCodecError> {
    profile.validate()?;
    let ProgramType::MooR(program) = program;
    check_program_depth(program, 0)?;
    let tree = program_to_tree(program)?;
    let text = unparse(&tree, false, true)?.join("\n");
    read_persistent_source(&text, profile)?;
    out.write_str(&text)?;
    Ok(())
}
pub fn read_persistent_source(
    text: &str,
    profile: &SourceProfile,
) -> Result<ProgramType, SourceCodecError> {
    profile.validate()?;
    validate_source_text(text)?;
    compile(text, profile.options.clone())
        .map(ProgramType::MooR)
        .map_err(SourceCodecError::Compile)
}

fn check_depth(value: &Var, depth: usize) -> Result<(), LiteralEncodeError> {
    if depth > crate::objdef::MAX_LITERAL_NESTING {
        return Err(LiteralEncodeError::Nesting(
            crate::objdef::MAX_LITERAL_NESTING,
        ));
    }
    match value.variant() {
        Variant::List(list) => {
            for value in list.iter_ref() {
                check_depth(value, depth + 1)?;
            }
        }
        Variant::Map(map) => {
            for (key, value) in map.iter_ref() {
                check_depth(key, depth + 1)?;
                check_depth(value, depth + 1)?;
            }
        }
        Variant::Flyweight(value) => {
            for (_, value) in value.slots_storage() {
                check_depth(value, depth + 1)?;
            }
            for value in value.contents().iter_ref() {
                check_depth(value, depth + 1)?;
            }
        }
        Variant::Err(error) => {
            if let Some(value) = error.value() {
                check_depth(value, depth + 1)?;
            }
        }
        Variant::Lambda(lambda) => {
            check_program_depth(&lambda.0.body, depth + 1)?;
            for frame in &lambda.0.captured_env {
                for value in frame {
                    check_depth(value, depth + 1)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_program_depth(program: &crate::Program, depth: usize) -> Result<(), LiteralEncodeError> {
    if depth > crate::objdef::MAX_LITERAL_NESTING {
        return Err(LiteralEncodeError::Nesting(
            crate::objdef::MAX_LITERAL_NESTING,
        ));
    }
    for value in &program.0.literals {
        check_depth(value, depth)?;
    }
    for program in &program.0.lambda_programs {
        check_program_depth(program, depth + 1)?;
    }
    Ok(())
}

pub(crate) fn parse_float(text: &str) -> Result<f64, String> {
    if let Some(hex) = text
        .strip_prefix("f\"")
        .and_then(|text| text.strip_suffix('"'))
    {
        if hex.len() != 16 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("float bits require exactly 16 hexadecimal digits".into());
        }
        let bits = u64::from_str_radix(hex, 16).map_err(|error| error.to_string())?;
        let value = f64::from_bits(bits);
        if value.is_finite() {
            return Err("finite floats must use decimal syntax".into());
        }
        return Ok(value);
    }
    let value = text.parse::<f64>().map_err(|error| error.to_string())?;
    if !value.is_finite() {
        return Err("decimal float is out of range".into());
    }
    Ok(value)
}
pub(crate) fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
/// Canonical escaping: ordinary Unicode stays UTF-8; C0 controls and DEL use hex.
pub(crate) fn write_quoted_string(out: &mut impl Write, text: &str) -> std::fmt::Result {
    out.write_char('"')?;
    for ch in text.chars() {
        match ch {
            '"' => out.write_str("\\\"")?,
            '\\' => out.write_str("\\\\")?,
            '\n' => out.write_str("\\n")?,
            '\r' => out.write_str("\\r")?,
            '\t' => out.write_str("\\t")?,
            ch if (ch as u32) < 32 || ch == '\u{7f}' => write!(out, "\\x{:02X}", ch as u32)?,
            ch => out.write_char(ch)?,
        }
    }
    out.write_char('"')
}

fn validate_source_text(text: &str) -> Result<(), LiteralDecodeError> {
    if let Some(offset) = text.find('\0') {
        return Err(LiteralDecodeError::Source {
            offset,
            message: "NUL must be escaped as \\x00".into(),
        });
    }
    for token in crate::lexer::lex(text) {
        let raw = &text[token.span.clone()];
        let quoted = match token.kind {
            crate::SyntaxKind::StringLit => Some(raw),
            crate::SyntaxKind::SymbolLit if raw.starts_with("'\"") => Some(&raw[1..]),
            crate::SyntaxKind::ErrorLit if raw.starts_with("e\"") => Some(&raw[1..]),
            _ => None,
        };
        let Some(quoted) = quoted else {
            continue;
        };
        let mut chars = quoted.char_indices();
        while let Some((offset, ch)) = chars.next() {
            if ch != '\\' {
                continue;
            }
            let escape = chars.next().map(|(_, ch)| ch);
            let digits = match escape {
                Some('"' | '\\' | '\'' | 'n' | 'r' | 't' | '0') => 0,
                Some('x') => 2,
                Some('u') => 4,
                _ => {
                    return Err(LiteralDecodeError::Source {
                        offset: token.span.start + offset,
                        message: "unsupported string escape".into(),
                    });
                }
            };
            for _ in 0..digits {
                if !chars.next().is_some_and(|(_, ch)| ch.is_ascii_hexdigit()) {
                    return Err(LiteralDecodeError::Source {
                        offset: token.span.start + offset,
                        message: "incomplete hexadecimal escape".into(),
                    });
                }
            }
        }
    }
    Ok(())
}
