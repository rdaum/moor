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

use moor_common::{
    model::WorldStateError,
    tasks::{AbortLimitReason, CommandError, SchedulerError, VerbProgramError, WorkerError},
};
use moor_schema::convert::{symbol_from_ref, var_from_ref};
use moor_schema::{
    common,
    convert::{
        compilation_error_to_flatbuffer_struct, error_to_flatbuffer_struct, exception_from_ref,
        obj_to_flatbuffer_struct, objectref_to_flatbuffer_struct, symbol_from_flatbuffer_struct,
        symbol_to_flatbuffer_struct, uuid_from_ref, var_to_flatbuffer,
    },
    rpc,
};
use std::time::Duration;

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
    error: &VerbProgramError,
) -> Result<rpc::VerbProgramError, String> {
    let error_union = match error {
        VerbProgramError::NoVerbToProgram => {
            rpc::VerbProgramErrorUnion::NoVerbToProgram(Box::new(rpc::NoVerbToProgram {}))
        }
        VerbProgramError::PermissionDenied => {
            rpc::VerbProgramErrorUnion::VerbPermissionDenied(Box::new(rpc::VerbPermissionDenied {}))
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
    error: &WorldStateError,
) -> Result<common::WorldStateError, String> {
    let error_union = match error {
        WorldStateError::ObjectNotFound(objref) => {
            common::WorldStateErrorUnion::ObjectNotFound(Box::new(common::ObjectNotFound {
                object_ref: Box::new(objectref_to_flatbuffer_struct(objref)),
            }))
        }
        WorldStateError::ObjectAlreadyExists(obj) => {
            common::WorldStateErrorUnion::ObjectAlreadyExists(Box::new(
                common::ObjectAlreadyExists {
                    obj: Box::new(obj_to_flatbuffer_struct(obj)),
                },
            ))
        }
        WorldStateError::RecursiveMove(from_obj, to_obj) => {
            common::WorldStateErrorUnion::RecursiveMove(Box::new(common::RecursiveMove {
                from_obj: Box::new(obj_to_flatbuffer_struct(from_obj)),
                to_obj: Box::new(obj_to_flatbuffer_struct(to_obj)),
            }))
        }
        WorldStateError::ObjectPermissionDenied => {
            common::WorldStateErrorUnion::ObjectPermissionDenied(Box::new(
                common::ObjectPermissionDenied {},
            ))
        }
        WorldStateError::PropertyNotFound(obj, property) => {
            common::WorldStateErrorUnion::PropertyNotFound(Box::new(common::PropertyNotFound {
                obj: Box::new(obj_to_flatbuffer_struct(obj)),
                property: property.clone(),
            }))
        }
        WorldStateError::PropertyPermissionDenied => {
            common::WorldStateErrorUnion::PropertyPermissionDenied(Box::new(
                common::PropertyPermissionDenied {},
            ))
        }
        WorldStateError::PropertyDefinitionNotFound(obj, property) => {
            common::WorldStateErrorUnion::PropertyDefinitionNotFound(Box::new(
                common::PropertyDefinitionNotFound {
                    obj: Box::new(obj_to_flatbuffer_struct(obj)),
                    property: property.clone(),
                },
            ))
        }
        WorldStateError::DuplicatePropertyDefinition(obj, property) => {
            common::WorldStateErrorUnion::DuplicatePropertyDefinition(Box::new(
                common::DuplicatePropertyDefinition {
                    obj: Box::new(obj_to_flatbuffer_struct(obj)),
                    property: property.clone(),
                },
            ))
        }
        WorldStateError::ChparentPropertyNameConflict(descendant, ancestor, property) => {
            common::WorldStateErrorUnion::ChparentPropertyNameConflict(Box::new(
                common::ChparentPropertyNameConflict {
                    descendant: Box::new(obj_to_flatbuffer_struct(descendant)),
                    ancestor: Box::new(obj_to_flatbuffer_struct(ancestor)),
                    property: property.clone(),
                },
            ))
        }
        WorldStateError::PropertyTypeMismatch => {
            common::WorldStateErrorUnion::PropertyTypeMismatch(Box::new(
                common::PropertyTypeMismatch {},
            ))
        }
        WorldStateError::VerbNotFound(obj, verb) => {
            common::WorldStateErrorUnion::VerbNotFound(Box::new(common::VerbNotFound {
                obj: Box::new(obj_to_flatbuffer_struct(obj)),
                verb: verb.clone(),
            }))
        }
        WorldStateError::InvalidVerb(vid) => {
            common::WorldStateErrorUnion::InvalidVerb(Box::new(common::InvalidVerb { vid: vid.0 }))
        }
        WorldStateError::VerbDecodeError(obj, verb) => {
            common::WorldStateErrorUnion::VerbDecodeError(Box::new(common::VerbDecodeError {
                obj: Box::new(obj_to_flatbuffer_struct(obj)),
                verb: Box::new(symbol_to_flatbuffer_struct(verb)),
            }))
        }
        WorldStateError::VerbPermissionDenied => {
            common::WorldStateErrorUnion::VerbPermissionDenied(Box::new(
                common::VerbPermissionDenied {},
            ))
        }
        WorldStateError::DuplicateVerb(obj, verb) => {
            common::WorldStateErrorUnion::DuplicateVerb(Box::new(common::DuplicateVerb {
                obj: Box::new(obj_to_flatbuffer_struct(obj)),
                verb: Box::new(symbol_to_flatbuffer_struct(verb)),
            }))
        }
        WorldStateError::FailedMatch(match_string) => {
            common::WorldStateErrorUnion::FailedMatch(Box::new(common::FailedMatch {
                match_string: match_string.clone(),
            }))
        }
        WorldStateError::AmbiguousMatch(match_string) => {
            common::WorldStateErrorUnion::AmbiguousMatch(Box::new(common::AmbiguousMatch {
                match_string: match_string.clone(),
            }))
        }
        WorldStateError::InvalidRenumber(message) => {
            common::WorldStateErrorUnion::InvalidRenumber(Box::new(common::InvalidRenumber {
                message: message.clone(),
            }))
        }
        WorldStateError::DatabaseError(message) => {
            common::WorldStateErrorUnion::WorldStateDatabaseError(Box::new(
                common::WorldStateDatabaseError {
                    message: message.clone(),
                },
            ))
        }
        WorldStateError::RollbackRetry => {
            common::WorldStateErrorUnion::RollbackRetry(Box::new(common::RollbackRetry {}))
        }
    };

    Ok(common::WorldStateError { error: error_union })
}

/// Convert from FlatBuffer VerbProgramErrorRef to VerbProgramError
fn verb_program_error_from_ref(
    error_ref: rpc::VerbProgramErrorRef<'_>,
) -> Result<VerbProgramError, String> {
    match error_ref
        .error()
        .map_err(|_| "Failed to read VerbProgramError union")?
    {
        rpc::VerbProgramErrorUnionRef::NoVerbToProgram(_) => Ok(VerbProgramError::NoVerbToProgram),
        rpc::VerbProgramErrorUnionRef::VerbPermissionDenied(_) => {
            Ok(VerbProgramError::PermissionDenied)
        }
        rpc::VerbProgramErrorUnionRef::VerbCompilationError(_compile_error) => {
            // TODO: Implement CompileError conversion if needed
            Err("VerbCompilationError conversion not yet implemented".to_string())
        }
        rpc::VerbProgramErrorUnionRef::VerbDatabaseError(_) => Ok(VerbProgramError::DatabaseError),
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
        rpc::SchedulerErrorUnionRef::TaskAbortedVerbNotFound(verbnf) => {
            let where_ = var_from_ref(verbnf.where_().map_err(|_| "Missing verb `where`")?)
                .map_err(|_| "Missing verb `where`")?;
            let what = symbol_from_ref(verbnf.what().map_err(|_| "Missing verb `what`")?)
                .map_err(|_| "Missing verb `where`")?;
            Ok(SchedulerError::TaskAbortedVerbNotFound(where_, what))
        }
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
            let stack_fb: Result<Vec<_>, _> = exception
                .stack
                .iter()
                .map(|v| {
                    var_to_flatbuffer(v).map_err(|e| {
                        moor_var::EncodingError::CouldNotEncode(format!(
                            "Failed to encode stack item: {e}"
                        ))
                    })
                })
                .collect();
            let backtrace_fb: Result<Vec<_>, _> = exception
                .backtrace
                .iter()
                .map(|v| {
                    var_to_flatbuffer(v).map_err(|e| {
                        moor_var::EncodingError::CouldNotEncode(format!(
                            "Failed to encode backtrace item: {e}"
                        ))
                    })
                })
                .collect();

            rpc::SchedulerErrorUnion::TaskAbortedException(Box::new(rpc::TaskAbortedException {
                exception: Box::new(rpc::Exception {
                    error: Box::new(error_bytes),
                    stack: stack_fb?,
                    backtrace: backtrace_fb?,
                }),
            }))
        }
        SchedulerError::TaskAbortedCancelled => {
            rpc::SchedulerErrorUnion::TaskAbortedCancelled(Box::new(rpc::TaskAbortedCancelled {}))
        }
        SchedulerError::TaskAbortedVerbNotFound(who, what) => {
            let where_ = var_to_flatbuffer(who);
            let where_ = where_.map_err(|e| {
                moor_var::EncodingError::CouldNotEncode(format!(
                    "Failed to encode verbnf `where`: {e}"
                ))
            })?;
            let what = symbol_to_flatbuffer_struct(what);
            rpc::SchedulerErrorUnion::TaskAbortedVerbNotFound(Box::new(
                rpc::TaskAbortedVerbNotFound {
                    where_: Box::new(where_),
                    what: Box::new(what),
                },
            ))
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
