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

use crate::flatbuffers_generated::moor_rpc;
use moor_common::model::ObjectRef;
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
pub fn symbol_to_flatbuffer_struct(symbol: &Symbol) -> moor_rpc::Symbol {
    moor_rpc::Symbol {
        value: symbol.as_string(),
    }
}

/// Convert from flatbuffer Symbol to moor_var::Symbol
pub fn symbol_from_flatbuffer_struct(symbol_string: &moor_rpc::Symbol) -> Symbol {
    Symbol::mk(&symbol_string.value)
}

/// Convert from moor_common::model::ObjectRef to flatbuffer ObjectRef
pub fn objectref_to_flatbuffer_struct(objref: &ObjectRef) -> moor_rpc::ObjectRef {
    match objref {
        ObjectRef::Id(obj) => moor_rpc::ObjectRef {
            ref_: moor_rpc::ObjectRefUnion::ObjectRefId(Box::new(moor_rpc::ObjectRefId {
                obj: Box::new(obj_to_flatbuffer_struct(obj)),
            })),
        },
        ObjectRef::SysObj(symbols) => moor_rpc::ObjectRef {
            ref_: moor_rpc::ObjectRefUnion::ObjectRefSysObj(Box::new(moor_rpc::ObjectRefSysObj {
                symbols: symbols.iter().map(symbol_to_flatbuffer_struct).collect(),
            })),
        },
        ObjectRef::Match(s) => moor_rpc::ObjectRef {
            ref_: moor_rpc::ObjectRefUnion::ObjectRefMatch(Box::new(moor_rpc::ObjectRefMatch {
                match_string: s.clone(),
            })),
        },
    }
}

/// Convert from moor_var::Obj to flatbuffer Obj struct
pub fn obj_to_flatbuffer_struct(obj: &Obj) -> moor_rpc::Obj {
    if obj.is_anonymous() {
        let anonymous_id = obj.anonymous_objid().unwrap();
        let (autoincrement, rng, epoch_ms) = anonymous_id.components();
        // Pack the components back into the 62-bit value
        let packed_value = ((autoincrement as u64) << 46) | ((rng as u64) << 40) | epoch_ms;

        moor_rpc::Obj {
            obj: moor_rpc::ObjUnion::AnonymousObjId(Box::new(moor_rpc::AnonymousObjId {
                packed_value,
            })),
        }
    } else if obj.is_uuobjid() {
        let uuobj_id = obj.uuobjid().unwrap();
        let (autoincrement, rng, epoch_ms) = uuobj_id.components();
        // Pack the components back into the 62-bit value
        let packed_value = ((autoincrement as u64) << 46) | ((rng as u64) << 40) | epoch_ms;

        moor_rpc::Obj {
            obj: moor_rpc::ObjUnion::UuObjId(Box::new(moor_rpc::UuObjId { packed_value })),
        }
    } else {
        moor_rpc::Obj {
            obj: moor_rpc::ObjUnion::ObjId(Box::new(moor_rpc::ObjId { id: obj.id().0 })),
        }
    }
}

/// Convert from flatbuffer Obj struct to moor_var::Obj
pub fn obj_from_flatbuffer_struct(
    fb_obj: &moor_rpc::Obj,
) -> Result<Obj, Box<dyn std::error::Error>> {
    match &fb_obj.obj {
        moor_rpc::ObjUnion::ObjId(obj_id) => Ok(Obj::mk_id(obj_id.id)),
        moor_rpc::ObjUnion::UuObjId(uuobj_id) => {
            let packed_value = uuobj_id.packed_value;
            let autoincrement = ((packed_value >> 46) & 0xFFFF) as u16;
            let rng = ((packed_value >> 40) & 0x3F) as u8;
            let epoch_ms = packed_value & 0x00FF_FFFF_FFFF;

            let uuid = moor_var::UuObjid::new(autoincrement, rng, epoch_ms);
            Ok(Obj::mk_uuobjid(uuid))
        }
        moor_rpc::ObjUnion::AnonymousObjId(anon_id) => {
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
pub fn uuid_to_flatbuffer_struct(uuid: &uuid::Uuid) -> moor_rpc::Uuid {
    moor_rpc::Uuid {
        data: uuid.as_bytes().to_vec(),
    }
}

/// Convert from FlatBuffer UuidRef to uuid::Uuid
pub fn uuid_from_ref(uuid_ref: moor_rpc::UuidRef<'_>) -> Result<uuid::Uuid, String> {
    let data = uuid_ref.data().map_err(|_| "Missing UUID data")?;
    uuid::Uuid::from_slice(data).map_err(|e| format!("Invalid UUID data: {}", e))
}

/// Convert from FlatBuffer SymbolRef to moor_var::Symbol
pub fn symbol_from_ref(symbol_ref: moor_rpc::SymbolRef<'_>) -> Result<Symbol, String> {
    let value = symbol_ref.value().map_err(|_| "Missing symbol value")?;
    Ok(Symbol::mk(value))
}

/// Convert from FlatBuffer VarBytesRef to moor_var::Var
pub fn var_from_ref(var_ref: moor_rpc::VarBytesRef<'_>) -> Result<Var, String> {
    let data = var_ref.data().map_err(|_| "Missing var data")?;
    var_from_flatbuffer_bytes(data).map_err(|e| format!("Failed to decode var: {}", e))
}

/// Convert from FlatBuffer ObjRef to moor_var::Obj
pub fn obj_from_ref(obj_ref: moor_rpc::ObjRef<'_>) -> Result<Obj, String> {
    match obj_ref.obj().map_err(|_| "Failed to read obj union")? {
        moor_rpc::ObjUnionRef::ObjId(obj_id) => {
            let id = obj_id.id().map_err(|_| "Failed to read obj id")?;
            Ok(Obj::mk_id(id))
        }
        moor_rpc::ObjUnionRef::UuObjId(uuobj_id) => {
            let packed_value = uuobj_id
                .packed_value()
                .map_err(|_| "Failed to read packed_value")?;
            let autoincrement = ((packed_value >> 46) & 0xFFFF) as u16;
            let rng = ((packed_value >> 40) & 0x3F) as u8;
            let epoch_ms = packed_value & 0x00FF_FFFF_FFFF;

            let uuid = moor_var::UuObjid::new(autoincrement, rng, epoch_ms);
            Ok(Obj::mk_uuobjid(uuid))
        }
        moor_rpc::ObjUnionRef::AnonymousObjId(anon_id) => {
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
pub fn objectref_from_ref(objref: moor_rpc::ObjectRefRef<'_>) -> Result<ObjectRef, String> {
    match objref
        .ref_()
        .map_err(|_| "Failed to read ObjectRef union")?
    {
        moor_rpc::ObjectRefUnionRef::ObjectRefId(id_ref) => {
            let obj_ref = id_ref.obj().map_err(|_| "Missing obj in ObjectRefId")?;
            let obj = obj_from_ref(obj_ref)?;
            Ok(ObjectRef::Id(obj))
        }
        moor_rpc::ObjectRefUnionRef::ObjectRefSysObj(sysobj_ref) => {
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
        moor_rpc::ObjectRefUnionRef::ObjectRefMatch(match_ref) => {
            let match_string = match_ref
                .match_string()
                .map_err(|_| "Missing match_string in ObjectRefMatch")?
                .to_string();
            Ok(ObjectRef::Match(match_string))
        }
    }
}
