// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Conversion between kernel task types and FlatBuffer representations

use crate::{
    tasks::{
        TaskStart as KernelTaskStart,
        task::Task as KernelTask,
        task::TaskState as KernelTaskState,
        task_q::{SuspendedTask as KernelSuspendedTask, WakeCondition as KernelWakeCondition},
    },
    vm::{
        FinallyReason as KernelFinallyReason, Fork,
        activation::{
            Activation as KernelActivation, BfFrame as KernelBfFrame, Frame as KernelFrame,
        },
        exec_state::VMExecState as KernelVMExecState,
        moo_frame::{
            CatchType as KernelCatchType, MooStackFrame as KernelMooStackFrame,
            PcType as KernelPcType, Scope as KernelScope, ScopeType as KernelScopeType,
        },
        vm_host::VmHost as KernelVmHost,
    },
};
use moor_common::util::BitEnum;
use moor_compiler::{Label, Offset};
use moor_schema::{
    common as fb_common, convert as convert_schema,
    convert::{error_from_ref, var_from_db_flatbuffer_ref, var_to_db_flatbuffer, verbdef_from_ref},
    convert_program::{decode_stored_program_ref, encode_program_to_fb},
    program as fb_program, task as fb,
};
use moor_var::program::names::Name;
use moor_var::v_str;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TaskConversionError {
    #[error("Failed to decode FlatBuffer: {0}")]
    DecodingError(String),

    #[error("Failed to encode to FlatBuffer: {0}")]
    EncodingError(String),

    #[error("Var conversion error: {0}")]
    VarError(String),

    #[error("Program conversion error: {0}")]
    ProgramError(String),
}

impl From<moor_schema::convert::VarConversionError> for TaskConversionError {
    fn from(e: moor_schema::convert::VarConversionError) -> Self {
        TaskConversionError::VarError(e.to_string())
    }
}

const CURRENT_TASK_VERSION: u16 = 1;

// ============================================================================
// Helper Functions for Common Types
// ============================================================================

fn name_to_stored(name: &Name) -> Result<fb_program::StoredName, TaskConversionError> {
    Ok(fb_program::StoredName {
        offset: name.0,
        scope_depth: name.1,
        scope_id: name.2,
    })
}

fn name_from_ref(stored: fb_program::StoredNameRef<'_>) -> Result<Name, TaskConversionError> {
    let offset = stored
        .offset()
        .map_err(|e| TaskConversionError::DecodingError(format!("offset: {e}")))?;
    let scope_depth = stored
        .scope_depth()
        .map_err(|e| TaskConversionError::DecodingError(format!("scope_depth: {e}")))?;
    let scope_id = stored
        .scope_id()
        .map_err(|e| TaskConversionError::DecodingError(format!("scope_id: {e}")))?;
    Ok(Name(offset, scope_depth, scope_id))
}

fn exception_to_flatbuffer(
    exception: &moor_common::tasks::Exception,
) -> Result<fb_common::Exception, TaskConversionError> {
    let fb_error = convert_schema::error_to_flatbuffer_struct(&exception.error)
        .map_err(|e| TaskConversionError::EncodingError(format!("Error encoding error: {e}")))?;

    let fb_stack: Result<Vec<_>, _> = exception.stack.iter().map(var_to_db_flatbuffer).collect();
    let fb_stack = fb_stack
        .map_err(|e| TaskConversionError::VarError(format!("Error encoding stack: {e}")))?;

    let fb_backtrace: Result<Vec<_>, _> = exception
        .backtrace
        .iter()
        .map(var_to_db_flatbuffer)
        .collect();
    let fb_backtrace = fb_backtrace
        .map_err(|e| TaskConversionError::VarError(format!("Error encoding backtrace: {e}")))?;

    Ok(fb_common::Exception {
        error: Box::new(fb_error),
        stack: fb_stack,
        backtrace: fb_backtrace,
    })
}

fn exception_from_ref(
    fb: fb_common::ExceptionRef<'_>,
) -> Result<moor_common::tasks::Exception, TaskConversionError> {
    let error_ref = fb
        .error()
        .map_err(|e| TaskConversionError::DecodingError(format!("error: {e}")))?;
    let error = error_from_ref(error_ref)
        .map_err(|e| TaskConversionError::DecodingError(format!("error: {e}")))?;

    let stack_vec = fb
        .stack()
        .map_err(|e| TaskConversionError::DecodingError(format!("stack: {e}")))?;
    let stack: Result<Vec<_>, TaskConversionError> = stack_vec
        .iter()
        .map(|v_result| {
            let v = v_result
                .map_err(|e| TaskConversionError::DecodingError(format!("stack item: {e}")))?;
            var_from_db_flatbuffer_ref(v)
                .map_err(|e| TaskConversionError::VarError(format!("stack item: {e}")))
        })
        .collect();
    let stack = stack?;

    let backtrace_vec = fb
        .backtrace()
        .map_err(|e| TaskConversionError::DecodingError(format!("backtrace: {e}")))?;
    let backtrace: Result<Vec<_>, TaskConversionError> = backtrace_vec
        .iter()
        .map(|v_result| {
            let v = v_result
                .map_err(|e| TaskConversionError::DecodingError(format!("backtrace item: {e}")))?;
            var_from_db_flatbuffer_ref(v)
                .map_err(|e| TaskConversionError::VarError(format!("backtrace item: {e}")))
        })
        .collect();
    let backtrace = backtrace?;

    Ok(moor_common::tasks::Exception {
        error,
        stack,
        backtrace,
    })
}

// ============================================================================
// WakeCondition Conversion
// ============================================================================

pub(crate) fn wake_condition_to_flatbuffer(
    wake: &KernelWakeCondition,
) -> Result<fb::WakeCondition, TaskConversionError> {
    use fb::{WakeConditionUnion::*, *};
    use minstant::Instant;

    let condition = match wake {
        KernelWakeCondition::Time(t) => {
            // Convert Instant to absolute epoch time for storage
            let now_system = SystemTime::now();
            let now_instant = Instant::now();

            let epoch_time = if *t >= now_instant {
                // Future time - add the difference
                let time_diff = t.duration_since(now_instant);
                now_system + time_diff
            } else {
                // Past time - subtract the difference (but don't go before epoch)
                let time_diff = now_instant.duration_since(*t);
                now_system.checked_sub(time_diff).unwrap_or(UNIX_EPOCH)
            };

            let time_since_epoch = epoch_time
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO);
            let nanos = time_since_epoch.as_nanos() as u64;

            WakeTime(Box::new(fb::WakeTime { nanos }))
        }
        KernelWakeCondition::Never => WakeNever(Box::new(fb::WakeNever {})),
        KernelWakeCondition::Input(uuid) => {
            let uuid_bytes = uuid.as_bytes();
            WakeInput(Box::new(fb::WakeInput {
                uuid: Box::new(fb_common::Uuid {
                    data: uuid_bytes.to_vec(),
                }),
            }))
        }
        KernelWakeCondition::Immediate(return_value) => {
            let return_value_fb = if let Some(val) = return_value {
                Some(Box::new(var_to_db_flatbuffer(val)?))
            } else {
                None
            };
            WakeImmediate(Box::new(fb::WakeImmediate {
                return_value: return_value_fb,
            }))
        }
        KernelWakeCondition::Task(task_id) => WakeTask(Box::new(fb::WakeTask {
            task_id: *task_id as u64,
        })),
        KernelWakeCondition::Worker(uuid) => {
            let uuid_bytes = uuid.as_bytes();
            WakeWorker(Box::new(fb::WakeWorker {
                uuid: Box::new(fb_common::Uuid {
                    data: uuid_bytes.to_vec(),
                }),
            }))
        }
        KernelWakeCondition::GCComplete => WakeGcComplete(Box::new(fb::WakeGcComplete {})),
        KernelWakeCondition::TaskMessage(t) => {
            // Convert Instant to absolute epoch time for storage (same pattern as Time)
            let now_system = SystemTime::now();
            let now_instant = Instant::now();

            let epoch_time = if *t >= now_instant {
                let time_diff = t.duration_since(now_instant);
                now_system + time_diff
            } else {
                let time_diff = now_instant.duration_since(*t);
                now_system.checked_sub(time_diff).unwrap_or(UNIX_EPOCH)
            };

            let time_since_epoch = epoch_time
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO);
            let nanos = time_since_epoch.as_nanos() as u64;

            WakeTaskMessage(Box::new(fb::WakeTaskMessage {
                deadline_nanos: nanos,
            }))
        }
        KernelWakeCondition::Retry(_) => {
            // Retry tasks are transient and should never be persisted
            return Err(TaskConversionError::EncodingError(
                "Retry wake condition should not be serialized".to_string(),
            ));
        }
    };

    Ok(WakeCondition { condition })
}

pub(crate) fn wake_condition_from_ref(
    fb: fb::WakeConditionRef<'_>,
) -> Result<KernelWakeCondition, TaskConversionError> {
    use fb::WakeConditionUnionRef;
    use minstant::Instant;

    let condition = fb
        .condition()
        .map_err(|e| TaskConversionError::DecodingError(format!("condition: {e}")))?;

    match condition {
        WakeConditionUnionRef::WakeTime(wt) => {
            let nanos = wt
                .nanos()
                .map_err(|e| TaskConversionError::DecodingError(format!("nanos: {e}")))?;
            let epoch_duration = Duration::from_nanos(nanos);
            let epoch_time = UNIX_EPOCH + epoch_duration;

            let now_system = SystemTime::now();
            let now_instant = Instant::now();

            let wake_instant = if epoch_time >= now_system {
                let time_diff = epoch_time
                    .duration_since(now_system)
                    .unwrap_or(Duration::ZERO);
                now_instant + time_diff
            } else {
                let time_diff = now_system
                    .duration_since(epoch_time)
                    .unwrap_or(Duration::ZERO);
                now_instant.checked_sub(time_diff).unwrap_or(now_instant)
            };

            Ok(KernelWakeCondition::Time(wake_instant))
        }
        WakeConditionUnionRef::WakeNever(_) => Ok(KernelWakeCondition::Never),
        WakeConditionUnionRef::WakeInput(wi) => {
            let uuid_ref = wi
                .uuid()
                .map_err(|e| TaskConversionError::DecodingError(format!("uuid: {e}")))?;
            let data = uuid_ref
                .data()
                .map_err(|e| TaskConversionError::DecodingError(format!("uuid data: {e}")))?;
            let uuid_bytes: [u8; 16] = data.try_into().map_err(|_| {
                TaskConversionError::DecodingError("Invalid UUID bytes".to_string())
            })?;
            let uuid = uuid::Uuid::from_bytes(uuid_bytes);
            Ok(KernelWakeCondition::Input(uuid))
        }
        WakeConditionUnionRef::WakeImmediate(wi) => {
            let return_value = match wi.return_value() {
                Ok(Some(rv)) => Some(var_from_db_flatbuffer_ref(rv)?),
                Ok(None) => None,
                Err(e) => {
                    return Err(TaskConversionError::DecodingError(format!(
                        "return_value: {e}"
                    )));
                }
            };
            Ok(KernelWakeCondition::Immediate(return_value))
        }
        WakeConditionUnionRef::WakeTask(wt) => {
            let task_id = wt
                .task_id()
                .map_err(|e| TaskConversionError::DecodingError(format!("task_id: {e}")))?;
            Ok(KernelWakeCondition::Task(task_id as usize))
        }
        WakeConditionUnionRef::WakeWorker(ww) => {
            let uuid_ref = ww
                .uuid()
                .map_err(|e| TaskConversionError::DecodingError(format!("uuid: {e}")))?;
            let data = uuid_ref
                .data()
                .map_err(|e| TaskConversionError::DecodingError(format!("uuid data: {e}")))?;
            let uuid_bytes: [u8; 16] = data.try_into().map_err(|_| {
                TaskConversionError::DecodingError("Invalid UUID bytes".to_string())
            })?;
            let uuid = uuid::Uuid::from_bytes(uuid_bytes);
            Ok(KernelWakeCondition::Worker(uuid))
        }
        WakeConditionUnionRef::WakeGcComplete(_) => Ok(KernelWakeCondition::GCComplete),
        WakeConditionUnionRef::WakeTaskMessage(wtm) => {
            let nanos = wtm
                .deadline_nanos()
                .map_err(|e| TaskConversionError::DecodingError(format!("deadline_nanos: {e}")))?;
            let epoch_duration = Duration::from_nanos(nanos);
            let epoch_time = UNIX_EPOCH + epoch_duration;

            let now_system = SystemTime::now();
            let now_instant = Instant::now();

            let wake_instant = if epoch_time >= now_system {
                let time_diff = epoch_time
                    .duration_since(now_system)
                    .unwrap_or(Duration::ZERO);
                now_instant + time_diff
            } else {
                let time_diff = now_system
                    .duration_since(epoch_time)
                    .unwrap_or(Duration::ZERO);
                now_instant.checked_sub(time_diff).unwrap_or(now_instant)
            };

            Ok(KernelWakeCondition::TaskMessage(wake_instant))
        }
    }
}

// ============================================================================
// AbortLimitReason Conversion
// ============================================================================

// ============================================================================
// PcType Conversion
// ============================================================================

pub(crate) fn pc_type_to_flatbuffer(pc: &KernelPcType) -> Result<fb::PcType, TaskConversionError> {
    use fb::*;

    let pc_type = match pc {
        KernelPcType::Main => PcTypeUnion::PcMain(Box::new(PcMain {})),
        KernelPcType::ForkVector(offset) => PcTypeUnion::PcForkVector(Box::new(PcForkVector {
            offset: offset.0 as u64,
        })),
        KernelPcType::Lambda(offset) => PcTypeUnion::PcLambda(Box::new(PcLambda {
            offset: offset.0 as u64,
        })),
    };

    Ok(PcType { pc_type })
}

pub(crate) fn pc_type_from_ref(fb: fb::PcTypeRef<'_>) -> Result<KernelPcType, TaskConversionError> {
    use fb::PcTypeUnionRef;

    let pc_type = fb
        .pc_type()
        .map_err(|e| TaskConversionError::DecodingError(format!("pc_type: {e}")))?;

    match pc_type {
        PcTypeUnionRef::PcMain(_) => Ok(KernelPcType::Main),
        PcTypeUnionRef::PcForkVector(fv) => {
            let offset = fv
                .offset()
                .map_err(|e| TaskConversionError::DecodingError(format!("offset: {e}")))?;
            Ok(KernelPcType::ForkVector(Offset(offset as u16)))
        }
        PcTypeUnionRef::PcLambda(l) => {
            let offset = l
                .offset()
                .map_err(|e| TaskConversionError::DecodingError(format!("offset: {e}")))?;
            Ok(KernelPcType::Lambda(Offset(offset as u16)))
        }
    }
}

// ============================================================================
// CatchType Conversion
// ============================================================================

pub(crate) fn catch_type_to_flatbuffer(
    catch: &KernelCatchType,
) -> Result<fb::CatchType, TaskConversionError> {
    use fb::*;

    let catch_type = match catch {
        KernelCatchType::Any => CatchTypeUnion::CatchAny(Box::new(CatchAny {})),
        KernelCatchType::Errors(errors) => {
            let fb_errors: Result<Vec<_>, _> = errors
                .iter()
                .map(|e| convert_schema::error_to_flatbuffer_struct(e))
                .collect();
            CatchTypeUnion::CatchErrors(Box::new(CatchErrors {
                errors: fb_errors.map_err(|e| {
                    TaskConversionError::EncodingError(format!("Error encoding errors: {e}"))
                })?,
            }))
        }
    };

    Ok(CatchType { catch_type })
}

pub(crate) fn catch_type_from_ref(
    fb: fb::CatchTypeRef<'_>,
) -> Result<KernelCatchType, TaskConversionError> {
    use fb::CatchTypeUnionRef;

    let catch_type = fb
        .catch_type()
        .map_err(|e| TaskConversionError::DecodingError(format!("catch_type: {e}")))?;

    match catch_type {
        CatchTypeUnionRef::CatchAny(_) => Ok(KernelCatchType::Any),
        CatchTypeUnionRef::CatchErrors(ce) => {
            let errors_vec = ce
                .errors()
                .map_err(|e| TaskConversionError::DecodingError(format!("errors: {e}")))?;
            let errors: Result<Vec<_>, TaskConversionError> = errors_vec
                .iter()
                .map(|e_result| {
                    let e = e_result.map_err(|e| {
                        TaskConversionError::DecodingError(format!("error item: {e}"))
                    })?;
                    convert_schema::error_from_ref(e)
                        .map_err(|e| TaskConversionError::DecodingError(format!("error: {e}")))
                })
                .collect();
            Ok(KernelCatchType::Errors(errors?))
        }
    }
}

// ============================================================================
// FinallyReason Conversion
// ============================================================================

pub(crate) fn finally_reason_to_flatbuffer(
    reason: &KernelFinallyReason,
) -> Result<fb::FinallyReason, TaskConversionError> {
    use fb::*;

    let reason_union = match reason {
        KernelFinallyReason::Fallthrough => {
            FinallyReasonUnion::FinallyFallthrough(Box::new(FinallyFallthrough {}))
        }
        KernelFinallyReason::Raise(exception) => {
            let fb_exception = exception_to_flatbuffer(exception).map_err(|e| {
                TaskConversionError::EncodingError(format!("Error encoding exception: {e}"))
            })?;
            FinallyReasonUnion::FinallyRaise(Box::new(FinallyRaise {
                exception: Box::new(fb_exception),
            }))
        }
        KernelFinallyReason::Return(var) => {
            let fb_var = var_to_db_flatbuffer(var)
                .map_err(|e| TaskConversionError::VarError(format!("Error encoding var: {e}")))?;
            FinallyReasonUnion::FinallyReturn(Box::new(FinallyReturn {
                value: Box::new(fb_var),
            }))
        }
        KernelFinallyReason::Abort => FinallyReasonUnion::FinallyAbort(Box::new(FinallyAbort {})),
        KernelFinallyReason::Exit { stack, label } => {
            FinallyReasonUnion::FinallyExit(Box::new(FinallyExit {
                stack: stack.0 as u64,
                label: label.0,
            }))
        }
    };

    Ok(FinallyReason {
        reason: reason_union,
    })
}

pub(crate) fn finally_reason_from_ref(
    fb: fb::FinallyReasonRef<'_>,
) -> Result<KernelFinallyReason, TaskConversionError> {
    use fb::FinallyReasonUnionRef;

    let reason = fb
        .reason()
        .map_err(|e| TaskConversionError::DecodingError(format!("reason: {e}")))?;

    match reason {
        FinallyReasonUnionRef::FinallyFallthrough(_) => Ok(KernelFinallyReason::Fallthrough),
        FinallyReasonUnionRef::FinallyRaise(fr) => {
            let exception_ref = fr
                .exception()
                .map_err(|e| TaskConversionError::DecodingError(format!("exception: {e}")))?;
            let exception = convert_schema::exception_from_ref(exception_ref)
                .map_err(|e| TaskConversionError::DecodingError(format!("exception: {e}")))?;
            Ok(KernelFinallyReason::Raise(Box::new(exception)))
        }
        FinallyReasonUnionRef::FinallyReturn(fr) => {
            let value_ref = fr
                .value()
                .map_err(|e| TaskConversionError::DecodingError(format!("value: {e}")))?;
            let var = var_from_db_flatbuffer_ref(value_ref)?;
            Ok(KernelFinallyReason::Return(var))
        }
        FinallyReasonUnionRef::FinallyAbort(_) => Ok(KernelFinallyReason::Abort),
        FinallyReasonUnionRef::FinallyExit(fe) => {
            let stack = fe
                .stack()
                .map_err(|e| TaskConversionError::DecodingError(format!("stack: {e}")))?;
            let label = fe
                .label()
                .map_err(|e| TaskConversionError::DecodingError(format!("label: {e}")))?;
            Ok(KernelFinallyReason::Exit {
                stack: Offset(stack as u16),
                label: Label(label),
            })
        }
    }
}

// Helper: CatchHandler conversion
fn catch_handler_to_flatbuffer(
    catch_type: &KernelCatchType,
    label: &Label,
) -> Result<fb::CatchHandler, TaskConversionError> {
    Ok(fb::CatchHandler {
        catch_type: Box::new(catch_type_to_flatbuffer(catch_type)?),
        label: label.0,
    })
}

fn catch_handler_from_ref(
    fb: fb::CatchHandlerRef<'_>,
) -> Result<(KernelCatchType, Label), TaskConversionError> {
    let catch_type_ref = fb
        .catch_type()
        .map_err(|e| TaskConversionError::DecodingError(format!("catch_type: {e}")))?;
    let label = fb
        .label()
        .map_err(|e| TaskConversionError::DecodingError(format!("label: {e}")))?;
    Ok((catch_type_from_ref(catch_type_ref)?, Label(label)))
}

// ============================================================================
// ScopeType Conversion
// ============================================================================

pub(crate) fn scope_type_to_flatbuffer(
    scope: &KernelScopeType,
) -> Result<fb::ScopeType, TaskConversionError> {
    use fb::*;

    let scope_type = match scope {
        KernelScopeType::TryFinally(label) => {
            ScopeTypeUnion::ScopeTryFinally(Box::new(ScopeTryFinally { label: label.0 }))
        }
        KernelScopeType::TryCatch(handlers) => {
            let fb_handlers: Result<Vec<_>, _> = handlers
                .iter()
                .map(|(ct, l)| catch_handler_to_flatbuffer(ct, l))
                .collect();
            ScopeTypeUnion::ScopeTryCatch(Box::new(ScopeTryCatch {
                handlers: fb_handlers?,
            }))
        }
        KernelScopeType::If => ScopeTypeUnion::ScopeIf(Box::new(ScopeIf {})),
        KernelScopeType::Eif => ScopeTypeUnion::ScopeEif(Box::new(ScopeEif {})),
        KernelScopeType::While => ScopeTypeUnion::ScopeWhile(Box::new(ScopeWhile {})),
        KernelScopeType::For => ScopeTypeUnion::ScopeFor(Box::new(ScopeFor {})),
        KernelScopeType::ForSequence {
            sequence,
            current_index,
            current_key,
            value_bind,
            key_bind,
            end_label,
        } => {
            let fb_seq = var_to_db_flatbuffer(sequence).map_err(|e| {
                TaskConversionError::VarError(format!("Error encoding sequence: {e}"))
            })?;
            let fb_value_bind = name_to_stored(value_bind).map_err(|e| {
                TaskConversionError::ProgramError(format!("Error encoding value_bind: {e}"))
            })?;
            let fb_key_bind = key_bind
                .as_ref()
                .map(name_to_stored)
                .transpose()
                .map_err(|e| {
                    TaskConversionError::ProgramError(format!("Error encoding key_bind: {e}"))
                })?;
            let fb_current_key = current_key
                .as_ref()
                .map(var_to_db_flatbuffer)
                .transpose()
                .map_err(|e| {
                    TaskConversionError::VarError(format!("Error encoding current_key: {e}"))
                })?;

            ScopeTypeUnion::ScopeForSequence(Box::new(ScopeForSequence {
                sequence: Box::new(fb_seq),
                current_index: *current_index as u64,
                value_bind: Box::new(fb_value_bind),
                key_bind: fb_key_bind.map(Box::new),
                end_label: end_label.0,
                current_key: fb_current_key.map(Box::new),
            }))
        }
        KernelScopeType::ForRange {
            current_value,
            end_value,
            loop_variable,
            end_label,
        } => {
            let fb_current = var_to_db_flatbuffer(current_value).map_err(|e| {
                TaskConversionError::VarError(format!("Error encoding current_value: {e}"))
            })?;
            let fb_end = var_to_db_flatbuffer(end_value).map_err(|e| {
                TaskConversionError::VarError(format!("Error encoding end_value: {e}"))
            })?;
            let fb_loop_var = name_to_stored(loop_variable).map_err(|e| {
                TaskConversionError::ProgramError(format!("Error encoding loop_variable: {e}"))
            })?;

            ScopeTypeUnion::ScopeForRange(Box::new(ScopeForRange {
                current_value: Box::new(fb_current),
                end_value: Box::new(fb_end),
                loop_variable: Box::new(fb_loop_var),
                end_label: end_label.0,
            }))
        }
        KernelScopeType::Block => ScopeTypeUnion::ScopeBlock(Box::new(ScopeBlock {})),
        KernelScopeType::Comprehension => {
            ScopeTypeUnion::ScopeComprehension(Box::new(ScopeComprehension {}))
        }
    };

    Ok(ScopeType { scope_type })
}

pub(crate) fn scope_type_from_ref(
    fb: fb::ScopeTypeRef<'_>,
) -> Result<KernelScopeType, TaskConversionError> {
    use fb::ScopeTypeUnionRef;

    let scope_type = fb
        .scope_type()
        .map_err(|e| TaskConversionError::DecodingError(format!("scope_type: {e}")))?;

    match scope_type {
        ScopeTypeUnionRef::ScopeTryFinally(stf) => {
            let label = stf
                .label()
                .map_err(|e| TaskConversionError::DecodingError(format!("label: {e}")))?;
            Ok(KernelScopeType::TryFinally(Label(label)))
        }
        ScopeTypeUnionRef::ScopeTryCatch(stc) => {
            let handlers_vec = stc
                .handlers()
                .map_err(|e| TaskConversionError::DecodingError(format!("handlers: {e}")))?;
            let handlers: Result<Vec<_>, TaskConversionError> = handlers_vec
                .iter()
                .map(|h_result| {
                    let h = h_result
                        .map_err(|e| TaskConversionError::DecodingError(format!("handler: {e}")))?;
                    catch_handler_from_ref(h)
                })
                .collect();
            Ok(KernelScopeType::TryCatch(handlers?))
        }
        ScopeTypeUnionRef::ScopeIf(_) => Ok(KernelScopeType::If),
        ScopeTypeUnionRef::ScopeEif(_) => Ok(KernelScopeType::Eif),
        ScopeTypeUnionRef::ScopeWhile(_) => Ok(KernelScopeType::While),
        ScopeTypeUnionRef::ScopeFor(_) => Ok(KernelScopeType::For),
        ScopeTypeUnionRef::ScopeForSequence(sfs) => {
            let sequence_ref = sfs
                .sequence()
                .map_err(|e| TaskConversionError::DecodingError(format!("sequence: {e}")))?;
            let sequence = var_from_db_flatbuffer_ref(sequence_ref)?;

            let value_bind_ref = sfs
                .value_bind()
                .map_err(|e| TaskConversionError::DecodingError(format!("value_bind: {e}")))?;
            let value_bind = name_from_ref(value_bind_ref)?;

            let key_bind = match sfs.key_bind() {
                Ok(Some(kb)) => Some(name_from_ref(kb)?),
                Ok(None) => None,
                Err(e) => {
                    return Err(TaskConversionError::DecodingError(format!("key_bind: {e}")));
                }
            };

            let current_key = match sfs.current_key() {
                Ok(Some(ck)) => Some(var_from_db_flatbuffer_ref(ck)?),
                Ok(None) => None,
                Err(e) => {
                    return Err(TaskConversionError::DecodingError(format!(
                        "current_key: {e}"
                    )));
                }
            };

            let current_index = sfs
                .current_index()
                .map_err(|e| TaskConversionError::DecodingError(format!("current_index: {e}")))?;
            let end_label = sfs
                .end_label()
                .map_err(|e| TaskConversionError::DecodingError(format!("end_label: {e}")))?;

            Ok(KernelScopeType::ForSequence {
                sequence,
                current_index: current_index as usize,
                current_key,
                value_bind,
                key_bind,
                end_label: Label(end_label),
            })
        }
        ScopeTypeUnionRef::ScopeForRange(sfr) => {
            let current_value_ref = sfr
                .current_value()
                .map_err(|e| TaskConversionError::DecodingError(format!("current_value: {e}")))?;
            let current_value = var_from_db_flatbuffer_ref(current_value_ref)?;

            let end_value_ref = sfr
                .end_value()
                .map_err(|e| TaskConversionError::DecodingError(format!("end_value: {e}")))?;
            let end_value = var_from_db_flatbuffer_ref(end_value_ref)?;

            let loop_variable_ref = sfr
                .loop_variable()
                .map_err(|e| TaskConversionError::DecodingError(format!("loop_variable: {e}")))?;
            let loop_variable = name_from_ref(loop_variable_ref)?;

            let end_label = sfr
                .end_label()
                .map_err(|e| TaskConversionError::DecodingError(format!("end_label: {e}")))?;

            Ok(KernelScopeType::ForRange {
                current_value,
                end_value,
                loop_variable,
                end_label: Label(end_label),
            })
        }
        ScopeTypeUnionRef::ScopeBlock(_) => Ok(KernelScopeType::Block),
        ScopeTypeUnionRef::ScopeComprehension(_) => Ok(KernelScopeType::Comprehension),
    }
}

// ============================================================================
// Scope Conversion
// ============================================================================

pub(crate) fn scope_to_flatbuffer(scope: &KernelScope) -> Result<fb::Scope, TaskConversionError> {
    Ok(fb::Scope {
        scope_type: Box::new(scope_type_to_flatbuffer(&scope.scope_type)?),
        valstack_pos: scope.valstack_pos as u64,
        start_pos: scope.start_pos as u64,
        end_pos: scope.end_pos as u64,
        has_environment: scope.environment,
    })
}

pub(crate) fn scope_from_ref(fb: fb::ScopeRef<'_>) -> Result<KernelScope, TaskConversionError> {
    let scope_type_ref = fb
        .scope_type()
        .map_err(|e| TaskConversionError::DecodingError(format!("scope_type: {e}")))?;
    let valstack_pos = fb
        .valstack_pos()
        .map_err(|e| TaskConversionError::DecodingError(format!("valstack_pos: {e}")))?;
    let start_pos = fb
        .start_pos()
        .map_err(|e| TaskConversionError::DecodingError(format!("start_pos: {e}")))?;
    let end_pos = fb
        .end_pos()
        .map_err(|e| TaskConversionError::DecodingError(format!("end_pos: {e}")))?;
    let has_environment = fb
        .has_environment()
        .map_err(|e| TaskConversionError::DecodingError(format!("has_environment: {e}")))?;

    Ok(KernelScope {
        scope_type: scope_type_from_ref(scope_type_ref)?,
        valstack_pos: valstack_pos as usize,
        start_pos: start_pos as usize,
        end_pos: end_pos as usize,
        environment: has_environment,
    })
}

// ============================================================================
// MooStackFrame Conversion
// ============================================================================

pub(crate) fn moo_stack_frame_to_flatbuffer(
    frame: &KernelMooStackFrame,
) -> Result<fb::MooStackFrame, TaskConversionError> {
    let fb_program = encode_program_to_fb(&frame.program)
        .map_err(|e| TaskConversionError::ProgramError(format!("Error encoding program: {e}")))?;

    let fb_pc_type = pc_type_to_flatbuffer(&frame.pc_type)?;

    // Convert environment: Vec<Vec<Option<Var>>>
    let fb_environment: Result<Vec<_>, _> = frame
        .environment
        .to_vec()
        .iter()
        .map(|scope| {
            let vars: Result<Vec<_>, _> = scope
                .iter()
                .map(|opt_var| {
                    match opt_var {
                        Some(v) => var_to_db_flatbuffer(v),
                        // Use empty/None marker - we'll need to represent this as an empty Var
                        None => Ok(var_to_db_flatbuffer(&moor_var::v_none())?),
                    }
                })
                .collect();
            Ok(fb::EnvironmentScope { vars: vars? })
        })
        .collect();
    let fb_environment =
        fb_environment.map_err(|e: moor_schema::convert::VarConversionError| {
            TaskConversionError::VarError(format!("Error encoding environment: {e}"))
        })?;

    let fb_valstack: Result<Vec<_>, _> = frame.valstack.iter().map(var_to_db_flatbuffer).collect();
    let fb_valstack = fb_valstack
        .map_err(|e| TaskConversionError::VarError(format!("Error encoding valstack: {e}")))?;

    let fb_scope_stack: Result<Vec<_>, _> =
        frame.scope_stack.iter().map(scope_to_flatbuffer).collect();
    let fb_scope_stack = fb_scope_stack?;

    let fb_temp = var_to_db_flatbuffer(&frame.temp)
        .map_err(|e| TaskConversionError::VarError(format!("Error encoding temp: {e}")))?;

    let fb_catch_stack: Result<Vec<_>, _> = frame
        .catch_stack
        .iter()
        .map(|(ct, l)| catch_handler_to_flatbuffer(ct, l))
        .collect();
    let fb_catch_stack = fb_catch_stack?;

    let fb_finally_stack: Result<Vec<_>, _> = frame
        .finally_stack
        .iter()
        .map(finally_reason_to_flatbuffer)
        .collect();
    let fb_finally_stack = fb_finally_stack?;

    let fb_capture_stack: Result<Vec<_>, TaskConversionError> = frame
        .capture_stack
        .iter()
        .map(|(name, var)| {
            Ok(fb::CapturedVar {
                name: Box::new(name_to_stored(name)?),
                value: Box::new(var_to_db_flatbuffer(var).map_err(|e| {
                    TaskConversionError::VarError(format!("Error encoding captured var: {e}"))
                })?),
            })
        })
        .collect();
    let fb_capture_stack = fb_capture_stack?;

    Ok(fb::MooStackFrame {
        program: Box::new(fb_program),
        pc: frame.pc as u64,
        pc_type: Box::new(fb_pc_type),
        environment: fb_environment,
        valstack: fb_valstack,
        scope_stack: fb_scope_stack,
        temp: Box::new(fb_temp),
        catch_stack: fb_catch_stack,
        finally_stack: fb_finally_stack,
        capture_stack: fb_capture_stack,
    })
}

pub(crate) fn moo_stack_frame_from_ref(
    fb: fb::MooStackFrameRef<'_>,
) -> Result<KernelMooStackFrame, TaskConversionError> {
    let program_ref = fb
        .program()
        .map_err(|e| TaskConversionError::DecodingError(format!("program: {e}")))?;
    let program = decode_stored_program_ref(program_ref)
        .map_err(|e| TaskConversionError::ProgramError(format!("Error decoding program: {e}")))?;

    let pc_type_ref = fb
        .pc_type()
        .map_err(|e| TaskConversionError::DecodingError(format!("pc_type: {e}")))?;
    let pc_type = pc_type_from_ref(pc_type_ref)?;

    let pc = fb
        .pc()
        .map_err(|e| TaskConversionError::DecodingError(format!("pc: {e}")))?;

    // Convert environment - v_none() is used as sentinel for uninitialized slots
    let environment_vec = fb
        .environment()
        .map_err(|e| TaskConversionError::DecodingError(format!("environment: {e}")))?;
    let environment: Result<Vec<Vec<moor_var::Var>>, TaskConversionError> = environment_vec
        .iter()
        .map(|scope_result| {
            let scope = scope_result
                .map_err(|e| TaskConversionError::DecodingError(format!("env scope: {e}")))?;
            let vars_vec = scope
                .vars()
                .map_err(|e| TaskConversionError::DecodingError(format!("vars: {e}")))?;
            let vars: Result<Vec<_>, TaskConversionError> = vars_vec
                .iter()
                .map(|v_result| {
                    let v = v_result
                        .map_err(|e| TaskConversionError::DecodingError(format!("var: {e}")))?;
                    let var = var_from_db_flatbuffer_ref(v)
                        .map_err(|e| TaskConversionError::VarError(format!("{e}")))?;
                    Ok(var)
                })
                .collect();
            vars
        })
        .collect();
    let environment = environment?;

    let valstack_vec = fb
        .valstack()
        .map_err(|e| TaskConversionError::DecodingError(format!("valstack: {e}")))?;
    let valstack: Result<Vec<_>, TaskConversionError> = valstack_vec
        .iter()
        .map(|v_result| {
            let v =
                v_result.map_err(|e| TaskConversionError::DecodingError(format!("val: {e}")))?;
            var_from_db_flatbuffer_ref(v).map_err(|e| TaskConversionError::VarError(format!("{e}")))
        })
        .collect();
    let valstack = valstack?;

    let scope_stack_vec = fb
        .scope_stack()
        .map_err(|e| TaskConversionError::DecodingError(format!("scope_stack: {e}")))?;
    let scope_stack: Result<Vec<_>, TaskConversionError> = scope_stack_vec
        .iter()
        .map(|s_result| {
            let s =
                s_result.map_err(|e| TaskConversionError::DecodingError(format!("scope: {e}")))?;
            scope_from_ref(s)
        })
        .collect();
    let scope_stack = scope_stack?;

    let temp_ref = fb
        .temp()
        .map_err(|e| TaskConversionError::DecodingError(format!("temp: {e}")))?;
    let temp = var_from_db_flatbuffer_ref(temp_ref)
        .map_err(|e| TaskConversionError::VarError(format!("{e}")))?;

    let catch_stack_vec = fb
        .catch_stack()
        .map_err(|e| TaskConversionError::DecodingError(format!("catch_stack: {e}")))?;
    let catch_stack: Result<Vec<_>, TaskConversionError> = catch_stack_vec
        .iter()
        .map(|c_result| {
            let c =
                c_result.map_err(|e| TaskConversionError::DecodingError(format!("catch: {e}")))?;
            catch_handler_from_ref(c)
        })
        .collect();
    let catch_stack = catch_stack?;

    let finally_stack_vec = fb
        .finally_stack()
        .map_err(|e| TaskConversionError::DecodingError(format!("finally_stack: {e}")))?;
    let finally_stack: Result<Vec<_>, TaskConversionError> = finally_stack_vec
        .iter()
        .map(|f_result| {
            let f = f_result
                .map_err(|e| TaskConversionError::DecodingError(format!("finally: {e}")))?;
            finally_reason_from_ref(f)
        })
        .collect();
    let finally_stack = finally_stack?;

    let capture_stack_vec = fb
        .capture_stack()
        .map_err(|e| TaskConversionError::DecodingError(format!("capture_stack: {e}")))?;
    let capture_stack: Result<Vec<_>, TaskConversionError> = capture_stack_vec
        .iter()
        .map(|c_result| {
            let c = c_result
                .map_err(|e| TaskConversionError::DecodingError(format!("capture: {e}")))?;
            let name_ref = c
                .name()
                .map_err(|e| TaskConversionError::DecodingError(format!("capture name: {e}")))?;
            let value_ref = c
                .value()
                .map_err(|e| TaskConversionError::DecodingError(format!("capture value: {e}")))?;
            Ok((
                name_from_ref(name_ref)?,
                var_from_db_flatbuffer_ref(value_ref)?,
            ))
        })
        .collect();
    let capture_stack = capture_stack?;

    let mut frame = KernelMooStackFrame::with_environment(program, environment);
    frame.pc = pc as usize;
    frame.pc_type = pc_type;
    frame.valstack = valstack;
    frame.scope_stack = scope_stack;
    frame.temp = temp;
    frame.catch_stack = catch_stack;
    frame.finally_stack = finally_stack;
    frame.capture_stack = capture_stack;

    Ok(frame)
}

// ============================================================================
// BfFrame Conversion
// ============================================================================

pub(crate) fn bf_frame_to_flatbuffer(
    frame: &KernelBfFrame,
) -> Result<fb::BfFrame, TaskConversionError> {
    let fb_trampoline_arg = frame
        .bf_trampoline_arg
        .as_ref()
        .map(var_to_db_flatbuffer)
        .transpose()
        .map_err(|e| {
            TaskConversionError::VarError(format!("Error encoding bf_trampoline_arg: {e}"))
        })?;

    let fb_return_value = frame
        .return_value
        .as_ref()
        .map(var_to_db_flatbuffer)
        .transpose()
        .map_err(|e| TaskConversionError::VarError(format!("Error encoding return_value: {e}")))?;

    Ok(fb::BfFrame {
        bf_id: frame.bf_id.0,
        bf_trampoline: frame.bf_trampoline.unwrap_or(0) as u64,
        has_trampoline: frame.bf_trampoline.is_some(),
        bf_trampoline_arg: fb_trampoline_arg.map(Box::new),
        return_value: fb_return_value.map(Box::new),
    })
}

pub(crate) fn bf_frame_from_ref(
    fb: fb::BfFrameRef<'_>,
) -> Result<KernelBfFrame, TaskConversionError> {
    let has_trampoline = fb
        .has_trampoline()
        .map_err(|e| TaskConversionError::DecodingError(format!("has_trampoline: {e}")))?;
    let bf_trampoline = if has_trampoline {
        let val = fb
            .bf_trampoline()
            .map_err(|e| TaskConversionError::DecodingError(format!("bf_trampoline: {e}")))?;
        Some(val as usize)
    } else {
        None
    };

    let bf_trampoline_arg = fb
        .bf_trampoline_arg()
        .map_err(|e| TaskConversionError::DecodingError(format!("bf_trampoline_arg: {e}")))?
        .map(|v| var_from_db_flatbuffer_ref(v))
        .transpose()
        .map_err(|e| TaskConversionError::VarError(format!("bf_trampoline_arg: {e}")))?;

    let return_value = fb
        .return_value()
        .map_err(|e| TaskConversionError::DecodingError(format!("return_value: {e}")))?
        .map(|v| var_from_db_flatbuffer_ref(v))
        .transpose()
        .map_err(|e| TaskConversionError::VarError(format!("return_value: {e}")))?;

    let bf_id = fb
        .bf_id()
        .map_err(|e| TaskConversionError::DecodingError(format!("bf_id: {e}")))?;

    Ok(KernelBfFrame {
        bf_id: moor_compiler::BuiltinId(bf_id),
        bf_trampoline,
        bf_trampoline_arg,
        return_value,
        caller_perms_override: None,
    })
}

// ============================================================================
// Frame Conversion
// ============================================================================

pub(crate) fn frame_to_flatbuffer(frame: &KernelFrame) -> Result<fb::Frame, TaskConversionError> {
    use fb::FrameUnion;

    let frame_union = match frame {
        KernelFrame::Moo(moo_frame) => {
            let fb_moo = moo_stack_frame_to_flatbuffer(moo_frame)?;
            FrameUnion::MooFrame(Box::new(fb::MooFrame {
                frame: Box::new(fb_moo),
            }))
        }
        KernelFrame::Bf(bf_frame) => {
            let fb_bf = bf_frame_to_flatbuffer(bf_frame)?;
            FrameUnion::BfFrame(Box::new(fb_bf))
        }
    };

    Ok(fb::Frame { frame: frame_union })
}

pub(crate) fn frame_from_ref(fb: fb::FrameRef<'_>) -> Result<KernelFrame, TaskConversionError> {
    use fb::FrameUnionRef;

    let frame_union = fb
        .frame()
        .map_err(|e| TaskConversionError::DecodingError(format!("frame union: {e}")))?;

    match frame_union {
        FrameUnionRef::MooFrame(mf) => {
            let moo_frame_ref = mf
                .frame()
                .map_err(|e| TaskConversionError::DecodingError(format!("moo frame: {e}")))?;
            let moo_frame = moo_stack_frame_from_ref(moo_frame_ref)?;
            Ok(KernelFrame::Moo(moo_frame))
        }
        FrameUnionRef::BfFrame(bf) => {
            let bf_frame = bf_frame_from_ref(bf)?;
            Ok(KernelFrame::Bf(bf_frame))
        }
    }
}

// ============================================================================
// Activation Conversion
// ============================================================================

pub(crate) fn activation_to_flatbuffer(
    activation: &KernelActivation,
) -> Result<fb::Activation, TaskConversionError> {
    let fb_frame = frame_to_flatbuffer(&activation.frame)?;

    let fb_this = var_to_db_flatbuffer(&activation.this)
        .map_err(|e| TaskConversionError::VarError(format!("Error encoding this: {e}")))?;

    let fb_player = convert_schema::obj_to_flatbuffer_struct(&activation.player);

    let fb_args: Result<Vec<_>, _> = activation
        .args
        .iter()
        .map(|v| var_to_db_flatbuffer(&v))
        .collect();
    let fb_args =
        fb_args.map_err(|e| TaskConversionError::VarError(format!("Error encoding args: {e}")))?;

    let fb_verb_name = convert_schema::symbol_to_flatbuffer_struct(&activation.verb_name);

    let fb_verbdef = convert_schema::verbdef_to_flatbuffer(&activation.verbdef)
        .map_err(|e| TaskConversionError::EncodingError(format!("Error encoding verbdef: {e}")))?;

    let fb_permissions = convert_schema::obj_to_flatbuffer_struct(&activation.permissions);

    Ok(fb::Activation {
        frame: Box::new(fb_frame),
        this: Box::new(fb_this),
        player: Box::new(fb_player),
        args: fb_args,
        verb_name: Box::new(fb_verb_name),
        verbdef: Box::new(fb_verbdef),
        permissions: Box::new(fb_permissions),
        permissions_flags: activation.permissions_flags.to_u16(),
    })
}

pub(crate) fn activation_from_ref(
    fb: fb::ActivationRef<'_>,
) -> Result<KernelActivation, TaskConversionError> {
    let frame_ref = fb
        .frame()
        .map_err(|e| TaskConversionError::DecodingError(format!("frame: {e}")))?;
    let frame = frame_from_ref(frame_ref)?;

    let this_ref = fb
        .this()
        .map_err(|e| TaskConversionError::DecodingError(format!("this: {e}")))?;
    let this = var_from_db_flatbuffer_ref(this_ref)
        .map_err(|e| TaskConversionError::VarError(format!("this: {e}")))?;

    let player_ref = fb
        .player()
        .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;
    let player = convert_schema::obj_from_ref(player_ref)
        .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;

    let args_vec = fb
        .args()
        .map_err(|e| TaskConversionError::DecodingError(format!("args: {e}")))?;
    let args: Result<Vec<_>, TaskConversionError> = args_vec
        .iter()
        .map(|v_result| {
            let v =
                v_result.map_err(|e| TaskConversionError::DecodingError(format!("arg: {e}")))?;
            var_from_db_flatbuffer_ref(v)
                .map_err(|e| TaskConversionError::VarError(format!("arg: {e}")))
        })
        .collect();
    let args = moor_var::List::mk_list(&args?);

    let verb_name_ref = fb
        .verb_name()
        .map_err(|e| TaskConversionError::DecodingError(format!("verb_name: {e}")))?;
    let verb_name = convert_schema::symbol_from_ref(verb_name_ref)
        .map_err(|e| TaskConversionError::DecodingError(format!("verb_name: {e}")))?;

    let verbdef_ref = fb
        .verbdef()
        .map_err(|e| TaskConversionError::DecodingError(format!("verbdef: {e}")))?;
    let verbdef = verbdef_from_ref(verbdef_ref)
        .map_err(|e| TaskConversionError::DecodingError(format!("verbdef: {e}")))?;

    let permissions_ref = fb
        .permissions()
        .map_err(|e| TaskConversionError::DecodingError(format!("permissions: {e}")))?;
    let permissions = convert_schema::obj_from_ref(permissions_ref)
        .map_err(|e| TaskConversionError::DecodingError(format!("permissions: {e}")))?;

    let permissions_flags_raw = fb
        .permissions_flags()
        .map_err(|e| TaskConversionError::DecodingError(format!("permissions_flags: {e}")))?;
    let permissions_flags = BitEnum::from_u16(permissions_flags_raw);

    Ok(KernelActivation {
        frame,
        this,
        player,
        args,
        verb_name,
        verbdef,
        permissions,
        permissions_flags,
    })
}

// ============================================================================
// VMExecState Conversion
// ============================================================================

pub(crate) fn vm_exec_state_to_flatbuffer(
    state: &KernelVMExecState,
) -> Result<fb::VmExecState, TaskConversionError> {
    let fb_activation_stack: Result<Vec<_>, _> =
        state.stack.iter().map(activation_to_flatbuffer).collect();
    let fb_activation_stack = fb_activation_stack?;

    let start_time_nanos = state
        .start_time
        .and_then(|st| st.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    Ok(fb::VmExecState {
        activation_stack: fb_activation_stack,
        tick_count: state.tick_count as u64,
        start_time_nanos,
    })
}

pub(crate) fn vm_exec_state_from_ref(
    fb: fb::VmExecStateRef<'_>,
) -> Result<KernelVMExecState, TaskConversionError> {
    let activation_stack_vec = fb
        .activation_stack()
        .map_err(|e| TaskConversionError::DecodingError(format!("activation_stack: {e}")))?;
    let stack: Result<Vec<_>, TaskConversionError> = activation_stack_vec
        .iter()
        .map(|a_result| {
            let a = a_result
                .map_err(|e| TaskConversionError::DecodingError(format!("activation: {e}")))?;
            activation_from_ref(a)
        })
        .collect();
    let stack = stack?;

    let tick_count = fb
        .tick_count()
        .map_err(|e| TaskConversionError::DecodingError(format!("tick_count: {e}")))?;

    let start_time_nanos = fb
        .start_time_nanos()
        .map_err(|e| TaskConversionError::DecodingError(format!("start_time_nanos: {e}")))?;
    let start_time = if start_time_nanos > 0 {
        Some(SystemTime::UNIX_EPOCH + Duration::from_nanos(start_time_nanos))
    } else {
        None
    };

    Ok(KernelVMExecState {
        task_id: 0, // Will be set by caller
        stack,
        tick_slice: 0,
        max_ticks: 0, // Will be set by caller
        tick_count: tick_count as usize,
        start_time,
        maximum_time: None, // Will be set by caller
        pending_raise_error: None,
        unsync: Default::default(),
    })
}

// ============================================================================
// VmHost Conversion
// ============================================================================

pub(crate) fn vm_host_to_flatbuffer(
    host: &KernelVmHost,
) -> Result<fb::VmHost, TaskConversionError> {
    let fb_exec_state = vm_exec_state_to_flatbuffer(&host.vm_exec_state)?;

    Ok(fb::VmHost {
        task_id: host.vm_exec_state.task_id as u64,
        max_stack_depth: host.max_stack_depth as u64,
        max_ticks: host.max_ticks as u64,
        max_time_ms: host.max_time.as_millis() as u64,
        exec_state: Box::new(fb_exec_state),
    })
}

pub(crate) fn vm_host_from_ref(fb: fb::VmHostRef<'_>) -> Result<KernelVmHost, TaskConversionError> {
    let task_id = fb
        .task_id()
        .map_err(|e| TaskConversionError::DecodingError(format!("task_id: {e}")))?;
    let max_stack_depth = fb
        .max_stack_depth()
        .map_err(|e| TaskConversionError::DecodingError(format!("max_stack_depth: {e}")))?;
    let max_ticks = fb
        .max_ticks()
        .map_err(|e| TaskConversionError::DecodingError(format!("max_ticks: {e}")))?;
    let max_time_ms = fb
        .max_time_ms()
        .map_err(|e| TaskConversionError::DecodingError(format!("max_time_ms: {e}")))?;
    let exec_state_ref = fb
        .exec_state()
        .map_err(|e| TaskConversionError::DecodingError(format!("exec_state: {e}")))?;

    let mut exec_state = vm_exec_state_from_ref(exec_state_ref)?;
    exec_state.task_id = task_id as usize;
    exec_state.max_ticks = max_ticks as usize;
    exec_state.maximum_time = Some(Duration::from_millis(max_time_ms));

    Ok(KernelVmHost {
        vm_exec_state: exec_state,
        max_stack_depth: max_stack_depth as usize,
        max_ticks: max_ticks as usize,
        max_time: Duration::from_millis(max_time_ms),
        running: true,
        unsync: Default::default(),
    })
}

// ============================================================================
// TaskState Conversion
// ============================================================================

fn task_state_to_flatbuffer(state: &KernelTaskState) -> Result<fb::TaskState, TaskConversionError> {
    let state_union = match state {
        KernelTaskState::Pending(start) => {
            let fb_start = task_start_to_flatbuffer(start)?;
            fb::TaskStateUnion::TaskCreated(Box::new(fb::TaskCreated {
                start: fb_start.start,
            }))
        }
        KernelTaskState::Prepared(start) => {
            let fb_start = task_start_to_flatbuffer(start)?;
            fb::TaskStateUnion::TaskRunning(Box::new(fb::TaskRunning {
                start: fb_start.start,
            }))
        }
    };

    Ok(fb::TaskState { state: state_union })
}

// ============================================================================
// TaskStart Conversion
// ============================================================================

pub(crate) fn task_start_to_flatbuffer(
    task_start: &KernelTaskStart,
) -> Result<fb::TaskStart, TaskConversionError> {
    use fb::TaskStartUnion;

    let start_union = match task_start {
        KernelTaskStart::StartCommandVerb {
            handler_object,
            player,
            command,
        } => TaskStartUnion::StartCommandVerb(Box::new(fb::StartCommandVerb {
            handler_object: Box::new(convert_schema::obj_to_flatbuffer_struct(handler_object)),
            player: Box::new(convert_schema::obj_to_flatbuffer_struct(player)),
            command: command.clone(),
        })),
        KernelTaskStart::StartDoCommand {
            handler_object,
            player,
            command,
        } => TaskStartUnion::StartDoCommand(Box::new(fb::StartDoCommand {
            handler_object: Box::new(convert_schema::obj_to_flatbuffer_struct(handler_object)),
            player: Box::new(convert_schema::obj_to_flatbuffer_struct(player)),
            command: command.clone(),
        })),
        KernelTaskStart::StartVerb {
            player,
            vloc,
            verb,
            args,
            argstr,
        } => {
            let fb_vloc = var_to_db_flatbuffer(vloc)
                .map_err(|e| TaskConversionError::VarError(format!("Error encoding vloc: {e}")))?;
            let fb_args: Result<Vec<_>, _> =
                args.iter().map(|v| var_to_db_flatbuffer(&v)).collect();
            let fb_args = fb_args
                .map_err(|e| TaskConversionError::VarError(format!("Error encoding args: {e}")))?;

            TaskStartUnion::StartVerb(Box::new(fb::StartVerb {
                player: Box::new(convert_schema::obj_to_flatbuffer_struct(player)),
                vloc: Box::new(fb_vloc),
                verb: Box::new(convert_schema::symbol_to_flatbuffer_struct(verb)),
                args: fb_args,
                argstr: argstr
                    .as_string()
                    .map(|x| x.to_string())
                    .unwrap_or("".to_string()),
            }))
        }
        KernelTaskStart::StartFork {
            fork_request,
            suspended,
        } => {
            let fb_fork = fork_to_flatbuffer(fork_request)?;
            let suspended_nanos = if *suspended {
                // Use a sentinel value to indicate suspended
                u64::MAX
            } else {
                0
            };

            TaskStartUnion::StartFork(Box::new(fb::StartFork {
                fork_request: Box::new(fb_fork),
                suspended_nanos,
            }))
        }
        KernelTaskStart::StartEval {
            player, program, ..
        } => {
            // initial_env is not serialized - it's ephemeral and only used during task creation.
            // By the time a task could suspend, the environment has already been applied to the frame.
            let fb_program = encode_program_to_fb(program).map_err(|e| {
                TaskConversionError::ProgramError(format!("Error encoding program: {e}"))
            })?;

            TaskStartUnion::StartEval(Box::new(fb::StartEval {
                player: Box::new(convert_schema::obj_to_flatbuffer_struct(player)),
                program: Box::new(fb_program),
            }))
        }
        KernelTaskStart::StartExceptionHandler { .. } => {
            // Exception handlers don't get suspended, so they shouldn't be serialized
            panic!("Attempted to serialize StartExceptionHandler task state");
        }
    };

    Ok(fb::TaskStart { start: start_union })
}

pub(crate) fn task_start_from_ref_union(
    fb: fb::TaskStartUnionRef<'_>,
) -> Result<KernelTaskStart, TaskConversionError> {
    use fb::TaskStartUnionRef;

    match fb {
        TaskStartUnionRef::StartCommandVerb(scv) => {
            let handler_object_ref = scv
                .handler_object()
                .map_err(|e| TaskConversionError::DecodingError(format!("handler_object: {e}")))?;
            let handler_object = convert_schema::obj_from_ref(handler_object_ref)
                .map_err(|e| TaskConversionError::DecodingError(format!("handler_object: {e}")))?;

            let player_ref = scv
                .player()
                .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;
            let player = convert_schema::obj_from_ref(player_ref)
                .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;

            let command = scv
                .command()
                .map_err(|e| TaskConversionError::DecodingError(format!("command: {e}")))?;

            Ok(KernelTaskStart::StartCommandVerb {
                handler_object,
                player,
                command: command.to_string(),
            })
        }
        TaskStartUnionRef::StartDoCommand(sdc) => {
            let handler_object_ref = sdc
                .handler_object()
                .map_err(|e| TaskConversionError::DecodingError(format!("handler_object: {e}")))?;
            let handler_object = convert_schema::obj_from_ref(handler_object_ref)
                .map_err(|e| TaskConversionError::DecodingError(format!("handler_object: {e}")))?;

            let player_ref = sdc
                .player()
                .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;
            let player = convert_schema::obj_from_ref(player_ref)
                .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;

            let command = sdc
                .command()
                .map_err(|e| TaskConversionError::DecodingError(format!("command: {e}")))?;

            Ok(KernelTaskStart::StartDoCommand {
                handler_object,
                player,
                command: command.to_string(),
            })
        }
        TaskStartUnionRef::StartVerb(sv) => {
            let player_ref = sv
                .player()
                .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;
            let player = convert_schema::obj_from_ref(player_ref)
                .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;

            let vloc_ref = sv
                .vloc()
                .map_err(|e| TaskConversionError::DecodingError(format!("vloc: {e}")))?;
            let vloc = var_from_db_flatbuffer_ref(vloc_ref)
                .map_err(|e| TaskConversionError::VarError(format!("vloc: {e}")))?;

            let verb_ref = sv
                .verb()
                .map_err(|e| TaskConversionError::DecodingError(format!("verb: {e}")))?;
            let verb = convert_schema::symbol_from_ref(verb_ref)
                .map_err(|e| TaskConversionError::DecodingError(format!("verb: {e}")))?;

            let args_vec = sv
                .args()
                .map_err(|e| TaskConversionError::DecodingError(format!("args: {e}")))?;
            let args: Result<Vec<_>, TaskConversionError> = args_vec
                .iter()
                .map(|v_result| {
                    let v = v_result
                        .map_err(|e| TaskConversionError::DecodingError(format!("arg: {e}")))?;
                    var_from_db_flatbuffer_ref(v)
                        .map_err(|e| TaskConversionError::VarError(format!("arg: {e}")))
                })
                .collect();
            let args = moor_var::List::mk_list(&args?);

            let argstr = sv
                .argstr()
                .map_err(|e| TaskConversionError::DecodingError(format!("argstr: {e}")))?;

            Ok(KernelTaskStart::StartVerb {
                player,
                vloc,
                verb,
                args,
                argstr: v_str(argstr),
            })
        }
        TaskStartUnionRef::StartFork(sf) => {
            let fork_request_ref = sf
                .fork_request()
                .map_err(|e| TaskConversionError::DecodingError(format!("fork_request: {e}")))?;
            let fork_request = fork_from_ref(fork_request_ref)?;

            let suspended_nanos = sf
                .suspended_nanos()
                .map_err(|e| TaskConversionError::DecodingError(format!("suspended_nanos: {e}")))?;
            let suspended = suspended_nanos == u64::MAX;

            Ok(KernelTaskStart::StartFork {
                fork_request: Box::new(fork_request),
                suspended,
            })
        }
        TaskStartUnionRef::StartEval(se) => {
            let player_ref = se
                .player()
                .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;
            let player = convert_schema::obj_from_ref(player_ref)
                .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;

            let program_ref = se
                .program()
                .map_err(|e| TaskConversionError::DecodingError(format!("program: {e}")))?;
            let program = decode_stored_program_ref(program_ref)
                .map_err(|e| TaskConversionError::ProgramError(format!("program: {e}")))?;

            // initial_env is None because it's not serialized - it's ephemeral
            Ok(KernelTaskStart::StartEval {
                player,
                program,
                initial_env: None,
            })
        }
    }
}

fn task_state_from_ref(fb: fb::TaskStateRef<'_>) -> Result<KernelTaskState, TaskConversionError> {
    use fb::TaskStateUnionRef;

    let state_union = fb
        .state()
        .map_err(|e| TaskConversionError::DecodingError(format!("state: {e}")))?;

    match state_union {
        TaskStateUnionRef::TaskCreated(created) => {
            let start_union = created
                .start()
                .map_err(|e| TaskConversionError::DecodingError(format!("start: {e}")))?;
            let task_start = task_start_from_ref_union(start_union)?;
            Ok(KernelTaskState::Pending(task_start))
        }
        TaskStateUnionRef::TaskRunning(running) => {
            let start_union = running
                .start()
                .map_err(|e| TaskConversionError::DecodingError(format!("start: {e}")))?;
            let task_start = task_start_from_ref_union(start_union)?;
            Ok(KernelTaskState::Prepared(task_start))
        }
    }
}

// ============================================================================
// Fork Conversion
// ============================================================================

pub(crate) fn fork_to_flatbuffer(fork: &Fork) -> Result<fb::Fork, TaskConversionError> {
    let fb_activation = activation_to_flatbuffer(&fork.activation)?;

    let (delay_nanos, has_delay) = match fork.delay {
        Some(d) => (d.as_nanos() as u64, true),
        None => (0, false),
    };

    let fb_task_id = fork.task_id.as_ref().map(name_to_stored).transpose()?;

    Ok(fb::Fork {
        player: Box::new(convert_schema::obj_to_flatbuffer_struct(&fork.player)),
        progr: Box::new(convert_schema::obj_to_flatbuffer_struct(&fork.progr)),
        parent_task_id: fork.parent_task_id as u64,
        delay_nanos,
        has_delay,
        activation: Box::new(fb_activation),
        fork_vector_offset: fork.fork_vector_offset.0 as u64,
        task_id: fb_task_id.map(Box::new),
    })
}

pub(crate) fn fork_from_ref(fb: fb::ForkRef<'_>) -> Result<Fork, TaskConversionError> {
    let activation_ref = fb
        .activation()
        .map_err(|e| TaskConversionError::DecodingError(format!("activation: {e}")))?;
    let activation = activation_from_ref(activation_ref)?;

    let has_delay = fb
        .has_delay()
        .map_err(|e| TaskConversionError::DecodingError(format!("has_delay: {e}")))?;
    let delay = if has_delay {
        let delay_nanos = fb
            .delay_nanos()
            .map_err(|e| TaskConversionError::DecodingError(format!("delay_nanos: {e}")))?;
        Some(Duration::from_nanos(delay_nanos))
    } else {
        None
    };

    let task_id = fb
        .task_id()
        .map_err(|e| TaskConversionError::DecodingError(format!("task_id: {e}")))?
        .map(|n| name_from_ref(n))
        .transpose()?;

    let player_ref = fb
        .player()
        .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;
    let player = convert_schema::obj_from_ref(player_ref)
        .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;

    let progr_ref = fb
        .progr()
        .map_err(|e| TaskConversionError::DecodingError(format!("progr: {e}")))?;
    let progr = convert_schema::obj_from_ref(progr_ref)
        .map_err(|e| TaskConversionError::DecodingError(format!("progr: {e}")))?;

    let parent_task_id = fb
        .parent_task_id()
        .map_err(|e| TaskConversionError::DecodingError(format!("parent_task_id: {e}")))?;

    let fork_vector_offset = fb
        .fork_vector_offset()
        .map_err(|e| TaskConversionError::DecodingError(format!("fork_vector_offset: {e}")))?;

    Ok(Fork {
        player,
        progr,
        parent_task_id: parent_task_id as usize,
        delay,
        activation,
        fork_vector_offset: Offset(fork_vector_offset as u16),
        task_id,
    })
}

// ============================================================================
// Task Conversion
// ============================================================================

pub(crate) fn task_to_flatbuffer(task: &KernelTask) -> Result<fb::Task, TaskConversionError> {
    let fb_task_state = task_state_to_flatbuffer(&task.state)?;
    let fb_vm_host = vm_host_to_flatbuffer(&task.vm_host)?;
    let fb_retry_state = vm_exec_state_to_flatbuffer(&task.retry_state)?;

    let fb_pending_exception = task
        .pending_exception
        .as_ref()
        .map(exception_to_flatbuffer)
        .transpose()?;

    Ok(fb::Task {
        version: CURRENT_TASK_VERSION,
        task_id: task.task_id as u64,
        player: Box::new(convert_schema::obj_to_flatbuffer_struct(&task.player)),
        state: Box::new(fb_task_state),
        vm_host: Box::new(fb_vm_host),
        perms: Box::new(convert_schema::obj_to_flatbuffer_struct(&task.perms)),
        retries: task.retries,
        retry_state: Box::new(fb_retry_state),
        handling_uncaught_error: task.handling_uncaught_error,
        pending_exception: fb_pending_exception.map(Box::new),
        // Note: waiting_for_exception_handler_task removed - exception handlers no longer suspend
        waiting_for_exception_handler_task: 0,
        has_waiting_for_exception_handler_task: false,
        // Message queue is stored externally in TaskQ; not serialized with task for now.
        // On restart, message queues for suspended tasks will be empty.
        message_queue: None,
    })
}

pub(crate) fn task_from_ref(fb: fb::TaskRef<'_>) -> Result<KernelTask, TaskConversionError> {
    let version = fb
        .version()
        .map_err(|e| TaskConversionError::DecodingError(format!("version: {e}")))?;
    if version != CURRENT_TASK_VERSION {
        return Err(TaskConversionError::DecodingError(format!(
            "Unsupported task version: {version} (expected {CURRENT_TASK_VERSION})"
        )));
    }

    let task_id = fb
        .task_id()
        .map_err(|e| TaskConversionError::DecodingError(format!("task_id: {e}")))?;

    let state_ref = fb
        .state()
        .map_err(|e| TaskConversionError::DecodingError(format!("state: {e}")))?;
    let task_state = task_state_from_ref(state_ref)?;

    let vm_host_ref = fb
        .vm_host()
        .map_err(|e| TaskConversionError::DecodingError(format!("vm_host: {e}")))?;
    let vm_host = vm_host_from_ref(vm_host_ref)?;

    let retry_state_ref = fb
        .retry_state()
        .map_err(|e| TaskConversionError::DecodingError(format!("retry_state: {e}")))?;
    let mut retry_state = vm_exec_state_from_ref(retry_state_ref)?;
    retry_state.task_id = task_id as usize;

    let retries = fb
        .retries()
        .map_err(|e| TaskConversionError::DecodingError(format!("retries: {e}")))?;

    let handling_uncaught_error = fb
        .handling_uncaught_error()
        .map_err(|e| TaskConversionError::DecodingError(format!("handling_uncaught_error: {e}")))?;

    let pending_exception = fb
        .pending_exception()
        .map_err(|e| TaskConversionError::DecodingError(format!("pending_exception: {e}")))?
        .map(|e| exception_from_ref(e))
        .transpose()?;

    let player_ref = fb
        .player()
        .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;
    let player = convert_schema::obj_from_ref(player_ref)
        .map_err(|e| TaskConversionError::DecodingError(format!("player: {e}")))?;

    let perms_ref = fb
        .perms()
        .map_err(|e| TaskConversionError::DecodingError(format!("perms: {e}")))?;
    let perms = convert_schema::obj_from_ref(perms_ref)
        .map_err(|e| TaskConversionError::DecodingError(format!("perms: {e}")))?;

    Ok(KernelTask {
        task_id: task_id as usize,
        creation_time: minstant::Instant::now(),
        player,
        state: task_state,
        vm_host,
        perms,
        kill_switch: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        retries,
        retry_state,
        handling_uncaught_error,
        pending_exception,
    })
}

// ============================================================================
// SuspendedTask Conversion
// ============================================================================

/// Convert a kernel SuspendedTask to a FlatBuffer representation.
/// Note: session and result_sender fields are not serialized as they are runtime-specific.
pub fn suspended_task_to_flatbuffer(
    suspended_task: &KernelSuspendedTask,
) -> Result<fb::SuspendedTask, TaskConversionError> {
    let wake_condition = &suspended_task.wake_condition;
    let task = &*suspended_task.task;
    let fb_wake_condition = wake_condition_to_flatbuffer(wake_condition)?;
    let fb_task = task_to_flatbuffer(task)?;

    Ok(fb::SuspendedTask {
        version: CURRENT_TASK_VERSION,
        wake_condition: Box::new(fb_wake_condition),
        task: Box::new(fb_task),
    })
}

/// Convert a FlatBuffer SuspendedTaskRef directly to a kernel SuspendedTask without copying.
pub fn suspended_task_from_ref(
    fb: fb::SuspendedTaskRef<'_>,
) -> Result<KernelSuspendedTask, TaskConversionError> {
    use moor_common::tasks::NoopClientSession;
    use std::sync::Arc;

    let version = fb
        .version()
        .map_err(|e| TaskConversionError::DecodingError(format!("version: {e}")))?;
    if version != CURRENT_TASK_VERSION {
        return Err(TaskConversionError::DecodingError(format!(
            "Unsupported suspended task version: {version} (expected {CURRENT_TASK_VERSION})"
        )));
    }

    let wake_condition_ref = fb
        .wake_condition()
        .map_err(|e| TaskConversionError::DecodingError(format!("wake_condition: {e}")))?;
    let wake_condition = wake_condition_from_ref(wake_condition_ref)?;

    let task_ref = fb
        .task()
        .map_err(|e| TaskConversionError::DecodingError(format!("task: {e}")))?;
    let task = Box::new(task_from_ref(task_ref)?);

    Ok(KernelSuspendedTask {
        wake_condition,
        task,
        session: Arc::new(NoopClientSession::new()),
        result_sender: None,
    })
}
