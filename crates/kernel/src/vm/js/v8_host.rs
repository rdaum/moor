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
use moor_var::{
    Binary, Var, Variant, v_binary, v_bool, v_float, v_int, v_list, v_none, v_objid, v_str,
    v_string,
};
use std::cell::RefCell;
use std::sync::{Mutex, Once};
use v8;

static V8_INIT: Once = Once::new();

lazy_static! {
    /// Global V8 platform instance
    static ref V8_PLATFORM: Mutex<Option<v8::SharedRef<v8::Platform>>> = {
        Mutex::new(None)
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
/// This is thread-local, so we don't need Arc - just Mutex for interior mutability.
pub struct V8IsolatePool {
    isolates: Mutex<Vec<v8::OwnedIsolate>>,
    max_isolates: usize,
}

impl V8IsolatePool {
    /// Create a new isolate pool with a maximum size
    pub fn new(max_isolates: usize) -> Self {
        initialize_v8();
        Self {
            isolates: Mutex::new(Vec::new()),
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

/// Deleter callback for Binary ArrayBuffer backing stores
/// Called by V8 when the ArrayBuffer is garbage collected
unsafe extern "C" fn binary_deleter(
    _data_ptr: *mut std::ffi::c_void,
    _byte_len: usize,
    deleter_data: *mut std::ffi::c_void,
) {
    // Reconstruct and drop the Box<Binary>
    // Safety: deleter_data was created from Box::into_raw in var_to_v8
    unsafe {
        let _ = Box::from_raw(deleter_data as *mut Binary);
    }
}

/// Convert a MOO Var to a V8 value
pub fn var_to_v8<'s>(scope: &mut v8::HandleScope<'s>, var: &Var) -> v8::Local<'s, v8::Value> {
    match var.variant() {
        Variant::None => v8::null(scope).into(),
        Variant::Bool(b) => v8::Boolean::new(scope, *b).into(),
        Variant::Int(i) => v8::Number::new(scope, *i as f64).into(),
        Variant::Float(f) => v8::Number::new(scope, *f).into(),
        Variant::Str(s) => {
            let s = s.as_str();
            v8::String::new(scope, s).unwrap().into()
        }
        Variant::Sym(s) => {
            // JavaScript doesn't have MOO-style symbols, convert to string
            let s_str = s.as_string();
            v8::String::new(scope, &s_str).unwrap().into()
        }
        Variant::Obj(o) => {
            // Reject anonymous objects - they're temporary and would leak references
            if o.is_anonymous() {
                let msg = v8::String::new(scope, "Anonymous objects cannot be passed to JavaScript").unwrap();
                let exception = v8::Exception::type_error(scope, msg);
                scope.throw_exception(exception);
                return v8::undefined(scope).into();
            }

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
                    Variant::Sym(s) => s.as_string(),
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
        Variant::Binary(b) => {
            // Zero-copy wrapper: expose Binary as Uint8Array backed by Binary's data
            // Binary is CoW (uses ByteView), so cloning is cheap
            let binary = (**b).clone();

            // Box the Binary to keep it alive while JS holds the ArrayBuffer
            let boxed_binary = Box::new(binary);

            // Get pointer to the bytes AFTER boxing (important - pointer is stable in Box)
            let byte_slice = boxed_binary.as_bytes();
            let byte_len = byte_slice.len();
            let data_ptr = byte_slice.as_ptr() as *mut u8;

            let binary_ptr = Box::into_raw(boxed_binary) as *mut std::ffi::c_void;

            // Create backing store with deleter callback
            // Safety: data_ptr points to bytes owned by boxed_binary
            // Deleter will drop boxed_binary when V8 GCs the ArrayBuffer
            let backing_store = unsafe {
                v8::ArrayBuffer::new_backing_store_from_ptr(
                    data_ptr as *mut std::ffi::c_void,
                    byte_len,
                    binary_deleter,
                    binary_ptr,
                )
            };

            let backing_store_shared = backing_store.make_shared();
            let array_buffer = v8::ArrayBuffer::with_backing_store(scope, &backing_store_shared);

            // Expose as Uint8Array for JavaScript TypedArray API
            let uint8_array = v8::Uint8Array::new(scope, array_buffer, 0, byte_len).unwrap();
            uint8_array.into()
        }
        Variant::Lambda(_) => {
            // Lambda functions cannot safely cross the JavaScript boundary
            // They contain references to MOO code that JS can't execute
            let msg =
                v8::String::new(scope, "Lambda functions cannot be passed to JavaScript").unwrap();
            let exception = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exception);
            v8::undefined(scope).into()
        }
        Variant::Flyweight(_) => {
            // Anonymous/flyweight objects cannot safely cross the boundary
            // They're temporary and don't have stable identities
            let msg =
                v8::String::new(scope, "Anonymous objects cannot be passed to JavaScript").unwrap();
            let exception = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exception);
            v8::undefined(scope).into()
        }
    }
}

/// Convert a V8 value to a MOO Var
/// Returns Err if the conversion fails (e.g., Infinity or NaN)
pub fn v8_to_var<'s>(
    scope: &mut v8::HandleScope<'s>,
    value: v8::Local<'s, v8::Value>,
) -> Result<Var, moor_var::Error> {
    if value.is_null() || value.is_undefined() {
        return Ok(v_none());
    }

    if value.is_boolean() {
        let b = value.boolean_value(scope);
        return Ok(v_bool(b));
    }

    if value.is_number() {
        let n = value.number_value(scope).unwrap();

        // MOO doesn't support Infinity or NaN - return errors
        if n.is_infinite() {
            return Err(moor_var::Error::new(
                moor_var::E_DIV,
                Some("Division by zero".to_string()),
                None,
            ));
        }
        if n.is_nan() {
            return Err(moor_var::Error::new(
                moor_var::E_FLOAT,
                Some("Floating point error (NaN)".to_string()),
                None,
            ));
        }

        return if n.fract() == 0.0 && n.is_finite() {
            Ok(v_int(n as i64))
        } else {
            Ok(v_float(n))
        };
    }

    if value.is_string() {
        let s = value.to_string(scope).unwrap();
        let s = s.to_rust_string_lossy(scope);
        return Ok(v_string(s));
    }

    if value.is_array() {
        let array = v8::Local::<v8::Array>::try_from(value).unwrap();
        let len = array.length();
        let mut items = Vec::with_capacity(len as usize);

        for i in 0..len {
            if let Some(item) = array.get_index(scope, i) {
                items.push(v8_to_var(scope, item)?);
            } else {
                items.push(v_none());
            }
        }

        return Ok(v_list(&items));
    }

    // Check for typed arrays (Uint8Array, etc.) - convert to Binary
    if value.is_typed_array()
        && let Ok(typed_array) = v8::Local::<v8::TypedArray>::try_from(value)
    {
        let byte_length = typed_array.byte_length();
        let mut bytes = vec![0u8; byte_length];
        let copied = typed_array.copy_contents(&mut bytes);
        if copied == byte_length {
            return Ok(v_binary(bytes));
        }
    }

    // Check for ArrayBuffer - convert to Binary
    if value.is_array_buffer()
        && let Ok(array_buffer) = v8::Local::<v8::ArrayBuffer>::try_from(value)
    {
        let backing_store = array_buffer.get_backing_store();
        let bytes = unsafe {
            std::slice::from_raw_parts(
                backing_store.data().unwrap().as_ptr() as *const u8,
                backing_store.byte_length(),
            )
        };
        return Ok(v_binary(bytes.to_vec()));
    }

    if value.is_object() {
        let obj = v8::Local::<v8::Object>::try_from(value).unwrap();

        // Check for special MOO object marker
        let moo_obj_key = v8::String::new(scope, "__moo_obj").unwrap();
        if let Some(moo_obj_val) = obj.get(scope, moo_obj_key.into())
            && moo_obj_val.is_number()
        {
            let n = moo_obj_val.number_value(scope).unwrap() as i32;
            return Ok(v_objid(n));
        }

        // Check for MOO error marker
        let moo_err_key = v8::String::new(scope, "__moo_error").unwrap();
        if let Some(moo_err_val) = obj.get(scope, moo_err_key.into())
            && moo_err_val.is_number()
        {
            let code = moo_err_val.number_value(scope).unwrap() as i32;
            let msg_key = v8::String::new(scope, "msg").unwrap();
            let msg = obj
                .get(scope, msg_key.into())
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
            return Ok(Var::mk_error(moor_var::Error::new(error_code, msg, None)));
        }

        // Regular object - convert to map
        let property_names = obj
            .get_own_property_names(scope, Default::default())
            .unwrap();
        let len = property_names.length();
        let mut pairs = Vec::new();

        for i in 0..len {
            if let Some(key) = property_names.get_index(scope, i) {
                let key_str = key.to_string(scope).unwrap();
                let key_rust = key_str.to_rust_string_lossy(scope);
                if let Some(val) = obj.get(scope, key) {
                    pairs.push((v_str(&key_rust), v8_to_var(scope, val)?));
                }
            }
        }

        return Ok(moor_var::v_map(&pairs));
    }

    // Fallback
    Ok(v_none())
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
