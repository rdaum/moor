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
    js_frame::{JSContinuation, JSFrame},
    v8_host::{initialize_v8, var_to_v8, v8_to_var, V8_ISOLATE_POOL},
    vm_host::ExecutionResult,
};
use moor_var::{v_none, Obj, Var};
use v8;

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
            ExecutionResult::Complete(result.clone())
        }
    }
}

/// Execute JavaScript for the first time
fn execute_js_initial(js_frame: &mut JSFrame, this: &Var, player: Obj) -> ExecutionResult {
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

        // Install builtin functions
        install_builtins(scope);

        // Compile the JavaScript source
        let source_str = v8::String::new(scope, &js_frame.source).unwrap();
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

        // Execute the script
        match script.run(scope) {
            Some(value) => {
                // Convert V8 value back to MOO Var
                v8_to_var(scope, value)
            }
            None => {
                // Execution failed - check for exception
                return ExecutionResult::PushError(moor_var::Error::new(
                    moor_var::E_INVARG,
                    Some("JavaScript execution failed".to_string()),
                    None,
                ));
            }
        }
    }; // All scopes dropped here, isolate no longer borrowed

    // Release isolate back to pool
    V8_ISOLATE_POOL.with(|pool| pool.borrow_mut().release(isolate));

    // Return the result
    js_frame.set_return_value(result.clone());
    ExecutionResult::Complete(result)
}

/// Set up global variables in the V8 context
fn setup_globals(
    scope: &mut v8::HandleScope,
    this: &Var,
    player: Obj,
    args: &[Var],
) {
    let global = scope.get_current_context().global(scope);

    // Set 'this' global
    let this_key = v8::String::new(scope, "this").unwrap();
    let this_val = var_to_v8(scope, this);
    global.set(scope, this_key.into(), this_val);

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
            ExecutionResult::Complete(value) => {
                assert_eq!(value, v_int(42));
            }
            _ => panic!("Expected Complete, got {:?}", result),
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
            ExecutionResult::Complete(value) => {
                assert_eq!(value, v_str("test_value"));
            }
            _ => panic!("Expected Complete, got {:?}", result),
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
            ExecutionResult::Complete(value) => {
                assert_eq!(value, v_int(42));
            }
            _ => panic!("Expected Complete, got {:?}", result),
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
            ExecutionResult::Complete(value) => {
                // Type code for Int is 0
                assert_eq!(value, v_int(0));
            }
            _ => panic!("Expected Complete, got {:?}", result),
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
            ExecutionResult::Complete(value) => {
                assert_eq!(value, v_str("The answer is 42"));
            }
            _ => panic!("Expected Complete, got {:?}", result),
        }
    }
}

