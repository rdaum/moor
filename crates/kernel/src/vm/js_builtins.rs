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

use crate::vm::v8_host::{v8_to_var, var_to_v8};
use moor_var::{v_int, Var};
use v8;

/// Install builtin functions into the JavaScript global scope
pub fn install_builtins(scope: &mut v8::HandleScope) {
    let global = scope.get_current_context().global(scope);

    // Install typeof builtin
    install_typeof(scope, global);

    // Install tostr builtin
    install_tostr(scope, global);
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
