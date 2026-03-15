// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Var <-> V8 value marshalling.
//!
//! Primitives (int, float, string, bool) are copied into V8 values.
//! Compound types (list, obj) are wrapped as V8 objects holding a reference
//! to the underlying Var. Element access goes through synchronous native
//! callbacks — no trampoline, no deep copy.

use moor_var::{Var, Variant, v_float, v_int, v_none, v_string};

/// Install MoorList and MoorObj object templates into the context.
/// Call once per context setup.
pub(crate) fn install_wrapper_templates(scope: &mut v8::HandleScope<'_>) {
    install_moor_list_template(scope);
    install_moor_obj_template(scope);
}

// ---------------------------------------------------------------------------
// MoorList — wraps a Var::List as an array-like object
// ---------------------------------------------------------------------------

const MOOR_LIST_TEMPLATE_KEY: &str = "__moor_list_tpl";
const MOOR_OBJ_TEMPLATE_KEY: &str = "__moor_obj_tpl";

fn install_moor_list_template(scope: &mut v8::HandleScope<'_>) {
    let tpl = v8::ObjectTemplate::new(scope);
    tpl.set_internal_field_count(1);

    // Indexed property handler for list[i] access.
    let indexed_config =
        v8::IndexedPropertyHandlerConfiguration::new().getter(moor_list_indexed_getter);
    tpl.set_indexed_property_handler(indexed_config);

    // .length accessor
    let length_name = v8::String::new(scope, "length").unwrap();
    tpl.set_accessor(length_name.into(), moor_list_length_getter);

    // Stash the template as a Global via External on the context global.
    let tpl_global = v8::Global::new(scope, tpl);
    let boxed = Box::into_raw(Box::new(tpl_global));
    let ext = v8::External::new(scope, boxed as *mut std::ffi::c_void);
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, MOOR_LIST_TEMPLATE_KEY).unwrap();
    global.set(scope, key.into(), ext.into());
}

fn get_moor_list_template<'s>(
    scope: &mut v8::HandleScope<'s>,
) -> v8::Local<'s, v8::ObjectTemplate> {
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, MOOR_LIST_TEMPLATE_KEY).unwrap();
    let ext_val = global.get(scope, key.into()).unwrap();
    let ext: v8::Local<v8::External> = v8::Local::cast(ext_val);
    let ptr = ext.value() as *const v8::Global<v8::ObjectTemplate>;
    v8::Local::new(scope, unsafe { &*ptr })
}

fn moor_list_indexed_getter(
    scope: &mut v8::HandleScope<'_>,
    index: u32,
    _args: v8::PropertyCallbackArguments<'_>,
    mut rv: v8::ReturnValue<v8::Value>,
) -> v8::Intercepted {
    let this = _args.this();
    if let Some(var) = extract_var_from_internal(scope, this) {
        if let Variant::List(list) = var.variant() {
            if (index as usize) < list.len() {
                let item = list.iter().nth(index as usize).unwrap();
                let val = var_to_v8(scope, &item);
                rv.set(val);
                return v8::Intercepted::Yes;
            }
        }
    }
    v8::Intercepted::No
}

fn moor_list_length_getter(
    scope: &mut v8::HandleScope<'_>,
    _key: v8::Local<v8::Name>,
    args: v8::PropertyCallbackArguments<'_>,
    mut rv: v8::ReturnValue<v8::Value>,
) {
    let this = args.this();
    if let Some(var) = extract_var_from_internal(scope, this) {
        if let Variant::List(list) = var.variant() {
            rv.set(v8::Integer::new(scope, list.len() as i32).into());
        }
    }
}

// ---------------------------------------------------------------------------
// MoorObj — wraps a Var::Obj as a value object
// ---------------------------------------------------------------------------

fn install_moor_obj_template(scope: &mut v8::HandleScope<'_>) {
    let tpl = v8::ObjectTemplate::new(scope);
    tpl.set_internal_field_count(1);

    let id_name = v8::String::new(scope, "id").unwrap();
    tpl.set_accessor(id_name.into(), moor_obj_id_getter);

    let tpl_global = v8::Global::new(scope, tpl);
    let boxed = Box::into_raw(Box::new(tpl_global));
    let ext = v8::External::new(scope, boxed as *mut std::ffi::c_void);
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, MOOR_OBJ_TEMPLATE_KEY).unwrap();
    global.set(scope, key.into(), ext.into());
}

fn get_moor_obj_template<'s>(
    scope: &mut v8::HandleScope<'s>,
) -> v8::Local<'s, v8::ObjectTemplate> {
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, MOOR_OBJ_TEMPLATE_KEY).unwrap();
    let ext_val = global.get(scope, key.into()).unwrap();
    let ext: v8::Local<v8::External> = v8::Local::cast(ext_val);
    let ptr = ext.value() as *const v8::Global<v8::ObjectTemplate>;
    v8::Local::new(scope, unsafe { &*ptr })
}

fn moor_obj_id_getter(
    scope: &mut v8::HandleScope<'_>,
    _key: v8::Local<v8::Name>,
    args: v8::PropertyCallbackArguments<'_>,
    mut rv: v8::ReturnValue<v8::Value>,
) {
    let this = args.this();
    if let Some(var) = extract_var_from_internal(scope, this) {
        if let Variant::Obj(obj) = var.variant() {
            rv.set(v8::Integer::new(scope, obj.id().0).into());
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Store a Var in a V8 object's internal field 0 via v8::External.
fn store_var_in_internal(
    scope: &mut v8::HandleScope<'_>,
    obj: v8::Local<v8::Object>,
    var: Var,
) {
    let boxed = Box::into_raw(Box::new(var));
    let ext = v8::External::new(scope, boxed as *mut std::ffi::c_void);
    obj.set_internal_field(0, ext.into());
}

/// Extract a Var reference from a V8 object's internal field 0.
pub(crate) fn extract_var_from_internal<'a>(
    scope: &mut v8::HandleScope<'_>,
    obj: v8::Local<v8::Object>,
) -> Option<&'a Var> {
    let field = obj.get_internal_field(scope, 0)?;
    let ext = v8::Local::<v8::External>::try_from(field).ok()?;
    let ptr = ext.value() as *const Var;
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { &*ptr })
}

fn wrap_list<'s>(scope: &mut v8::HandleScope<'s>, var: Var) -> v8::Local<'s, v8::Object> {
    let tpl = get_moor_list_template(scope);
    let obj = tpl.new_instance(scope).unwrap();
    store_var_in_internal(scope, obj, var);
    obj
}

fn wrap_obj<'s>(scope: &mut v8::HandleScope<'s>, var: Var) -> v8::Local<'s, v8::Object> {
    let tpl = get_moor_obj_template(scope);
    let obj = tpl.new_instance(scope).unwrap();
    store_var_in_internal(scope, obj, var);
    obj
}

/// Convert a moor Var to a V8 value.
/// Primitives are copied; compound types are wrapped by reference.
pub(crate) fn var_to_v8<'s>(
    scope: &mut v8::HandleScope<'s>,
    var: &Var,
) -> v8::Local<'s, v8::Value> {
    match var.variant() {
        Variant::None => v8::undefined(scope).into(),
        Variant::Int(i) => v8::Integer::new(scope, i as i32).into(),
        Variant::Float(f) => v8::Number::new(scope, f).into(),
        Variant::Str(s) => {
            let s = v8::String::new(scope, s.as_str()).unwrap();
            s.into()
        }
        Variant::Bool(b) => v8::Boolean::new(scope, b).into(),
        Variant::Obj(_) => wrap_obj(scope, var.clone()).into(),
        Variant::List(_) => wrap_list(scope, var.clone()).into(),
        // Map, Error, Flyweight, Lambda — not yet wrapped in the spike.
        _ => v8::undefined(scope).into(),
    }
}

/// Convert a V8 value back to a moor Var.
/// Unwraps MoorList/MoorObj wrappers back to the original Var (no copy).
/// Native JS scalars are converted to their moor equivalents.
pub(crate) fn v8_to_var(scope: &mut v8::HandleScope<'_>, val: v8::Local<v8::Value>) -> Var {
    if val.is_undefined() || val.is_null() {
        return v_none();
    }
    if val.is_boolean() {
        return Var::mk_bool(val.boolean_value(scope));
    }
    if val.is_int32() {
        return v_int(val.int32_value(scope).unwrap_or(0) as i64);
    }
    if val.is_number() {
        return v_float(val.number_value(scope).unwrap_or(0.0));
    }
    if val.is_string() {
        let s = val.to_rust_string_lossy(scope);
        return v_string(s);
    }
    // Check if this is a wrapper object (has internal field with a Var).
    if let Ok(obj) = v8::Local::<v8::Object>::try_from(val) {
        if let Some(var) = extract_var_from_internal(scope, obj) {
            return var.clone();
        }
    }
    v_none()
}
