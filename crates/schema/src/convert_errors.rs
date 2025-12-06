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

//! Error type conversions between moor types and FlatBuffer types
//!
//! This module handles conversion of error types like Error, WorkerError, SchedulerError,
//! CompileError, CommandError, VerbProgramError, and WorldStateError.

use crate::convert_var::var_from_flatbuffer_ref;
use crate::{
    StrErr, common,
    common::CompileErrorUnionRef,
    convert_common::{symbol_from_flatbuffer_struct, symbol_from_ref, symbol_to_flatbuffer_struct},
    convert_var::{var_from_flatbuffer, var_to_flatbuffer},
    fb_read,
};
use moor_common::model::{CompileContext, CompileError, ParseErrorDetails};
use moor_var::Var;
// ============================================================================
// Helper functions for reducing boilerplate
// ============================================================================

/// Convert a FlatBuffer CompileContextRef to CompileContext
fn compile_context_from_ref(
    ctx_ref: common::CompileContextRef<'_>,
) -> Result<CompileContext, String> {
    let line = fb_read!(ctx_ref, line) as usize;
    let col = fb_read!(ctx_ref, col) as usize;
    Ok(CompileContext::new((line, col)))
}

/// Create a FlatBuffer CompileContext from a CompileContext
fn compile_context_to_flatbuffer(ctx: &CompileContext) -> common::CompileContext {
    common::CompileContext {
        line: ctx.line_col.0 as u64,
        col: ctx.line_col.1 as u64,
    }
}

/// Convert moor_var::ErrorCode to flatbuffer ErrorCode (without custom symbol data)
fn error_code_to_flatbuffer(err: &moor_var::ErrorCode) -> common::ErrorCode {
    use moor_var::ErrorCode::*;
    match err {
        E_NONE => common::ErrorCode::ENone,
        E_TYPE => common::ErrorCode::EType,
        E_DIV => common::ErrorCode::EDiv,
        E_PERM => common::ErrorCode::EPerm,
        E_PROPNF => common::ErrorCode::EPropnf,
        E_VERBNF => common::ErrorCode::EVerbnf,
        E_VARNF => common::ErrorCode::EVarnf,
        E_INVIND => common::ErrorCode::EInvind,
        E_RECMOVE => common::ErrorCode::ERecmove,
        E_MAXREC => common::ErrorCode::EMaxrec,
        E_RANGE => common::ErrorCode::ERange,
        E_ARGS => common::ErrorCode::EArgs,
        E_NACC => common::ErrorCode::ENacc,
        E_INVARG => common::ErrorCode::EInvarg,
        E_QUOTA => common::ErrorCode::EQuota,
        E_FLOAT => common::ErrorCode::EFloat,
        E_FILE => common::ErrorCode::EFile,
        E_EXEC => common::ErrorCode::EExec,
        E_INTRPT => common::ErrorCode::EIntrpt,
        ErrCustom(_) => common::ErrorCode::ErrCustom,
    }
}

/// Convert flatbuffer ErrorCode to moor_var::ErrorCode.
/// For ErrCustom, caller must provide the symbol separately.
fn error_code_from_flatbuffer(
    code: common::ErrorCode,
    custom_symbol: Option<moor_var::Symbol>,
) -> moor_var::ErrorCode {
    use moor_var::ErrorCode::*;
    match code {
        common::ErrorCode::ENone => E_NONE,
        common::ErrorCode::EType => E_TYPE,
        common::ErrorCode::EDiv => E_DIV,
        common::ErrorCode::EPerm => E_PERM,
        common::ErrorCode::EPropnf => E_PROPNF,
        common::ErrorCode::EVerbnf => E_VERBNF,
        common::ErrorCode::EVarnf => E_VARNF,
        common::ErrorCode::EInvind => E_INVIND,
        common::ErrorCode::ERecmove => E_RECMOVE,
        common::ErrorCode::EMaxrec => E_MAXREC,
        common::ErrorCode::ERange => E_RANGE,
        common::ErrorCode::EArgs => E_ARGS,
        common::ErrorCode::ENacc => E_NACC,
        common::ErrorCode::EInvarg => E_INVARG,
        common::ErrorCode::EQuota => E_QUOTA,
        common::ErrorCode::EFloat => E_FLOAT,
        common::ErrorCode::EFile => E_FILE,
        common::ErrorCode::EExec => E_EXEC,
        common::ErrorCode::EIntrpt => E_INTRPT,
        common::ErrorCode::ErrCustom => {
            ErrCustom(custom_symbol.expect("ErrCustom requires custom_symbol"))
        }
    }
}

/// Convert from moor_var::Error to flatbuffer Error struct
pub fn error_to_flatbuffer_struct(
    error: &moor_var::Error,
) -> Result<common::Error, Box<dyn std::error::Error>> {
    let err_code = error_code_to_flatbuffer(&error.err_type);
    let msg = error.msg.as_ref().map(|m| m.as_str().to_string());
    let value = match &error.value {
        Some(v) => Some(Box::new(var_to_flatbuffer(v).str_err()?)),
        None => None,
    };
    let custom_symbol = match &error.err_type {
        moor_var::ErrorCode::ErrCustom(sym) => Some(Box::new(symbol_to_flatbuffer_struct(sym))),
        _ => None,
    };

    Ok(common::Error {
        err_type: err_code,
        msg,
        value,
        custom_symbol,
    })
}

/// Convert from flatbuffer Error struct to moor_var::Error
pub fn error_from_flatbuffer_struct(
    fb_error: &common::Error,
) -> Result<moor_var::Error, Box<dyn std::error::Error>> {
    let custom_symbol = if fb_error.err_type == common::ErrorCode::ErrCustom {
        let sym_struct = fb_error
            .custom_symbol
            .as_ref()
            .ok_or("ErrCustom missing custom_symbol")?;
        Some(symbol_from_flatbuffer_struct(sym_struct))
    } else {
        None
    };
    let err_type = error_code_from_flatbuffer(fb_error.err_type, custom_symbol);

    let msg = fb_error.msg.clone();
    let value = match &fb_error.value {
        Some(v) => Some(var_from_flatbuffer(v).str_err()?),
        None => None,
    };

    Ok(moor_var::Error::new(err_type, msg, value))
}

/// Convert from FlatBuffer ErrorRef to moor_var::Error
pub fn error_from_ref(error_ref: common::ErrorRef<'_>) -> Result<moor_var::Error, String> {
    let error_code = fb_read!(error_ref, err_type);

    let custom_symbol = if error_code == common::ErrorCode::ErrCustom {
        let custom_symbol_ref = error_ref
            .custom_symbol()
            .map_err(|e| format!("Failed to access custom_symbol: {e}"))?
            .ok_or("ErrCustom missing custom_symbol")?;
        Some(symbol_from_ref(custom_symbol_ref)?)
    } else {
        None
    };
    let err_type = error_code_from_flatbuffer(error_code, custom_symbol);

    let msg = error_ref
        .msg()
        .ok()
        .flatten()
        .map(|s| Box::new(s.to_string()));

    let value = if let Ok(Some(value_var_ref)) = error_ref.value() {
        Some(Box::new(
            var_from_flatbuffer_ref(value_var_ref)
                .map_err(|e| format!("Failed to decode error value: {e}"))?,
        ))
    } else {
        None
    };

    Ok(moor_var::Error {
        err_type,
        msg,
        value,
    })
}

/// Convert from FlatBuffer ExceptionRef to Exception
pub fn exception_from_ref(
    exception_ref: common::ExceptionRef<'_>,
) -> Result<moor_common::tasks::Exception, String> {
    let error_value = error_from_ref(fb_read!(exception_ref, error))?;

    let stack_vec = fb_read!(exception_ref, stack);
    let stack: Result<Vec<_>, String> = stack_vec
        .iter()
        .map(|var_ref_result| -> Result<Var, String> {
            let var_ref = var_ref_result.map_err(|e| format!("Failed to get stack item: {e}"))?;
            var_from_flatbuffer_ref(var_ref).map_err(|e| format!("Failed to decode stack var: {e}"))
        })
        .collect();
    let stack = stack?;

    let backtrace_vec = fb_read!(exception_ref, backtrace);
    let backtrace: Result<Vec<_>, String> = backtrace_vec
        .iter()
        .map(|var_ref_result| -> Result<Var, String> {
            let var_ref =
                var_ref_result.map_err(|e| format!("Failed to get backtrace item: {e}"))?;
            var_from_flatbuffer_ref(var_ref)
                .map_err(|e| format!("Failed to decode backtrace var: {e}"))
        })
        .collect();
    let backtrace = backtrace?;

    Ok(moor_common::tasks::Exception {
        error: error_value,
        stack,
        backtrace,
    })
}

/// Convert from FlatBuffer CompileErrorRef to moor_common::model::CompileError
pub fn compilation_error_from_ref(
    error_ref: common::CompileErrorRef<'_>,
) -> Result<CompileError, String> {
    match fb_read!(error_ref, error) {
        CompileErrorUnionRef::StringLexError(e) => {
            let ctx = compile_context_from_ref(fb_read!(e, context))?;
            let msg = fb_read!(e, message).to_string();
            Ok(CompileError::StringLexError(ctx, msg))
        }
        CompileErrorUnionRef::ParseError(e) => {
            let error_position = compile_context_from_ref(fb_read!(e, error_position))?;
            let context = fb_read!(e, context).to_string();
            let message = fb_read!(e, message).to_string();
            let end_line_col = if fb_read!(e, has_end) {
                Some((
                    fb_read!(e, end_line) as usize,
                    fb_read!(e, end_col) as usize,
                ))
            } else {
                None
            };
            let span = if fb_read!(e, has_span) {
                Some((
                    fb_read!(e, span_start) as usize,
                    fb_read!(e, span_end) as usize,
                ))
            } else {
                None
            };

            let expected_tokens = match fb_read!(e, expected_tokens) {
                Some(tokens_vec) => tokens_vec
                    .iter()
                    .map(|token_ref| {
                        token_ref
                            .map_err(|e| format!("Failed to read expected token: {e}"))
                            .map(|token| token.to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                None => Vec::new(),
            };

            let notes = match fb_read!(e, notes) {
                Some(notes_vec) => notes_vec
                    .iter()
                    .map(|note_ref| {
                        note_ref
                            .map_err(|e| format!("Failed to read note: {e}"))
                            .map(|note| note.to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                None => Vec::new(),
            };

            Ok(CompileError::ParseError {
                error_position,
                context,
                end_line_col,
                message,
                details: Box::new(ParseErrorDetails {
                    span,
                    expected_tokens,
                    notes,
                }),
            })
        }
        CompileErrorUnionRef::UnknownBuiltinFunction(e) => {
            let ctx = compile_context_from_ref(fb_read!(e, context))?;
            let name = fb_read!(e, name).to_string();
            Ok(CompileError::UnknownBuiltinFunction(ctx, name))
        }
        CompileErrorUnionRef::UnknownTypeConstant(e) => {
            let ctx = compile_context_from_ref(fb_read!(e, context))?;
            let name = fb_read!(e, name).to_string();
            Ok(CompileError::UnknownTypeConstant(ctx, name))
        }
        CompileErrorUnionRef::UnknownLoopLabel(e) => {
            let ctx = compile_context_from_ref(fb_read!(e, context))?;
            let label = fb_read!(e, label).to_string();
            Ok(CompileError::UnknownLoopLabel(ctx, label))
        }
        CompileErrorUnionRef::DuplicateVariable(e) => {
            let ctx = compile_context_from_ref(fb_read!(e, context))?;
            let var_ref = fb_read!(e, var_name);
            let var_struct = common::Symbol::try_from(var_ref)
                .map_err(|e| format!("Failed to convert var_name: {e}"))?;
            let var_name = symbol_from_flatbuffer_struct(&var_struct);
            Ok(CompileError::DuplicateVariable(ctx, var_name))
        }
        CompileErrorUnionRef::AssignToConst(e) => {
            let ctx = compile_context_from_ref(fb_read!(e, context))?;
            let var_ref = fb_read!(e, var_name);
            let var_struct = common::Symbol::try_from(var_ref)
                .map_err(|e| format!("Failed to convert var_name: {e}"))?;
            let var_name = symbol_from_flatbuffer_struct(&var_struct);
            Ok(CompileError::AssignToConst(ctx, var_name))
        }
        CompileErrorUnionRef::DisabledFeature(e) => {
            let ctx = compile_context_from_ref(fb_read!(e, context))?;
            let feature = fb_read!(e, feature).to_string();
            Ok(CompileError::DisabledFeature(ctx, feature))
        }
        CompileErrorUnionRef::BadSlotName(e) => {
            let ctx = compile_context_from_ref(fb_read!(e, context))?;
            let slot = fb_read!(e, slot).to_string();
            Ok(CompileError::BadSlotName(ctx, slot))
        }
        CompileErrorUnionRef::InvalidAssignment(e) => {
            let ctx = compile_context_from_ref(fb_read!(e, context))?;
            Ok(CompileError::InvalidAssignmentTarget(ctx))
        }
        CompileErrorUnionRef::InvalidTypeLiteralAssignment(e) => {
            let ctx = compile_context_from_ref(fb_read!(e, context))?;
            let literal = fb_read!(e, literal).to_string();
            Ok(CompileError::InvalidTypeLiteralAssignment(literal, ctx))
        }
    }
}

/// Convert from moor_common::model::CompileError to FlatBuffer CompileError
pub fn compilation_error_to_flatbuffer_struct(
    error: &CompileError,
) -> Result<common::CompileError, String> {
    let error_union = match error {
        CompileError::StringLexError(ctx, msg) => {
            common::CompileErrorUnion::StringLexError(Box::new(common::StringLexError {
                context: Box::new(compile_context_to_flatbuffer(ctx)),
                message: msg.clone(),
            }))
        }
        CompileError::ParseError {
            error_position,
            context,
            end_line_col,
            message,
            details,
        } => common::CompileErrorUnion::ParseError(Box::new(common::ParseError {
            error_position: Box::new(compile_context_to_flatbuffer(error_position)),
            context: context.clone(),
            end_line: end_line_col.map(|(l, _)| l as u64).unwrap_or(0),
            end_col: end_line_col.map(|(_, c)| c as u64).unwrap_or(0),
            has_end: end_line_col.is_some(),
            message: message.clone(),
            span_start: details.span.map(|(s, _)| s as u64).unwrap_or(0),
            span_end: details.span.map(|(_, e)| e as u64).unwrap_or(0),
            has_span: details.span.is_some(),
            expected_tokens: if details.expected_tokens.is_empty() {
                None
            } else {
                Some(details.expected_tokens.clone())
            },
            notes: if details.notes.is_empty() {
                None
            } else {
                Some(details.notes.clone())
            },
        })),
        CompileError::UnknownBuiltinFunction(ctx, name) => {
            common::CompileErrorUnion::UnknownBuiltinFunction(Box::new(
                common::UnknownBuiltinFunction {
                    context: Box::new(compile_context_to_flatbuffer(ctx)),
                    name: name.clone(),
                },
            ))
        }
        CompileError::UnknownTypeConstant(ctx, name) => {
            common::CompileErrorUnion::UnknownTypeConstant(Box::new(common::UnknownTypeConstant {
                context: Box::new(compile_context_to_flatbuffer(ctx)),
                name: name.clone(),
            }))
        }
        CompileError::UnknownLoopLabel(ctx, label) => {
            common::CompileErrorUnion::UnknownLoopLabel(Box::new(common::UnknownLoopLabel {
                context: Box::new(compile_context_to_flatbuffer(ctx)),
                label: label.clone(),
            }))
        }
        CompileError::DuplicateVariable(ctx, var_name) => {
            common::CompileErrorUnion::DuplicateVariable(Box::new(common::DuplicateVariable {
                context: Box::new(compile_context_to_flatbuffer(ctx)),
                var_name: Box::new(symbol_to_flatbuffer_struct(var_name)),
            }))
        }
        CompileError::AssignToConst(ctx, var_name) => {
            common::CompileErrorUnion::AssignToConst(Box::new(common::AssignToConst {
                context: Box::new(compile_context_to_flatbuffer(ctx)),
                var_name: Box::new(symbol_to_flatbuffer_struct(var_name)),
            }))
        }
        CompileError::DisabledFeature(ctx, feature) => {
            common::CompileErrorUnion::DisabledFeature(Box::new(common::DisabledFeature {
                context: Box::new(compile_context_to_flatbuffer(ctx)),
                feature: feature.clone(),
            }))
        }
        CompileError::BadSlotName(ctx, slot) => {
            common::CompileErrorUnion::BadSlotName(Box::new(common::BadSlotName {
                context: Box::new(compile_context_to_flatbuffer(ctx)),
                slot: slot.clone(),
            }))
        }
        CompileError::InvalidAssignmentTarget(ctx) => {
            common::CompileErrorUnion::InvalidAssignment(Box::new(common::InvalidAssignment {
                context: Box::new(compile_context_to_flatbuffer(ctx)),
            }))
        }
        CompileError::InvalidTypeLiteralAssignment(literal, ctx) => {
            common::CompileErrorUnion::InvalidTypeLiteralAssignment(Box::new(
                common::InvalidTypeLiteralAssignment {
                    context: Box::new(compile_context_to_flatbuffer(ctx)),
                    literal: literal.clone(),
                },
            ))
        }
    };

    Ok(common::CompileError { error: error_union })
}
