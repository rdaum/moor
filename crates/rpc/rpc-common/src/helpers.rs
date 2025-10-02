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

//! Helper functions for constructing FlatBuffer types from Rust types
//!
//! These helpers prioritize zero-copy and move semantics to avoid unnecessary allocations.

use crate::{AuthToken, ClientToken, WorkerToken};
use moor_common::{
    model::ObjectRef,
    schema::{
        convert::{
            obj_to_flatbuffer_struct, objectref_to_flatbuffer_struct, symbol_to_flatbuffer_struct,
            var_to_flatbuffer,
        },
        rpc, var,
    },
};
use moor_var::{Obj, Symbol, Var};

/// Create a FlatBuffer ClientToken from a reference (avoids moving the token)
#[inline]
pub fn client_token_fb(token: &ClientToken) -> Box<rpc::ClientToken> {
    Box::new(rpc::ClientToken {
        token: token.0.clone(),
    })
}

/// Create a FlatBuffer AuthToken from a reference (avoids moving the token)
#[inline]
pub fn auth_token_fb(token: &AuthToken) -> Box<rpc::AuthToken> {
    Box::new(rpc::AuthToken {
        token: token.0.clone(),
    })
}

/// Create a FlatBuffer WorkerToken from a reference (avoids moving the token)
#[inline]
pub fn mk_worker_token(token: &WorkerToken) -> Box<rpc::WorkerToken> {
    Box::new(rpc::WorkerToken {
        token: token.0.clone(),
    })
}

/// Create a boxed FlatBuffer Obj struct from an Obj reference
#[inline]
pub fn obj_fb(obj: &Obj) -> Box<rpc::Obj> {
    Box::new(obj_to_flatbuffer_struct(obj))
}

/// Create a boxed FlatBuffer Symbol struct from a Symbol reference
#[inline]
pub fn symbol_fb(symbol: &Symbol) -> Box<rpc::Symbol> {
    Box::new(symbol_to_flatbuffer_struct(symbol))
}

/// Create a boxed FlatBuffer ObjectRef struct from an ObjectRef reference
#[inline]
pub fn objectref_fb(objref: &ObjectRef) -> Box<rpc::ObjectRef> {
    Box::new(objectref_to_flatbuffer_struct(objref))
}

/// Create a boxed FlatBuffer Var from a Var reference
/// Returns None if serialization fails
#[inline]
pub fn var_fb(var: &Var) -> Option<Box<var::Var>> {
    Some(Box::new(var_to_flatbuffer(var).ok()?))
}

/// Create a boxed FlatBuffer Uuid from a uuid::Uuid
#[inline]
pub fn uuid_fb(uuid: uuid::Uuid) -> Box<rpc::Uuid> {
    Box::new(rpc::Uuid {
        data: uuid.as_bytes().to_vec(),
    })
}

/// Create a FlatBuffer string list from an iterator of strings (moves strings)
#[inline]
pub fn string_list_fb(strings: impl IntoIterator<Item = String>) -> Vec<String> {
    strings.into_iter().collect()
}

/// Create a FlatBuffer symbol list from an iterator of Symbols
#[inline]
pub fn symbol_list_fb(symbols: impl IntoIterator<Item = Symbol>) -> Vec<rpc::Symbol> {
    symbols
        .into_iter()
        .map(|s| rpc::Symbol {
            value: s.as_string(),
        })
        .collect()
}
