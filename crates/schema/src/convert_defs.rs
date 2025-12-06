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

//! Conversion between moor model types (VerbDef, PropDef, etc.) and FlatBuffers representation

use crate::{common, convert_common, define_enum_mapping};
use moor_common::{
    matching::Preposition,
    model::{
        ArgSpec as ModelArgSpec, Defs, HasUuid, Named, PrepSpec as ModelPrepSpec,
        PropDef as ModelPropDef, ValSet, VerbArgsSpec as ModelVerbArgsSpec,
        VerbDef as ModelVerbDef,
    },
    util::BitEnum,
};
use moor_var::Symbol;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DefConversionError {
    #[error("Failed to decode FlatBuffer: {0}")]
    DecodingError(String),

    #[error("Failed to encode to FlatBuffer: {0}")]
    EncodingError(String),
}

// ============================================================================
// VerbArgsSpec Conversion
// ============================================================================

// Use macro to generate bidirectional ArgSpec conversion
define_enum_mapping! {
    ModelArgSpec <=> common::ArgSpec {
        None <=> None,
        Any <=> Any,
        This <=> This,
    }
}

fn prepspec_to_i16(prep: ModelPrepSpec) -> i16 {
    match prep {
        ModelPrepSpec::Any => -2,
        ModelPrepSpec::None => -1,
        ModelPrepSpec::Other(p) => p as i16,
    }
}

fn prepspec_from_i16(value: i16) -> ModelPrepSpec {
    match value {
        -2 => ModelPrepSpec::Any,
        -1 => ModelPrepSpec::None,
        v if v >= 0 => {
            // Convert to Preposition - this is safe as long as the value is valid
            ModelPrepSpec::Other(
                Preposition::from_repr(v as u16)
                    .unwrap_or_else(|| panic!("Invalid preposition value: {v}")),
            )
        }
        _ => ModelPrepSpec::None, // Fallback for invalid values
    }
}

fn verb_args_spec_to_flatbuffer(args: &ModelVerbArgsSpec) -> common::VerbArgsSpec {
    common::VerbArgsSpec {
        dobj: args.dobj.into(),
        prep: prepspec_to_i16(args.prep),
        iobj: args.iobj.into(),
    }
}

fn verb_args_spec_from_flatbuffer(fb: &common::VerbArgsSpec) -> ModelVerbArgsSpec {
    ModelVerbArgsSpec {
        dobj: fb.dobj.into(),
        prep: prepspec_from_i16(fb.prep),
        iobj: fb.iobj.into(),
    }
}

fn verb_args_spec_from_ref(
    fb: common::VerbArgsSpecRef<'_>,
) -> Result<ModelVerbArgsSpec, DefConversionError> {
    let dobj = fb
        .dobj()
        .map_err(|e| DefConversionError::DecodingError(format!("dobj: {e}")))?;
    let prep = fb
        .prep()
        .map_err(|e| DefConversionError::DecodingError(format!("prep: {e}")))?;
    let iobj = fb
        .iobj()
        .map_err(|e| DefConversionError::DecodingError(format!("iobj: {e}")))?;

    Ok(ModelVerbArgsSpec {
        dobj: dobj.into(),
        prep: prepspec_from_i16(prep),
        iobj: iobj.into(),
    })
}

// ============================================================================
// VerbDef Conversion
// ============================================================================

pub fn verbdef_to_flatbuffer(verb: &ModelVerbDef) -> Result<common::VerbDef, DefConversionError> {
    let uuid = verb.uuid();
    let uuid_bytes = uuid.as_bytes();
    let uuid_fb = common::Uuid {
        data: uuid_bytes.to_vec(),
    };

    let names: Vec<common::Symbol> = verb
        .names()
        .iter()
        .map(convert_common::symbol_to_flatbuffer_struct)
        .collect();

    Ok(common::VerbDef {
        uuid: Box::new(uuid_fb),
        location: Box::new(convert_common::obj_to_flatbuffer_struct(&verb.location())),
        owner: Box::new(convert_common::obj_to_flatbuffer_struct(&verb.owner())),
        names,
        flags: verb.flags().to_u16(),
        args: Box::new(verb_args_spec_to_flatbuffer(&verb.args())),
    })
}

pub fn verbdef_from_flatbuffer(fb: &common::VerbDef) -> Result<ModelVerbDef, DefConversionError> {
    let uuid_bytes: [u8; 16] = fb
        .uuid
        .data
        .as_slice()
        .try_into()
        .map_err(|_| DefConversionError::DecodingError("Invalid UUID bytes".to_string()))?;
    let uuid = Uuid::from_bytes(uuid_bytes);

    let location = convert_common::obj_from_flatbuffer_struct(&fb.location)
        .map_err(|e| DefConversionError::DecodingError(e.to_string()))?;
    let owner = convert_common::obj_from_flatbuffer_struct(&fb.owner)
        .map_err(|e| DefConversionError::DecodingError(e.to_string()))?;

    let names: Vec<Symbol> = fb
        .names
        .iter()
        .map(convert_common::symbol_from_flatbuffer_struct)
        .collect();

    let flags = BitEnum::from_u16(fb.flags);
    let args = verb_args_spec_from_flatbuffer(&fb.args);

    Ok(ModelVerbDef::new(
        uuid, location, owner, &names, flags, args,
    ))
}

pub fn verbdef_from_ref(fb: common::VerbDefRef<'_>) -> Result<ModelVerbDef, DefConversionError> {
    let uuid_ref = fb
        .uuid()
        .map_err(|e| DefConversionError::DecodingError(format!("uuid: {e}")))?;
    let uuid =
        convert_common::uuid_from_ref(uuid_ref).map_err(DefConversionError::DecodingError)?;

    let location_ref = fb
        .location()
        .map_err(|e| DefConversionError::DecodingError(format!("location: {e}")))?;
    let location =
        convert_common::obj_from_ref(location_ref).map_err(DefConversionError::DecodingError)?;

    let owner_ref = fb
        .owner()
        .map_err(|e| DefConversionError::DecodingError(format!("owner: {e}")))?;
    let owner =
        convert_common::obj_from_ref(owner_ref).map_err(DefConversionError::DecodingError)?;

    let names_vec = fb
        .names()
        .map_err(|e| DefConversionError::DecodingError(format!("names: {e}")))?;
    let names: Result<Vec<Symbol>, DefConversionError> = names_vec
        .iter()
        .map(|name_result| {
            let name_ref =
                name_result.map_err(|e| DefConversionError::DecodingError(format!("name: {e}")))?;
            convert_common::symbol_from_ref(name_ref).map_err(DefConversionError::DecodingError)
        })
        .collect();
    let names = names?;

    let flags = fb
        .flags()
        .map_err(|e| DefConversionError::DecodingError(format!("flags: {e}")))?;
    let flags = BitEnum::from_u16(flags);

    let args_ref = fb
        .args()
        .map_err(|e| DefConversionError::DecodingError(format!("args: {e}")))?;
    let args = verb_args_spec_from_ref(args_ref)?;

    Ok(ModelVerbDef::new(
        uuid, location, owner, &names, flags, args,
    ))
}

// ============================================================================
// PropDef Conversion
// ============================================================================

pub fn propdef_to_flatbuffer(prop: &ModelPropDef) -> Result<common::PropDef, DefConversionError> {
    let uuid = prop.uuid();
    let uuid_bytes = uuid.as_bytes();
    let uuid_fb = common::Uuid {
        data: uuid_bytes.to_vec(),
    };

    Ok(common::PropDef {
        uuid: Box::new(uuid_fb),
        definer: Box::new(convert_common::obj_to_flatbuffer_struct(&prop.definer())),
        location: Box::new(convert_common::obj_to_flatbuffer_struct(&prop.location())),
        name: Box::new(convert_common::symbol_to_flatbuffer_struct(&prop.name())),
    })
}

pub fn propdef_from_flatbuffer(fb: &common::PropDef) -> Result<ModelPropDef, DefConversionError> {
    let uuid_bytes: [u8; 16] = fb
        .uuid
        .data
        .as_slice()
        .try_into()
        .map_err(|_| DefConversionError::DecodingError("Invalid UUID bytes".to_string()))?;
    let uuid = Uuid::from_bytes(uuid_bytes);

    let definer = convert_common::obj_from_flatbuffer_struct(&fb.definer)
        .map_err(|e| DefConversionError::DecodingError(e.to_string()))?;
    let location = convert_common::obj_from_flatbuffer_struct(&fb.location)
        .map_err(|e| DefConversionError::DecodingError(e.to_string()))?;
    let name = convert_common::symbol_from_flatbuffer_struct(&fb.name);

    Ok(ModelPropDef::new(uuid, definer, location, name))
}

// ============================================================================
// VerbDefs Collection Conversion
// ============================================================================

pub fn verbdefs_to_flatbuffer(
    defs: &Defs<ModelVerbDef>,
) -> Result<common::VerbDefs, DefConversionError> {
    let verbs: Result<Vec<_>, _> = defs.iter().map(|v| verbdef_to_flatbuffer(&v)).collect();
    Ok(common::VerbDefs { verbs: verbs? })
}

pub fn verbdefs_from_flatbuffer(
    fb: &common::VerbDefs,
) -> Result<Defs<ModelVerbDef>, DefConversionError> {
    let verbs: Result<Vec<_>, _> = fb.verbs.iter().map(verbdef_from_flatbuffer).collect();
    Ok(Defs::from_items(&verbs?))
}

// ============================================================================
// PropDefs Collection Conversion
// ============================================================================

pub fn propdefs_to_flatbuffer(
    defs: &Defs<ModelPropDef>,
) -> Result<common::PropDefs, DefConversionError> {
    let props: Result<Vec<_>, _> = defs.iter().map(|p| propdef_to_flatbuffer(&p)).collect();
    Ok(common::PropDefs { props: props? })
}

pub fn propdefs_from_flatbuffer(
    fb: &common::PropDefs,
) -> Result<Defs<ModelPropDef>, DefConversionError> {
    let props: Result<Vec<_>, _> = fb.props.iter().map(propdef_from_flatbuffer).collect();
    Ok(Defs::from_items(&props?))
}
