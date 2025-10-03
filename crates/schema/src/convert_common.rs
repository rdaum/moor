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

use moor_common::model::ObjectRef;
use moor_var::{Obj, Symbol, Var};

use crate::{common, var};

/// Convert from moor_var::Symbol to flatbuffer Symbol
pub fn symbol_to_flatbuffer_struct(symbol: &Symbol) -> common::Symbol {
    common::Symbol {
        value: symbol.as_string(),
    }
}

/// Convert from flatbuffer Symbol to moor_var::Symbol
pub fn symbol_from_flatbuffer_struct(symbol_string: &common::Symbol) -> Symbol {
    Symbol::mk(&symbol_string.value)
}

/// Convert from moor_common::model::ObjectRef to flatbuffer ObjectRef
pub fn objectref_to_flatbuffer_struct(objref: &ObjectRef) -> common::ObjectRef {
    match objref {
        ObjectRef::Id(obj) => common::ObjectRef {
            ref_: common::ObjectRefUnion::ObjectRefId(Box::new(common::ObjectRefId {
                obj: Box::new(obj_to_flatbuffer_struct(obj)),
            })),
        },
        ObjectRef::SysObj(symbols) => common::ObjectRef {
            ref_: common::ObjectRefUnion::ObjectRefSysObj(Box::new(common::ObjectRefSysObj {
                symbols: symbols.iter().map(symbol_to_flatbuffer_struct).collect(),
            })),
        },
        ObjectRef::Match(s) => common::ObjectRef {
            ref_: common::ObjectRefUnion::ObjectRefMatch(Box::new(common::ObjectRefMatch {
                match_string: s.clone(),
            })),
        },
    }
}

/// Convert from moor_var::Obj to flatbuffer Obj struct
pub fn obj_to_flatbuffer_struct(obj: &Obj) -> common::Obj {
    if obj.is_anonymous() {
        let anonymous_id = obj.anonymous_objid().unwrap();
        let (autoincrement, rng, epoch_ms) = anonymous_id.components();
        // Pack the components back into the 62-bit value
        let packed_value = ((autoincrement as u64) << 46) | ((rng as u64) << 40) | epoch_ms;

        common::Obj {
            obj: common::ObjUnion::AnonymousObjId(Box::new(common::AnonymousObjId {
                packed_value,
            })),
        }
    } else if obj.is_uuobjid() {
        let uuobj_id = obj.uuobjid().unwrap();
        let (autoincrement, rng, epoch_ms) = uuobj_id.components();
        // Pack the components back into the 62-bit value
        let packed_value = ((autoincrement as u64) << 46) | ((rng as u64) << 40) | epoch_ms;

        common::Obj {
            obj: common::ObjUnion::UuObjId(Box::new(common::UuObjId { packed_value })),
        }
    } else {
        common::Obj {
            obj: common::ObjUnion::ObjId(Box::new(common::ObjId { id: obj.id().0 })),
        }
    }
}

/// Convert from flatbuffer Obj struct to moor_var::Obj
pub fn obj_from_flatbuffer_struct(fb_obj: &common::Obj) -> Result<Obj, Box<dyn std::error::Error>> {
    match &fb_obj.obj {
        common::ObjUnion::ObjId(obj_id) => Ok(Obj::mk_id(obj_id.id)),
        common::ObjUnion::UuObjId(uuobj_id) => {
            let packed_value = uuobj_id.packed_value;
            let autoincrement = ((packed_value >> 46) & 0xFFFF) as u16;
            let rng = ((packed_value >> 40) & 0x3F) as u8;
            let epoch_ms = packed_value & 0x00FF_FFFF_FFFF;

            let uuid = moor_var::UuObjid::new(autoincrement, rng, epoch_ms);
            Ok(Obj::mk_uuobjid(uuid))
        }
        common::ObjUnion::AnonymousObjId(anon_id) => {
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
pub fn uuid_to_flatbuffer_struct(uuid: &uuid::Uuid) -> common::Uuid {
    common::Uuid {
        data: uuid.as_bytes().to_vec(),
    }
}

/// Convert from FlatBuffer UuidRef to uuid::Uuid
pub fn uuid_from_ref(uuid_ref: common::UuidRef<'_>) -> Result<uuid::Uuid, String> {
    let data = uuid_ref.data().map_err(|_| "Missing UUID data")?;
    uuid::Uuid::from_slice(data).map_err(|e| format!("Invalid UUID data: {e}"))
}

/// Convert from FlatBuffer SymbolRef to moor_var::Symbol
pub fn symbol_from_ref(symbol_ref: common::SymbolRef<'_>) -> Result<Symbol, String> {
    let value = symbol_ref.value().map_err(|_| "Missing symbol value")?;
    Ok(Symbol::mk(value))
}

/// Convert from FlatBuffer VarBytesRef to moor_var::Var
pub fn var_from_ref(var_ref: var::VarRef<'_>) -> Result<Var, String> {
    let var_struct = var::Var::try_from(var_ref).map_err(|_| "Failed to convert var ref")?;
    crate::convert_var::var_from_flatbuffer(&var_struct)
        .map_err(|e| format!("Failed to decode var: {e}"))
}

/// Convert from FlatBuffer ObjRef to moor_var::Obj
pub fn obj_from_ref(obj_ref: common::ObjRef<'_>) -> Result<Obj, String> {
    match obj_ref.obj().map_err(|_| "Failed to read obj union")? {
        common::ObjUnionRef::ObjId(obj_id) => {
            let id = obj_id.id().map_err(|_| "Failed to read obj id")?;
            Ok(Obj::mk_id(id))
        }
        common::ObjUnionRef::UuObjId(uuobj_id) => {
            let packed_value = uuobj_id
                .packed_value()
                .map_err(|_| "Failed to read packed_value")?;
            let autoincrement = ((packed_value >> 46) & 0xFFFF) as u16;
            let rng = ((packed_value >> 40) & 0x3F) as u8;
            let epoch_ms = packed_value & 0x00FF_FFFF_FFFF;

            let uuid = moor_var::UuObjid::new(autoincrement, rng, epoch_ms);
            Ok(Obj::mk_uuobjid(uuid))
        }
        common::ObjUnionRef::AnonymousObjId(anon_id) => {
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
pub fn objectref_from_ref(objref: common::ObjectRefRef<'_>) -> Result<ObjectRef, String> {
    match objref
        .ref_()
        .map_err(|_| "Failed to read ObjectRef union")?
    {
        common::ObjectRefUnionRef::ObjectRefId(id_ref) => {
            let obj_ref = id_ref.obj().map_err(|_| "Missing obj in ObjectRefId")?;
            let obj = obj_from_ref(obj_ref)?;
            Ok(ObjectRef::Id(obj))
        }
        common::ObjectRefUnionRef::ObjectRefSysObj(sysobj_ref) => {
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
        common::ObjectRefUnionRef::ObjectRefMatch(match_ref) => {
            let match_string = match_ref
                .match_string()
                .map_err(|_| "Missing match_string in ObjectRefMatch")?
                .to_string();
            Ok(ObjectRef::Match(match_string))
        }
    }
}
