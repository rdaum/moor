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

//! JavaScript execution engine for MOO verbs.
//! Handles V8 context creation, global setup, and execution.

use crate::vm::js::js_builtins::install_builtins_on_template;
use crate::vm::js::js_frame::{JSContinuation, JSFrame, PendingDispatch};
use crate::vm::js::js_watchdog::{register_execution, is_watchdog_exception, WatchdogGuard};
use crate::vm::js::v8_host::{acquire_isolate, initialize_v8, release_isolate, v8_to_var, var_to_v8};
use crate::vm::vm_host::ExecutionResult;
use moor_common::tasks::TaskId;
use moor_var::{Obj, Var, v_none};
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tracing::info;
use v8;

thread_local! {
    /// Thread-local storage for pending dispatch operations (verb or builtin calls) from JavaScript
    /// The call_verb and call_builtin functions store pending calls here
    pub(crate) static PENDING_DISPATCH: RefCell<Option<PendingDispatch>> = const { RefCell::new(None) };

    /// Thread-local reference to the current JSFrame being executed
    /// Allows builtins to check continuation state for cached results
    pub(crate) static CURRENT_JS_FRAME: RefCell<Option<*const JSFrame>> = const { RefCell::new(None) };

    /// Thread-local storage for the permissions of the current JavaScript execution
    /// Used by property access and other operations that need permissions
    pub(crate) static JS_PERMISSIONS: RefCell<Option<Obj>> = const { RefCell::new(None) };
}

/// Validate JavaScript source code by attempting to compile it with V8.
/// Returns Ok(()) if valid, or Err with a list of error messages.
pub fn validate_javascript(source: &str) -> Result<(), Vec<String>> {
    initialize_v8();

    // Acquire the global isolate
    let mut isolate = acquire_isolate();

    let mut errors = Vec::new();

    {
        let scope = &mut v8::HandleScope::new(&mut isolate);
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);
        let tc_scope = &mut v8::TryCatch::new(scope);

        // Wrap in async function like we do during execution
        let wrapped_source = format!("(async function() {{\n{}\n}})();", source);
        let source_str = v8::String::new(tc_scope, &wrapped_source).unwrap();

        // Try to compile
        if v8::Script::compile(tc_scope, source_str, None).is_none() {
            // Compilation failed - extract error message
            if let Some(exception) = tc_scope.exception() {
                let exception_str = exception.to_string(tc_scope).unwrap();
                let error_msg = exception_str.to_rust_string_lossy(tc_scope);

                // Try to get more detailed error info
                if let Some(message) = tc_scope.message() {
                    let line = message.get_line_number(tc_scope).unwrap_or(0);
                    let source_line = message
                        .get_source_line(tc_scope)
                        .map(|s| s.to_rust_string_lossy(tc_scope))
                        .unwrap_or_default();

                    errors.push(format!("Line {}: {}", line.saturating_sub(1), error_msg));
                    if !source_line.is_empty() {
                        errors.push(format!("  {}", source_line.trim()));
                    }
                } else {
                    errors.push(error_msg);
                }
            } else {
                errors.push("JavaScript compilation failed".to_string());
            }
        }
    }

    // Release isolate back to pool
    release_isolate(isolate);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Execute a JavaScript frame
/// Acquires an isolate from the thread-local pool, creates a context, and runs the JS code
#[allow(clippy::too_many_arguments)]
pub fn execute_js_frame(
    js_frame: &mut JSFrame,
    this: &Var,
    player: Obj,
    permissions: Obj,
    task_id: TaskId,
    ticks_remaining: usize,
    time_remaining: Duration,
    kill_flag: Arc<AtomicBool>,
) -> ExecutionResult {
    // Initialize V8 if needed
    initialize_v8();

    // Check continuation state
    match &js_frame.continuation {
        JSContinuation::Initial => {
            // First time execution - run the JavaScript
            execute_js_initial(js_frame, this, player, permissions, task_id, ticks_remaining, time_remaining, kill_flag)
        }
        JSContinuation::AwaitingVerbCall { .. } => {
            // Resuming from a verb call - re-execute with cached result
            // call_verb will see the cached result and return a resolved Promise
            execute_js_resume(js_frame, this, player, permissions, task_id, ticks_remaining, time_remaining, kill_flag)
        }
        JSContinuation::AwaitingBuiltinCall { .. } => {
            // Resuming from a builtin call - re-execute with cached result
            // call_builtin will see the cached result and return a resolved Promise
            execute_js_resume(js_frame, this, player, permissions, task_id, ticks_remaining, time_remaining, kill_flag)
        }
        JSContinuation::AwaitingPromise { .. } => {
            // Resuming from a promise - not yet implemented
            ExecutionResult::PushError(moor_var::Error::new(
                moor_var::E_VARNF,
                Some("JavaScript resume not yet implemented".to_string()),
                None,
            ))
        }
        JSContinuation::Complete { result } => {
            // Already complete - return the result
            ExecutionResult::Return(result.clone())
        }
    }
}

/// Execute JavaScript for the first time
#[allow(clippy::too_many_arguments)]
fn execute_js_initial(
    js_frame: &mut JSFrame,
    this: &Var,
    player: Obj,
    permissions: Obj,
    task_id: TaskId,
    ticks_remaining: usize,
    time_remaining: Duration,
    kill_flag: Arc<AtomicBool>,
) -> ExecutionResult {
    info!(
        "execute_js_initial: Starting execution of JS source: {:?}",
        &js_frame.source
    );

    // Store reference to current frame and permissions for builtins to access
    CURRENT_JS_FRAME.with(|f| *f.borrow_mut() = Some(js_frame as *const JSFrame));
    JS_PERMISSIONS.with(|p| *p.borrow_mut() = Some(permissions));

    // Acquire the global isolate
    let mut isolate = acquire_isolate();

    // Get thread-safe handle for watchdog
    let isolate_handle = isolate.thread_safe_handle();

    // Register with watchdog for time/tick limit enforcement
    register_execution(task_id, isolate_handle, ticks_remaining, time_remaining, kill_flag);

    // Guard ensures unregister happens on any return path
    let _guard = WatchdogGuard::new(task_id);

    // Execute within a scope so all borrows end before we release the isolate
    let result = {
        // Create a handle scope for V8 handles
        let scope = &mut v8::HandleScope::new(&mut isolate);

        // Create a new context with builtins pre-installed via template
        let context = create_context_with_builtins(scope);
        let scope = &mut v8::ContextScope::new(scope, context);

        // Set up global variables (self, player, args)
        setup_globals(scope, this, player, &js_frame.args);

        // Install JavaScript helpers (like Proxy-based obj() wrapper for method syntax)
        install_js_helpers(scope);

        // Wrap user code in an async function to support return statements and await
        let wrapped_source = format!("(async function() {{\n{}\n}})();", js_frame.source);
        info!("execute_js_initial: Wrapped source: {:?}", &wrapped_source);

        // Compile the JavaScript source
        let source_str = v8::String::new(scope, &wrapped_source).unwrap();
        let script = match v8::Script::compile(scope, source_str, None) {
            Some(s) => {
                info!("execute_js_initial: Compilation succeeded");
                s
            }
            None => {
                // Compilation failed
                info!("execute_js_initial: Compilation failed");
                return ExecutionResult::PushError(moor_var::Error::new(
                    moor_var::E_INVARG,
                    Some("JavaScript compilation failed".to_string()),
                    None,
                ));
            }
        };

        // Execute the script (returns a Promise from the async function)
        // Wrap in TryCatch to handle watchdog interrupts and other exceptions
        info!("execute_js_initial: Running script");
        let tc_scope = &mut v8::TryCatch::new(scope);
        let promise_value = match script.run(tc_scope) {
            Some(value) => value,
            None => {
                // Execution failed - could be watchdog termination or an actual error
                // If there's no exception, it's likely watchdog termination via terminate_execution()
                // Let the outer exec_interpreter check limits and report the proper abort reason
                let Some(exception) = tc_scope.exception() else {
                    // No exception - likely watchdog termination
                    return ExecutionResult::PushError(moor_var::Error::new(
                        moor_var::E_QUOTA,
                        Some("JavaScript execution exceeded time or tick limit".to_string()),
                        None,
                    ));
                };

                // There is an exception - check if it's watchdog-related or a real error
                let Some(exception_str) = exception.to_string(tc_scope) else {
                    // Can't convert to string, but there was an exception - regular failure
                    return ExecutionResult::PushError(moor_var::Error::new(
                        moor_var::E_INVARG,
                        Some("JavaScript execution failed".to_string()),
                        None,
                    ));
                };

                let exception_msg = exception_str.to_rust_string_lossy(tc_scope);

                // Check if it's a watchdog termination (terminate_execution creates "null" exception)
                // or one of our custom watchdog exceptions
                if exception_msg == "null" || is_watchdog_exception(&exception_msg).is_some() {
                    // Watchdog interrupted - a limit was exceeded
                    // Return an appropriate error message
                    return ExecutionResult::PushError(moor_var::Error::new(
                        moor_var::E_QUOTA,
                        Some("JavaScript execution exceeded time or tick limit".to_string()),
                        None,
                    ));
                }

                // Real JavaScript exception
                return ExecutionResult::PushError(moor_var::Error::new(
                    moor_var::E_INVARG,
                    Some(format!("JavaScript error: {}", exception_msg)),
                    None,
                ));
            }
        };

        let scope = tc_scope;

        // Process microtasks to allow Promise to resolve
        info!("execute_js_initial: Processing microtasks");
        scope.perform_microtask_checkpoint();

        // Extract the resolved value from the Promise
        if promise_value.is_promise() {
            info!("execute_js_initial: Result is a Promise");
            let promise = v8::Local::<v8::Promise>::try_from(promise_value).unwrap();
            match promise.state() {
                v8::PromiseState::Fulfilled => {
                    // Promise resolved - get the value
                    info!("execute_js_initial: Promise is Fulfilled");
                    let result_val = promise.result(scope);
                    match v8_to_var(scope, result_val) {
                        Ok(converted) => {
                            info!("execute_js_initial: Converted result: {:?}", converted);
                            converted
                        }
                        Err(err) => {
                            // Conversion error (e.g., Infinity/NaN) - treat as exception
                            return ExecutionResult::PushError(err);
                        }
                    }
                }
                v8::PromiseState::Rejected => {
                    // Promise rejected - extract the MOO error if present
                    info!("execute_js_initial: Promise is Rejected");
                    let rejection_value = promise.result(scope);

                    // Check if this is a MOO error (has moo_error_var property)
                    if rejection_value.is_object() {
                        let obj = rejection_value.to_object(scope).unwrap();
                        let err_var_key = v8::String::new(scope, "moo_error_var").unwrap();

                        if let Some(err_var_val) = obj.get(scope, err_var_key.into()) {
                            // This is a MOO error - extract it
                            match v8_to_var(scope, err_var_val) {
                                Ok(err_var) => {
                                    if let moor_var::Variant::Err(moo_err) = err_var.variant() {
                                        return ExecutionResult::PushError(moo_err.as_ref().clone());
                                    }
                                }
                                Err(err) => {
                                    return ExecutionResult::PushError(err);
                                }
                            }
                        }
                    }

                    // Not a MOO error - extract generic error message
                    let error_msg = if rejection_value.is_string() {
                        rejection_value
                            .to_string(scope)
                            .map(|s| s.to_rust_string_lossy(scope))
                            .unwrap_or_else(|| "JavaScript Promise rejected".to_string())
                    } else if rejection_value.is_object() {
                        let obj = rejection_value.to_object(scope).unwrap();
                        let message_key = v8::String::new(scope, "message").unwrap();
                        if let Some(msg_val) = obj.get(scope, message_key.into()) {
                            msg_val
                                .to_string(scope)
                                .map(|s| s.to_rust_string_lossy(scope))
                                .unwrap_or_else(|| "JavaScript Promise rejected".to_string())
                        } else {
                            rejection_value
                                .to_string(scope)
                                .map(|s| s.to_rust_string_lossy(scope))
                                .unwrap_or_else(|| "JavaScript Promise rejected".to_string())
                        }
                    } else {
                        rejection_value
                            .to_string(scope)
                            .map(|s| s.to_rust_string_lossy(scope))
                            .unwrap_or_else(|| "JavaScript Promise rejected".to_string())
                    };

                    return ExecutionResult::PushError(moor_var::Error::new(
                        moor_var::E_INVARG,
                        Some(error_msg),
                        None,
                    ));
                }
                v8::PromiseState::Pending => {
                    // Still pending - this means call_verb was called
                    // The PENDING_VERB_CALL check below will handle this
                    info!("execute_js_initial: Promise is Pending (call_verb was called)");
                    v_none()
                }
            }
        } else {
            // Not a Promise - just convert the value
            info!("execute_js_initial: Result is not a Promise");
            match v8_to_var(scope, promise_value) {
                Ok(converted) => converted,
                Err(err) => {
                    // Conversion error (e.g., Infinity/NaN) - treat as exception
                    return ExecutionResult::PushError(err);
                }
            }
        }
    }; // All scopes dropped here, isolate no longer borrowed

    // Release isolate back to pool
    release_isolate(isolate);

    // Clear current frame reference and permissions
    CURRENT_JS_FRAME.with(|f| *f.borrow_mut() = None);
    JS_PERMISSIONS.with(|p| *p.borrow_mut() = None);

    // Check if there's a pending dispatch operation from JavaScript
    let pending_dispatch = PENDING_DISPATCH.with(|pd| pd.borrow_mut().take());

    if let Some(dispatch) = pending_dispatch {
        match dispatch {
            PendingDispatch::VerbCall(call_info) => {
                // Store the pending verb call in the frame and return PrepareVerbDispatch
                info!("execute_js_initial: Pending verb call detected, suspending for dispatch");
                info!("  this: {:?}", call_info.this);
                info!("  verb_name: {:?}", call_info.verb_name);
                info!("  args: {:?}", call_info.args);
                js_frame.continuation = JSContinuation::AwaitingVerbCall {
                    call_info: call_info.clone(),
                };

                return ExecutionResult::PrepareVerbDispatch {
                    this: call_info.this,
                    verb_name: call_info.verb_name,
                    args: call_info.args,
                };
            }
            PendingDispatch::BuiltinCall(call_info) => {
                // Store the pending builtin call in the frame and return DispatchBuiltin
                info!("execute_js_initial: Pending builtin call detected, suspending for dispatch");
                info!("  builtin_id: {:?}", call_info.builtin_id);
                info!("  args: {:?}", call_info.args);
                js_frame.continuation = JSContinuation::AwaitingBuiltinCall {
                    call_info: call_info.clone(),
                };

                return ExecutionResult::DispatchBuiltin {
                    builtin: call_info.builtin_id,
                    arguments: call_info.args,
                };
            }
        }
    }

    // No pending call - execution completed normally
    info!(
        "execute_js_initial: Execution complete with result: {:?}",
        result
    );
    js_frame.set_return_value(result.clone());
    ExecutionResult::Return(result)
}

/// Resume JavaScript execution after a verb call completes
/// Re-executes the entire script, but call_verb will see the cached result
#[allow(clippy::too_many_arguments)]
fn execute_js_resume(
    js_frame: &mut JSFrame,
    this: &Var,
    player: Obj,
    permissions: Obj,
    task_id: TaskId,
    ticks_remaining: usize,
    time_remaining: Duration,
    kill_flag: Arc<AtomicBool>,
) -> ExecutionResult {
    // Extract the call result and update the continuation
    match js_frame.continuation.clone() {
        JSContinuation::AwaitingVerbCall { mut call_info } => {
            // Get the verb result from the frame's return value
            if let Some(result) = js_frame.return_value.clone() {
                call_info.result = Some(result);
                js_frame.continuation = JSContinuation::AwaitingVerbCall { call_info };
            }
        }
        JSContinuation::AwaitingBuiltinCall { mut call_info } => {
            // Get the builtin result from the frame's return value
            if let Some(result) = js_frame.return_value.clone() {
                call_info.result = Some(result);
                js_frame.continuation = JSContinuation::AwaitingBuiltinCall { call_info };
            }
        }
        _ => {}
    }

    // Store reference to current frame and permissions for builtins to access
    CURRENT_JS_FRAME.with(|f| *f.borrow_mut() = Some(js_frame as *const JSFrame));
    JS_PERMISSIONS.with(|p| *p.borrow_mut() = Some(permissions));

    // Acquire the global isolate
    let mut isolate = acquire_isolate();

    // Get thread-safe handle for watchdog
    let isolate_handle = isolate.thread_safe_handle();

    // Register with watchdog for time/tick limit enforcement
    register_execution(task_id, isolate_handle, ticks_remaining, time_remaining, kill_flag);

    // Guard ensures unregister happens on any return path
    let _guard = WatchdogGuard::new(task_id);

    // Execute within a scope so all borrows end before we release the isolate
    let result = {
        // Create a handle scope for V8 handles
        let scope = &mut v8::HandleScope::new(&mut isolate);

        // Create a new context with builtins pre-installed via template
        let context = create_context_with_builtins(scope);
        let scope = &mut v8::ContextScope::new(scope, context);

        // Set up global variables (self, player, args)
        setup_globals(scope, this, player, &js_frame.args);

        // Install JavaScript helpers (like Proxy-based obj() wrapper for method syntax)
        install_js_helpers(scope);

        // Wrap user code in an async function to support return statements and await
        let wrapped_source = format!("(async function() {{\n{}\n}})();", js_frame.source);

        // Compile the JavaScript source
        let source_str = v8::String::new(scope, &wrapped_source).unwrap();
        let script = match v8::Script::compile(scope, source_str, None) {
            Some(s) => s,
            None => {
                // Compilation failed
                return ExecutionResult::PushError(moor_var::Error::new(
                    moor_var::E_INVARG,
                    Some("JavaScript compilation failed".to_string()),
                    None,
                ));
            }
        };

        // Execute the script (returns a Promise from the async function)
        // Wrap in TryCatch to handle watchdog interrupts and other exceptions
        let tc_scope = &mut v8::TryCatch::new(scope);
        let promise_value = match script.run(tc_scope) {
            Some(value) => value,
            None => {
                // Execution failed - could be watchdog termination or an actual error
                // If there's no exception, it's likely watchdog termination via terminate_execution()
                // Let the outer exec_interpreter check limits and report the proper abort reason
                let Some(exception) = tc_scope.exception() else {
                    // No exception - likely watchdog termination
                    return ExecutionResult::PushError(moor_var::Error::new(
                        moor_var::E_QUOTA,
                        Some("JavaScript execution exceeded time or tick limit".to_string()),
                        None,
                    ));
                };

                // There is an exception - check if it's watchdog-related or a real error
                let Some(exception_str) = exception.to_string(tc_scope) else {
                    // Can't convert to string, but there was an exception - regular failure
                    return ExecutionResult::PushError(moor_var::Error::new(
                        moor_var::E_INVARG,
                        Some("JavaScript execution failed".to_string()),
                        None,
                    ));
                };

                let exception_msg = exception_str.to_rust_string_lossy(tc_scope);

                // Check if it's a watchdog termination (terminate_execution creates "null" exception)
                // or one of our custom watchdog exceptions
                if exception_msg == "null" || is_watchdog_exception(&exception_msg).is_some() {
                    // Watchdog interrupted - a limit was exceeded
                    // Return an appropriate error message
                    return ExecutionResult::PushError(moor_var::Error::new(
                        moor_var::E_QUOTA,
                        Some("JavaScript execution exceeded time or tick limit".to_string()),
                        None,
                    ));
                }

                // Real JavaScript exception
                return ExecutionResult::PushError(moor_var::Error::new(
                    moor_var::E_INVARG,
                    Some(format!("JavaScript error: {}", exception_msg)),
                    None,
                ));
            }
        };

        let scope = tc_scope;

        // Process microtasks to allow Promise to resolve
        scope.perform_microtask_checkpoint();

        // Extract the resolved value from the Promise
        if promise_value.is_promise() {
            let promise = v8::Local::<v8::Promise>::try_from(promise_value).unwrap();
            match promise.state() {
                v8::PromiseState::Fulfilled => {
                    // Promise resolved - get the value
                    let result_val = promise.result(scope);
                    match v8_to_var(scope, result_val) {
                        Ok(converted) => converted,
                        Err(err) => {
                            // Conversion error (e.g., Infinity/NaN) - treat as exception
                            return ExecutionResult::PushError(err);
                        }
                    }
                }
                v8::PromiseState::Rejected => {
                    // Promise rejected - extract the MOO error if present
                    let rejection_value = promise.result(scope);

                    // Check if this is a MOO error (has moo_error_var property)
                    if rejection_value.is_object() {
                        let obj = rejection_value.to_object(scope).unwrap();
                        let err_var_key = v8::String::new(scope, "moo_error_var").unwrap();

                        if let Some(err_var_val) = obj.get(scope, err_var_key.into()) {
                            // This is a MOO error - extract it
                            match v8_to_var(scope, err_var_val) {
                                Ok(err_var) => {
                                    if let moor_var::Variant::Err(moo_err) = err_var.variant() {
                                        return ExecutionResult::PushError(moo_err.as_ref().clone());
                                    }
                                }
                                Err(err) => {
                                    return ExecutionResult::PushError(err);
                                }
                            }
                        }
                    }

                    // Not a MOO error - extract generic error message
                    let error_msg = if rejection_value.is_string() {
                        rejection_value
                            .to_string(scope)
                            .map(|s| s.to_rust_string_lossy(scope))
                            .unwrap_or_else(|| "JavaScript Promise rejected".to_string())
                    } else if rejection_value.is_object() {
                        let obj = rejection_value.to_object(scope).unwrap();
                        let message_key = v8::String::new(scope, "message").unwrap();
                        if let Some(msg_val) = obj.get(scope, message_key.into()) {
                            msg_val
                                .to_string(scope)
                                .map(|s| s.to_rust_string_lossy(scope))
                                .unwrap_or_else(|| "JavaScript Promise rejected".to_string())
                        } else {
                            rejection_value
                                .to_string(scope)
                                .map(|s| s.to_rust_string_lossy(scope))
                                .unwrap_or_else(|| "JavaScript Promise rejected".to_string())
                        }
                    } else {
                        rejection_value
                            .to_string(scope)
                            .map(|s| s.to_rust_string_lossy(scope))
                            .unwrap_or_else(|| "JavaScript Promise rejected".to_string())
                    };

                    return ExecutionResult::PushError(moor_var::Error::new(
                        moor_var::E_INVARG,
                        Some(error_msg),
                        None,
                    ));
                }
                v8::PromiseState::Pending => {
                    // Still pending - this means another call_verb was called
                    // The PENDING_VERB_CALL check below will handle this
                    v_none()
                }
            }
        } else {
            // Not a Promise - just convert the value
            match v8_to_var(scope, promise_value) {
                Ok(converted) => converted,
                Err(err) => {
                    // Conversion error (e.g., Infinity/NaN) - treat as exception
                    return ExecutionResult::PushError(err);
                }
            }
        }
    }; // All scopes dropped here, isolate no longer borrowed

    // Release isolate back to pool
    release_isolate(isolate);

    // Check if there's another pending dispatch operation from JavaScript
    let pending_dispatch = PENDING_DISPATCH.with(|pd| pd.borrow_mut().take());

    // Clear current frame reference and permissions
    CURRENT_JS_FRAME.with(|f| *f.borrow_mut() = None);
    JS_PERMISSIONS.with(|p| *p.borrow_mut() = None);

    if let Some(dispatch) = pending_dispatch {
        match dispatch {
            PendingDispatch::VerbCall(call_info) => {
                // Store the pending verb call in the frame and return PrepareVerbDispatch
                js_frame.continuation = JSContinuation::AwaitingVerbCall {
                    call_info: call_info.clone(),
                };

                return ExecutionResult::PrepareVerbDispatch {
                    this: call_info.this,
                    verb_name: call_info.verb_name,
                    args: call_info.args,
                };
            }
            PendingDispatch::BuiltinCall(call_info) => {
                // Store the pending builtin call in the frame and return DispatchBuiltin
                js_frame.continuation = JSContinuation::AwaitingBuiltinCall {
                    call_info: call_info.clone(),
                };

                return ExecutionResult::DispatchBuiltin {
                    builtin: call_info.builtin_id,
                    arguments: call_info.args,
                };
            }
        }
    }

    // No pending call - execution completed normally
    js_frame.set_return_value(result.clone());
    ExecutionResult::Return(result)
}

/// Create a V8 context with builtins pre-installed via object template
/// This is more efficient than installing builtins after context creation
fn create_context_with_builtins<'s>(
    scope: &mut v8::HandleScope<'s, ()>,
) -> v8::Local<'s, v8::Context> {
    // Create global object template with builtins
    let global_template = v8::ObjectTemplate::new(scope);
    install_builtins_on_template(scope, global_template);

    // Create context with this global template
    let context_options = v8::ContextOptions {
        global_template: Some(global_template),
        ..Default::default()
    };
    v8::Context::new(scope, context_options)
}

/// Install JavaScript helper functions (executed as JS code in the context)
/// This includes the Proxy-based obj() wrapper for method syntax
fn install_js_helpers(scope: &mut v8::HandleScope) {
    // JavaScript code that wraps the native obj() to support method syntax
    // This allows: obj(1).verb_name() instead of call_verb(obj(1), 'verb_name')
    let helper_code = r#"
        (function() {
            // Save reference to native obj() function
            const native_obj = obj;

            // Replace with Proxy-wrapped version for method syntax
            obj = function(id) {
                const moo_obj = native_obj(id);

                return new Proxy(moo_obj, {
                    get(target, prop) {
                        // Return actual properties if they exist
                        if (prop in target) {
                            return target[prop];
                        }

                        // Don't intercept special JavaScript properties
                        // These cause issues with async/await and other JS internals
                        if (prop === 'then' || prop === 'catch' || prop === 'finally' ||
                            prop === 'constructor' || prop === 'toString' || prop === 'valueOf' ||
                            typeof prop === 'symbol') {
                            return undefined;
                        }

                        // Otherwise, return a function that calls the verb
                        return function(...args) {
                            return call_verb(target, String(prop), ...args);
                        };
                    }
                });
            };

            // Create $ helper for #0 property/verb access
            // Usage: $("room") reads #0.room property and wraps objects for chaining
            globalThis.$ = function(prop_name) {
                // Called as a function - read property from #0
                const value = get_prop(obj(0), prop_name);

                // If it's a MOO object, wrap it with obj() for Proxy support
                if (value && typeof value === 'object' && '__moo_obj' in value) {
                    return obj(value.__moo_obj);
                }

                return value;
            };

            // Add .code getter to moo_error objects for cleaner error code access
            // This allows: err.code instead of err.__moo_error
            const original_moo_error = moo_error;
            moo_error = function(code, msg) {
                const err = original_moo_error(code, msg);
                Object.defineProperty(err, 'code', {
                    get() { return this.__moo_error; },
                    enumerable: true
                });
                return err;
            };
        })();
    "#;

    let code = v8::String::new(scope, helper_code).unwrap();
    let script = v8::Script::compile(scope, code, None).unwrap();
    script.run(scope);
}

/// Set up global variables in the V8 context
fn setup_globals(scope: &mut v8::HandleScope, this: &Var, player: Obj, args: &[Var]) {
    let global = scope.get_current_context().global(scope);

    // Set 'self' global (can't use 'this' - it's a JavaScript keyword that refers to function context)
    let self_key = v8::String::new(scope, "self").unwrap();
    let self_val = var_to_v8(scope, this);
    global.set(scope, self_key.into(), self_val);

    // Set 'player' global
    let player_key = v8::String::new(scope, "player").unwrap();
    let player_val = var_to_v8(scope, &moor_var::v_obj(player));
    global.set(scope, player_key.into(), player_val);

    // Set 'args' global as an array
    let args_key = v8::String::new(scope, "args").unwrap();
    let args_array = v8::Array::new(scope, args.len() as i32);
    for (i, arg) in args.iter().enumerate() {
        let arg_val = var_to_v8(scope, arg);
        args_array.set_index(scope, i as u32, arg_val);
    }
    global.set(scope, args_key.into(), args_array.into());

    // Set error code constants as globals
    // These match the numeric values used in v8_to_var for error conversion
    let error_constants = [
        ("E_NONE", 0),
        ("E_TYPE", 1),
        ("E_DIV", 2),
        ("E_PERM", 3),
        ("E_PROPNF", 4),
        ("E_VERBNF", 5),
        ("E_VARNF", 6),
        ("E_INVIND", 7),
        ("E_RECMOVE", 8),
        ("E_MAXREC", 9),
        ("E_RANGE", 10),
        ("E_ARGS", 11),
        ("E_NACC", 12),
        ("E_INVARG", 13),
        ("E_QUOTA", 14),
        ("E_FLOAT", 15),
    ];

    for (name, code) in error_constants {
        let key = v8::String::new(scope, name).unwrap();
        let val = v8::Number::new(scope, code as f64);
        global.set(scope, key.into(), val.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{v_int, v_str};

    #[test]
    fn test_simple_js_execution() {
        // Initialize V8
        initialize_v8();

        // Create a simple JavaScript frame that returns a number
        let source = "return 42;".to_string();
        let args = vec![];
        let mut js_frame = JSFrame::new(source, args);

        // Execute it
        let this = v_int(0);
        let player = moor_var::Obj::mk_id(1);
        let permissions = moor_var::Obj::mk_id(1);

        let result = execute_js_frame(
            &mut js_frame,
            &this,
            player,
            permissions,
            0,
            90000,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
        );

        // Check result
        match result {
            ExecutionResult::Return(value) => {
                assert_eq!(value, v_int(42));
            }
            _ => panic!("Expected Return, got {:?}", result),
        }
    }

    #[test]
    fn test_js_with_globals() {
        initialize_v8();

        // JavaScript that accesses the 'self' global (can't use 'this' - it's a JavaScript keyword)
        let source = "return self;".to_string();
        let args = vec![];
        let mut js_frame = JSFrame::new(source, args);

        let this = v_str("test_value");
        let player = moor_var::Obj::mk_id(1);
        let permissions = moor_var::Obj::mk_id(1);

        let result = execute_js_frame(
            &mut js_frame,
            &this,
            player,
            permissions,
            0,
            90000,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
        );

        match result {
            ExecutionResult::Return(value) => {
                assert_eq!(value, v_str("test_value"));
            }
            _ => panic!("Expected Return, got {:?}", result),
        }
    }

    #[test]
    fn test_js_with_args() {
        initialize_v8();

        // JavaScript that accesses args
        let source = "return args[0] + args[1];".to_string();
        let args = vec![v_int(10), v_int(32)];
        let mut js_frame = JSFrame::new(source, args);

        let this = v_int(0);
        let player = moor_var::Obj::mk_id(1);
        let permissions = moor_var::Obj::mk_id(1);

        let result = execute_js_frame(
            &mut js_frame,
            &this,
            player,
            permissions,
            0,
            90000,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
        );

        match result {
            ExecutionResult::Return(value) => {
                assert_eq!(value, v_int(42));
            }
            _ => panic!("Expected Return, got {:?}", result),
        }
    }

    #[test]
    fn test_js_with_moo_typeof() {
        initialize_v8();

        // JavaScript that calls MOO builtin
        let source = "return moo_typeof(42);".to_string();
        let args = vec![];
        let mut js_frame = JSFrame::new(source, args);

        let this = v_int(0);
        let player = moor_var::Obj::mk_id(1);
        let permissions = moor_var::Obj::mk_id(1);

        let result = execute_js_frame(
            &mut js_frame,
            &this,
            player,
            permissions,
            0,
            90000,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
        );

        match result {
            ExecutionResult::Return(value) => {
                // Type code for Int is 0
                assert_eq!(value, v_int(0));
            }
            _ => panic!("Expected Return, got {:?}", result),
        }
    }

    #[test]
    fn test_js_with_tostr() {
        initialize_v8();

        // JavaScript that calls tostr
        let source = "return tostr('The answer is ', 42);".to_string();
        let args = vec![];
        let mut js_frame = JSFrame::new(source, args);

        let this = v_int(0);
        let player = moor_var::Obj::mk_id(1);
        let permissions = moor_var::Obj::mk_id(1);

        let result = execute_js_frame(
            &mut js_frame,
            &this,
            player,
            permissions,
            0,
            90000,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
        );

        match result {
            ExecutionResult::Return(value) => {
                assert_eq!(value, v_str("The answer is 42"));
            }
            _ => panic!("Expected Return, got {:?}", result),
        }
    }
}
