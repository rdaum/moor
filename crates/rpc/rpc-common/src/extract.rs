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

//! Helper functions for extracting and converting FlatBuffer message fields

use crate::RpcMessageError;
use moor_common::model::ObjectRef;
use moor_schema::{
    StrErr,
    convert::{obj_from_ref, objectref_from_ref, symbol_from_ref, uuid_from_ref, var_from_flatbuffer_ref},
    rpc, var,
};
use moor_var::{Obj, Symbol, Var};
use uuid::Uuid;

/// Extract a required field and convert it, handling errors uniformly
pub fn extract_obj<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<rpc::ObjRef, planus::Error>,
) -> Result<Obj, String> {
    let field_ref = get_field(msg).map_err(|_| format!("Missing {field_name}"))?;
    obj_from_ref(field_ref).str_err()
}

/// Extract a required ObjectRef field
pub fn extract_object_ref<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<rpc::ObjectRefRef, planus::Error>,
) -> Result<ObjectRef, String> {
    let field_ref = get_field(msg).map_err(|_| format!("Missing {field_name}"))?;
    objectref_from_ref(field_ref).str_err()
}

/// Extract a required Symbol field
pub fn extract_symbol<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<rpc::SymbolRef, planus::Error>,
) -> Result<Symbol, String> {
    let field_ref = get_field(msg).map_err(|_| format!("Missing {field_name}"))?;
    symbol_from_ref(field_ref)
}

/// Extract a required Var field
pub fn extract_var<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<var::VarRef, planus::Error>,
) -> Result<Var, String> {
    let field_ref = get_field(msg).map_err(|_| format!("Missing {field_name}"))?;
    var_from_flatbuffer_ref(field_ref).str_err()
}

/// Extract a required UUID field
pub fn extract_uuid<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<rpc::UuidRef, planus::Error>,
) -> Result<Uuid, String> {
    let field_ref = get_field(msg).map_err(|_| format!("Missing {field_name}"))?;
    uuid_from_ref(field_ref).str_err()
}

/// Extract a required string field
pub fn extract_string<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<&str, planus::Error>,
) -> Result<String, String> {
    get_field(msg)
        .map(|s| s.to_string())
        .map_err(|_| format!("Missing {field_name}"))
}

/// Extract a required primitive field (u16, bool, etc.)
pub fn extract_field<T, F>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<F, planus::Error>,
) -> Result<F, String> {
    get_field(msg).map_err(|_| format!("Missing {field_name}"))
}

/// Extract an optional list of symbols
pub fn extract_symbol_list<T>(
    msg: &T,
    get_field: impl FnOnce(
        &T,
    ) -> Result<
        Option<planus::Vector<Result<rpc::SymbolRef, planus::Error>>>,
        planus::Error,
    >,
) -> Option<Vec<Symbol>> {
    get_field(msg).ok().and_then(|opt| {
        opt.map(|items| {
            items
                .iter()
                .filter_map(|item| item.ok().and_then(|i| symbol_from_ref(i).ok()))
                .collect()
        })
    })
}

/// Extract an optional list of vars
pub fn extract_var_list<T>(
    msg: &T,
    get_field: impl FnOnce(
        &T,
    ) -> Result<
        Option<planus::Vector<Result<var::VarRef, planus::Error>>>,
        planus::Error,
    >,
) -> Option<Vec<Var>> {
    get_field(msg).ok().and_then(|opt| {
        opt.map(|items| {
            items
                .iter()
                .filter_map(|item| item.ok().and_then(|var_ref| var_from_flatbuffer_ref(var_ref).ok()))
                .collect()
        })
    })
}

/// Extract a required list of strings
pub fn extract_string_list<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<planus::Vector<Result<&str, planus::Error>>, planus::Error>,
) -> Result<Vec<String>, String> {
    let items = get_field(msg).map_err(|_| format!("Missing {field_name}"))?;
    Ok(items
        .iter()
        .filter_map(|s| s.ok().map(|s| s.to_string()))
        .collect())
}

// ============================================================================
// RpcMessageError-returning variants for use in RPC handlers
// ============================================================================

/// Extract a required Obj field, returning RpcMessageError
pub fn extract_obj_rpc<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<rpc::ObjRef, planus::Error>,
) -> Result<Obj, RpcMessageError> {
    extract_obj(msg, field_name, get_field).map_err(RpcMessageError::InvalidRequest)
}

/// Extract a required ObjectRef field, returning RpcMessageError
pub fn extract_object_ref_rpc<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<rpc::ObjectRefRef, planus::Error>,
) -> Result<ObjectRef, RpcMessageError> {
    extract_object_ref(msg, field_name, get_field).map_err(RpcMessageError::InvalidRequest)
}

/// Extract a required Symbol field, returning RpcMessageError
pub fn extract_symbol_rpc<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<rpc::SymbolRef, planus::Error>,
) -> Result<Symbol, RpcMessageError> {
    extract_symbol(msg, field_name, get_field).map_err(RpcMessageError::InvalidRequest)
}

/// Extract a required Var field, returning RpcMessageError
pub fn extract_var_rpc<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<var::VarRef, planus::Error>,
) -> Result<Var, RpcMessageError> {
    extract_var(msg, field_name, get_field).map_err(RpcMessageError::InvalidRequest)
}

/// Extract a required UUID field, returning RpcMessageError
pub fn extract_uuid_rpc<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<rpc::UuidRef, planus::Error>,
) -> Result<Uuid, RpcMessageError> {
    extract_uuid(msg, field_name, get_field).map_err(RpcMessageError::InvalidRequest)
}

/// Extract a required string field, returning RpcMessageError
pub fn extract_string_rpc<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<&str, planus::Error>,
) -> Result<String, RpcMessageError> {
    extract_string(msg, field_name, get_field).map_err(RpcMessageError::InvalidRequest)
}

/// Extract a required primitive field, returning RpcMessageError
pub fn extract_field_rpc<T, F>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<F, planus::Error>,
) -> Result<F, RpcMessageError> {
    extract_field(msg, field_name, get_field).map_err(RpcMessageError::InvalidRequest)
}

/// Extract a required list of strings, returning RpcMessageError
pub fn extract_string_list_rpc<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<planus::Vector<Result<&str, planus::Error>>, planus::Error>,
) -> Result<Vec<String>, RpcMessageError> {
    extract_string_list(msg, field_name, get_field).map_err(RpcMessageError::InvalidRequest)
}
