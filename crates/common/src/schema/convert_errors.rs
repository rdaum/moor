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

use crate::{
    model::{CompileContext, CompileError},
    schema::{
        common,
        common::CompileErrorUnionRef,
        convert_common::{
            symbol_from_flatbuffer_struct, symbol_from_ref, symbol_to_flatbuffer_struct,
        },
        convert_var::{var_from_flatbuffer, var_to_flatbuffer},
    },
};
use moor_var::Var;

/// Convert from moor_var::Error to flatbuffer Error struct
pub fn error_to_flatbuffer_struct(
    error: &moor_var::Error,
) -> Result<common::Error, Box<dyn std::error::Error>> {
    use moor_var::ErrorCode as VarErrorCode;

    let err_code = match error.err_type {
        VarErrorCode::E_NONE => common::ErrorCode::ENone,
        VarErrorCode::E_TYPE => common::ErrorCode::EType,
        VarErrorCode::E_DIV => common::ErrorCode::EDiv,
        VarErrorCode::E_PERM => common::ErrorCode::EPerm,
        VarErrorCode::E_PROPNF => common::ErrorCode::EPropnf,
        VarErrorCode::E_VERBNF => common::ErrorCode::EVerbnf,
        VarErrorCode::E_VARNF => common::ErrorCode::EVarnf,
        VarErrorCode::E_INVIND => common::ErrorCode::EInvind,
        VarErrorCode::E_RECMOVE => common::ErrorCode::ERecmove,
        VarErrorCode::E_MAXREC => common::ErrorCode::EMaxrec,
        VarErrorCode::E_RANGE => common::ErrorCode::ERange,
        VarErrorCode::E_ARGS => common::ErrorCode::EArgs,
        VarErrorCode::E_NACC => common::ErrorCode::ENacc,
        VarErrorCode::E_INVARG => common::ErrorCode::EInvarg,
        VarErrorCode::E_QUOTA => common::ErrorCode::EQuota,
        VarErrorCode::E_FLOAT => common::ErrorCode::EFloat,
        VarErrorCode::E_FILE => common::ErrorCode::EFile,
        VarErrorCode::E_EXEC => common::ErrorCode::EExec,
        VarErrorCode::E_INTRPT => common::ErrorCode::EIntrpt,
        VarErrorCode::ErrCustom(_) => common::ErrorCode::ErrCustom,
    };

    let msg = error.msg.as_ref().map(|m| m.as_str().to_string());
    let value = match &error.value {
        Some(v) => Some(Box::new(var_to_flatbuffer(v).map_err(|e| e.to_string())?)),
        None => None,
    };
    let custom_symbol = match &error.err_type {
        VarErrorCode::ErrCustom(sym) => Some(Box::new(symbol_to_flatbuffer_struct(sym))),
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
    use moor_var::ErrorCode as VarErrorCode;

    let err_type = match fb_error.err_type {
        common::ErrorCode::ENone => VarErrorCode::E_NONE,
        common::ErrorCode::EType => VarErrorCode::E_TYPE,
        common::ErrorCode::EDiv => VarErrorCode::E_DIV,
        common::ErrorCode::EPerm => VarErrorCode::E_PERM,
        common::ErrorCode::EPropnf => VarErrorCode::E_PROPNF,
        common::ErrorCode::EVerbnf => VarErrorCode::E_VERBNF,
        common::ErrorCode::EVarnf => VarErrorCode::E_VARNF,
        common::ErrorCode::EInvind => VarErrorCode::E_INVIND,
        common::ErrorCode::ERecmove => VarErrorCode::E_RECMOVE,
        common::ErrorCode::EMaxrec => VarErrorCode::E_MAXREC,
        common::ErrorCode::ERange => VarErrorCode::E_RANGE,
        common::ErrorCode::EArgs => VarErrorCode::E_ARGS,
        common::ErrorCode::ENacc => VarErrorCode::E_NACC,
        common::ErrorCode::EInvarg => VarErrorCode::E_INVARG,
        common::ErrorCode::EQuota => VarErrorCode::E_QUOTA,
        common::ErrorCode::EFloat => VarErrorCode::E_FLOAT,
        common::ErrorCode::EFile => VarErrorCode::E_FILE,
        common::ErrorCode::EExec => VarErrorCode::E_EXEC,
        common::ErrorCode::EIntrpt => VarErrorCode::E_INTRPT,
        common::ErrorCode::ErrCustom => {
            let custom_symbol = fb_error
                .custom_symbol
                .as_ref()
                .ok_or("ErrCustom missing custom_symbol")?;
            VarErrorCode::ErrCustom(symbol_from_flatbuffer_struct(custom_symbol))
        }
    };

    let msg = fb_error.msg.clone();
    let value = match &fb_error.value {
        Some(v) => Some(var_from_flatbuffer(v).map_err(|e| e.to_string())?),
        None => None,
    };

    Ok(moor_var::Error::new(err_type, msg, value))
}

/// Convert from FlatBuffer ErrorRef to moor_var::Error
pub fn error_from_ref(error_ref: common::ErrorRef<'_>) -> Result<moor_var::Error, String> {
    use common::ErrorCode as FbErr;
    use moor_var::ErrorCode as VarErrorCode;

    let error_code = error_ref.err_type().map_err(|_| "Missing err_type")?;

    let err_type = match error_code {
        FbErr::ENone => VarErrorCode::E_NONE,
        FbErr::EType => VarErrorCode::E_TYPE,
        FbErr::EDiv => VarErrorCode::E_DIV,
        FbErr::EPerm => VarErrorCode::E_PERM,
        FbErr::EPropnf => VarErrorCode::E_PROPNF,
        FbErr::EVerbnf => VarErrorCode::E_VERBNF,
        FbErr::EVarnf => VarErrorCode::E_VARNF,
        FbErr::EInvind => VarErrorCode::E_INVIND,
        FbErr::ERecmove => VarErrorCode::E_RECMOVE,
        FbErr::EMaxrec => VarErrorCode::E_MAXREC,
        FbErr::ERange => VarErrorCode::E_RANGE,
        FbErr::EArgs => VarErrorCode::E_ARGS,
        FbErr::ENacc => VarErrorCode::E_NACC,
        FbErr::EInvarg => VarErrorCode::E_INVARG,
        FbErr::EQuota => VarErrorCode::E_QUOTA,
        FbErr::EFloat => VarErrorCode::E_FLOAT,
        FbErr::EFile => VarErrorCode::E_FILE,
        FbErr::EExec => VarErrorCode::E_EXEC,
        FbErr::EIntrpt => VarErrorCode::E_INTRPT,
        FbErr::ErrCustom => {
            let custom_symbol_ref = error_ref
                .custom_symbol()
                .map_err(|_| "Failed to access custom_symbol")?
                .ok_or("ErrCustom missing custom_symbol")?;
            let custom_symbol = symbol_from_ref(custom_symbol_ref)?;
            VarErrorCode::ErrCustom(custom_symbol)
        }
    };

    let msg = error_ref
        .msg()
        .ok()
        .flatten()
        .map(|s| Box::new(s.to_string()));

    let value = if let Ok(Some(value_var_ref)) = error_ref.value() {
        let var_struct = crate::schema::var::Var::try_from(value_var_ref)
            .map_err(|e| format!("Failed to convert value ref: {e}"))?;
        Some(Box::new(
            var_from_flatbuffer(&var_struct)
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
) -> Result<crate::tasks::Exception, String> {
    let error_ref = exception_ref.error().map_err(|_| "Missing error")?;
    let error_value = error_from_ref(error_ref)?;

    let stack_vec = exception_ref.stack().map_err(|_| "Missing stack")?;
    let stack: Result<Vec<_>, String> = stack_vec
        .iter()
        .map(|var_ref_result| -> Result<Var, String> {
            let var_ref = var_ref_result.map_err(|e| format!("Failed to get stack item: {e}"))?;
            let var_struct = crate::schema::var::Var::try_from(var_ref)
                .map_err(|e| format!("Failed to convert stack item: {e}"))?;
            var_from_flatbuffer(&var_struct).map_err(|e| format!("Failed to decode stack var: {e}"))
        })
        .collect();
    let stack = stack?;

    let backtrace_vec = exception_ref.backtrace().map_err(|_| "Missing backtrace")?;
    let backtrace: Result<Vec<_>, String> = backtrace_vec
        .iter()
        .map(|var_ref_result| -> Result<Var, String> {
            let var_ref =
                var_ref_result.map_err(|e| format!("Failed to get backtrace item: {e}"))?;
            let var_struct = crate::schema::var::Var::try_from(var_ref)
                .map_err(|e| format!("Failed to convert backtrace item: {e}"))?;
            var_from_flatbuffer(&var_struct)
                .map_err(|e| format!("Failed to decode backtrace var: {e}"))
        })
        .collect();
    let backtrace = backtrace?;

    Ok(crate::tasks::Exception {
        error: error_value,
        stack,
        backtrace,
    })
}

/// Convert from FlatBuffer CompileErrorRef to moor_common::model::CompileError
pub fn compilation_error_from_ref(
    error_ref: common::CompileErrorRef<'_>,
) -> Result<CompileError, String> {
    let error_union = error_ref.error().map_err(|_| "Missing error union")?;

    match error_union {
        CompileErrorUnionRef::StringLexError(e) => {
            let ctx_ref = e.context().map_err(|_| "Missing context")?;
            let ctx = CompileContext::new((
                ctx_ref.line().map_err(|_| "Missing line")? as usize,
                ctx_ref.col().map_err(|_| "Missing col")? as usize,
            ));
            let msg = e.message().map_err(|_| "Missing message")?.to_string();
            Ok(CompileError::StringLexError(ctx, msg))
        }
        CompileErrorUnionRef::ParseError(e) => {
            let pos_ref = e.error_position().map_err(|_| "Missing error_position")?;
            let error_position = CompileContext::new((
                pos_ref.line().map_err(|_| "Missing line")? as usize,
                pos_ref.col().map_err(|_| "Missing col")? as usize,
            ));
            let context = e.context().map_err(|_| "Missing context")?.to_string();
            let message = e.message().map_err(|_| "Missing message")?.to_string();
            let end_line_col = if e.has_end().map_err(|_| "Missing has_end")? {
                Some((
                    e.end_line().map_err(|_| "Missing end_line")? as usize,
                    e.end_col().map_err(|_| "Missing end_col")? as usize,
                ))
            } else {
                None
            };
            Ok(CompileError::ParseError {
                error_position,
                context,
                end_line_col,
                message,
            })
        }
        CompileErrorUnionRef::UnknownBuiltinFunction(e) => {
            let ctx_ref = e.context().map_err(|_| "Missing context")?;
            let ctx = CompileContext::new((
                ctx_ref.line().map_err(|_| "Missing line")? as usize,
                ctx_ref.col().map_err(|_| "Missing col")? as usize,
            ));
            let name = e.name().map_err(|_| "Missing name")?.to_string();
            Ok(CompileError::UnknownBuiltinFunction(ctx, name))
        }
        CompileErrorUnionRef::UnknownTypeConstant(e) => {
            let ctx_ref = e.context().map_err(|_| "Missing context")?;
            let ctx = CompileContext::new((
                ctx_ref.line().map_err(|_| "Missing line")? as usize,
                ctx_ref.col().map_err(|_| "Missing col")? as usize,
            ));
            let name = e.name().map_err(|_| "Missing name")?.to_string();
            Ok(CompileError::UnknownTypeConstant(ctx, name))
        }
        CompileErrorUnionRef::UnknownLoopLabel(e) => {
            let ctx_ref = e.context().map_err(|_| "Missing context")?;
            let ctx = CompileContext::new((
                ctx_ref.line().map_err(|_| "Missing line")? as usize,
                ctx_ref.col().map_err(|_| "Missing col")? as usize,
            ));
            let label = e.label().map_err(|_| "Missing label")?.to_string();
            Ok(CompileError::UnknownLoopLabel(ctx, label))
        }
        CompileErrorUnionRef::DuplicateVariable(e) => {
            let ctx_ref = e.context().map_err(|_| "Missing context")?;
            let ctx = CompileContext::new((
                ctx_ref.line().map_err(|_| "Missing line")? as usize,
                ctx_ref.col().map_err(|_| "Missing col")? as usize,
            ));
            let var_ref = e.var_name().map_err(|_| "Missing var_name")?;
            let var_struct =
                common::Symbol::try_from(var_ref).map_err(|_| "Failed to convert var_name")?;
            let var_name = symbol_from_flatbuffer_struct(&var_struct);
            Ok(CompileError::DuplicateVariable(ctx, var_name))
        }
        CompileErrorUnionRef::AssignToConst(e) => {
            let ctx_ref = e.context().map_err(|_| "Missing context")?;
            let ctx = CompileContext::new((
                ctx_ref.line().map_err(|_| "Missing line")? as usize,
                ctx_ref.col().map_err(|_| "Missing col")? as usize,
            ));
            let var_ref = e.var_name().map_err(|_| "Missing var_name")?;
            let var_struct =
                common::Symbol::try_from(var_ref).map_err(|_| "Failed to convert var_name")?;
            let var_name = symbol_from_flatbuffer_struct(&var_struct);
            Ok(CompileError::AssignToConst(ctx, var_name))
        }
        CompileErrorUnionRef::DisabledFeature(e) => {
            let ctx_ref = e.context().map_err(|_| "Missing context")?;
            let ctx = CompileContext::new((
                ctx_ref.line().map_err(|_| "Missing line")? as usize,
                ctx_ref.col().map_err(|_| "Missing col")? as usize,
            ));
            let feature = e.feature().map_err(|_| "Missing feature")?.to_string();
            Ok(CompileError::DisabledFeature(ctx, feature))
        }
        CompileErrorUnionRef::BadSlotName(e) => {
            let ctx_ref = e.context().map_err(|_| "Missing context")?;
            let ctx = CompileContext::new((
                ctx_ref.line().map_err(|_| "Missing line")? as usize,
                ctx_ref.col().map_err(|_| "Missing col")? as usize,
            ));
            let slot = e.slot().map_err(|_| "Missing slot")?.to_string();
            Ok(CompileError::BadSlotName(ctx, slot))
        }
        CompileErrorUnionRef::InvalidAssignment(e) => {
            let ctx_ref = e.context().map_err(|_| "Missing context")?;
            let ctx = CompileContext::new((
                ctx_ref.line().map_err(|_| "Missing line")? as usize,
                ctx_ref.col().map_err(|_| "Missing col")? as usize,
            ));
            Ok(CompileError::InvalidAssignemnt(ctx))
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
                context: Box::new(common::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                message: msg.clone(),
            }))
        }
        CompileError::ParseError {
            error_position,
            context,
            end_line_col,
            message,
        } => common::CompileErrorUnion::ParseError(Box::new(common::ParseError {
            error_position: Box::new(common::CompileContext {
                line: error_position.line_col.0 as u64,
                col: error_position.line_col.1 as u64,
            }),
            context: context.clone(),
            end_line: end_line_col.map(|(l, _)| l as u64).unwrap_or(0),
            end_col: end_line_col.map(|(_, c)| c as u64).unwrap_or(0),
            has_end: end_line_col.is_some(),
            message: message.clone(),
        })),
        CompileError::UnknownBuiltinFunction(ctx, name) => {
            common::CompileErrorUnion::UnknownBuiltinFunction(Box::new(
                common::UnknownBuiltinFunction {
                    context: Box::new(common::CompileContext {
                        line: ctx.line_col.0 as u64,
                        col: ctx.line_col.1 as u64,
                    }),
                    name: name.clone(),
                },
            ))
        }
        CompileError::UnknownTypeConstant(ctx, name) => {
            common::CompileErrorUnion::UnknownTypeConstant(Box::new(common::UnknownTypeConstant {
                context: Box::new(common::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                name: name.clone(),
            }))
        }
        CompileError::UnknownLoopLabel(ctx, label) => {
            common::CompileErrorUnion::UnknownLoopLabel(Box::new(common::UnknownLoopLabel {
                context: Box::new(common::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                label: label.clone(),
            }))
        }
        CompileError::DuplicateVariable(ctx, var_name) => {
            common::CompileErrorUnion::DuplicateVariable(Box::new(common::DuplicateVariable {
                context: Box::new(common::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                var_name: Box::new(symbol_to_flatbuffer_struct(var_name)),
            }))
        }
        CompileError::AssignToConst(ctx, var_name) => {
            common::CompileErrorUnion::AssignToConst(Box::new(common::AssignToConst {
                context: Box::new(common::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                var_name: Box::new(symbol_to_flatbuffer_struct(var_name)),
            }))
        }
        CompileError::DisabledFeature(ctx, feature) => {
            common::CompileErrorUnion::DisabledFeature(Box::new(common::DisabledFeature {
                context: Box::new(common::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                feature: feature.clone(),
            }))
        }
        CompileError::BadSlotName(ctx, slot) => {
            common::CompileErrorUnion::BadSlotName(Box::new(common::BadSlotName {
                context: Box::new(common::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                slot: slot.clone(),
            }))
        }
        CompileError::InvalidAssignemnt(ctx) => {
            common::CompileErrorUnion::InvalidAssignment(Box::new(common::InvalidAssignment {
                context: Box::new(common::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
            }))
        }
    };

    Ok(common::CompileError { error: error_union })
}
