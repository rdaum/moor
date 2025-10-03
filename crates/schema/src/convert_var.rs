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

use crate::{convert_common, convert_errors, program as fb_program, var};
use moor_var::{
    Var, Variant,
    program::{
        names::Name,
        opcode::{ScatterArgs, ScatterLabel},
        program::Program,
    },
    v_binary, v_bool, v_empty_list, v_error, v_float, v_flyweight, v_int, v_list, v_map, v_none,
    v_obj, v_str, v_sym,
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

// ============================================================================
// Helper functions for lambda component conversion
// ============================================================================

/// Convert a Name to FlatBuffer StoredName
fn name_to_flatbuffer(name: &Name) -> fb_program::StoredName {
    fb_program::StoredName {
        offset: name.0,
        scope_depth: name.1,
        scope_id: name.2,
    }
}

/// Convert a FlatBuffer StoredName to Name
fn name_from_flatbuffer(stored: &fb_program::StoredName) -> Name {
    Name(stored.offset, stored.scope_depth, stored.scope_id)
}

/// Convert Program to FlatBuffer StoredProgram struct
fn program_to_flatbuffer(
    program: &Program,
    _context: ConversionContext,
) -> Result<fb_program::StoredProgram, VarConversionError> {
    use crate::convert_program::program_to_stored;
    use planus::ReadAsRoot;

    // Convert Program to StoredProgram (ByteView wrapper)
    // TODO: This still uses bincode for Var literals - we'll update convert_program next
    let stored = program_to_stored(program)
        .map_err(|e| VarConversionError::EncodingError(format!("Failed to encode program: {e}")))?;

    // Get bytes from StoredProgram wrapper
    let bytes = stored.as_bytes();

    // Parse bytes to get FlatBuffer struct reference
    let fb_ref = fb_program::StoredProgramRef::read_as_root(bytes).map_err(|e| {
        VarConversionError::DecodingError(format!("Failed to parse stored program: {e}"))
    })?;

    // Convert to owned struct using TryInto
    fb_ref.try_into().map_err(|e| {
        VarConversionError::DecodingError(format!("Failed to convert StoredProgramRef: {e}"))
    })
}

/// Convert FlatBuffer StoredProgram struct to Program
fn program_from_flatbuffer(
    stored: fb_program::StoredProgram,
) -> Result<Program, VarConversionError> {
    use crate::convert_program::stored_to_program;
    use byteview::ByteView;
    use moor_var::program::stored_program::StoredProgram;
    use planus::WriteAs;

    // Serialize the FlatBuffer struct to bytes
    let mut builder = planus::Builder::new();
    let offset = stored.prepare(&mut builder);
    let bytes = builder.finish(offset, None);

    // Wrap in StoredProgram
    let stored_wrapper = StoredProgram::from(ByteView::from(bytes));

    // Convert to Program
    // TODO: This still uses bincode for Var literals - we'll update convert_program next
    stored_to_program(&stored_wrapper)
        .map_err(|e| VarConversionError::DecodingError(format!("Failed to decode program: {e}")))
}

/// Convert ScatterArgs to FlatBuffer StoredScatterArgs
fn scatter_args_to_flatbuffer(args: &ScatterArgs) -> fb_program::StoredScatterArgs {
    let labels = args
        .labels
        .iter()
        .map(|label| {
            let fb_label = match label {
                ScatterLabel::Required(name) => {
                    fb_program::StoredScatterLabelUnion::StoredScatterRequired(Box::new(
                        fb_program::StoredScatterRequired {
                            name: Box::new(name_to_flatbuffer(name)),
                        },
                    ))
                }
                ScatterLabel::Optional(name, default_label) => {
                    fb_program::StoredScatterLabelUnion::StoredScatterOptional(Box::new(
                        fb_program::StoredScatterOptional {
                            name: Box::new(name_to_flatbuffer(name)),
                            default_label: default_label.map(|l| l.0).unwrap_or(0),
                            has_default: default_label.is_some(),
                        },
                    ))
                }
                ScatterLabel::Rest(name) => fb_program::StoredScatterLabelUnion::StoredScatterRest(
                    Box::new(fb_program::StoredScatterRest {
                        name: Box::new(name_to_flatbuffer(name)),
                    }),
                ),
            };
            fb_program::StoredScatterLabel { label: fb_label }
        })
        .collect();

    fb_program::StoredScatterArgs {
        labels,
        done: args.done.0,
    }
}

/// Convert FlatBuffer StoredScatterArgs to ScatterArgs
fn scatter_args_from_flatbuffer(
    stored: &fb_program::StoredScatterArgs,
) -> Result<ScatterArgs, VarConversionError> {
    use moor_var::program::labels::Label;

    let labels: Result<Vec<_>, _> = stored
        .labels
        .iter()
        .map(|stored_label| {
            let label = match &stored_label.label {
                fb_program::StoredScatterLabelUnion::StoredScatterRequired(req) => {
                    ScatterLabel::Required(name_from_flatbuffer(&req.name))
                }
                fb_program::StoredScatterLabelUnion::StoredScatterOptional(opt) => {
                    let default_label = if opt.has_default {
                        Some(Label(opt.default_label))
                    } else {
                        None
                    };
                    ScatterLabel::Optional(name_from_flatbuffer(&opt.name), default_label)
                }
                fb_program::StoredScatterLabelUnion::StoredScatterRest(rest) => {
                    ScatterLabel::Rest(name_from_flatbuffer(&rest.name))
                }
            };
            Ok(label)
        })
        .collect();

    Ok(ScatterArgs {
        labels: labels?,
        done: Label(stored.done),
    })
}

// ============================================================================
// Conversion context - determines whether to allow lambdas/anonymous objects
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionContext {
    Rpc,      // Reject lambdas and anonymous objects
    Database, // Allow everything
}

// ============================================================================
// Main conversion functions
// ============================================================================

/// Convert from moor_var::Var to FlatBuffer Var struct for RPC transmission.
/// Rejects lambdas and anonymous object references.
pub fn var_to_flatbuffer(v: &Var) -> Result<var::Var, VarConversionError> {
    var_to_flatbuffer_internal(v, ConversionContext::Rpc)
}

/// Convert from moor_var::Var to FlatBuffer Var struct for database storage.
/// Allows lambdas and anonymous object references.
pub fn var_to_db_flatbuffer(v: &Var) -> Result<var::Var, VarConversionError> {
    var_to_flatbuffer_internal(v, ConversionContext::Database)
}

/// Internal conversion function with context
/// Exposed as pub(crate) so program_convert can use it for literals
pub fn var_to_flatbuffer_internal(
    v: &Var,
    context: ConversionContext,
) -> Result<var::Var, VarConversionError> {
    let variant_union = match v.variant() {
        Variant::None => VarUnion::VarNone(Box::new(var::VarNone {})),

        Variant::Bool(b) => VarUnion::VarBool(Box::new(var::VarBool { value: *b })),

        Variant::Int(i) => VarUnion::VarInt(Box::new(var::VarInt { value: *i })),

        Variant::Float(f) => VarUnion::VarFloat(Box::new(var::VarFloat { value: *f })),

        Variant::Str(s) => VarUnion::VarStr(Box::new(var::VarStr {
            value: s.as_str().to_string(),
        })),

        Variant::Obj(obj) => {
            if obj.is_anonymous() && context == ConversionContext::Rpc {
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
            let elements: Result<Vec<_>, _> = l
                .iter()
                .map(|elem| var_to_flatbuffer_internal(&elem, context))
                .collect();
            VarUnion::VarList(Box::new(var::VarList {
                elements: elements?,
            }))
        }

        Variant::Map(m) => {
            let pairs: Result<Vec<_>, _> = m
                .iter()
                .map(|(k, v)| {
                    Ok(var::VarMapPair {
                        key: Box::new(var_to_flatbuffer_internal(&k, context)?),
                        value: Box::new(var_to_flatbuffer_internal(&v, context)?),
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
                        value: Box::new(var_to_flatbuffer_internal(value, context)?),
                    })
                })
                .collect();

            let contents_elements: Result<Vec<_>, _> = f
                .contents()
                .iter()
                .map(|elem| var_to_flatbuffer_internal(&elem, context))
                .collect();

            VarUnion::VarFlyweight(Box::new(var::VarFlyweight {
                delegate: Box::new(convert_common::obj_to_flatbuffer_struct(f.delegate())),
                slots: slots?,
                contents: Box::new(var::VarList {
                    elements: contents_elements?,
                }),
            }))
        }

        Variant::Lambda(lambda) => {
            if context == ConversionContext::Rpc {
                return Err(VarConversionError::LambdaNotTransmittable);
            }

            // Convert lambda components
            let params = scatter_args_to_flatbuffer(&lambda.0.params);
            let body = program_to_flatbuffer(&lambda.0.body, context)?;

            // Convert captured_env: Vec<Vec<Var>>
            let captured_env_lists: Result<Vec<_>, _> = lambda
                .0
                .captured_env
                .iter()
                .map(|frame| {
                    let elements: Result<Vec<_>, _> = frame
                        .iter()
                        .map(|v| var_to_flatbuffer_internal(v, context))
                        .collect();
                    Ok(var::VarList {
                        elements: elements?,
                    })
                })
                .collect();

            // Convert optional self_var
            let self_var = lambda
                .0
                .self_var
                .as_ref()
                .map(|name| Box::new(name_to_flatbuffer(name)));

            VarUnion::VarLambda(Box::new(var::VarLambda {
                params: Box::new(params),
                body: Box::new(body),
                captured_env: captured_env_lists?,
                self_var,
            }))
        }
    };

    Ok(var::Var {
        variant: variant_union,
    })
}

/// Convert from FlatBuffer Var struct to moor_var::Var for RPC transmission.
/// Rejects lambdas and anonymous object references.
pub fn var_from_flatbuffer(fb_var: &var::Var) -> Result<Var, VarConversionError> {
    var_from_flatbuffer_internal(fb_var, ConversionContext::Rpc)
}

/// Convert from FlatBuffer Var struct to moor_var::Var for database storage.
/// Allows lambdas and anonymous object references.
pub fn var_from_db_flatbuffer(fb_var: &var::Var) -> Result<Var, VarConversionError> {
    var_from_flatbuffer_internal(fb_var, ConversionContext::Database)
}

/// Internal conversion function with context
/// Exposed as pub(crate) so program_convert can use it for literals
pub fn var_from_flatbuffer_internal(
    fb_var: &var::Var,
    context: ConversionContext,
) -> Result<Var, VarConversionError> {
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
            let elements: Result<Vec<_>, _> = l
                .elements
                .iter()
                .map(|e| var_from_flatbuffer_internal(e, context))
                .collect();
            Ok(v_list(&elements?))
        }

        VarUnion::VarMap(m) => {
            let pairs: Result<Vec<_>, _> = m
                .pairs
                .iter()
                .map(|pair| {
                    Ok((
                        var_from_flatbuffer_internal(&pair.key, context)?,
                        var_from_flatbuffer_internal(&pair.value, context)?,
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
                        var_from_flatbuffer_internal(&slot.value, context)?,
                    ))
                })
                .collect();

            let contents_elements: Result<Vec<_>, _> = f
                .contents
                .elements
                .iter()
                .map(|e| var_from_flatbuffer_internal(e, context))
                .collect();

            let contents_var = v_list(&contents_elements?);
            let contents = contents_var.as_list().ok_or_else(|| {
                VarConversionError::DecodingError("Failed to convert list".to_string())
            })?;

            Ok(v_flyweight(delegate, &slots?, contents.clone()))
        }

        VarUnion::VarLambda(lambda) => {
            // Lambdas should never appear in RPC context
            if context == ConversionContext::Rpc {
                return Err(VarConversionError::DecodingError(
                    "Unexpected lambda in RPC context".to_string(),
                ));
            }

            // Convert params
            let params = scatter_args_from_flatbuffer(&lambda.params)?;

            // Convert body (clone and deref Box)
            let body = program_from_flatbuffer((*lambda.body).clone())?;

            // Convert captured_env: [VarList] -> Vec<Vec<Var>>
            let captured_env: Result<Vec<Vec<Var>>, _> = lambda
                .captured_env
                .iter()
                .map(|frame| {
                    frame
                        .elements
                        .iter()
                        .map(|v| var_from_flatbuffer_internal(v, context))
                        .collect()
                })
                .collect();

            // Convert optional self_var
            let self_var = lambda.self_var.as_ref().map(|n| name_from_flatbuffer(n));

            // Create lambda using Var::mk_lambda
            use moor_var::Var;
            Ok(Var::mk_lambda(params, body, captured_env?, self_var))
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
        let var = v_float(42.5);
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

    #[test]
    fn test_anonymous_object_db_roundtrip() {
        // Anonymous objects should roundtrip successfully in DB context
        let obj = Obj::mk_anonymous_generated();
        let var = v_obj(obj);
        let fb = var_to_db_flatbuffer(&var).unwrap();
        let decoded = var_from_db_flatbuffer(&fb).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn test_anonymous_object_in_list_db_roundtrip() {
        use moor_var::v_list;
        let obj = Obj::mk_anonymous_generated();
        let list = v_list(&[v_int(1), v_obj(obj), v_str("test")]);
        let fb = var_to_db_flatbuffer(&list).unwrap();
        let decoded = var_from_db_flatbuffer(&fb).unwrap();
        assert_eq!(list, decoded);
    }

    #[test]
    fn test_db_and_rpc_functions_are_separate() {
        // Verify that RPC functions still reject what they should
        let anon_obj = Obj::mk_anonymous_generated();
        let var = v_obj(anon_obj);

        // RPC should reject
        assert!(var_to_flatbuffer(&var).is_err());

        // DB should accept
        assert!(var_to_db_flatbuffer(&var).is_ok());
    }
}
