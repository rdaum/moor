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

use moor_common::model::ObjectRef;
use moor_var::{Obj, Symbol, Var};
use uuid::Uuid;

use crate::{
    convert::{
        obj_from_ref, objectref_from_ref, symbol_from_ref, uuid_from_ref, var_from_flatbuffer_bytes,
    },
    flatbuffers_generated::moor_rpc,
};

/// Extract a required field and convert it, handling errors uniformly
pub fn extract_obj<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<moor_rpc::ObjRef, planus::Error>,
) -> Result<Obj, String> {
    let field_ref = get_field(msg).map_err(|_| format!("Missing {}", field_name))?;
    obj_from_ref(field_ref).map_err(|e| e.to_string())
}

/// Extract a required ObjectRef field
pub fn extract_object_ref<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<moor_rpc::ObjectRefRef, planus::Error>,
) -> Result<ObjectRef, String> {
    let field_ref = get_field(msg).map_err(|_| format!("Missing {}", field_name))?;
    objectref_from_ref(field_ref).map_err(|e| e.to_string())
}

/// Extract a required Symbol field
pub fn extract_symbol<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<moor_rpc::SymbolRef, planus::Error>,
) -> Result<Symbol, String> {
    let field_ref = get_field(msg).map_err(|_| format!("Missing {}", field_name))?;
    symbol_from_ref(field_ref)
}

/// Extract a required Var field (encoded as VarBytes)
pub fn extract_var<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<moor_rpc::VarBytesRef, planus::Error>,
) -> Result<Var, String> {
    let field_ref = get_field(msg).map_err(|_| format!("Missing {}", field_name))?;
    let data = field_ref
        .data()
        .map_err(|_| format!("Invalid {} data", field_name))?;
    var_from_flatbuffer_bytes(data).map_err(|e| e.to_string())
}

/// Extract a required UUID field
pub fn extract_uuid<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<moor_rpc::UuidRef, planus::Error>,
) -> Result<Uuid, String> {
    let field_ref = get_field(msg).map_err(|_| format!("Missing {}", field_name))?;
    uuid_from_ref(field_ref).map_err(|e| e.to_string())
}

/// Extract a required string field
pub fn extract_string<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<&str, planus::Error>,
) -> Result<String, String> {
    get_field(msg)
        .map(|s| s.to_string())
        .map_err(|_| format!("Missing {}", field_name))
}

/// Extract a required primitive field (u16, bool, etc.)
pub fn extract_field<T, F>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<F, planus::Error>,
) -> Result<F, String> {
    get_field(msg).map_err(|_| format!("Missing {}", field_name))
}

/// Extract an optional list of symbols
pub fn extract_symbol_list<T>(
    msg: &T,
    get_field: impl FnOnce(
        &T,
    ) -> Result<
        Option<planus::Vector<Result<moor_rpc::SymbolRef, planus::Error>>>,
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
        Option<planus::Vector<Result<moor_rpc::VarBytesRef, planus::Error>>>,
        planus::Error,
    >,
) -> Option<Vec<Var>> {
    get_field(msg).ok().and_then(|opt| {
        opt.map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.ok().and_then(|vb| {
                        let data = vb.data().ok()?;
                        var_from_flatbuffer_bytes(data).ok()
                    })
                })
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
    let items = get_field(msg).map_err(|_| format!("Missing {}", field_name))?;
    Ok(items
        .iter()
        .filter_map(|s| s.ok().map(|s| s.to_string()))
        .collect())
}
