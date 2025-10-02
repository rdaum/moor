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

//! Conversion between moor_var::Var and FlatBuffers Var representation

use crate::schema::{convert_common, convert_errors, var};
use moor_var::{
    Var, Variant, v_binary, v_bool, v_empty_list, v_error, v_float, v_flyweight, v_int, v_list,
    v_map, v_none, v_obj, v_str, v_sym,
};
use thiserror::Error;
use var::VarUnion;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum VarConversionError {
    #[error("Lambda values cannot be transmitted over RPC")]
    LambdaNotTransmittable,

    #[error("Anonymous object references cannot be transmitted over RPC")]
    AnonymousObjectNotTransmittable,

    #[error("Failed to decode FlatBuffer Var: {0}")]
    DecodingError(String),

    #[error("Failed to encode Var to FlatBuffer: {0}")]
    EncodingError(String),
}

/// Convert from moor_var::Var to FlatBuffer Var struct
pub fn var_to_flatbuffer(v: &Var) -> Result<var::Var, VarConversionError> {
    let variant_union = match v.variant() {
        Variant::None => VarUnion::VarNone(Box::new(var::VarNone {})),

        Variant::Bool(b) => VarUnion::VarBool(Box::new(var::VarBool { value: *b })),

        Variant::Int(i) => VarUnion::VarInt(Box::new(var::VarInt { value: *i })),

        Variant::Float(f) => VarUnion::VarFloat(Box::new(var::VarFloat { value: *f })),

        Variant::Str(s) => VarUnion::VarStr(Box::new(var::VarStr {
            value: s.as_str().to_string(),
        })),

        Variant::Obj(obj) => {
            if obj.is_anonymous() {
                return Err(VarConversionError::AnonymousObjectNotTransmittable);
            }
            VarUnion::VarObj(Box::new(var::VarObj {
                obj: Box::new(convert_common::obj_to_flatbuffer_struct(obj)),
            }))
        }

        Variant::Err(e) => {
            let fb_error = convert_errors::error_to_flatbuffer_struct(e)
                .map_err(|err| VarConversionError::EncodingError(err.to_string()))?;
            VarUnion::VarErr(Box::new(var::VarErr {
                error: Box::new(fb_error),
            }))
        }

        Variant::Sym(sym) => VarUnion::VarSym(Box::new(var::VarSym {
            symbol: Box::new(convert_common::symbol_to_flatbuffer_struct(sym)),
        })),

        Variant::Binary(b) => VarUnion::VarBinary(Box::new(var::VarBinary {
            data: b.as_bytes().to_vec(),
        })),

        Variant::List(l) => {
            let elements: Result<Vec<_>, _> =
                l.iter().map(|elem| var_to_flatbuffer(&elem)).collect();
            VarUnion::VarList(Box::new(var::VarList {
                elements: elements?,
            }))
        }

        Variant::Map(m) => {
            let pairs: Result<Vec<_>, _> = m
                .iter()
                .map(|(k, v)| {
                    Ok(var::VarMapPair {
                        key: Box::new(var_to_flatbuffer(&k)?),
                        value: Box::new(var_to_flatbuffer(&v)?),
                    })
                })
                .collect();
            VarUnion::VarMap(Box::new(var::VarMap { pairs: pairs? }))
        }

        Variant::Flyweight(f) => {
            let slots: Result<Vec<_>, _> = f
                .slots()
                .iter()
                .map(|(name, value)| {
                    Ok(var::FlyweightSlot {
                        name: Box::new(convert_common::symbol_to_flatbuffer_struct(name)),
                        value: Box::new(var_to_flatbuffer(value)?),
                    })
                })
                .collect();

            let contents_elements: Result<Vec<_>, _> = f
                .contents()
                .iter()
                .map(|elem| var_to_flatbuffer(&elem))
                .collect();

            VarUnion::VarFlyweight(Box::new(var::VarFlyweight {
                delegate: Box::new(convert_common::obj_to_flatbuffer_struct(f.delegate())),
                slots: slots?,
                contents: Box::new(var::VarList {
                    elements: contents_elements?,
                }),
            }))
        }

        Variant::Lambda(_) => {
            return Err(VarConversionError::LambdaNotTransmittable);
        }
    };

    Ok(var::Var {
        variant: variant_union,
    })
}

/// Convert from FlatBuffer Var struct to moor_var::Var
pub fn var_from_flatbuffer(fb_var: &var::Var) -> Result<Var, VarConversionError> {
    match &fb_var.variant {
        VarUnion::VarNone(_) => Ok(v_none()),

        VarUnion::VarBool(b) => Ok(v_bool(b.value)),

        VarUnion::VarInt(i) => Ok(v_int(i.value)),

        VarUnion::VarFloat(f) => Ok(v_float(f.value)),

        VarUnion::VarStr(s) => Ok(v_str(&s.value)),

        VarUnion::VarObj(o) => {
            let obj = convert_common::obj_from_flatbuffer_struct(&o.obj)
                .map_err(|e| VarConversionError::DecodingError(e.to_string()))?;
            Ok(v_obj(obj))
        }

        VarUnion::VarErr(e) => {
            let error = convert_errors::error_from_flatbuffer_struct(&e.error)
                .map_err(|e| VarConversionError::DecodingError(e.to_string()))?;
            Ok(v_error(error))
        }

        VarUnion::VarSym(s) => {
            let sym = convert_common::symbol_from_flatbuffer_struct(&s.symbol);
            Ok(v_sym(sym))
        }

        VarUnion::VarBinary(b) => Ok(v_binary(b.data.clone())),

        VarUnion::VarList(l) => {
            if l.elements.is_empty() {
                return Ok(v_empty_list());
            }
            let elements: Result<Vec<_>, _> = l.elements.iter().map(var_from_flatbuffer).collect();
            Ok(v_list(&elements?))
        }

        VarUnion::VarMap(m) => {
            let pairs: Result<Vec<_>, _> = m
                .pairs
                .iter()
                .map(|pair| {
                    Ok((
                        var_from_flatbuffer(&pair.key)?,
                        var_from_flatbuffer(&pair.value)?,
                    ))
                })
                .collect();
            Ok(v_map(&pairs?))
        }

        VarUnion::VarFlyweight(f) => {
            let delegate = convert_common::obj_from_flatbuffer_struct(&f.delegate)
                .map_err(|e| VarConversionError::DecodingError(e.to_string()))?;

            let slots: Result<Vec<_>, _> = f
                .slots
                .iter()
                .map(|slot| {
                    Ok((
                        convert_common::symbol_from_flatbuffer_struct(&slot.name),
                        var_from_flatbuffer(&slot.value)?,
                    ))
                })
                .collect();

            let contents_elements: Result<Vec<_>, _> = f
                .contents
                .elements
                .iter()
                .map(var_from_flatbuffer)
                .collect();

            let contents_var = v_list(&contents_elements?);
            let contents = contents_var.as_list().ok_or_else(|| {
                VarConversionError::DecodingError("Failed to convert list".to_string())
            })?;

            Ok(v_flyweight(delegate, &slots?, contents.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{
        E_PERM, Obj, v_bool, v_empty_list, v_empty_str, v_err, v_float, v_int, v_none, v_obj, v_str,
    };

    #[test]
    fn test_none_roundtrip() {
        let var = v_none();
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_bool_roundtrip() {
        let var = v_bool(true);
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_int_roundtrip() {
        let var = v_int(42);
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_float_roundtrip() {
        let var = v_float(3.14);
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_string_roundtrip() {
        let var = v_str("hello world");
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_empty_string_roundtrip() {
        let var = v_empty_str();
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_obj_roundtrip() {
        let var = v_obj(Obj::mk_id(123));
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_error_roundtrip() {
        let var = v_err(E_PERM);
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_empty_list_roundtrip() {
        let var = v_empty_list();
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_symbol_roundtrip() {
        use moor_var::{Symbol, v_sym};
        let var = v_sym(Symbol::mk("test_symbol"));
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_binary_roundtrip() {
        use moor_var::v_binary;
        let var = v_binary(vec![1, 2, 3, 4, 5]);
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_nested_list_roundtrip() {
        use moor_var::v_list;
        let inner = v_list(&[v_int(1), v_int(2), v_int(3)]);
        let outer = v_list(&[inner.clone(), v_str("test"), inner]);
        let fb = var_to_flatbuffer(&outer).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(outer, decoded);
    }

    #[test]
    fn test_map_roundtrip() {
        use moor_var::v_map;
        let map = v_map(&[
            (v_str("key1"), v_int(42)),
            (v_str("key2"), v_str("value2")),
            (v_int(3), v_bool(true)),
        ]);
        let fb = var_to_flatbuffer(&map).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(map, decoded);
    }

    #[test]
    fn test_complex_nested_structure() {
        use moor_var::{v_list, v_map};
        let inner_list = v_list(&[v_int(1), v_int(2), v_int(3)]);
        let inner_map = v_map(&[(v_str("a"), v_int(1)), (v_str("b"), v_int(2))]);
        let outer = v_list(&[inner_list, inner_map, v_str("test")]);
        let fb = var_to_flatbuffer(&outer).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(outer, decoded);
    }

    #[test]
    fn test_uuobjid_roundtrip() {
        let obj = Obj::mk_uuobjid_generated();
        let var = v_obj(obj);
        let fb = var_to_flatbuffer(&var).unwrap();
        let decoded = var_from_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_lambda_rejected() {
        // TODO: Need to construct a lambda Var to test rejection
        // let var = Var::mk_lambda(...);
        // let result = var_to_flatbuffer(&var);
        // assert!(matches!(result, Err(VarConversionError::LambdaNotTransmittable)));
    }

    #[test]
    fn test_anonymous_object_rejected() {
        let obj = Obj::mk_anonymous_generated();
        let var = v_obj(obj);
        let result = var_to_flatbuffer(&var);
        assert!(matches!(
            result,
            Err(VarConversionError::AnonymousObjectNotTransmittable)
        ));
    }

    #[test]
    fn test_anonymous_object_in_list_rejected() {
        use moor_var::v_list;
        let obj = Obj::mk_anonymous_generated();
        let list = v_list(&[v_int(1), v_obj(obj), v_str("test")]);
        let result = var_to_flatbuffer(&list);
        assert!(matches!(
            result,
            Err(VarConversionError::AnonymousObjectNotTransmittable)
        ));
    }
}
