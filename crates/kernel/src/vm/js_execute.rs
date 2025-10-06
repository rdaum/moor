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

use crate::vm::{
    js_builtins::install_builtins,
    js_frame::{JSContinuation, JSFrame, PendingVerbCall},
    v8_host::{initialize_v8, var_to_v8, v8_to_var, V8_ISOLATE_POOL},
    vm_host::ExecutionResult,
};
use moor_var::{Obj, Var, v_none};
use std::cell::RefCell;
use tracing::info;
use v8;

thread_local! {
    /// Thread-local storage for pending verb calls initiated from JavaScript
    /// The call_verb builtin stores pending calls here, and execute_js_initial checks it
    pub(crate) static PENDING_VERB_CALL: RefCell<Option<PendingVerbCall>> = RefCell::new(None);

    /// Thread-local reference to the current JSFrame being executed
    /// Allows builtins to check continuation state for cached results
    pub(crate) static CURRENT_JS_FRAME: RefCell<Option<*const JSFrame>> = RefCell::new(None);
}

/// Execute a JavaScript frame
/// Acquires an isolate from the thread-local pool, creates a context, and runs the JS code
pub fn execute_js_frame(
    js_frame: &mut JSFrame,
    this: &Var,
    player: Obj,
    _tick_slice: usize,
) -> ExecutionResult {
    // Initialize V8 if needed
    initialize_v8();

    // Check continuation state
    match &js_frame.continuation {
        JSContinuation::Initial => {
            // First time execution - run the JavaScript
            execute_js_initial(js_frame, this, player)
        }
        JSContinuation::AwaitingVerbCall { .. } => {
            // Resuming from a verb call - re-execute with cached result
            // call_verb will see the cached result and return a resolved Promise
            execute_js_resume(js_frame, this, player)
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
fn execute_js_initial(js_frame: &mut JSFrame, this: &Var, player: Obj) -> ExecutionResult {
    info!("execute_js_initial: Starting execution of JS source: {:?}", &js_frame.source);

    // Store reference to current frame for builtins to access
    CURRENT_JS_FRAME.with(|f| *f.borrow_mut() = Some(js_frame as *const JSFrame));

    // Acquire isolate from thread-local pool
    let mut isolate = V8_ISOLATE_POOL.with(|pool| pool.borrow_mut().acquire());

    // Execute within a scope so all borrows end before we release the isolate
    let result = {
        // Create a handle scope for V8 handles
        let scope = &mut v8::HandleScope::new(&mut isolate);

        // Create a new context
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);

        // Set up global variables
        setup_globals(scope, this, player, &js_frame.args);

        // Install builtin functions (including call_verb for cross-language calls)
        install_builtins(scope);

        // Wrap user code in an async function to support return statements and await
        let wrapped_source = format!(
            "(async function() {{\n{}\n}})();",
            js_frame.source
        );
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
        info!("execute_js_initial: Running script");
        let promise_value = match script.run(scope) {
            Some(value) => value,
            None => {
                // Execution failed - check for exception
                return ExecutionResult::PushError(moor_var::Error::new(
                    moor_var::E_INVARG,
                    Some("JavaScript execution failed".to_string()),
                    None,
                ));
            }
        };

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
                    let converted = v8_to_var(scope, result_val);
                    info!("execute_js_initial: Converted result: {:?}", converted);
                    converted
                }
                v8::PromiseState::Rejected => {
                    // Promise rejected - treat as error
                    info!("execute_js_initial: Promise is Rejected");
                    return ExecutionResult::PushError(moor_var::Error::new(
                        moor_var::E_INVARG,
                        Some("JavaScript Promise rejected".to_string()),
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
            v8_to_var(scope, promise_value)
        }
    }; // All scopes dropped here, isolate no longer borrowed

    // Release isolate back to pool
    V8_ISOLATE_POOL.with(|pool| pool.borrow_mut().release(isolate));

    // Check if there's a pending verb call from JavaScript
    let pending_call = PENDING_VERB_CALL.with(|pc| pc.borrow_mut().take());

    // Clear current frame reference
    CURRENT_JS_FRAME.with(|f| *f.borrow_mut() = None);

    if let Some(call_info) = pending_call {
        // Store the pending call in the frame and return PrepareVerbDispatch
        info!("execute_js_initial: Pending verb call detected, suspending for dispatch");
        info!("  this: {:?}", call_info.this);
        info!("  verb_name: {:?}", call_info.verb_name);
        info!("  args: {:?}", call_info.args);
        js_frame.continuation = JSContinuation::AwaitingVerbCall { call_info: call_info.clone() };

        return ExecutionResult::PrepareVerbDispatch {
            this: call_info.this,
            verb_name: call_info.verb_name,
            args: call_info.args,
        };
    }

    // No pending call - execution completed normally
    info!("execute_js_initial: Execution complete with result: {:?}", result);
    js_frame.set_return_value(result.clone());
    ExecutionResult::Return(result)
}

/// Resume JavaScript execution after a verb call completes
/// Re-executes the entire script, but call_verb will see the cached result
fn execute_js_resume(js_frame: &mut JSFrame, this: &Var, player: Obj) -> ExecutionResult {
    // Extract the verb call result and update the continuation
    if let JSContinuation::AwaitingVerbCall { mut call_info } = js_frame.continuation.clone() {
        // Get the verb result from the frame's return value
        if let Some(result) = js_frame.return_value.clone() {
            call_info.result = Some(result);
            js_frame.continuation = JSContinuation::AwaitingVerbCall { call_info };
        }
    }

    // Store reference to current frame for builtins to access
    CURRENT_JS_FRAME.with(|f| *f.borrow_mut() = Some(js_frame as *const JSFrame));

    // Acquire isolate from thread-local pool
    let mut isolate = V8_ISOLATE_POOL.with(|pool| pool.borrow_mut().acquire());

    // Execute within a scope so all borrows end before we release the isolate
    let result = {
        // Create a handle scope for V8 handles
        let scope = &mut v8::HandleScope::new(&mut isolate);

        // Create a new context
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);

        // Set up global variables
        setup_globals(scope, this, player, &js_frame.args);

        // Install builtin functions (including call_verb)
        install_builtins(scope);

        // Wrap user code in an async function to support return statements and await
        let wrapped_source = format!(
            "(async function() {{\n{}\n}})();",
            js_frame.source
        );

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
        let promise_value = match script.run(scope) {
            Some(value) => value,
            None => {
                // Execution failed - check for exception
                return ExecutionResult::PushError(moor_var::Error::new(
                    moor_var::E_INVARG,
                    Some("JavaScript execution failed".to_string()),
                    None,
                ));
            }
        };

        // Process microtasks to allow Promise to resolve
        scope.perform_microtask_checkpoint();

        // Extract the resolved value from the Promise
        if promise_value.is_promise() {
            let promise = v8::Local::<v8::Promise>::try_from(promise_value).unwrap();
            match promise.state() {
                v8::PromiseState::Fulfilled => {
                    // Promise resolved - get the value
                    let result_val = promise.result(scope);
                    v8_to_var(scope, result_val)
                }
                v8::PromiseState::Rejected => {
                    // Promise rejected - treat as error
                    return ExecutionResult::PushError(moor_var::Error::new(
                        moor_var::E_INVARG,
                        Some("JavaScript Promise rejected".to_string()),
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
            v8_to_var(scope, promise_value)
        }
    }; // All scopes dropped here, isolate no longer borrowed

    // Release isolate back to pool
    V8_ISOLATE_POOL.with(|pool| pool.borrow_mut().release(isolate));

    // Check if there's another pending verb call from JavaScript
    let pending_call = PENDING_VERB_CALL.with(|pc| pc.borrow_mut().take());

    // Clear current frame reference
    CURRENT_JS_FRAME.with(|f| *f.borrow_mut() = None);

    if let Some(call_info) = pending_call {
        // Store the pending call in the frame and return PrepareVerbDispatch
        js_frame.continuation = JSContinuation::AwaitingVerbCall { call_info: call_info.clone() };

        return ExecutionResult::PrepareVerbDispatch {
            this: call_info.this,
            verb_name: call_info.verb_name,
            args: call_info.args,
        };
    }

    // No pending call - execution completed normally
    js_frame.set_return_value(result.clone());
    ExecutionResult::Return(result)
}

/// Set up global variables in the V8 context
fn setup_globals(
    scope: &mut v8::HandleScope,
    this: &Var,
    player: Obj,
    args: &[Var],
) {
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

    // TODO: Add builtin functions (typeof, read, etc.)
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
        let source = "42".to_string();
        let args = vec![];
        let mut js_frame = JSFrame::new(source, args);

        // Execute it
        let this = v_int(0);
        let player = moor_var::Obj::mk_id(1);

        let result = execute_js_frame(&mut js_frame, &this, player, 1000);

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

        // JavaScript that accesses the 'this' global
        let source = "this".to_string();
        let args = vec![];
        let mut js_frame = JSFrame::new(source, args);

        let this = v_str("test_value");
        let player = moor_var::Obj::mk_id(1);

        let result = execute_js_frame(&mut js_frame, &this, player, 1000);

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
        let source = "args[0] + args[1]".to_string();
        let args = vec![v_int(10), v_int(32)];
        let mut js_frame = JSFrame::new(source, args);

        let this = v_int(0);
        let player = moor_var::Obj::mk_id(1);

        let result = execute_js_frame(&mut js_frame, &this, player, 1000);

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
        let source = "moo_typeof(42)".to_string();
        let args = vec![];
        let mut js_frame = JSFrame::new(source, args);

        let this = v_int(0);
        let player = moor_var::Obj::mk_id(1);

        let result = execute_js_frame(&mut js_frame, &this, player, 1000);

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
        let source = "tostr('The answer is ', 42)".to_string();
        let args = vec![];
        let mut js_frame = JSFrame::new(source, args);

        let this = v_int(0);
        let player = moor_var::Obj::mk_id(1);

        let result = execute_js_frame(&mut js_frame, &this, player, 1000);

        match result {
            ExecutionResult::Return(value) => {
                assert_eq!(value, v_str("The answer is 42"));
            }
            _ => panic!("Expected Return, got {:?}", result),
        }
    }
}

