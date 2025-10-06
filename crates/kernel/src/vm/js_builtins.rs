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

use crate::vm::{
    js_execute::{CURRENT_JS_FRAME, PENDING_VERB_CALL},
    js_frame::{JSContinuation, PendingVerbCall},
    v8_host::{v8_to_var, var_to_v8},
};
use moor_var::{v_int, v_list, Symbol};
use v8;

/// Install builtin functions into the JavaScript global scope
pub fn install_builtins(scope: &mut v8::HandleScope) {
    let global = scope.get_current_context().global(scope);

    // Install typeof builtin
    install_typeof(scope, global);

    // Install tostr builtin
    install_tostr(scope, global);

    // Install call_verb builtin for JS->MOO verb calls
    install_call_verb(scope, global);

    // Install obj() helper for creating object references
    install_obj(scope, global);
}

/// Install the `typeof` builtin function
/// Note: Using `moo_typeof` to avoid collision with JavaScript's built-in `typeof` operator
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
    let moo_var = v8_to_var(scope, arg);

    // Get type code
    let type_code = moo_var.type_code() as i64;
    let result = v_int(type_code);

    // Convert back to V8 and return
    let v8_result = var_to_v8(scope, &result);
    rv.set(v8_result);
}

/// Install the `tostr` builtin function
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
    use moor_var::{v_str, Variant};

    let mut result = String::new();

    // Convert all arguments to strings and concatenate
    for i in 0..args.length() {
        let arg = args.get(i);
        let moo_var = v8_to_var(scope, arg);

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
        let msg = v8::String::new(scope, "call_verb requires at least 2 arguments (object, verb_name)").unwrap();
        let exception = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exception);
        return;
    }

    // Parse arguments
    let this_arg = v8_to_var(scope, args.get(0));
    let verb_name_arg = v8_to_var(scope, args.get(1));

    // Convert verb_name to Symbol
    let verb_name = match verb_name_arg.variant() {
        moor_var::Variant::Str(s) => Symbol::mk(s.as_str()),
        _ => {
            let msg = v8::String::new(scope, "verb_name must be a string").unwrap();
            let exception = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exception);
            return;
        }
    };

    // Collect remaining arguments into a List
    let mut verb_args = Vec::new();
    for i in 2..args.length() {
        verb_args.push(v8_to_var(scope, args.get(i)));
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
        // We have a cached result - return a resolved Promise
        let resolver = v8::PromiseResolver::new(scope).unwrap();
        let promise = resolver.get_promise(scope);

        let result_v8 = var_to_v8(scope, &result);
        resolver.resolve(scope, result_v8);

        rv.set(promise.into());
    } else {
        // No cached result - create pending Promise and store call info
        let resolver = v8::PromiseResolver::new(scope).unwrap();
        let promise = resolver.get_promise(scope);

        // Store the pending verb call for execute_js_initial to find
        PENDING_VERB_CALL.with(|pc| {
            *pc.borrow_mut() = Some(PendingVerbCall {
                this: this_arg,
                verb_name,
                args: verb_args_list,
                result: None,
            });
        });

        rv.set(promise.into());
    }
}

/// Install the `obj` helper function for creating object references
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::v8_host::initialize_v8;
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

        let moo_result = v8_to_var(scope, result);

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

        let moo_result = v8_to_var(scope, result);
        assert_eq!(moo_result, v_str("Hello World 42"));
    }
}
