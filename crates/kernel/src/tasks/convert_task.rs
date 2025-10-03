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

//! Conversion between kernel task types and FlatBuffer representations

use moor_common::tasks::AbortLimitReason as KernelAbortLimitReason;
use crate::tasks::{TaskStart as KernelTaskStart, task::Task as KernelTask};
use crate::tasks::task_q::{SuspendedTask as KernelSuspendedTask, WakeCondition as KernelWakeCondition};
use crate::vm::{
    FinallyReason as KernelFinallyReason, Fork,
    activation::{Activation as KernelActivation, BfFrame as KernelBfFrame, Frame as KernelFrame},
    exec_state::VMExecState as KernelVMExecState,
    moo_frame::{
        CatchType as KernelCatchType, MooStackFrame as KernelMooStackFrame, PcType as KernelPcType,
        Scope as KernelScope, ScopeType as KernelScopeType,
    },
    vm_host::VmHost as KernelVmHost,
};
use moor_compiler::{Label, Offset};
use moor_schema::common as fb_common;
use moor_schema::convert as convert_schema;
use moor_schema::convert_program::{encode_program_to_fb, decode_stored_program_struct};
use moor_schema::program as fb_program;
use moor_schema::task as fb;
use moor_var::program::names::Name;
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

fn name_from_stored(stored: &fb_program::StoredName) -> Result<Name, TaskConversionError> {
    Ok(Name(stored.offset, stored.scope_depth, stored.scope_id))
}

fn exception_to_flatbuffer(
    exception: &moor_common::tasks::Exception,
) -> Result<fb_common::Exception, TaskConversionError> {
    let fb_error = convert_schema::error_to_flatbuffer_struct(&exception.error)
        .map_err(|e| TaskConversionError::EncodingError(format!("Error encoding error: {e}")))?;

    let fb_stack: Result<Vec<_>, _> = exception
        .stack
        .iter()
        .map(convert_schema::var_to_db_flatbuffer)
        .collect();
    let fb_stack = fb_stack.map_err(|e| TaskConversionError::VarError(format!("Error encoding stack: {e}")))?;

    let fb_backtrace: Result<Vec<_>, _> = exception
        .backtrace
        .iter()
        .map(convert_schema::var_to_db_flatbuffer)
        .collect();
    let fb_backtrace = fb_backtrace.map_err(|e| TaskConversionError::VarError(format!("Error encoding backtrace: {e}")))?;

    Ok(fb_common::Exception {
        error: Box::new(fb_error),
        stack: fb_stack,
        backtrace: fb_backtrace,
    })
}

fn exception_from_flatbuffer(
    fb: &fb_common::Exception,
) -> Result<moor_common::tasks::Exception, TaskConversionError> {
    let error = convert_schema::error_from_flatbuffer_struct(&fb.error)
        .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding error: {e}")))?;

    let stack: Result<Vec<_>, _> = fb
        .stack
        .iter()
        .map(convert_schema::var_from_db_flatbuffer)
        .collect();
    let stack = stack.map_err(|e| TaskConversionError::VarError(format!("Error decoding stack: {e}")))?;

    let backtrace: Result<Vec<_>, _> = fb
        .backtrace
        .iter()
        .map(convert_schema::var_from_db_flatbuffer)
        .collect();
    let backtrace = backtrace.map_err(|e| TaskConversionError::VarError(format!("Error decoding backtrace: {e}")))?;

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
    use fb::{*, WakeConditionUnion::*};
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
        KernelWakeCondition::Immedate => {
            WakeImmediate(Box::new(fb::WakeImmediate {}))
        }
        KernelWakeCondition::Task(task_id) => {
            WakeTask(Box::new(fb::WakeTask {
                task_id: *task_id as u64,
            }))
        }
        KernelWakeCondition::Worker(uuid) => {
            let uuid_bytes = uuid.as_bytes();
            WakeWorker(Box::new(fb::WakeWorker {
                uuid: Box::new(fb_common::Uuid {
                    data: uuid_bytes.to_vec(),
                }),
            }))
        }
        KernelWakeCondition::GCComplete => {
            WakeGcComplete(Box::new(fb::WakeGcComplete {}))
        }
    };

    Ok(WakeCondition {
        condition,
    })
}

pub(crate) fn wake_condition_from_flatbuffer(
    fb: &fb::WakeCondition,
) -> Result<KernelWakeCondition, TaskConversionError> {
    use fb::WakeConditionUnion;
    use minstant::Instant;

    match &fb.condition {
        WakeConditionUnion::WakeTime(wt) => {
            // Convert epoch nanos to Instant
            let epoch_duration = Duration::from_nanos(wt.nanos);
            let epoch_time = UNIX_EPOCH + epoch_duration;

            let now_system = SystemTime::now();
            let now_instant = Instant::now();

            let wake_instant = if epoch_time >= now_system {
                // Future time
                let time_diff = epoch_time.duration_since(now_system).unwrap_or(Duration::ZERO);
                now_instant + time_diff
            } else {
                // Past time
                let time_diff = now_system.duration_since(epoch_time).unwrap_or(Duration::ZERO);
                now_instant.checked_sub(time_diff).unwrap_or(now_instant)
            };

            Ok(KernelWakeCondition::Time(wake_instant))
        }
        WakeConditionUnion::WakeNever(_) => Ok(KernelWakeCondition::Never),
        WakeConditionUnion::WakeInput(wi) => {
            let uuid_bytes: [u8; 16] = wi.uuid.data.as_slice().try_into().map_err(|_| {
                TaskConversionError::DecodingError("Invalid UUID bytes".to_string())
            })?;
            let uuid = uuid::Uuid::from_bytes(uuid_bytes);
            Ok(KernelWakeCondition::Input(uuid))
        }
        WakeConditionUnion::WakeImmediate(_) => Ok(KernelWakeCondition::Immedate),
        WakeConditionUnion::WakeTask(wt) => Ok(KernelWakeCondition::Task(wt.task_id as usize)),
        WakeConditionUnion::WakeWorker(ww) => {
            let uuid_bytes: [u8; 16] = ww.uuid.data.as_slice().try_into().map_err(|_| {
                TaskConversionError::DecodingError("Invalid UUID bytes".to_string())
            })?;
            let uuid = uuid::Uuid::from_bytes(uuid_bytes);
            Ok(KernelWakeCondition::Worker(uuid))
        }
        WakeConditionUnion::WakeGcComplete(_) => Ok(KernelWakeCondition::GCComplete),
    }
}

// ============================================================================
// AbortLimitReason Conversion
// ============================================================================

pub(crate) fn abort_limit_reason_to_flatbuffer(
    reason: &KernelAbortLimitReason,
) -> Result<fb::AbortLimitReason, TaskConversionError> {
    use fb::*;

    let reason_union = match reason {
        KernelAbortLimitReason::Ticks(ticks) => {
            AbortLimitReasonUnion::AbortTicks(Box::new(AbortTicks { ticks: *ticks as u64 }))
        }
        KernelAbortLimitReason::Time(duration) => {
            AbortLimitReasonUnion::AbortTime(Box::new(AbortTime { seconds: duration.as_secs() }))
        }
    };

    Ok(AbortLimitReason {
        reason: reason_union,
    })
}

pub(crate) fn abort_limit_reason_from_flatbuffer(
    fb: &fb::AbortLimitReason,
) -> Result<KernelAbortLimitReason, TaskConversionError> {
    use fb::AbortLimitReasonUnion;

    match &fb.reason {
        AbortLimitReasonUnion::AbortTicks(at) => Ok(KernelAbortLimitReason::Ticks(at.ticks as usize)),
        AbortLimitReasonUnion::AbortTime(at) => Ok(KernelAbortLimitReason::Time(Duration::from_secs(at.seconds))),
    }
}

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

    Ok(PcType {
        pc_type,
    })
}

pub(crate) fn pc_type_from_flatbuffer(fb: &fb::PcType) -> Result<KernelPcType, TaskConversionError> {
    use fb::PcTypeUnion;

    match &fb.pc_type {
        PcTypeUnion::PcMain(_) => Ok(KernelPcType::Main),
        PcTypeUnion::PcForkVector(fv) => Ok(KernelPcType::ForkVector(Offset(fv.offset as u16))),
        PcTypeUnion::PcLambda(l) => Ok(KernelPcType::Lambda(Offset(l.offset as u16))),
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

    Ok(CatchType {
        catch_type,
    })
}

pub(crate) fn catch_type_from_flatbuffer(
    fb: &fb::CatchType,
) -> Result<KernelCatchType, TaskConversionError> {
    use fb::CatchTypeUnion;

    match &fb.catch_type {
        CatchTypeUnion::CatchAny(_) => Ok(KernelCatchType::Any),
        CatchTypeUnion::CatchErrors(ce) => {
            let errors: Result<Vec<_>, _> = ce
                .errors
                .iter()
                .map(|e| convert_schema::error_from_flatbuffer_struct(e))
                .collect();
            Ok(KernelCatchType::Errors(errors.map_err(|e| {
                TaskConversionError::DecodingError(format!("Error decoding errors: {e}"))
            })?))
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
            let fb_var = convert_schema::var_to_db_flatbuffer(var)
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

pub(crate) fn finally_reason_from_flatbuffer(
    fb: &fb::FinallyReason,
) -> Result<KernelFinallyReason, TaskConversionError> {
    use fb::FinallyReasonUnion;

    match &fb.reason {
        FinallyReasonUnion::FinallyFallthrough(_) => Ok(KernelFinallyReason::Fallthrough),
        FinallyReasonUnion::FinallyRaise(fr) => {
            let exception =
                exception_from_flatbuffer(&fr.exception).map_err(|e| {
                    TaskConversionError::DecodingError(format!("Error decoding exception: {e}"))
                })?;
            Ok(KernelFinallyReason::Raise(Box::new(exception)))
        }
        FinallyReasonUnion::FinallyReturn(fr) => {
            let var = convert_schema::var_from_db_flatbuffer(&fr.value)
                .map_err(|e| TaskConversionError::VarError(format!("Error decoding var: {e}")))?;
            Ok(KernelFinallyReason::Return(var))
        }
        FinallyReasonUnion::FinallyAbort(_) => Ok(KernelFinallyReason::Abort),
        FinallyReasonUnion::FinallyExit(fe) => Ok(KernelFinallyReason::Exit {
            stack: Offset(fe.stack as u16),
            label: Label(fe.label),
        }),
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

fn catch_handler_from_flatbuffer(
    fb: &fb::CatchHandler,
) -> Result<(KernelCatchType, Label), TaskConversionError> {
    Ok((catch_type_from_flatbuffer(&fb.catch_type)?, Label(fb.label)))
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
            value_bind,
            key_bind,
            end_label,
        } => {
            let fb_seq = convert_schema::var_to_db_flatbuffer(sequence).map_err(|e| {
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

            ScopeTypeUnion::ScopeForSequence(Box::new(ScopeForSequence {
                sequence: Box::new(fb_seq),
                current_index: *current_index as u64,
                value_bind: Box::new(fb_value_bind),
                key_bind: fb_key_bind.map(Box::new),
                end_label: end_label.0,
            }))
        }
        KernelScopeType::ForRange {
            current_value,
            end_value,
            loop_variable,
            end_label,
        } => {
            let fb_current = convert_schema::var_to_db_flatbuffer(current_value).map_err(|e| {
                TaskConversionError::VarError(format!("Error encoding current_value: {e}"))
            })?;
            let fb_end = convert_schema::var_to_db_flatbuffer(end_value).map_err(|e| {
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

    Ok(ScopeType {
        scope_type,
    })
}

pub(crate) fn scope_type_from_flatbuffer(
    fb: &fb::ScopeType,
) -> Result<KernelScopeType, TaskConversionError> {
    use fb::ScopeTypeUnion;

    match &fb.scope_type {
        ScopeTypeUnion::ScopeTryFinally(stf) => Ok(KernelScopeType::TryFinally(Label(stf.label))),
        ScopeTypeUnion::ScopeTryCatch(stc) => {
            let handlers: Result<Vec<_>, _> = stc
                .handlers
                .iter()
                .map(catch_handler_from_flatbuffer)
                .collect();
            Ok(KernelScopeType::TryCatch(handlers?))
        }
        ScopeTypeUnion::ScopeIf(_) => Ok(KernelScopeType::If),
        ScopeTypeUnion::ScopeEif(_) => Ok(KernelScopeType::Eif),
        ScopeTypeUnion::ScopeWhile(_) => Ok(KernelScopeType::While),
        ScopeTypeUnion::ScopeFor(_) => Ok(KernelScopeType::For),
        ScopeTypeUnion::ScopeForSequence(sfs) => {
            let sequence = convert_schema::var_from_db_flatbuffer(&sfs.sequence).map_err(|e| {
                TaskConversionError::VarError(format!("Error decoding sequence: {e}"))
            })?;
            let value_bind = name_from_stored(&sfs.value_bind).map_err(|e| {
                TaskConversionError::ProgramError(format!("Error decoding value_bind: {e}"))
            })?;
            let key_bind = sfs
                .key_bind
                .as_ref()
                .map(|kb| name_from_stored(kb))
                .transpose()
                .map_err(|e| {
                    TaskConversionError::ProgramError(format!("Error decoding key_bind: {e}"))
                })?;

            Ok(KernelScopeType::ForSequence {
                sequence,
                current_index: sfs.current_index as usize,
                value_bind,
                key_bind,
                end_label: Label(sfs.end_label),
            })
        }
        ScopeTypeUnion::ScopeForRange(sfr) => {
            let current_value =
                convert_schema::var_from_db_flatbuffer(&sfr.current_value).map_err(|e| {
                    TaskConversionError::VarError(format!("Error decoding current_value: {e}"))
                })?;
            let end_value = convert_schema::var_from_db_flatbuffer(&sfr.end_value).map_err(|e| {
                TaskConversionError::VarError(format!("Error decoding end_value: {e}"))
            })?;
            let loop_variable = name_from_stored(&sfr.loop_variable).map_err(|e| {
                TaskConversionError::ProgramError(format!("Error decoding loop_variable: {e}"))
            })?;

            Ok(KernelScopeType::ForRange {
                current_value,
                end_value,
                loop_variable,
                end_label: Label(sfr.end_label),
            })
        }
        ScopeTypeUnion::ScopeBlock(_) => Ok(KernelScopeType::Block),
        ScopeTypeUnion::ScopeComprehension(_) => Ok(KernelScopeType::Comprehension),
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

pub(crate) fn scope_from_flatbuffer(fb: &fb::Scope) -> Result<KernelScope, TaskConversionError> {
    Ok(KernelScope {
        scope_type: scope_type_from_flatbuffer(&fb.scope_type)?,
        valstack_pos: fb.valstack_pos as usize,
        start_pos: fb.start_pos as usize,
        end_pos: fb.end_pos as usize,
        environment: fb.has_environment,
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
        .iter()
        .map(|scope| {
            let vars: Result<Vec<_>, _> = scope
                .iter()
                .map(|opt_var| {
                    match opt_var {
                        Some(v) => convert_schema::var_to_db_flatbuffer(v),
                        // Use empty/None marker - we'll need to represent this as an empty Var
                        None => Ok(convert_schema::var_to_db_flatbuffer(&moor_var::v_none())?),
                    }
                })
                .collect();
            Ok(fb::EnvironmentScope { vars: vars? })
        })
        .collect();
    let fb_environment = fb_environment.map_err(|e: moor_schema::convert::VarConversionError| {
        TaskConversionError::VarError(format!("Error encoding environment: {e}"))
    })?;

    let fb_valstack: Result<Vec<_>, _> = frame
        .valstack
        .iter()
        .map(convert_schema::var_to_db_flatbuffer)
        .collect();
    let fb_valstack = fb_valstack
        .map_err(|e| TaskConversionError::VarError(format!("Error encoding valstack: {e}")))?;

    let fb_scope_stack: Result<Vec<_>, _> = frame
        .scope_stack
        .iter()
        .map(scope_to_flatbuffer)
        .collect();
    let fb_scope_stack = fb_scope_stack?;

    let fb_temp = convert_schema::var_to_db_flatbuffer(&frame.temp)
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

    let fb_capture_stack: Result<Vec<_>, _> = frame
        .capture_stack
        .iter()
        .map(|(name, var)| {
            Ok(fb::CapturedVar {
                name: Box::new(name_to_stored(name)?),
                value: Box::new(convert_schema::var_to_db_flatbuffer(var).map_err(|e| {
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

pub(crate) fn moo_stack_frame_from_flatbuffer(
    fb: &fb::MooStackFrame,
) -> Result<KernelMooStackFrame, TaskConversionError> {
    let program = decode_stored_program_struct(&fb.program)
        .map_err(|e| TaskConversionError::ProgramError(format!("Error decoding program: {e}")))?;

    let pc_type = pc_type_from_flatbuffer(&fb.pc_type)?;

    // Convert environment back
    let environment: Result<Vec<Vec<Option<moor_var::Var>>>, _> = fb
        .environment
        .iter()
        .map(|scope| {
            let vars: Result<Vec<_>, _> = scope
                .vars
                .iter()
                .map(|v| {
                    let var = convert_schema::var_from_db_flatbuffer(v).map_err(|e| {
                        TaskConversionError::VarError(format!("Error decoding var: {e}"))
                    })?;
                    // Check if this is our None marker
                    if var.is_none() {
                        Ok(None)
                    } else {
                        Ok(Some(var))
                    }
                })
                .collect();
            vars
        })
        .collect();
    let environment = environment?;

    let valstack: Result<Vec<_>, _> = fb
        .valstack
        .iter()
        .map(|v| {
            convert_schema::var_from_db_flatbuffer(v)
                .map_err(|e| TaskConversionError::VarError(format!("Error decoding valstack: {e}")))
        })
        .collect();
    let valstack = valstack?;

    let scope_stack: Result<Vec<_>, _> =
        fb.scope_stack.iter().map(scope_from_flatbuffer).collect();
    let scope_stack = scope_stack?;

    let temp = convert_schema::var_from_db_flatbuffer(&fb.temp)
        .map_err(|e| TaskConversionError::VarError(format!("Error decoding temp: {e}")))?;

    let catch_stack: Result<Vec<_>, _> = fb
        .catch_stack
        .iter()
        .map(catch_handler_from_flatbuffer)
        .collect();
    let catch_stack = catch_stack?;

    let finally_stack: Result<Vec<_>, _> = fb
        .finally_stack
        .iter()
        .map(finally_reason_from_flatbuffer)
        .collect();
    let finally_stack = finally_stack?;

    let capture_stack: Result<Vec<_>, _> = fb
        .capture_stack
        .iter()
        .map(|c| {
            Ok((
                name_from_stored(&c.name)?,
                convert_schema::var_from_db_flatbuffer(&c.value).map_err(|e| {
                    TaskConversionError::VarError(format!("Error decoding captured var: {e}"))
                })?,
            ))
        })
        .collect();
    let capture_stack = capture_stack?;

    Ok(KernelMooStackFrame {
        program,
        pc: fb.pc as usize,
        pc_type,
        environment,
        valstack,
        scope_stack,
        temp,
        catch_stack,
        finally_stack,
        capture_stack,
    })
}

// ============================================================================
// BfFrame Conversion
// ============================================================================

pub(crate) fn bf_frame_to_flatbuffer(frame: &KernelBfFrame) -> Result<fb::BfFrame, TaskConversionError> {
    let fb_trampoline_arg = frame
        .bf_trampoline_arg
        .as_ref()
        .map(convert_schema::var_to_db_flatbuffer)
        .transpose()
        .map_err(|e| {
            TaskConversionError::VarError(format!("Error encoding bf_trampoline_arg: {e}"))
        })?;

    let fb_return_value = frame
        .return_value
        .as_ref()
        .map(convert_schema::var_to_db_flatbuffer)
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

pub(crate) fn bf_frame_from_flatbuffer(fb: &fb::BfFrame) -> Result<KernelBfFrame, TaskConversionError> {
    let bf_trampoline = if fb.has_trampoline {
        Some(fb.bf_trampoline as usize)
    } else {
        None
    };

    let bf_trampoline_arg = fb
        .bf_trampoline_arg
        .as_ref()
        .map(|v| convert_schema::var_from_db_flatbuffer(v))
        .transpose()
        .map_err(|e| {
            TaskConversionError::VarError(format!("Error decoding bf_trampoline_arg: {e}"))
        })?;

    let return_value = fb
        .return_value
        .as_ref()
        .map(|v| convert_schema::var_from_db_flatbuffer(v))
        .transpose()
        .map_err(|e| TaskConversionError::VarError(format!("Error decoding return_value: {e}")))?;

    Ok(KernelBfFrame {
        bf_id: moor_compiler::BuiltinId(fb.bf_id),
        bf_trampoline,
        bf_trampoline_arg,
        return_value,
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

    Ok(fb::Frame {
        frame: frame_union,
    })
}

pub(crate) fn frame_from_flatbuffer(fb: &fb::Frame) -> Result<KernelFrame, TaskConversionError> {
    use fb::FrameUnion;

    match &fb.frame {
        FrameUnion::MooFrame(mf) => {
            let moo_frame = moo_stack_frame_from_flatbuffer(&mf.frame)?;
            Ok(KernelFrame::Moo(Box::new(moo_frame)))
        }
        FrameUnion::BfFrame(bf) => {
            let bf_frame = bf_frame_from_flatbuffer(bf)?;
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

    let fb_this = convert_schema::var_to_db_flatbuffer(&activation.this)
        .map_err(|e| TaskConversionError::VarError(format!("Error encoding this: {e}")))?;

    let fb_player = convert_schema::obj_to_flatbuffer_struct(&activation.player);

    let fb_args: Result<Vec<_>, _> = activation
        .args
        .iter()
        .map(|v| convert_schema::var_to_db_flatbuffer(&v))
        .collect();
    let fb_args = fb_args
        .map_err(|e| TaskConversionError::VarError(format!("Error encoding args: {e}")))?;

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
    })
}

pub(crate) fn activation_from_flatbuffer(
    fb: &fb::Activation,
) -> Result<KernelActivation, TaskConversionError> {
    let frame = frame_from_flatbuffer(&fb.frame)?;

    let this = convert_schema::var_from_db_flatbuffer(&fb.this)
        .map_err(|e| TaskConversionError::VarError(format!("Error decoding this: {e}")))?;

    let player = convert_schema::obj_from_flatbuffer_struct(&fb.player)
        .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding player: {e}")))?;

    let args: Result<Vec<_>, _> = fb
        .args
        .iter()
        .map(|v| {
            convert_schema::var_from_db_flatbuffer(v)
                .map_err(|e| TaskConversionError::VarError(format!("Error decoding args: {e}")))
        })
        .collect();
    let args = moor_var::List::mk_list(&args?);

    let verb_name = convert_schema::symbol_from_flatbuffer_struct(&fb.verb_name);

    let verbdef = convert_schema::verbdef_from_flatbuffer(&fb.verbdef)
        .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding verbdef: {e}")))?;

    let permissions = convert_schema::obj_from_flatbuffer_struct(&fb.permissions)
        .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding permissions: {e}")))?;

    Ok(KernelActivation {
        frame,
        this,
        player,
        args,
        verb_name,
        verbdef,
        permissions,
    })
}

// ============================================================================
// VMExecState Conversion
// ============================================================================

pub(crate) fn vm_exec_state_to_flatbuffer(
    state: &KernelVMExecState,
) -> Result<fb::VmExecState, TaskConversionError> {
    let fb_activation_stack: Result<Vec<_>, _> = state
        .stack
        .iter()
        .map(activation_to_flatbuffer)
        .collect();
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

pub(crate) fn vm_exec_state_from_flatbuffer(
    fb: &fb::VmExecState,
) -> Result<KernelVMExecState, TaskConversionError> {
    let stack: Result<Vec<_>, _> = fb
        .activation_stack
        .iter()
        .map(activation_from_flatbuffer)
        .collect();
    let stack = stack?;

    let start_time = if fb.start_time_nanos > 0 {
        Some(
            SystemTime::UNIX_EPOCH
                + Duration::from_nanos(fb.start_time_nanos),
        )
    } else {
        None
    };

    // Note: We can't fully reconstruct VMExecState without task_id and other fields
    // Those will need to be set by the caller
    Ok(KernelVMExecState {
        task_id: 0, // Will be set by caller
        stack,
        tick_slice: 0,
        max_ticks: 0, // Will be set by caller
        tick_count: fb.tick_count as usize,
        start_time,
        maximum_time: None, // Will be set by caller
        pending_raise_error: None,
        unsync: Default::default(),
    })
}

// ============================================================================
// VmHost Conversion
// ============================================================================

pub(crate) fn vm_host_to_flatbuffer(host: &KernelVmHost) -> Result<fb::VmHost, TaskConversionError> {
    let fb_exec_state = vm_exec_state_to_flatbuffer(&host.vm_exec_state)?;

    Ok(fb::VmHost {
        task_id: host.vm_exec_state.task_id as u64,
        max_stack_depth: host.max_stack_depth as u64,
        max_ticks: host.max_ticks as u64,
        max_time_ms: host.max_time.as_millis() as u64,
        exec_state: Box::new(fb_exec_state),
    })
}

pub(crate) fn vm_host_from_flatbuffer(fb: &fb::VmHost) -> Result<KernelVmHost, TaskConversionError> {
    let mut exec_state = vm_exec_state_from_flatbuffer(&fb.exec_state)?;
    exec_state.task_id = fb.task_id as usize;
    exec_state.max_ticks = fb.max_ticks as usize;
    exec_state.maximum_time = Some(Duration::from_millis(fb.max_time_ms));

    let host = KernelVmHost {
        vm_exec_state: exec_state,
        max_stack_depth: fb.max_stack_depth as usize,
        max_ticks: fb.max_ticks as usize,
        max_time: Duration::from_millis(fb.max_time_ms),
        running: true,
        unsync: Default::default(),
    };

    Ok(host)
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
            let fb_vloc = convert_schema::var_to_db_flatbuffer(vloc)
                .map_err(|e| TaskConversionError::VarError(format!("Error encoding vloc: {e}")))?;
            let fb_args: Result<Vec<_>, _> = args.iter().map(|v| convert_schema::var_to_db_flatbuffer(&v)).collect();
            let fb_args = fb_args
                .map_err(|e| TaskConversionError::VarError(format!("Error encoding args: {e}")))?;

            TaskStartUnion::StartVerb(Box::new(fb::StartVerb {
                player: Box::new(convert_schema::obj_to_flatbuffer_struct(player)),
                vloc: Box::new(fb_vloc),
                verb: Box::new(convert_schema::symbol_to_flatbuffer_struct(verb)),
                args: fb_args,
                argstr: argstr.clone(),
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
        KernelTaskStart::StartEval { player, program } => {
            let fb_program = encode_program_to_fb(program).map_err(|e| {
                TaskConversionError::ProgramError(format!("Error encoding program: {e}"))
            })?;

            TaskStartUnion::StartEval(Box::new(fb::StartEval {
                player: Box::new(convert_schema::obj_to_flatbuffer_struct(player)),
                program: Box::new(fb_program),
            }))
        }
    };

    Ok(fb::TaskStart {
        start: start_union,
    })
}

pub(crate) fn task_start_from_flatbuffer(
    fb: &fb::TaskStart,
) -> Result<KernelTaskStart, TaskConversionError> {
    use fb::TaskStartUnion;

    match &fb.start {
        TaskStartUnion::StartCommandVerb(scv) => Ok(KernelTaskStart::StartCommandVerb {
            handler_object: convert_schema::obj_from_flatbuffer_struct(&scv.handler_object)
                .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding handler_object: {e}")))?,
            player: convert_schema::obj_from_flatbuffer_struct(&scv.player)
                .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding player: {e}")))?,
            command: scv.command.clone(),
        }),
        TaskStartUnion::StartDoCommand(sdc) => Ok(KernelTaskStart::StartDoCommand {
            handler_object: convert_schema::obj_from_flatbuffer_struct(&sdc.handler_object)
                .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding handler_object: {e}")))?,
            player: convert_schema::obj_from_flatbuffer_struct(&sdc.player)
                .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding player: {e}")))?,
            command: sdc.command.clone(),
        }),
        TaskStartUnion::StartVerb(sv) => {
            let vloc = convert_schema::var_from_db_flatbuffer(&sv.vloc)
                .map_err(|e| TaskConversionError::VarError(format!("Error decoding vloc: {e}")))?;
            let args: Result<Vec<_>, _> = sv
                .args
                .iter()
                .map(|v| {
                    convert_schema::var_from_db_flatbuffer(v).map_err(|e| {
                        TaskConversionError::VarError(format!("Error decoding args: {e}"))
                    })
                })
                .collect();
            let args = moor_var::List::mk_list(&args?);

            Ok(KernelTaskStart::StartVerb {
                player: convert_schema::obj_from_flatbuffer_struct(&sv.player)
                    .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding player: {e}")))?,
                vloc,
                verb: convert_schema::symbol_from_flatbuffer_struct(&sv.verb),
                args,
                argstr: sv.argstr.clone(),
            })
        }
        TaskStartUnion::StartFork(sf) => {
            let fork_request = fork_from_flatbuffer(&sf.fork_request)?;
            let suspended = sf.suspended_nanos == u64::MAX;

            Ok(KernelTaskStart::StartFork {
                fork_request: Box::new(fork_request),
                suspended,
            })
        }
        TaskStartUnion::StartEval(se) => {
            let program = decode_stored_program_struct(&se.program).map_err(|e| {
                TaskConversionError::ProgramError(format!("Error decoding program: {e}"))
            })?;

            Ok(KernelTaskStart::StartEval {
                player: convert_schema::obj_from_flatbuffer_struct(&se.player)
                    .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding player: {e}")))?,
                program,
            })
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

pub(crate) fn fork_from_flatbuffer(fb: &fb::Fork) -> Result<Fork, TaskConversionError> {
    let activation = activation_from_flatbuffer(&fb.activation)?;

    let delay = if fb.has_delay {
        Some(Duration::from_nanos(fb.delay_nanos))
    } else {
        None
    };

    let task_id = fb
        .task_id
        .as_ref()
        .map(|n| name_from_stored(n))
        .transpose()?;

    Ok(Fork {
        player: convert_schema::obj_from_flatbuffer_struct(&fb.player)
            .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding player: {e}")))?,
        progr: convert_schema::obj_from_flatbuffer_struct(&fb.progr)
            .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding progr: {e}")))?,
        parent_task_id: fb.parent_task_id as usize,
        delay,
        activation,
        fork_vector_offset: Offset(fb.fork_vector_offset as u16),
        task_id,
    })
}

// ============================================================================
// PendingTimeout Conversion
// ============================================================================

pub(crate) fn pending_timeout_to_flatbuffer(
    timeout: &(KernelAbortLimitReason, moor_var::Var, moor_var::Symbol, usize),
) -> Result<fb::PendingTimeout, TaskConversionError> {
    let (reason, this, verb_name, line_number) = timeout;

    let fb_reason = abort_limit_reason_to_flatbuffer(reason)?;
    let fb_this = convert_schema::var_to_db_flatbuffer(this)
        .map_err(|e| TaskConversionError::VarError(format!("Error encoding this: {e}")))?;
    let fb_verb_name = convert_schema::symbol_to_flatbuffer_struct(verb_name);

    Ok(fb::PendingTimeout {
        reason: Box::new(fb_reason),
        this: Box::new(fb_this),
        verb_name: Box::new(fb_verb_name),
        line_number: *line_number as u64,
    })
}

pub(crate) fn pending_timeout_from_flatbuffer(
    fb: &fb::PendingTimeout,
) -> Result<(KernelAbortLimitReason, moor_var::Var, moor_var::Symbol, usize), TaskConversionError> {
    let reason = abort_limit_reason_from_flatbuffer(&fb.reason)?;
    let this = convert_schema::var_from_db_flatbuffer(&fb.this)
        .map_err(|e| TaskConversionError::VarError(format!("Error decoding this: {e}")))?;
    let verb_name = convert_schema::symbol_from_flatbuffer_struct(&fb.verb_name);
    let line_number = fb.line_number as usize;

    Ok((reason, this, verb_name, line_number))
}

// ============================================================================
// Task Conversion
// ============================================================================

pub(crate) fn task_to_flatbuffer(task: &KernelTask) -> Result<fb::Task, TaskConversionError> {
    let fb_task_start = task_start_to_flatbuffer(&task.task_start)?;
    let fb_vm_host = vm_host_to_flatbuffer(&task.vm_host)?;
    let fb_retry_state = vm_exec_state_to_flatbuffer(&task.retry_state)?;

    let fb_pending_exception = task
        .pending_exception
        .as_ref()
        .map(exception_to_flatbuffer)
        .transpose()?;

    let fb_pending_timeout = task
        .pending_timeout
        .as_ref()
        .map(pending_timeout_to_flatbuffer)
        .transpose()?;

    Ok(fb::Task {
        version: CURRENT_TASK_VERSION,
        task_id: task.task_id as u64,
        player: Box::new(convert_schema::obj_to_flatbuffer_struct(&task.player)),
        task_start: Box::new(fb_task_start),
        vm_host: Box::new(fb_vm_host),
        perms: Box::new(convert_schema::obj_to_flatbuffer_struct(&task.perms)),
        retries: task.retries,
        retry_state: Box::new(fb_retry_state),
        handling_uncaught_error: task.handling_uncaught_error,
        pending_exception: fb_pending_exception.map(Box::new),
        handling_task_timeout: task.handling_task_timeout,
        pending_timeout: fb_pending_timeout.map(Box::new),
    })
}

pub(crate) fn task_from_flatbuffer(fb: &fb::Task) -> Result<KernelTask, TaskConversionError> {
    // Check version
    if fb.version != CURRENT_TASK_VERSION {
        return Err(TaskConversionError::DecodingError(format!(
            "Unsupported task version: {} (expected {})",
            fb.version, CURRENT_TASK_VERSION
        )));
    }

    let task_start = task_start_from_flatbuffer(&fb.task_start)?;
    let vm_host = vm_host_from_flatbuffer(&fb.vm_host)?;
    let mut retry_state = vm_exec_state_from_flatbuffer(&fb.retry_state)?;
    retry_state.task_id = fb.task_id as usize;

    let pending_exception = fb
        .pending_exception
        .as_ref()
        .map(|e| exception_from_flatbuffer(e))
        .transpose()?;

    let pending_timeout = fb
        .pending_timeout
        .as_ref()
        .map(|t| pending_timeout_from_flatbuffer(t))
        .transpose()?;

    Ok(KernelTask {
        task_id: fb.task_id as usize,
        player: convert_schema::obj_from_flatbuffer_struct(&fb.player)
            .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding player: {e}")))?,
        task_start,
        vm_host,
        perms: convert_schema::obj_from_flatbuffer_struct(&fb.perms)
            .map_err(|e| TaskConversionError::DecodingError(format!("Error decoding perms: {e}")))?,
        kill_switch: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        retries: fb.retries,
        retry_state,
        handling_uncaught_error: fb.handling_uncaught_error,
        pending_exception,
        handling_task_timeout: fb.handling_task_timeout,
        pending_timeout,
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

/// Convert a FlatBuffer SuspendedTask back to a kernel SuspendedTask.
/// Note: session and result_sender fields are not serialized and are reconstructed as no-op versions,
/// matching the bincode implementation.
pub fn suspended_task_from_flatbuffer(
    fb: &fb::SuspendedTask,
) -> Result<KernelSuspendedTask, TaskConversionError> {
    use moor_common::tasks::NoopClientSession;
    use std::sync::Arc;

    // Check version
    if fb.version != CURRENT_TASK_VERSION {
        return Err(TaskConversionError::DecodingError(format!(
            "Unsupported suspended task version: {} (expected {})",
            fb.version, CURRENT_TASK_VERSION
        )));
    }

    let wake_condition = wake_condition_from_flatbuffer(&fb.wake_condition)?;
    let task = Box::new(task_from_flatbuffer(&fb.task)?);

    Ok(KernelSuspendedTask {
        wake_condition,
        task,
        session: Arc::new(NoopClientSession::new()),
        result_sender: None,
    })
}
