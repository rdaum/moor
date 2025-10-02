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
    convert::{
        obj_to_flatbuffer_struct, objectref_to_flatbuffer_struct, symbol_to_flatbuffer_struct,
        var_from_flatbuffer_bytes, var_to_flatbuffer_bytes,
    },
    symbol_from_flatbuffer_struct, symbol_from_ref, uuid_from_ref,
};
use moor_common::{
    model,
    model::{CompileContext, CompileError, WorldStateError},
    schema::{
        common as moor_common_schema,
        common::{CompileErrorUnion, CompileErrorUnionRef},
        rpc,
    },
    tasks::{AbortLimitReason, CommandError, SchedulerError, VerbProgramError, WorkerError},
};
use moor_var::Var;
use std::time::Duration;

/// Convert from moor_var::Error to flatbuffer Error struct
pub fn error_to_flatbuffer_struct(
    error: &moor_var::Error,
) -> Result<moor_common_schema::Error, Box<dyn std::error::Error>> {
    use moor_var::ErrorCode as VarErrorCode;

    let err_code = match error.err_type {
        VarErrorCode::E_NONE => moor_common_schema::ErrorCode::ENone,
        VarErrorCode::E_TYPE => moor_common_schema::ErrorCode::EType,
        VarErrorCode::E_DIV => moor_common_schema::ErrorCode::EDiv,
        VarErrorCode::E_PERM => moor_common_schema::ErrorCode::EPerm,
        VarErrorCode::E_PROPNF => moor_common_schema::ErrorCode::EPropnf,
        VarErrorCode::E_VERBNF => moor_common_schema::ErrorCode::EVerbnf,
        VarErrorCode::E_VARNF => moor_common_schema::ErrorCode::EVarnf,
        VarErrorCode::E_INVIND => moor_common_schema::ErrorCode::EInvind,
        VarErrorCode::E_RECMOVE => moor_common_schema::ErrorCode::ERecmove,
        VarErrorCode::E_MAXREC => moor_common_schema::ErrorCode::EMaxrec,
        VarErrorCode::E_RANGE => moor_common_schema::ErrorCode::ERange,
        VarErrorCode::E_ARGS => moor_common_schema::ErrorCode::EArgs,
        VarErrorCode::E_NACC => moor_common_schema::ErrorCode::ENacc,
        VarErrorCode::E_INVARG => moor_common_schema::ErrorCode::EInvarg,
        VarErrorCode::E_QUOTA => moor_common_schema::ErrorCode::EQuota,
        VarErrorCode::E_FLOAT => moor_common_schema::ErrorCode::EFloat,
        VarErrorCode::E_FILE => moor_common_schema::ErrorCode::EFile,
        VarErrorCode::E_EXEC => moor_common_schema::ErrorCode::EExec,
        VarErrorCode::E_INTRPT => moor_common_schema::ErrorCode::EIntrpt,
        VarErrorCode::ErrCustom(_) => moor_common_schema::ErrorCode::ErrCustom,
    };

    let msg = error.msg.as_ref().map(|m| m.as_str().to_string());
    let value = match &error.value {
        Some(v) => var_to_flatbuffer_bytes(v)
            .ok()
            .map(|data| Box::new(moor_common_schema::VarBytes { data })),
        None => None,
    };
    let custom_symbol = match &error.err_type {
        VarErrorCode::ErrCustom(sym) => Some(Box::new(symbol_to_flatbuffer_struct(sym))),
        _ => None,
    };

    Ok(moor_common_schema::Error {
        err_type: err_code,
        msg,
        value,
        custom_symbol,
    })
}

/// Convert from flatbuffer Error struct to moor_var::Error
pub fn error_from_flatbuffer_struct(
    fb_error: &moor_common_schema::Error,
) -> Result<moor_var::Error, Box<dyn std::error::Error>> {
    use moor_var::ErrorCode as VarErrorCode;

    let err_type = match fb_error.err_type {
        moor_common_schema::ErrorCode::ENone => VarErrorCode::E_NONE,
        moor_common_schema::ErrorCode::EType => VarErrorCode::E_TYPE,
        moor_common_schema::ErrorCode::EDiv => VarErrorCode::E_DIV,
        moor_common_schema::ErrorCode::EPerm => VarErrorCode::E_PERM,
        moor_common_schema::ErrorCode::EPropnf => VarErrorCode::E_PROPNF,
        moor_common_schema::ErrorCode::EVerbnf => VarErrorCode::E_VERBNF,
        moor_common_schema::ErrorCode::EVarnf => VarErrorCode::E_VARNF,
        moor_common_schema::ErrorCode::EInvind => VarErrorCode::E_INVIND,
        moor_common_schema::ErrorCode::ERecmove => VarErrorCode::E_RECMOVE,
        moor_common_schema::ErrorCode::EMaxrec => VarErrorCode::E_MAXREC,
        moor_common_schema::ErrorCode::ERange => VarErrorCode::E_RANGE,
        moor_common_schema::ErrorCode::EArgs => VarErrorCode::E_ARGS,
        moor_common_schema::ErrorCode::ENacc => VarErrorCode::E_NACC,
        moor_common_schema::ErrorCode::EInvarg => VarErrorCode::E_INVARG,
        moor_common_schema::ErrorCode::EQuota => VarErrorCode::E_QUOTA,
        moor_common_schema::ErrorCode::EFloat => VarErrorCode::E_FLOAT,
        moor_common_schema::ErrorCode::EFile => VarErrorCode::E_FILE,
        moor_common_schema::ErrorCode::EExec => VarErrorCode::E_EXEC,
        moor_common_schema::ErrorCode::EIntrpt => VarErrorCode::E_INTRPT,
        moor_common_schema::ErrorCode::ErrCustom => {
            let custom_symbol = fb_error
                .custom_symbol
                .as_ref()
                .ok_or("ErrCustom missing custom_symbol")?;
            VarErrorCode::ErrCustom(symbol_from_flatbuffer_struct(custom_symbol))
        }
    };

    let msg = fb_error.msg.clone();
    let value = match &fb_error.value {
        Some(v) => Some(var_from_flatbuffer_bytes(&v.data)?),
        None => None,
    };

    Ok(moor_var::Error::new(err_type, msg, value))
}

/// Convert from WorkerError to flatbuffer WorkerError struct
pub fn worker_error_to_flatbuffer_struct(error: &WorkerError) -> rpc::WorkerError {
    let error_union = match error {
        WorkerError::PermissionDenied(msg) => {
            rpc::WorkerErrorUnion::WorkerPermissionDenied(Box::new(rpc::WorkerPermissionDenied {
                message: msg.clone(),
            }))
        }
        WorkerError::InvalidRequest(msg) => {
            rpc::WorkerErrorUnion::WorkerInvalidRequest(Box::new(rpc::WorkerInvalidRequest {
                message: msg.clone(),
            }))
        }
        WorkerError::InternalError(msg) => {
            rpc::WorkerErrorUnion::WorkerInternalError(Box::new(rpc::WorkerInternalError {
                message: msg.clone(),
            }))
        }
        WorkerError::RequestTimedOut(msg) => {
            rpc::WorkerErrorUnion::WorkerRequestTimedOut(Box::new(rpc::WorkerRequestTimedOut {
                message: msg.clone(),
            }))
        }
        WorkerError::RequestError(msg) => {
            rpc::WorkerErrorUnion::WorkerRequestError(Box::new(rpc::WorkerRequestError {
                message: msg.clone(),
            }))
        }
        WorkerError::WorkerDetached(msg) => {
            rpc::WorkerErrorUnion::WorkerDetached(Box::new(rpc::WorkerDetached {
                message: msg.clone(),
            }))
        }
        WorkerError::NoWorkerAvailable(worker_type) => {
            let worker_type_struct = symbol_to_flatbuffer_struct(worker_type);
            rpc::WorkerErrorUnion::NoWorkerAvailable(Box::new(rpc::NoWorkerAvailable {
                worker_type: Box::new(worker_type_struct),
            }))
        }
    };

    rpc::WorkerError { error: error_union }
}

/// Convert from flatbuffer WorkerError struct to WorkerError
pub fn worker_error_from_flatbuffer_struct(
    fb_error: &rpc::WorkerError,
) -> Result<WorkerError, Box<dyn std::error::Error>> {
    match &fb_error.error {
        rpc::WorkerErrorUnion::WorkerPermissionDenied(perm_denied) => {
            Ok(WorkerError::PermissionDenied(perm_denied.message.clone()))
        }
        rpc::WorkerErrorUnion::WorkerInvalidRequest(invalid_req) => {
            Ok(WorkerError::InvalidRequest(invalid_req.message.clone()))
        }
        rpc::WorkerErrorUnion::WorkerInternalError(internal_err) => {
            Ok(WorkerError::InternalError(internal_err.message.clone()))
        }
        rpc::WorkerErrorUnion::WorkerRequestTimedOut(timeout) => {
            Ok(WorkerError::RequestTimedOut(timeout.message.clone()))
        }
        rpc::WorkerErrorUnion::WorkerRequestError(req_err) => {
            Ok(WorkerError::RequestError(req_err.message.clone()))
        }
        rpc::WorkerErrorUnion::WorkerDetached(detached) => {
            Ok(WorkerError::WorkerDetached(detached.message.clone()))
        }
        rpc::WorkerErrorUnion::NoWorkerAvailable(no_worker) => {
            let worker_type = symbol_from_flatbuffer_struct(&no_worker.worker_type);
            Ok(WorkerError::NoWorkerAvailable(worker_type))
        }
    }
}

/// Convert VerbProgramError to FlatBuffer struct
pub fn verb_program_error_to_flatbuffer_struct(
    error: &moor_common::tasks::VerbProgramError,
) -> Result<rpc::VerbProgramError, String> {
    let error_union = match error {
        VerbProgramError::NoVerbToProgram => {
            rpc::VerbProgramErrorUnion::NoVerbToProgram(Box::new(rpc::NoVerbToProgram {}))
        }
        VerbProgramError::CompilationError(compile_error) => {
            let compile_error_fb = compilation_error_to_flatbuffer_struct(compile_error)?;
            rpc::VerbProgramErrorUnion::VerbCompilationError(Box::new(rpc::VerbCompilationError {
                error: Box::new(compile_error_fb),
            }))
        }
        VerbProgramError::DatabaseError => {
            rpc::VerbProgramErrorUnion::VerbDatabaseError(Box::new(rpc::VerbDatabaseError {}))
        }
    };

    Ok(rpc::VerbProgramError { error: error_union })
}

/// Convert WorldStateError to FlatBuffer struct
pub fn world_state_error_to_flatbuffer_struct(
    error: &model::WorldStateError,
) -> Result<moor_common_schema::WorldStateError, String> {
    let error_union = match error {
        WorldStateError::ObjectNotFound(objref) => {
            moor_common_schema::WorldStateErrorUnion::ObjectNotFound(Box::new(
                moor_common_schema::ObjectNotFound {
                    object_ref: Box::new(objectref_to_flatbuffer_struct(objref)),
                },
            ))
        }
        WorldStateError::ObjectAlreadyExists(obj) => {
            moor_common_schema::WorldStateErrorUnion::ObjectAlreadyExists(Box::new(
                moor_common_schema::ObjectAlreadyExists {
                    obj: Box::new(obj_to_flatbuffer_struct(obj)),
                },
            ))
        }
        WorldStateError::RecursiveMove(from_obj, to_obj) => {
            moor_common_schema::WorldStateErrorUnion::RecursiveMove(Box::new(
                moor_common_schema::RecursiveMove {
                    from_obj: Box::new(obj_to_flatbuffer_struct(from_obj)),
                    to_obj: Box::new(obj_to_flatbuffer_struct(to_obj)),
                },
            ))
        }
        WorldStateError::ObjectPermissionDenied => {
            moor_common_schema::WorldStateErrorUnion::ObjectPermissionDenied(Box::new(
                moor_common_schema::ObjectPermissionDenied {},
            ))
        }
        WorldStateError::PropertyNotFound(obj, property) => {
            moor_common_schema::WorldStateErrorUnion::PropertyNotFound(Box::new(
                moor_common_schema::PropertyNotFound {
                    obj: Box::new(obj_to_flatbuffer_struct(obj)),
                    property: property.clone(),
                },
            ))
        }
        WorldStateError::PropertyPermissionDenied => {
            moor_common_schema::WorldStateErrorUnion::PropertyPermissionDenied(Box::new(
                moor_common_schema::PropertyPermissionDenied {},
            ))
        }
        WorldStateError::PropertyDefinitionNotFound(obj, property) => {
            moor_common_schema::WorldStateErrorUnion::PropertyDefinitionNotFound(Box::new(
                moor_common_schema::PropertyDefinitionNotFound {
                    obj: Box::new(obj_to_flatbuffer_struct(obj)),
                    property: property.clone(),
                },
            ))
        }
        WorldStateError::DuplicatePropertyDefinition(obj, property) => {
            moor_common_schema::WorldStateErrorUnion::DuplicatePropertyDefinition(Box::new(
                moor_common_schema::DuplicatePropertyDefinition {
                    obj: Box::new(obj_to_flatbuffer_struct(obj)),
                    property: property.clone(),
                },
            ))
        }
        WorldStateError::ChparentPropertyNameConflict(descendant, ancestor, property) => {
            moor_common_schema::WorldStateErrorUnion::ChparentPropertyNameConflict(Box::new(
                moor_common_schema::ChparentPropertyNameConflict {
                    descendant: Box::new(obj_to_flatbuffer_struct(descendant)),
                    ancestor: Box::new(obj_to_flatbuffer_struct(ancestor)),
                    property: property.clone(),
                },
            ))
        }
        WorldStateError::PropertyTypeMismatch => {
            moor_common_schema::WorldStateErrorUnion::PropertyTypeMismatch(Box::new(
                moor_common_schema::PropertyTypeMismatch {},
            ))
        }
        WorldStateError::VerbNotFound(obj, verb) => {
            moor_common_schema::WorldStateErrorUnion::VerbNotFound(Box::new(
                moor_common_schema::VerbNotFound {
                    obj: Box::new(obj_to_flatbuffer_struct(obj)),
                    verb: verb.clone(),
                },
            ))
        }
        WorldStateError::InvalidVerb(vid) => moor_common_schema::WorldStateErrorUnion::InvalidVerb(
            Box::new(moor_common_schema::InvalidVerb { vid: vid.0 }),
        ),
        WorldStateError::VerbDecodeError(obj, verb) => {
            moor_common_schema::WorldStateErrorUnion::VerbDecodeError(Box::new(
                moor_common_schema::VerbDecodeError {
                    obj: Box::new(obj_to_flatbuffer_struct(obj)),
                    verb: Box::new(symbol_to_flatbuffer_struct(verb)),
                },
            ))
        }
        WorldStateError::VerbPermissionDenied => {
            moor_common_schema::WorldStateErrorUnion::VerbPermissionDenied(Box::new(
                moor_common_schema::VerbPermissionDenied {},
            ))
        }
        WorldStateError::DuplicateVerb(obj, verb) => {
            moor_common_schema::WorldStateErrorUnion::DuplicateVerb(Box::new(
                moor_common_schema::DuplicateVerb {
                    obj: Box::new(obj_to_flatbuffer_struct(obj)),
                    verb: Box::new(symbol_to_flatbuffer_struct(verb)),
                },
            ))
        }
        WorldStateError::FailedMatch(match_string) => {
            moor_common_schema::WorldStateErrorUnion::FailedMatch(Box::new(
                moor_common_schema::FailedMatch {
                    match_string: match_string.clone(),
                },
            ))
        }
        WorldStateError::AmbiguousMatch(match_string) => {
            moor_common_schema::WorldStateErrorUnion::AmbiguousMatch(Box::new(
                moor_common_schema::AmbiguousMatch {
                    match_string: match_string.clone(),
                },
            ))
        }
        WorldStateError::InvalidRenumber(message) => {
            moor_common_schema::WorldStateErrorUnion::InvalidRenumber(Box::new(
                moor_common_schema::InvalidRenumber {
                    message: message.clone(),
                },
            ))
        }
        WorldStateError::DatabaseError(message) => {
            moor_common_schema::WorldStateErrorUnion::WorldStateDatabaseError(Box::new(
                moor_common_schema::WorldStateDatabaseError {
                    message: message.clone(),
                },
            ))
        }
        WorldStateError::RollbackRetry => moor_common_schema::WorldStateErrorUnion::RollbackRetry(
            Box::new(moor_common_schema::RollbackRetry {}),
        ),
    };

    Ok(moor_common_schema::WorldStateError { error: error_union })
}

/// Convert from FlatBuffer ErrorRef to moor_var::Error
pub fn error_from_ref(
    error_ref: moor_common_schema::ErrorRef<'_>,
) -> Result<moor_var::Error, String> {
    use moor_common_schema::ErrorCode as FbErr;
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

    let value = if let Ok(Some(value_bytes_ref)) = error_ref.value() {
        let value_data = value_bytes_ref.data().map_err(|_| "Missing value data")?;
        Some(Box::new(var_from_flatbuffer_bytes(value_data).map_err(
            |e| format!("Failed to decode error value: {e}"),
        )?))
    } else {
        None
    };

    Ok(moor_var::Error {
        err_type,
        msg,
        value,
    })
}

/// Convert from FlatBuffer VerbProgramErrorRef to VerbProgramError
fn verb_program_error_from_ref(
    error_ref: rpc::VerbProgramErrorRef<'_>,
) -> Result<moor_common::tasks::VerbProgramError, String> {
    match error_ref
        .error()
        .map_err(|_| "Failed to read VerbProgramError union")?
    {
        rpc::VerbProgramErrorUnionRef::NoVerbToProgram(_) => {
            Ok(moor_common::tasks::VerbProgramError::NoVerbToProgram)
        }
        rpc::VerbProgramErrorUnionRef::VerbCompilationError(_compile_error) => {
            // TODO: Implement CompileError conversion if needed
            Err("VerbCompilationError conversion not yet implemented".to_string())
        }
        rpc::VerbProgramErrorUnionRef::VerbDatabaseError(_) => {
            Ok(moor_common::tasks::VerbProgramError::DatabaseError)
        }
    }
}

/// Convert from FlatBuffer SchedulerErrorRef to SchedulerError
pub fn scheduler_error_from_ref(
    error_ref: rpc::SchedulerErrorRef<'_>,
) -> Result<SchedulerError, String> {
    match error_ref
        .error()
        .map_err(|_| "Failed to read SchedulerError union")?
    {
        rpc::SchedulerErrorUnionRef::SchedulerNotResponding(_) => {
            Ok(SchedulerError::SchedulerNotResponding)
        }
        rpc::SchedulerErrorUnionRef::TaskNotFound(task_not_found) => {
            let task_id = task_not_found.task_id().map_err(|_| "Missing task_id")? as usize;
            Ok(SchedulerError::TaskNotFound(task_id))
        }
        rpc::SchedulerErrorUnionRef::InputRequestNotFound(input_not_found) => {
            let request_id = uuid_from_ref(
                input_not_found
                    .request_id()
                    .map_err(|_| "Missing request_id")?,
            )?;
            Ok(SchedulerError::InputRequestNotFound(request_id.as_u128()))
        }
        rpc::SchedulerErrorUnionRef::CouldNotStartTask(_) => Ok(SchedulerError::CouldNotStartTask),
        rpc::SchedulerErrorUnionRef::CompilationError(_compile_error) => {
            // CompileError is too complex to fully deserialize from bytes here
            // Return a placeholder for now
            Err("CompilationError deserialization not yet fully implemented".to_string())
        }
        rpc::SchedulerErrorUnionRef::CommandExecutionError(cmd_error) => {
            let command_error =
                command_error_from_ref(cmd_error.error().map_err(|_| "Missing command error")?)?;
            Ok(SchedulerError::CommandExecutionError(command_error))
        }
        rpc::SchedulerErrorUnionRef::TaskAbortedLimit(task_aborted) => {
            let limit_ref = task_aborted.limit().map_err(|_| "Missing limit")?;
            let reason_enum = limit_ref.reason().map_err(|_| "Missing reason")?;
            let abort_reason = match reason_enum {
                rpc::AbortLimitReason::Ticks => {
                    let ticks = limit_ref.ticks().map_err(|_| "Missing ticks")? as usize;
                    AbortLimitReason::Ticks(ticks)
                }
                rpc::AbortLimitReason::Time => {
                    let time_nanos = limit_ref.time_nanos().map_err(|_| "Missing time_nanos")?;
                    AbortLimitReason::Time(Duration::from_nanos(time_nanos))
                }
            };
            Ok(SchedulerError::TaskAbortedLimit(abort_reason))
        }
        rpc::SchedulerErrorUnionRef::TaskAbortedError(_) => Ok(SchedulerError::TaskAbortedError),
        rpc::SchedulerErrorUnionRef::TaskAbortedException(task_aborted) => {
            let exception_ref = task_aborted.exception().map_err(|_| "Missing exception")?;
            let exception = exception_from_ref(exception_ref)?;
            Ok(SchedulerError::TaskAbortedException(exception))
        }
        rpc::SchedulerErrorUnionRef::TaskAbortedCancelled(_) => {
            Ok(SchedulerError::TaskAbortedCancelled)
        }
        rpc::SchedulerErrorUnionRef::VerbProgramFailed(verb_failed) => {
            let verb_error = verb_program_error_from_ref(
                verb_failed.error().map_err(|_| "Missing verb_error")?,
            )?;
            Ok(SchedulerError::VerbProgramFailed(verb_error))
        }
        rpc::SchedulerErrorUnionRef::PropertyRetrievalFailed(_prop_failed) => {
            // WorldStateError is too complex to fully deserialize from bytes
            Err("PropertyRetrievalFailed deserialization not yet fully implemented".to_string())
        }
        rpc::SchedulerErrorUnionRef::VerbRetrievalFailed(_verb_failed) => {
            // WorldStateError is too complex to fully deserialize from bytes
            Err("VerbRetrievalFailed deserialization not yet fully implemented".to_string())
        }
        rpc::SchedulerErrorUnionRef::ObjectResolutionFailed(_obj_failed) => {
            // WorldStateError is too complex to fully deserialize from bytes
            Err("ObjectResolutionFailed deserialization not yet fully implemented".to_string())
        }
        rpc::SchedulerErrorUnionRef::GarbageCollectionFailed(gc_failed) => {
            let message = gc_failed
                .message()
                .map_err(|_| "Missing message")?
                .to_string();
            Ok(SchedulerError::GarbageCollectionFailed(message))
        }
    }
}

/// Convert from FlatBuffer CommandErrorRef to CommandError
fn command_error_from_ref(error_ref: rpc::CommandErrorRef<'_>) -> Result<CommandError, String> {
    match error_ref
        .error()
        .map_err(|_| "Failed to read CommandError union")?
    {
        rpc::CommandErrorUnionRef::CouldNotParseCommand(_) => {
            Ok(CommandError::CouldNotParseCommand)
        }
        rpc::CommandErrorUnionRef::NoObjectMatch(_) => Ok(CommandError::NoObjectMatch),
        rpc::CommandErrorUnionRef::NoCommandMatch(_) => Ok(CommandError::NoCommandMatch),
        rpc::CommandErrorUnionRef::DatabaseError(_db_error) => {
            // WorldStateError deserialization is complex
            Err("CommandError::DatabaseError deserialization not yet fully implemented".to_string())
        }
        rpc::CommandErrorUnionRef::PermissionDenied(_) => Ok(CommandError::PermissionDenied),
    }
}

/// Convert CommandError to FlatBuffer struct
pub fn command_error_to_flatbuffer_struct(
    error: &CommandError,
) -> Result<rpc::CommandError, moor_var::EncodingError> {
    let error_union = match error {
        CommandError::CouldNotParseCommand => {
            rpc::CommandErrorUnion::CouldNotParseCommand(Box::new(rpc::CouldNotParseCommand {}))
        }
        CommandError::NoObjectMatch => {
            rpc::CommandErrorUnion::NoObjectMatch(Box::new(rpc::NoObjectMatch {}))
        }
        CommandError::NoCommandMatch => {
            rpc::CommandErrorUnion::NoCommandMatch(Box::new(rpc::NoCommandMatch {}))
        }
        CommandError::DatabaseError(ws_error) => {
            let ws_error_fb = world_state_error_to_flatbuffer_struct(ws_error).map_err(|e| {
                moor_var::EncodingError::CouldNotEncode(format!(
                    "Failed to encode WorldStateError: {e}"
                ))
            })?;
            rpc::CommandErrorUnion::DatabaseError(Box::new(rpc::DatabaseError {
                error: Box::new(ws_error_fb),
            }))
        }
        CommandError::PermissionDenied => {
            rpc::CommandErrorUnion::PermissionDenied(Box::new(rpc::PermissionDenied {}))
        }
    };

    Ok(rpc::CommandError { error: error_union })
}

/// Convert SchedulerError to FlatBuffer struct
pub fn scheduler_error_to_flatbuffer_struct(
    error: &SchedulerError,
) -> Result<rpc::SchedulerError, moor_var::EncodingError> {
    let error_union = match error {
        SchedulerError::SchedulerNotResponding => rpc::SchedulerErrorUnion::SchedulerNotResponding(
            Box::new(rpc::SchedulerNotResponding {}),
        ),
        SchedulerError::TaskNotFound(task_id) => {
            rpc::SchedulerErrorUnion::TaskNotFound(Box::new(rpc::TaskNotFound {
                task_id: *task_id as u64,
            }))
        }
        SchedulerError::InputRequestNotFound(request_id) => {
            rpc::SchedulerErrorUnion::InputRequestNotFound(Box::new(rpc::InputRequestNotFound {
                request_id: Box::new(rpc::Uuid {
                    data: request_id.to_be_bytes().to_vec(),
                }),
            }))
        }
        SchedulerError::CouldNotStartTask => {
            rpc::SchedulerErrorUnion::CouldNotStartTask(Box::new(rpc::CouldNotStartTask {}))
        }
        SchedulerError::CompilationError(compile_error) => {
            let compile_error_fb = compilation_error_to_flatbuffer_struct(compile_error)
                .map_err(moor_var::EncodingError::CouldNotEncode)?;
            rpc::SchedulerErrorUnion::CompilationError(Box::new(rpc::CompilationError {
                error: Box::new(compile_error_fb),
            }))
        }
        SchedulerError::CommandExecutionError(command_error) => {
            let cmd_error = command_error_to_flatbuffer_struct(command_error)?;
            rpc::SchedulerErrorUnion::CommandExecutionError(Box::new(rpc::CommandExecutionError {
                error: Box::new(cmd_error),
            }))
        }
        SchedulerError::TaskAbortedLimit(abort_reason) => {
            let (reason_enum, ticks, time_nanos) = match abort_reason {
                AbortLimitReason::Ticks(t) => (rpc::AbortLimitReason::Ticks, *t as u64, 0u64),
                AbortLimitReason::Time(d) => {
                    (rpc::AbortLimitReason::Time, 0u64, d.as_nanos() as u64)
                }
            };
            rpc::SchedulerErrorUnion::TaskAbortedLimit(Box::new(rpc::TaskAbortedLimit {
                limit: Box::new(rpc::AbortLimit {
                    reason: reason_enum,
                    ticks,
                    time_nanos,
                }),
            }))
        }
        SchedulerError::TaskAbortedError => {
            rpc::SchedulerErrorUnion::TaskAbortedError(Box::new(rpc::TaskAbortedError {}))
        }
        SchedulerError::TaskAbortedException(exception) => {
            // Serialize the exception
            let error_bytes = error_to_flatbuffer_struct(&exception.error).map_err(|e| {
                moor_var::EncodingError::CouldNotEncode(format!(
                    "Failed to encode exception error: {e}"
                ))
            })?;
            let stack_bytes: Result<Vec<_>, _> = exception
                .stack
                .iter()
                .map(|v| var_to_flatbuffer_bytes(v).map(|data| rpc::VarBytes { data }))
                .collect();
            let backtrace_bytes: Result<Vec<_>, _> = exception
                .backtrace
                .iter()
                .map(|v| var_to_flatbuffer_bytes(v).map(|data| rpc::VarBytes { data }))
                .collect();

            rpc::SchedulerErrorUnion::TaskAbortedException(Box::new(rpc::TaskAbortedException {
                exception: Box::new(rpc::Exception {
                    error: Box::new(error_bytes),
                    stack: stack_bytes?,
                    backtrace: backtrace_bytes?,
                }),
            }))
        }
        SchedulerError::TaskAbortedCancelled => {
            rpc::SchedulerErrorUnion::TaskAbortedCancelled(Box::new(rpc::TaskAbortedCancelled {}))
        }
        SchedulerError::VerbProgramFailed(verb_error) => {
            let verb_error_fb = verb_program_error_to_flatbuffer_struct(verb_error)
                .map_err(moor_var::EncodingError::CouldNotEncode)?;
            rpc::SchedulerErrorUnion::VerbProgramFailed(Box::new(rpc::VerbProgramFailed {
                error: Box::new(verb_error_fb),
            }))
        }
        SchedulerError::PropertyRetrievalFailed(ws_error) => {
            let ws_error_fb = world_state_error_to_flatbuffer_struct(ws_error)
                .map_err(moor_var::EncodingError::CouldNotEncode)?;
            rpc::SchedulerErrorUnion::PropertyRetrievalFailed(Box::new(
                rpc::PropertyRetrievalFailed {
                    error: Box::new(ws_error_fb),
                },
            ))
        }
        SchedulerError::VerbRetrievalFailed(ws_error) => {
            let ws_error_fb = world_state_error_to_flatbuffer_struct(ws_error)
                .map_err(moor_var::EncodingError::CouldNotEncode)?;
            rpc::SchedulerErrorUnion::VerbRetrievalFailed(Box::new(rpc::VerbRetrievalFailed {
                error: Box::new(ws_error_fb),
            }))
        }
        SchedulerError::ObjectResolutionFailed(ws_error) => {
            let ws_error_fb = world_state_error_to_flatbuffer_struct(ws_error)
                .map_err(moor_var::EncodingError::CouldNotEncode)?;
            rpc::SchedulerErrorUnion::ObjectResolutionFailed(Box::new(
                rpc::ObjectResolutionFailed {
                    error: Box::new(ws_error_fb),
                },
            ))
        }
        SchedulerError::GarbageCollectionFailed(msg) => {
            rpc::SchedulerErrorUnion::GarbageCollectionFailed(Box::new(
                rpc::GarbageCollectionFailed {
                    message: msg.clone(),
                },
            ))
        }
    };

    Ok(rpc::SchedulerError { error: error_union })
}

/// Convert from FlatBuffer CompileErrorRef to moor_common::model::CompileError
pub fn compilation_error_from_ref(
    error_ref: moor_common_schema::CompileErrorRef<'_>,
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
            let var_struct = moor_common_schema::Symbol::try_from(var_ref)
                .map_err(|_| "Failed to convert var_name")?;
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
            let var_struct = moor_common_schema::Symbol::try_from(var_ref)
                .map_err(|_| "Failed to convert var_name")?;
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
) -> Result<moor_common_schema::CompileError, String> {
    let error_union = match error {
        CompileError::StringLexError(ctx, msg) => {
            CompileErrorUnion::StringLexError(Box::new(moor_common_schema::StringLexError {
                context: Box::new(moor_common_schema::CompileContext {
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
        } => CompileErrorUnion::ParseError(Box::new(moor_common_schema::ParseError {
            error_position: Box::new(moor_common_schema::CompileContext {
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
            CompileErrorUnion::UnknownBuiltinFunction(Box::new(
                moor_common_schema::UnknownBuiltinFunction {
                    context: Box::new(moor_common_schema::CompileContext {
                        line: ctx.line_col.0 as u64,
                        col: ctx.line_col.1 as u64,
                    }),
                    name: name.clone(),
                },
            ))
        }
        CompileError::UnknownTypeConstant(ctx, name) => CompileErrorUnion::UnknownTypeConstant(
            Box::new(moor_common_schema::UnknownTypeConstant {
                context: Box::new(moor_common_schema::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                name: name.clone(),
            }),
        ),
        CompileError::UnknownLoopLabel(ctx, label) => {
            CompileErrorUnion::UnknownLoopLabel(Box::new(moor_common_schema::UnknownLoopLabel {
                context: Box::new(moor_common_schema::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                label: label.clone(),
            }))
        }
        CompileError::DuplicateVariable(ctx, var_name) => {
            CompileErrorUnion::DuplicateVariable(Box::new(moor_common_schema::DuplicateVariable {
                context: Box::new(moor_common_schema::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                var_name: Box::new(symbol_to_flatbuffer_struct(var_name)),
            }))
        }
        CompileError::AssignToConst(ctx, var_name) => {
            CompileErrorUnion::AssignToConst(Box::new(moor_common_schema::AssignToConst {
                context: Box::new(moor_common_schema::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                var_name: Box::new(symbol_to_flatbuffer_struct(var_name)),
            }))
        }
        CompileError::DisabledFeature(ctx, feature) => {
            CompileErrorUnion::DisabledFeature(Box::new(moor_common_schema::DisabledFeature {
                context: Box::new(moor_common_schema::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                feature: feature.clone(),
            }))
        }
        CompileError::BadSlotName(ctx, slot) => {
            CompileErrorUnion::BadSlotName(Box::new(moor_common_schema::BadSlotName {
                context: Box::new(moor_common_schema::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
                slot: slot.clone(),
            }))
        }
        CompileError::InvalidAssignemnt(ctx) => {
            CompileErrorUnion::InvalidAssignment(Box::new(moor_common_schema::InvalidAssignment {
                context: Box::new(moor_common_schema::CompileContext {
                    line: ctx.line_col.0 as u64,
                    col: ctx.line_col.1 as u64,
                }),
            }))
        }
    };

    Ok(moor_common_schema::CompileError { error: error_union })
}

/// Convert from FlatBuffer ExceptionRef to Exception
pub(crate) fn exception_from_ref(
    exception_ref: moor_common_schema::ExceptionRef<'_>,
) -> Result<moor_common::tasks::Exception, String> {
    let error_ref = exception_ref.error().map_err(|_| "Missing error")?;
    let error_value = error_from_ref(error_ref)?;

    let stack_vec = exception_ref.stack().map_err(|_| "Missing stack")?;
    let stack: Result<Vec<_>, String> = stack_vec
        .iter()
        .map(|vb_result| -> Result<Var, String> {
            let vb = vb_result.map_err(|e| format!("Failed to get stack item: {e}"))?;
            let data = vb
                .data()
                .map_err(|e| format!("Missing stack item data: {e}"))?;
            var_from_flatbuffer_bytes(data)
                .map_err(|e| format!("Failed to decode stack var: {e}"))
        })
        .collect();
    let stack = stack?;

    let backtrace_vec = exception_ref.backtrace().map_err(|_| "Missing backtrace")?;
    let backtrace: Result<Vec<_>, String> = backtrace_vec
        .iter()
        .map(|vb_result| -> Result<Var, String> {
            let vb = vb_result.map_err(|e| format!("Failed to get backtrace item: {e}"))?;
            let data = vb
                .data()
                .map_err(|e| format!("Missing backtrace item data: {e}"))?;
            var_from_flatbuffer_bytes(data)
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
