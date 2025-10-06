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

//! JavaScript builtin function wrappers.
//! Exposes MOO builtin functions to JavaScript code.

use crate::task_context::with_current_transaction;
use crate::vm::js::js_execute::{CURRENT_JS_FRAME, JS_PERMISSIONS, PENDING_DISPATCH};
use crate::vm::js::js_frame::{JSContinuation, PendingDispatch, PendingVerbCall};
use crate::vm::js::v8_host::{v8_to_var, var_to_v8};
use moor_var::{Symbol, Var, v_int, v_list};
use v8;

/// Helper to convert V8 value to Var, throwing a JavaScript exception on error
fn v8_to_var_or_throw<'s>(
    scope: &mut v8::HandleScope<'s>,
    value: v8::Local<'s, v8::Value>,
    context: &str,
) -> Option<Var> {
    match v8_to_var(scope, value) {
        Ok(var) => Some(var),
        Err(err) => {
            let msg_text = format!(
                "{}: {}",
                context,
                err.msg.as_ref().map(|s| s.as_str()).unwrap_or("unknown error")
            );
            let msg = v8::String::new(scope, &msg_text).unwrap();
            let exception = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exception);
            None
        }
    }
}

/// Install builtin functions onto an object template
/// This is called during context creation for efficiency
pub fn install_builtins_on_template<'s>(
    scope: &mut v8::HandleScope<'s, ()>,
    template: v8::Local<'s, v8::ObjectTemplate>,
) {
    // Install typeof builtin
    let typeof_fn = v8::FunctionTemplate::new(scope, typeof_callback);
    let typeof_key = v8::String::new(scope, "moo_typeof").unwrap();
    template.set(typeof_key.into(), typeof_fn.into());

    // Install tostr builtin
    let tostr_fn = v8::FunctionTemplate::new(scope, tostr_callback);
    let tostr_key = v8::String::new(scope, "tostr").unwrap();
    template.set(tostr_key.into(), tostr_fn.into());

    // Install call_verb builtin
    let call_verb_fn = v8::FunctionTemplate::new(scope, call_verb_callback);
    let call_verb_key = v8::String::new(scope, "call_verb").unwrap();
    template.set(call_verb_key.into(), call_verb_fn.into());

    // Install obj() helper
    let obj_fn = v8::FunctionTemplate::new(scope, obj_callback);
    let obj_key = v8::String::new(scope, "obj").unwrap();
    template.set(obj_key.into(), obj_fn.into());

    // Install get_prop() for property access
    let get_prop_fn = v8::FunctionTemplate::new(scope, get_prop_callback);
    let get_prop_key = v8::String::new(scope, "get_prop").unwrap();
    template.set(get_prop_key.into(), get_prop_fn.into());

    // Install call_builtin() for calling MOO builtins
    let call_builtin_fn = v8::FunctionTemplate::new(scope, call_builtin_callback);
    let call_builtin_key = v8::String::new(scope, "call_builtin").unwrap();
    template.set(call_builtin_key.into(), call_builtin_fn.into());

    // Install moo_error() constructor for creating MOO error objects
    let moo_error_fn = v8::FunctionTemplate::new(scope, moo_error_callback);

    // Add prototype methods
    let proto_template = moo_error_fn.prototype_template(scope);

    // Add .is(code) method
    let is_method = v8::FunctionTemplate::new(scope, moo_error_is_callback);
    let is_key = v8::String::new(scope, "is").unwrap();
    proto_template.set(is_key.into(), is_method.into());

    // Add .code getter property (using set instead of set_accessor for simplicity)
    // We'll just expose __moo_error as .code via JavaScript helper code

    let moo_error_key = v8::String::new(scope, "moo_error").unwrap();
    template.set(moo_error_key.into(), moo_error_fn.into());
}

/// Helper function for tests: install builtins on an existing context's global object
#[cfg(test)]
fn install_builtins(scope: &mut v8::HandleScope) {
    let global = scope.get_current_context().global(scope);

    let typeof_fn = v8::FunctionTemplate::new(scope, typeof_callback);
    let typeof_val = typeof_fn.get_function(scope).unwrap();
    let typeof_key = v8::String::new(scope, "moo_typeof").unwrap();
    global.set(scope, typeof_key.into(), typeof_val.into());

    let tostr_fn = v8::FunctionTemplate::new(scope, tostr_callback);
    let tostr_val = tostr_fn.get_function(scope).unwrap();
    let tostr_key = v8::String::new(scope, "tostr").unwrap();
    global.set(scope, tostr_key.into(), tostr_val.into());
}

/// Install the `typeof` builtin function
/// Note: Using `moo_typeof` to avoid collision with JavaScript's built-in `typeof` operator
#[cfg(test)]
#[allow(dead_code)]
fn install_typeof(scope: &mut v8::HandleScope, global: v8::Local<v8::Object>) {
    let typeof_fn = v8::FunctionTemplate::new(scope, typeof_callback);
    let typeof_val = typeof_fn
        .get_function(scope)
        .expect("Failed to create typeof function");

    let typeof_key = v8::String::new(scope, "moo_typeof").unwrap();
    global.set(scope, typeof_key.into(), typeof_val.into());
}

/// JavaScript callback for `typeof(value)`
fn typeof_callback<'a>(
    scope: &mut v8::HandleScope<'a>,
    args: v8::FunctionCallbackArguments<'a>,
    mut rv: v8::ReturnValue,
) {
    // Check argument count
    if args.length() < 1 {
        // Return error or throw exception
        let msg = v8::String::new(scope, "typeof requires 1 argument").unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    }

    // Convert V8 value to MOO Var
    let arg = args.get(0);
    let Some(moo_var) = v8_to_var_or_throw(scope, arg, "Error converting argument") else {
        return;
    };

    // Get type code
    let type_code = moo_var.type_code() as i64;
    let result = v_int(type_code);

    // Convert back to V8 and return
    let v8_result = var_to_v8(scope, &result);
    rv.set(v8_result);
}

/// Install the `tostr` builtin function
#[cfg(test)]
#[allow(dead_code)]
fn install_tostr(scope: &mut v8::HandleScope, global: v8::Local<v8::Object>) {
    let tostr_fn = v8::FunctionTemplate::new(scope, tostr_callback);
    let tostr_val = tostr_fn
        .get_function(scope)
        .expect("Failed to create tostr function");

    let tostr_key = v8::String::new(scope, "tostr").unwrap();
    global.set(scope, tostr_key.into(), tostr_val.into());
}

/// JavaScript callback for `tostr(...values)`
fn tostr_callback<'a>(
    scope: &mut v8::HandleScope<'a>,
    args: v8::FunctionCallbackArguments<'a>,
    mut rv: v8::ReturnValue,
) {
    use moor_var::{Variant, v_str};

    let mut result = String::new();

    // Convert all arguments to strings and concatenate
    for i in 0..args.length() {
        let arg = args.get(i);
        let Some(moo_var) = v8_to_var_or_throw(scope, arg, "Error converting argument") else {
            return;
        };

        // Convert to string representation (similar to bf_tostr logic)
        match moo_var.variant() {
            Variant::None => result.push_str("None"),
            Variant::Bool(b) => result.push_str(&format!("{b}")),
            Variant::Int(i) => result.push_str(&i.to_string()),
            Variant::Float(f) => result.push_str(&format!("{f:?}")),
            Variant::Str(s) => result.push_str(s.as_str()),
            Variant::Binary(b) => result.push_str(&format!("<binary {} bytes>", b.len())),
            Variant::Obj(o) => result.push_str(&o.to_string()),
            Variant::List(_) => result.push_str("{list}"),
            Variant::Map(_) => result.push_str("[map]"),
            Variant::Sym(s) => result.push_str(&s.to_string()),
            Variant::Err(e) => result.push_str(&e.name().as_arc_string()),
            Variant::Flyweight(_) => result.push_str("<flyweight>"),
            Variant::Lambda(_) => result.push_str("<lambda>"),
        }
    }

    let moo_result = v_str(&result);
    let v8_result = var_to_v8(scope, &moo_result);
    rv.set(v8_result);
}

/// Install the `call_verb` builtin function for JS->MOO verb calls
#[cfg(test)]
#[allow(dead_code)]
fn install_call_verb(scope: &mut v8::HandleScope, global: v8::Local<v8::Object>) {
    let call_verb_fn = v8::FunctionTemplate::new(scope, call_verb_callback);
    let call_verb_val = call_verb_fn
        .get_function(scope)
        .expect("Failed to create call_verb function");

    let call_verb_key = v8::String::new(scope, "call_verb").unwrap();
    global.set(scope, call_verb_key.into(), call_verb_val.into());
}

/// JavaScript callback for `call_verb(object, verb_name, ...args)`
/// Returns a Promise that resolves with the verb result
fn call_verb_callback<'a>(
    scope: &mut v8::HandleScope<'a>,
    args: v8::FunctionCallbackArguments<'a>,
    mut rv: v8::ReturnValue,
) {
    // Check argument count - need at least object and verb_name
    if args.length() < 2 {
        let msg = v8::String::new(
            scope,
            "call_verb requires at least 2 arguments (object, verb_name)",
        )
        .unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    }

    // Parse arguments
    let Some(this_arg) = v8_to_var_or_throw(scope, args.get(0), "Error converting 'this' argument") else {
        return;
    };
    let Some(verb_name_arg) = v8_to_var_or_throw(scope, args.get(1), "Error converting verb_name argument") else {
        return;
    };

    // Convert verb_name to Symbol
    let Ok(verb_name) = verb_name_arg.as_symbol() else {
        let msg = v8::String::new(scope, "Invalid verb name (must be string or symbol)").unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    };

    // Collect remaining arguments into a List
    let mut verb_args = Vec::new();
    for i in 2..args.length() {
        let Some(arg) = v8_to_var_or_throw(scope, args.get(i), &format!("Error converting argument {}", i)) else {
            return;
        };
        verb_args.push(arg);
    }
    let verb_args_var = v_list(&verb_args);
    let verb_args_list = match verb_args_var.variant() {
        moor_var::Variant::List(l) => l.clone(),
        _ => unreachable!("v_list should always return a List variant"),
    };

    // Check if we have a cached result (resuming from verb call)
    let cached_result = CURRENT_JS_FRAME.with(|frame_ref| {
        let frame_ptr = frame_ref.borrow();
        if let Some(ptr) = *frame_ptr {
            unsafe {
                let frame = &*ptr;
                if let JSContinuation::AwaitingVerbCall { call_info } = &frame.continuation {
                    call_info.result.clone()
                } else {
                    None
                }
            }
        } else {
            None
        }
    });

    if let Some(result) = cached_result {
        // We have a cached result - try to convert it and return a Promise
        let resolver = v8::PromiseResolver::new(scope).unwrap();
        let promise = resolver.get_promise(scope);

        // Use TryCatch to detect if var_to_v8 throws an exception
        let tc_scope = &mut v8::TryCatch::new(scope);
        let result_v8 = var_to_v8(tc_scope, &result);

        if tc_scope.has_caught() {
            // var_to_v8 threw an exception (Binary/Lambda/Flyweight)
            // Reject the Promise with the exception
            if let Some(exception) = tc_scope.exception() {
                resolver.reject(tc_scope, exception);
            }
            rv.set(promise.into());
            return;
        }

        resolver.resolve(tc_scope, result_v8);
        rv.set(promise.into());
    } else {
        // No cached result - create pending Promise and store call info
        let resolver = v8::PromiseResolver::new(scope).unwrap();
        let promise = resolver.get_promise(scope);

        // Store the pending verb call for execute_js_initial to find
        PENDING_DISPATCH.with(|pd| {
            *pd.borrow_mut() = Some(PendingDispatch::VerbCall(PendingVerbCall {
                this: this_arg,
                verb_name,
                args: verb_args_list,
                result: None,
            }));
        });

        rv.set(promise.into());
    }
}

/// Install the `obj` helper function for creating object references
#[cfg(test)]
#[allow(dead_code)]
fn install_obj(scope: &mut v8::HandleScope, global: v8::Local<v8::Object>) {
    let obj_fn = v8::FunctionTemplate::new(scope, obj_callback);
    let obj_val = obj_fn
        .get_function(scope)
        .expect("Failed to create obj function");

    let obj_key = v8::String::new(scope, "obj").unwrap();
    global.set(scope, obj_key.into(), obj_val.into());
}

/// JavaScript callback for `obj(id)` - creates a MOO object reference
fn obj_callback<'a>(
    scope: &mut v8::HandleScope<'a>,
    args: v8::FunctionCallbackArguments<'a>,
    mut rv: v8::ReturnValue,
) {
    // Check argument count
    if args.length() != 1 {
        let msg = v8::String::new(scope, "obj requires 1 argument (object id)").unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    }

    // Get the object ID
    let arg = args.get(0);
    if !arg.is_number() {
        let msg = v8::String::new(scope, "obj argument must be a number").unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    }

    let obj_id = arg.number_value(scope).unwrap() as i32;

    // Create MOO object representation: { __moo_obj: id }
    let obj = v8::Object::new(scope);
    let key = v8::String::new(scope, "__moo_obj").unwrap();
    let value = v8::Number::new(scope, obj_id as f64);
    obj.set(scope, key.into(), value.into());

    rv.set(obj.into());
}

/// JavaScript callback for `get_prop(object, prop_name)` - reads a MOO property
/// Synchronously retrieves a property value from the world state
fn get_prop_callback<'a>(
    scope: &mut v8::HandleScope<'a>,
    args: v8::FunctionCallbackArguments<'a>,
    mut rv: v8::ReturnValue,
) {
    // Check argument count
    if args.length() != 2 {
        let msg = v8::String::new(
            scope,
            "get_prop requires 2 arguments (object, property_name)",
        )
        .unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    }

    // Parse object argument
    let Some(obj_arg) = v8_to_var_or_throw(scope, args.get(0), "Error converting object argument") else {
        return;
    };
    let obj = match obj_arg.variant() {
        moor_var::Variant::Obj(o) => o,
        _ => {
            let msg = v8::String::new(scope, "get_prop: first argument must be an object").unwrap();
            let exception = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exception);
            return;
        }
    };

    // Parse property name argument
    let Some(prop_name_arg) = v8_to_var_or_throw(scope, args.get(1), "Error converting property name argument") else {
        return;
    };
    let prop_name = match prop_name_arg.variant() {
        moor_var::Variant::Str(s) => Symbol::mk(s.as_str()),
        _ => {
            let msg = v8::String::new(scope, "get_prop: property name must be a string").unwrap();
            let exception = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exception);
            return;
        }
    };

    // Get permissions from thread-local (set by execute_js_frame)
    let perms = JS_PERMISSIONS.with(|p| {
        p.borrow()
            .expect("JS_PERMISSIONS not set - execute_js_frame must be called first")
    });

    // Retrieve the property from world state
    let result = with_current_transaction(|ws| ws.retrieve_property(&perms, obj, prop_name));

    match result {
        Ok(value) => {
            // Convert and return the property value
            let v8_value = var_to_v8(scope, &value);
            rv.set(v8_value);
        }
        Err(ws_err) => {
            // Convert WorldStateError to MOO Error
            let moo_error = ws_err.to_error();

            // Create JavaScript Error object with MOO error encoded
            let error_msg_str = moo_error
                .msg
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("Error");
            let error_msg = v8::String::new(scope, error_msg_str).unwrap();
            let exception = v8::Exception::error(scope, error_msg);

            // Add MOO error code and value as properties for extraction later
            if let Some(err_obj) = exception.to_object(scope) {
                // Store the full error as a Var for proper reconstruction
                let err_var_key = v8::String::new(scope, "moo_error_var").unwrap();
                let err_var = moor_var::v_error(moo_error);
                let err_var_val = var_to_v8(scope, &err_var);
                err_obj.set(scope, err_var_key.into(), err_var_val);
            }

            scope.throw_exception(exception);
        }
    }
}

/// JavaScript callback for `call_builtin(builtin_name, ...args)` - calls a MOO builtin
/// Returns a Promise that resolves with the builtin result
fn call_builtin_callback<'a>(
    scope: &mut v8::HandleScope<'a>,
    args: v8::FunctionCallbackArguments<'a>,
    mut rv: v8::ReturnValue,
) {
    use crate::vm::js::js_frame::PendingBuiltinCall;
    use moor_compiler::BUILTINS;

    // Check argument count - need at least builtin name
    if args.length() < 1 {
        let msg = v8::String::new(scope, "call_builtin requires at least 1 argument (builtin_name)").unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    }

    // Parse builtin name argument
    let Some(builtin_name_arg) = v8_to_var_or_throw(scope, args.get(0), "Error converting builtin name argument") else {
        return;
    };
    let Ok(builtin_name) = builtin_name_arg.as_symbol() else {
        let msg = v8::String::new(scope, "Builtin name must be a string or symbol").unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    };

    // Look up the builtin ID
    let Some(builtin_id) = BUILTINS.find_builtin(builtin_name) else {
        let msg = v8::String::new(scope, &format!("Unknown builtin: {}", builtin_name)).unwrap();
        let exception = v8::Exception::error(scope, msg);
        scope.throw_exception(exception);
        return;
    };

    // Collect remaining arguments into a List
    let mut builtin_args = Vec::new();
    for i in 1..args.length() {
        let Some(arg) = v8_to_var_or_throw(scope, args.get(i), &format!("Error converting argument {}", i)) else {
            return;
        };
        builtin_args.push(arg);
    }
    let builtin_args_var = v_list(&builtin_args);
    let builtin_args_list = match builtin_args_var.variant() {
        moor_var::Variant::List(l) => l.clone(),
        _ => unreachable!("v_list should always return a List variant"),
    };

    // Check if we have a cached result (resuming from builtin call)
    let cached_result = CURRENT_JS_FRAME.with(|frame_ref| {
        let frame_ptr = frame_ref.borrow();
        if let Some(ptr) = *frame_ptr {
            unsafe {
                let frame = &*ptr;
                if let JSContinuation::AwaitingBuiltinCall { call_info } = &frame.continuation {
                    call_info.result.clone()
                } else {
                    None
                }
            }
        } else {
            None
        }
    });

    if let Some(result) = cached_result {
        // We have a cached result - try to convert it and return a Promise
        let resolver = v8::PromiseResolver::new(scope).unwrap();
        let promise = resolver.get_promise(scope);

        // Use TryCatch to detect if var_to_v8 throws an exception
        let tc_scope = &mut v8::TryCatch::new(scope);
        let result_v8 = var_to_v8(tc_scope, &result);

        if tc_scope.has_caught() {
            // var_to_v8 threw an exception (Binary/Lambda/Flyweight)
            // Reject the Promise with the exception
            if let Some(exception) = tc_scope.exception() {
                resolver.reject(tc_scope, exception);
            }
            rv.set(promise.into());
            return;
        }

        resolver.resolve(tc_scope, result_v8);
        rv.set(promise.into());
    } else {
        // No cached result - create pending Promise and store call info
        let resolver = v8::PromiseResolver::new(scope).unwrap();
        let promise = resolver.get_promise(scope);

        // Store the pending builtin call for execute_js_initial to find
        PENDING_DISPATCH.with(|pd| {
            *pd.borrow_mut() = Some(PendingDispatch::BuiltinCall(PendingBuiltinCall {
                builtin_id,
                args: builtin_args_list,
                result: None,
            }));
        });

        rv.set(promise.into());
    }
}

/// JavaScript callback for `moo_error(code, message?)`
/// Creates a MOO error object that can be thrown or returned
fn moo_error_callback<'a>(
    scope: &mut v8::HandleScope<'a>,
    args: v8::FunctionCallbackArguments<'a>,
    mut rv: v8::ReturnValue,
) {
    // Check argument count
    if args.length() < 1 {
        let msg = v8::String::new(scope, "moo_error requires at least 1 argument (error code)").unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    }

    // Get error code
    let code_arg = args.get(0);
    if !code_arg.is_number() {
        let msg = v8::String::new(scope, "moo_error: first argument must be a number (error code)").unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    }
    let code = code_arg.number_value(scope).unwrap() as i32;

    // Get optional message
    let message = if args.length() >= 2 {
        let msg_arg = args.get(1);
        if msg_arg.is_string() {
            let msg_str = msg_arg.to_string(scope).unwrap();
            Some(msg_str.to_rust_string_lossy(scope))
        } else if msg_arg.is_null() || msg_arg.is_undefined() {
            None
        } else {
            // Convert to string
            let msg_str = msg_arg.to_string(scope).unwrap();
            Some(msg_str.to_rust_string_lossy(scope))
        }
    } else {
        None
    };

    // Create error object in the format expected by v8_to_var
    let obj = v8::Object::new(scope);

    // Set __moo_error field
    let error_key = v8::String::new(scope, "__moo_error").unwrap();
    let error_val = v8::Number::new(scope, code as f64);
    obj.set(scope, error_key.into(), error_val.into());

    // Set msg field if provided
    if let Some(msg) = message {
        let msg_key = v8::String::new(scope, "msg").unwrap();
        let msg_val = v8::String::new(scope, &msg).unwrap();
        obj.set(scope, msg_key.into(), msg_val.into());
    }

    rv.set(obj.into());
}

/// JavaScript callback for MooError.is(code) method
/// Checks if this error matches the given error code
fn moo_error_is_callback<'a>(
    scope: &mut v8::HandleScope<'a>,
    args: v8::FunctionCallbackArguments<'a>,
    mut rv: v8::ReturnValue,
) {
    // Get 'this' (the error object)
    let this = args.this();

    // Check argument
    if args.length() < 1 || !args.get(0).is_number() {
        rv.set(v8::Boolean::new(scope, false).into());
        return;
    }

    let check_code = args.get(0).number_value(scope).unwrap() as i32;

    // Get __moo_error field from this
    let error_key = v8::String::new(scope, "__moo_error").unwrap();
    if let Some(error_val) = this.get(scope, error_key.into())
        && error_val.is_number()
    {
        let this_code = error_val.number_value(scope).unwrap() as i32;
        rv.set(v8::Boolean::new(scope, this_code == check_code).into());
        return;
    }

    rv.set(v8::Boolean::new(scope, false).into());
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::js::v8_host::initialize_v8;
    use moor_var::{v_int, v_str};

    #[test]
    fn test_typeof_builtin() {
        initialize_v8();

        // Create an isolate and context
        let mut isolate = v8::Isolate::new(Default::default());
        let scope = &mut v8::HandleScope::new(&mut isolate);
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);

        // Install builtins
        install_builtins(scope);

        // Test moo_typeof (note: different from JS typeof operator)
        let code = v8::String::new(scope, "moo_typeof(42)").unwrap();
        let script = v8::Script::compile(scope, code, None).unwrap();
        let result = script.run(scope).unwrap();

        let moo_result = v8_to_var(scope, result).unwrap();

        // Type code for Int should be 0
        assert_eq!(moo_result, v_int(0));
    }

    #[test]
    fn test_tostr_builtin() {
        initialize_v8();

        let mut isolate = v8::Isolate::new(Default::default());
        let scope = &mut v8::HandleScope::new(&mut isolate);
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);

        install_builtins(scope);

        // Test tostr with multiple arguments
        let code = v8::String::new(scope, "tostr('Hello', ' ', 'World', ' ', 42)").unwrap();
        let script = v8::Script::compile(scope, code, None).unwrap();
        let result = script.run(scope).unwrap();

        let moo_result = v8_to_var(scope, result).unwrap();
        assert_eq!(moo_result, v_str("Hello World 42"));
    }
}
