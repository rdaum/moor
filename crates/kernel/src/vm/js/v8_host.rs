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

//! V8 JavaScript engine integration.
//! Manages V8 platform initialization, isolate pooling, and type conversions.

use lazy_static::lazy_static;
use std::cell::RefCell;
use std::sync::{Arc, Mutex, Once};
use moor_var::{Var, Variant, v_int, v_float, v_str, v_string, v_objid, v_none, v_list, v_bool, v_binary};
use v8;

static V8_INIT: Once = Once::new();

lazy_static! {
    /// Global V8 platform instance
    static ref V8_PLATFORM: Arc<Mutex<Option<v8::SharedRef<v8::Platform>>>> = {
        Arc::new(Mutex::new(None))
    };
}

thread_local! {
    /// Thread-local V8 isolate pool
    /// Each worker thread has its own pool of isolates that are reused
    pub(crate) static V8_ISOLATE_POOL: RefCell<V8IsolatePool> = RefCell::new(V8IsolatePool::new(4));
}

/// Initialize the V8 platform. Must be called before any V8 operations.
/// Safe to call multiple times - initialization happens only once.
pub fn initialize_v8() {
    V8_INIT.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform.clone());
        v8::V8::initialize();

        let mut guard = V8_PLATFORM.lock().unwrap();
        *guard = Some(platform);
    });
}

/// Pool of V8 isolates for reuse.
/// Each isolate is expensive to create, so we pool them.
pub struct V8IsolatePool {
    isolates: Arc<Mutex<Vec<v8::OwnedIsolate>>>,
    max_isolates: usize,
}

impl V8IsolatePool {
    /// Create a new isolate pool with a maximum size
    pub fn new(max_isolates: usize) -> Self {
        initialize_v8();
        Self {
            isolates: Arc::new(Mutex::new(Vec::new())),
            max_isolates,
        }
    }

    /// Acquire an isolate from the pool, or create a new one if none available
    pub fn acquire(&self) -> v8::OwnedIsolate {
        let mut pool = self.isolates.lock().unwrap();

        if let Some(isolate) = pool.pop() {
            isolate
        } else {
            // Create new isolate with default parameters
            v8::Isolate::new(Default::default())
        }
    }

    /// Return an isolate to the pool for reuse
    pub fn release(&self, isolate: v8::OwnedIsolate) {
        let mut pool = self.isolates.lock().unwrap();

        if pool.len() < self.max_isolates {
            pool.push(isolate);
        }
        // Otherwise, just drop it (isolate goes out of scope)
    }
}

/// Convert a MOO Var to a V8 value
pub fn var_to_v8<'s>(
    scope: &mut v8::HandleScope<'s>,
    var: &Var,
) -> v8::Local<'s, v8::Value> {
    match var.variant() {
        Variant::None => v8::null(scope).into(),
        Variant::Int(i) => v8::Number::new(scope, *i as f64).into(),
        Variant::Float(f) => v8::Number::new(scope, *f).into(),
        Variant::Str(s) => {
            let s = s.as_str();
            v8::String::new(scope, s).unwrap().into()
        }
        Variant::Obj(o) => {
            // Objects represented as { __moo_obj: number }
            let obj = v8::Object::new(scope);
            let key = v8::String::new(scope, "__moo_obj").unwrap();
            let value = v8::Number::new(scope, o.id().0 as f64);
            obj.set(scope, key.into(), value.into());
            obj.into()
        }
        Variant::List(l) => {
            // Count items using iterator
            let len = l.iter().count();
            let array = v8::Array::new(scope, len as i32);
            for (i, item) in l.iter().enumerate() {
                let value = var_to_v8(scope, &item);
                array.set_index(scope, i as u32, value);
            }
            array.into()
        }
        Variant::Map(m) => {
            let obj = v8::Object::new(scope);
            for (k, v) in m.iter() {
                let key_str = match k.variant() {
                    Variant::Str(s) => s.as_str().to_string(),
                    Variant::Int(i) => i.to_string(),
                    _ => format!("{:?}", k),
                };
                let key = v8::String::new(scope, &key_str).unwrap();
                let value = var_to_v8(scope, &v);
                obj.set(scope, key.into(), value);
            }
            obj.into()
        }
        Variant::Err(e) => {
            // Errors represented as { __moo_error: code, msg: string }
            let obj = v8::Object::new(scope);
            let error_key = v8::String::new(scope, "__moo_error").unwrap();
            // Convert Error to int using to_int()
            let error_code = e.to_int().unwrap_or(0);
            let error_val = v8::Number::new(scope, error_code as f64);
            obj.set(scope, error_key.into(), error_val.into());

            if let Some(msg) = &e.msg {
                let msg_key = v8::String::new(scope, "msg").unwrap();
                let msg_val = v8::String::new(scope, msg.as_str()).unwrap();
                obj.set(scope, msg_key.into(), msg_val.into());
            }
            obj.into()
        }
        Variant::Binary(_) => {
            // Binary data cannot safely cross the JavaScript boundary yet
            // TODO: Implement zero-copy buffer wrapper similar to Deno's ZeroCopyBuf
            //   - Needs v8::External + finalizer callbacks to manage Rust ownership
            //   - Should provide accessor methods (get, slice, etc.) without copying
            //   - Must prevent both V8 GC and Rust drop while in use
            let msg = v8::String::new(scope, "Binary data cannot be passed to JavaScript").unwrap();
            let exception = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exception);
            v8::undefined(scope).into()
        }
        Variant::Lambda(_) => {
            // Lambda functions cannot safely cross the JavaScript boundary
            // They contain references to MOO code that JS can't execute
            let msg = v8::String::new(scope, "Lambda functions cannot be passed to JavaScript").unwrap();
            let exception = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exception);
            v8::undefined(scope).into()
        }
        Variant::Flyweight(_) => {
            // Anonymous/flyweight objects cannot safely cross the boundary
            // They're temporary and don't have stable identities
            let msg = v8::String::new(scope, "Anonymous objects cannot be passed to JavaScript").unwrap();
            let exception = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exception);
            v8::undefined(scope).into()
        }
        _ => {
            // Catch-all for any remaining types
            let msg = v8::String::new(scope, &format!("Cannot convert {:?} to JavaScript", var.type_code())).unwrap();
            let exception = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exception);
            v8::undefined(scope).into()
        }
    }
}

/// Convert a V8 value to a MOO Var
pub fn v8_to_var<'s>(
    scope: &mut v8::HandleScope<'s>,
    value: v8::Local<'s, v8::Value>,
) -> Var {
    if value.is_null() || value.is_undefined() {
        return v_none();
    }

    if value.is_boolean() {
        let b = value.boolean_value(scope);
        return v_bool(b);
    }

    if value.is_number() {
        let n = value.number_value(scope).unwrap();
        if n.fract() == 0.0 && n.is_finite() {
            return v_int(n as i64);
        } else {
            return v_float(n);
        }
    }

    if value.is_string() {
        let s = value.to_string(scope).unwrap();
        let s = s.to_rust_string_lossy(scope);
        return v_string(s);
    }

    if value.is_array() {
        let array = v8::Local::<v8::Array>::try_from(value).unwrap();
        let len = array.length();
        let mut items = Vec::with_capacity(len as usize);

        for i in 0..len {
            if let Some(item) = array.get_index(scope, i) {
                items.push(v8_to_var(scope, item));
            } else {
                items.push(v_none());
            }
        }

        return v_list(&items);
    }

    // Check for typed arrays (Uint8Array, etc.) - convert to Binary
    if value.is_typed_array() {
        if let Ok(typed_array) = v8::Local::<v8::TypedArray>::try_from(value) {
            let byte_length = typed_array.byte_length();
            let mut bytes = vec![0u8; byte_length];
            let copied = typed_array.copy_contents(&mut bytes);
            if copied == byte_length {
                return v_binary(bytes);
            }
        }
    }

    // Check for ArrayBuffer - convert to Binary
    if value.is_array_buffer() {
        if let Ok(array_buffer) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
            let backing_store = array_buffer.get_backing_store();
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    backing_store.data().unwrap().as_ptr() as *const u8,
                    backing_store.byte_length(),
                )
            };
            return v_binary(bytes.to_vec());
        }
    }

    if value.is_object() {
        let obj = v8::Local::<v8::Object>::try_from(value).unwrap();

        // Check for special MOO object marker
        let moo_obj_key = v8::String::new(scope, "__moo_obj").unwrap();
        if let Some(moo_obj_val) = obj.get(scope, moo_obj_key.into()) {
            if moo_obj_val.is_number() {
                let n = moo_obj_val.number_value(scope).unwrap() as i32;
                return v_objid(n);
            }
        }

        // Check for MOO error marker
        let moo_err_key = v8::String::new(scope, "__moo_error").unwrap();
        if let Some(moo_err_val) = obj.get(scope, moo_err_key.into()) {
            if moo_err_val.is_number() {
                let code = moo_err_val.number_value(scope).unwrap() as i32;
                let msg_key = v8::String::new(scope, "msg").unwrap();
                let msg = obj.get(scope, msg_key.into())
                    .and_then(|v| v.to_string(scope))
                    .map(|s| s.to_rust_string_lossy(scope));

                // Convert code to ErrorCode - match common error codes
                let error_code = match code {
                    0 => moor_var::E_NONE,
                    1 => moor_var::E_TYPE,
                    2 => moor_var::E_DIV,
                    3 => moor_var::E_PERM,
                    4 => moor_var::E_PROPNF,
                    5 => moor_var::E_VERBNF,
                    6 => moor_var::E_VARNF,
                    7 => moor_var::E_INVIND,
                    8 => moor_var::E_RECMOVE,
                    9 => moor_var::E_MAXREC,
                    10 => moor_var::E_RANGE,
                    11 => moor_var::E_ARGS,
                    12 => moor_var::E_NACC,
                    13 => moor_var::E_INVARG,
                    14 => moor_var::E_QUOTA,
                    15 => moor_var::E_FLOAT,
                    _ => moor_var::E_ARGS,
                };
                return Var::mk_error(moor_var::Error::new(error_code, msg, None));
            }
        }

        // Regular object - convert to map
        let property_names = obj.get_own_property_names(scope, Default::default()).unwrap();
        let len = property_names.length();
        let mut pairs = Vec::new();

        for i in 0..len {
            if let Some(key) = property_names.get_index(scope, i) {
                let key_str = key.to_string(scope).unwrap();
                let key_rust = key_str.to_rust_string_lossy(scope);
                if let Some(val) = obj.get(scope, key) {
                    pairs.push((v_str(&key_rust), v8_to_var(scope, val)));
                }
            }
        }

        return moor_var::v_map(&pairs);
    }

    // Fallback
    v_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v8_initialization() {
        initialize_v8();
        // Should not panic
    }

}
