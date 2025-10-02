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

//! Generic type conversions between moor types and FlatBuffer types
//!
//! This module handles conversion of basic types like Var, Obj, Symbol, Uuid, Error
//! that are used across all message types.

use moor_common::{model::ObjectRef, schema::rpc};
use moor_var::{AsByteBuffer, Obj, Symbol, Var};

/// Convert from moor_var::Var to flatbuffer VarBytes (serialized)
pub fn var_to_flatbuffer_bytes(var: &Var) -> Result<Vec<u8>, moor_var::EncodingError> {
    var.make_copy_as_vec()
}

/// Convert from flatbuffer VarBytes data to moor_var::Var (deserialized)
pub fn var_from_flatbuffer_bytes(data: &[u8]) -> Result<Var, moor_var::DecodingError> {
    let bytes = byteview::ByteView::from(data.to_vec());
    Var::from_bytes(bytes)
}

/// Convert from moor_var::Symbol to flatbuffer Symbol
pub fn symbol_to_flatbuffer_struct(symbol: &Symbol) -> rpc::Symbol {
    rpc::Symbol {
        value: symbol.as_string(),
    }
}

/// Convert from flatbuffer Symbol to moor_var::Symbol
pub fn symbol_from_flatbuffer_struct(symbol_string: &rpc::Symbol) -> Symbol {
    Symbol::mk(&symbol_string.value)
}

/// Convert from moor_common::model::ObjectRef to flatbuffer ObjectRef
pub fn objectref_to_flatbuffer_struct(objref: &ObjectRef) -> rpc::ObjectRef {
    match objref {
        ObjectRef::Id(obj) => rpc::ObjectRef {
            ref_: rpc::ObjectRefUnion::ObjectRefId(Box::new(rpc::ObjectRefId {
                obj: Box::new(obj_to_flatbuffer_struct(obj)),
            })),
        },
        ObjectRef::SysObj(symbols) => rpc::ObjectRef {
            ref_: rpc::ObjectRefUnion::ObjectRefSysObj(Box::new(rpc::ObjectRefSysObj {
                symbols: symbols.iter().map(symbol_to_flatbuffer_struct).collect(),
            })),
        },
        ObjectRef::Match(s) => rpc::ObjectRef {
            ref_: rpc::ObjectRefUnion::ObjectRefMatch(Box::new(rpc::ObjectRefMatch {
                match_string: s.clone(),
            })),
        },
    }
}

/// Convert from moor_var::Obj to flatbuffer Obj struct
pub fn obj_to_flatbuffer_struct(obj: &Obj) -> rpc::Obj {
    if obj.is_anonymous() {
        let anonymous_id = obj.anonymous_objid().unwrap();
        let (autoincrement, rng, epoch_ms) = anonymous_id.components();
        // Pack the components back into the 62-bit value
        let packed_value = ((autoincrement as u64) << 46) | ((rng as u64) << 40) | epoch_ms;

        rpc::Obj {
            obj: rpc::ObjUnion::AnonymousObjId(Box::new(rpc::AnonymousObjId { packed_value })),
        }
    } else if obj.is_uuobjid() {
        let uuobj_id = obj.uuobjid().unwrap();
        let (autoincrement, rng, epoch_ms) = uuobj_id.components();
        // Pack the components back into the 62-bit value
        let packed_value = ((autoincrement as u64) << 46) | ((rng as u64) << 40) | epoch_ms;

        rpc::Obj {
            obj: rpc::ObjUnion::UuObjId(Box::new(rpc::UuObjId { packed_value })),
        }
    } else {
        rpc::Obj {
            obj: rpc::ObjUnion::ObjId(Box::new(rpc::ObjId { id: obj.id().0 })),
        }
    }
}

/// Convert from flatbuffer Obj struct to moor_var::Obj
pub fn obj_from_flatbuffer_struct(fb_obj: &rpc::Obj) -> Result<Obj, Box<dyn std::error::Error>> {
    match &fb_obj.obj {
        rpc::ObjUnion::ObjId(obj_id) => Ok(Obj::mk_id(obj_id.id)),
        rpc::ObjUnion::UuObjId(uuobj_id) => {
            let packed_value = uuobj_id.packed_value;
            let autoincrement = ((packed_value >> 46) & 0xFFFF) as u16;
            let rng = ((packed_value >> 40) & 0x3F) as u8;
            let epoch_ms = packed_value & 0x00FF_FFFF_FFFF;

            let uuid = moor_var::UuObjid::new(autoincrement, rng, epoch_ms);
            Ok(Obj::mk_uuobjid(uuid))
        }
        rpc::ObjUnion::AnonymousObjId(anon_id) => {
            let packed_value = anon_id.packed_value;
            let autoincrement = ((packed_value >> 46) & 0xFFFF) as u16;
            let rng = ((packed_value >> 40) & 0x3F) as u8;
            let epoch_ms = packed_value & 0x00FF_FFFF_FFFF;

            let anonymous = moor_var::AnonymousObjid::new(autoincrement, rng, epoch_ms);
            Ok(Obj::mk_anonymous(anonymous))
        }
    }
}

/// Convert uuid::Uuid to FlatBuffer Uuid struct
pub fn uuid_to_flatbuffer_struct(uuid: &uuid::Uuid) -> rpc::Uuid {
    rpc::Uuid {
        data: uuid.as_bytes().to_vec(),
    }
}

/// Convert from FlatBuffer UuidRef to uuid::Uuid
pub fn uuid_from_ref(uuid_ref: rpc::UuidRef<'_>) -> Result<uuid::Uuid, String> {
    let data = uuid_ref.data().map_err(|_| "Missing UUID data")?;
    uuid::Uuid::from_slice(data).map_err(|e| format!("Invalid UUID data: {e}"))
}

/// Convert from FlatBuffer SymbolRef to moor_var::Symbol
pub fn symbol_from_ref(symbol_ref: rpc::SymbolRef<'_>) -> Result<Symbol, String> {
    let value = symbol_ref.value().map_err(|_| "Missing symbol value")?;
    Ok(Symbol::mk(value))
}

/// Convert from FlatBuffer VarBytesRef to moor_var::Var
pub fn var_from_ref(var_ref: rpc::VarBytesRef<'_>) -> Result<Var, String> {
    let data = var_ref.data().map_err(|_| "Missing var data")?;
    var_from_flatbuffer_bytes(data).map_err(|e| format!("Failed to decode var: {e}"))
}

/// Convert from FlatBuffer ObjRef to moor_var::Obj
pub fn obj_from_ref(obj_ref: rpc::ObjRef<'_>) -> Result<Obj, String> {
    match obj_ref.obj().map_err(|_| "Failed to read obj union")? {
        rpc::ObjUnionRef::ObjId(obj_id) => {
            let id = obj_id.id().map_err(|_| "Failed to read obj id")?;
            Ok(Obj::mk_id(id))
        }
        rpc::ObjUnionRef::UuObjId(uuobj_id) => {
            let packed_value = uuobj_id
                .packed_value()
                .map_err(|_| "Failed to read packed_value")?;
            let autoincrement = ((packed_value >> 46) & 0xFFFF) as u16;
            let rng = ((packed_value >> 40) & 0x3F) as u8;
            let epoch_ms = packed_value & 0x00FF_FFFF_FFFF;

            let uuid = moor_var::UuObjid::new(autoincrement, rng, epoch_ms);
            Ok(Obj::mk_uuobjid(uuid))
        }
        rpc::ObjUnionRef::AnonymousObjId(anon_id) => {
            let packed_value = anon_id
                .packed_value()
                .map_err(|_| "Failed to read packed_value")?;
            let autoincrement = ((packed_value >> 46) & 0xFFFF) as u16;
            let rng = ((packed_value >> 40) & 0x3F) as u8;
            let epoch_ms = packed_value & 0x00FF_FFFF_FFFF;

            let anonymous = moor_var::AnonymousObjid::new(autoincrement, rng, epoch_ms);
            Ok(Obj::mk_anonymous(anonymous))
        }
    }
}

/// Convert from FlatBuffer ObjectRefRef to moor_common::model::ObjectRef
pub fn objectref_from_ref(objref: rpc::ObjectRefRef<'_>) -> Result<ObjectRef, String> {
    match objref
        .ref_()
        .map_err(|_| "Failed to read ObjectRef union")?
    {
        rpc::ObjectRefUnionRef::ObjectRefId(id_ref) => {
            let obj_ref = id_ref.obj().map_err(|_| "Missing obj in ObjectRefId")?;
            let obj = obj_from_ref(obj_ref)?;
            Ok(ObjectRef::Id(obj))
        }
        rpc::ObjectRefUnionRef::ObjectRefSysObj(sysobj_ref) => {
            let symbols_vec = sysobj_ref
                .symbols()
                .map_err(|_| "Missing symbols in ObjectRefSysObj")?;
            let mut symbols = Vec::new();
            for s in symbols_vec.iter() {
                let s = s.map_err(|_| "Failed to read symbol")?;
                symbols.push(symbol_from_ref(s)?);
            }
            Ok(ObjectRef::SysObj(symbols))
        }
        rpc::ObjectRefUnionRef::ObjectRefMatch(match_ref) => {
            let match_string = match_ref
                .match_string()
                .map_err(|_| "Missing match_string in ObjectRefMatch")?
                .to_string();
            Ok(ObjectRef::Match(match_string))
        }
    }
}
