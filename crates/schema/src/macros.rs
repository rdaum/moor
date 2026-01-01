// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Helper macros for FlatBuffer conversion boilerplate reduction.

/// Read a required field from a FlatBuffer reference with automatic error context.
///
/// Transforms `ref.field()` into `ref.field().map_err(|e| format!("...: {e}"))?`
///
/// # Examples
///
/// ```ignore
/// // Instead of:
/// let value = obj_ref.field_name().map_err(|e| format!("Failed to read field_name: {e}"))?;
///
/// // Write:
/// let value = fb_read!(obj_ref, field_name);
/// ```
#[macro_export]
macro_rules! fb_read {
    ($ref:expr, $field:ident) => {
        $ref.$field()
            .map_err(|e| format!("Failed to read {}: {e}", stringify!($field)))?
    };
}

/// Read an optional field from a FlatBuffer reference.
///
/// Returns the Option from the underlying call, with error on read failure.
#[macro_export]
macro_rules! fb_read_opt {
    ($ref:expr, $field:ident) => {
        $ref.$field()
            .map_err(|e| format!("Failed to read {}: {e}", stringify!($field)))?
    };
}

/// Read a required field from a FlatBuffer reference, converting to owned type.
///
/// Useful for fields that return `Ref` types that need conversion via TryFrom.
#[macro_export]
macro_rules! fb_read_into {
    ($ref:expr, $field:ident, $target:ty) => {{
        let field_ref = $ref
            .$field()
            .map_err(|e| format!("Failed to read {}: {e}", stringify!($field)))?;
        <$target>::try_from(field_ref)
            .map_err(|e| format!("Failed to convert {}: {e}", stringify!($field)))?
    }};
}

/// Convert a DecodeError string into a DecodeError::DecodeFailed variant.
///
/// Reduces `DecodeError::DecodeFailed(format!("..."))` to `decode_err!("...")`.
#[macro_export]
macro_rules! decode_err {
    ($msg:literal) => {
        $crate::convert_program::DecodeError::DecodeFailed($msg.to_string())
    };
    ($fmt:literal, $($arg:tt)*) => {
        $crate::convert_program::DecodeError::DecodeFailed(format!($fmt, $($arg)*))
    };
}

/// Read a field with DecodeError result type.
///
/// Like fb_read! but returns DecodeError instead of String.
#[macro_export]
macro_rules! fb_decode {
    ($ref:expr, $field:ident) => {
        $ref.$field().map_err(|e| {
            $crate::convert_program::DecodeError::DecodeFailed(format!(
                "Failed to read {}: {e}",
                stringify!($field)
            ))
        })?
    };
}

/// Read a field with DecodeError result type and custom context.
#[macro_export]
macro_rules! fb_decode_ctx {
    ($ref:expr, $field:ident, $ctx:expr) => {
        $ref.$field().map_err(|e| {
            $crate::convert_program::DecodeError::DecodeFailed(format!(
                "{} {}: {e}",
                $ctx,
                stringify!($field)
            ))
        })?
    };
}
