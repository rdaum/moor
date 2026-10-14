// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Readable PostgreSQL field codecs. No Fjall framing or binary program encoding enters SQL.

use super::PostgresError;
use moor_common::{
    model::{
        ArgSpec, HasUuid, Named, ObjFlag, PrepSpec, PropDef, PropDefs, PropFlag, ValSet,
        VerbArgsSpec, VerbDef, VerbDefs, VerbFlag, preposition_to_string,
    },
    util::{BitEnum, BitFlag},
};
use moor_compiler::SourceProfile;
use moor_var::{Obj, Symbol, Var, Variant};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn invalid(field: &'static str, reason: &'static str) -> PostgresError {
    PostgresError::Format { field, reason }
}

pub(super) fn object(text: &str) -> Result<Obj, PostgresError> {
    if !text.starts_with('#') || text.len() > 23 {
        return Err(invalid("object_ref", "invalid object identity"));
    }
    let value = moor_compiler::read_persistent_literal(text, &SourceProfile::default())
        .map_err(|_| invalid("object_ref", "invalid object identity"))?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("object_ref", "expected object identity"))?;
    if object.to_literal() != text {
        return Err(invalid("object_ref", "noncanonical object identity"));
    }
    Ok(object)
}

/// Raw text stays readable; the escaped representation is reserved for strings containing NUL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SqlText {
    pub text: String,
    pub encoding: &'static str,
}
impl SqlText {
    pub fn encode(text: &str) -> Self {
        if !text.contains('\0') {
            return Self {
                text: text.to_owned(),
                encoding: "utf8",
            };
        }
        Self {
            text: serde_json::to_string(text).expect("string JSON encoding is infallible"),
            encoding: "json_string",
        }
    }
    pub fn decode(text: &str, encoding: &str) -> Result<String, PostgresError> {
        let decoded = match encoding {
            "utf8" => text.to_owned(),
            "json_string" => serde_json::from_str::<String>(text)
                .map_err(|_| invalid("text", "invalid escaped string"))?,
            _ => return Err(invalid("text", "unsupported text encoding")),
        };
        let canonical = Self::encode(&decoded);
        if canonical.text != text || canonical.encoding != encoding {
            return Err(invalid("text", "noncanonical text encoding"));
        }
        Ok(decoded)
    }
}

/// JSONB itself cannot contain NUL, so definition names use an envelope only when necessary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum DefinitionName {
    Text(String),
    Escaped { encoding: String, value: String },
}
impl DefinitionName {
    fn encode(symbol: Symbol) -> Self {
        let text = SqlText::encode(symbol.as_str());
        if text.encoding == "utf8" {
            return Self::Text(text.text);
        }
        Self::Escaped {
            encoding: text.encoding.into(),
            value: text.text,
        }
    }
    fn decode(self) -> Result<Symbol, PostgresError> {
        let text = match self {
            Self::Text(text) => SqlText::decode(&text, "utf8")?,
            Self::Escaped { encoding, value } => {
                if encoding != "json_string" {
                    return Err(invalid("name", "invalid escaped name envelope"));
                }
                SqlText::decode(&value, &encoding)?
            }
        };
        Ok(Symbol::mk(&text))
    }
}

pub(super) trait FlagNames: BitFlag + Copy + 'static {
    const NAMED: &'static [(Self, &'static str)];
}
impl FlagNames for ObjFlag {
    const NAMED: &'static [(Self, &'static str)] = &[
        (Self::User, "user"),
        (Self::Programmer, "programmer"),
        (Self::Wizard, "wizard"),
        (Self::Obsolete1, "obsolete1"),
        (Self::Read, "read"),
        (Self::Write, "write"),
        (Self::Obsolete2, "obsolete2"),
        (Self::Fertile, "fertile"),
    ];
}
impl FlagNames for PropFlag {
    const NAMED: &'static [(Self, &'static str)] = &[
        (Self::Read, "read"),
        (Self::Write, "write"),
        (Self::Chown, "chown"),
        (Self::Clobber, "clobber"),
    ];
}
impl FlagNames for VerbFlag {
    const NAMED: &'static [(Self, &'static str)] = &[
        (Self::Read, "read"),
        (Self::Write, "write"),
        (Self::Exec, "exec"),
        (Self::Debug, "debug"),
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Flags {
    pub names: Vec<String>,
    pub bits: u16,
}
impl Flags {
    pub fn encode<T: FlagNames + 'static>(flags: BitEnum<T>) -> Self {
        Self {
            bits: flags.to_u16(),
            names: T::NAMED
                .iter()
                .filter(|(bit, _)| flags.contains(*bit))
                .map(|(_, name)| (*name).to_owned())
                .collect(),
        }
    }
    pub fn decode<T: FlagNames + 'static>(self) -> Result<BitEnum<T>, PostgresError> {
        let flags = BitEnum::<T>::from_u16(self.bits);
        let mut expected = Self::encode(flags).names;
        let mut names = self.names;
        expected.sort_unstable();
        names.sort_unstable();
        if expected != names {
            return Err(invalid(
                "flags",
                "flag names disagree with authoritative bits",
            ));
        }
        Ok(flags)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PropertyDefinition {
    uuid: Uuid,
    definer_ref: String,
    location_ref: String,
    name: DefinitionName,
}

pub(super) fn encode_propdefs(definitions: &PropDefs) -> Result<String, PostgresError> {
    let definitions: Vec<_> = definitions
        .iter()
        .map(|definition| PropertyDefinition {
            uuid: definition.uuid(),
            definer_ref: definition.definer().to_literal(),
            location_ref: definition.location().to_literal(),
            name: DefinitionName::encode(definition.name()),
        })
        .collect();
    serde_json::to_string(&definitions)
        .map_err(|_| invalid("definitions", "cannot encode property definitions"))
}
pub(super) fn decode_propdefs(text: &str) -> Result<PropDefs, PostgresError> {
    let definitions: Vec<PropertyDefinition> = serde_json::from_str(text)
        .map_err(|_| invalid("definitions", "invalid property definitions"))?;
    let mut seen = std::collections::HashSet::new();
    definitions
        .into_iter()
        .map(|definition| {
            if !seen.insert(definition.uuid) {
                return Err(invalid("definitions", "duplicate property UUID"));
            }
            Ok(PropDef::new(
                definition.uuid,
                object(&definition.definer_ref)?,
                object(&definition.location_ref)?,
                definition.name.decode()?,
            ))
        })
        .collect()
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    dobj: String,
    prep: String,
    iobj: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerbDefinition {
    uuid: Uuid,
    location_ref: String,
    owner_ref: String,
    names: Vec<DefinitionName>,
    flags: Flags,
    args: Arguments,
}
pub(super) fn encode_verbdefs(definitions: &VerbDefs) -> Result<String, PostgresError> {
    let definitions: Vec<_> = definitions
        .iter()
        .map(|definition| {
            let args = definition.args();
            VerbDefinition {
                uuid: definition.uuid(),
                location_ref: definition.location().to_literal(),
                owner_ref: definition.owner().to_literal(),
                names: definition
                    .names()
                    .iter()
                    .copied()
                    .map(DefinitionName::encode)
                    .collect(),
                flags: Flags::encode(definition.flags()),
                args: Arguments {
                    dobj: args.dobj.to_string().into(),
                    prep: preposition_to_string(&args.prep).into(),
                    iobj: args.iobj.to_string().into(),
                },
            }
        })
        .collect();
    serde_json::to_string(&definitions)
        .map_err(|_| invalid("definitions", "cannot encode verb definitions"))
}
pub(super) fn decode_verbdefs(text: &str) -> Result<VerbDefs, PostgresError> {
    let definitions: Vec<VerbDefinition> = serde_json::from_str(text)
        .map_err(|_| invalid("definitions", "invalid verb definitions"))?;
    let mut seen = std::collections::HashSet::new();
    definitions
        .into_iter()
        .map(|definition| {
            if !seen.insert(definition.uuid) {
                return Err(invalid("definitions", "duplicate verb UUID"));
            }
            let prep = PrepSpec::parse(&definition.args.prep)
                .ok_or_else(|| invalid("args.prep", "invalid preposition"))?;
            if preposition_to_string(&prep) != definition.args.prep {
                return Err(invalid("args.prep", "noncanonical preposition"));
            }
            let args = VerbArgsSpec {
                dobj: ArgSpec::from_string(&definition.args.dobj)
                    .ok_or_else(|| invalid("args.dobj", "invalid argument specifier"))?,
                prep,
                iobj: ArgSpec::from_string(&definition.args.iobj)
                    .ok_or_else(|| invalid("args.iobj", "invalid argument specifier"))?,
            };
            let names = definition
                .names
                .into_iter()
                .map(DefinitionName::decode)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(VerbDef::new(
                definition.uuid,
                object(&definition.location_ref)?,
                object(&definition.owner_ref)?,
                &names,
                definition.flags.decode()?,
                args,
            ))
        })
        .collect()
}

pub(super) fn profile_json(profile: &SourceProfile) -> Result<Value, PostgresError> {
    profile
        .validate()
        .map_err(|_| invalid("compiler_profile", "unsupported source profile"))?;
    let options = &profile.options;
    Ok(json!({
        "language": profile.language,
        "literal_version": profile.literal_version,
        "source_version": profile.source_version,
        "compiler_profile_version": profile.compiler_profile_version,
        "options": {
            "flyweight_type": options.flyweight_type,
            "bool_type": options.bool_type,
            "symbol_type": options.symbol_type,
            "custom_errors": options.custom_errors,
            "call_unsupported_builtins": options.call_unsupported_builtins,
            "legacy_type_constants": options.legacy_type_constants,
        }
    }))
}

pub(super) fn value_kind(value: &Var) -> &'static str {
    match value.variant() {
        Variant::None => "none",
        Variant::Bool(_) => "bool",
        Variant::Int(_) => "int",
        Variant::Float(_) => "float",
        Variant::Obj(_) => "object",
        Variant::Sym(_) => "symbol",
        Variant::Str(_) => "string",
        Variant::List(_) => "list",
        Variant::Map(_) => "map",
        Variant::Err(_) => "error",
        Variant::Flyweight(_) => "flyweight",
        Variant::Binary(_) => "binary",
        Variant::Lambda(_) => "lambda",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_object_keys_reject_aliases() {
        for text in ["#0", "#-1", "#048D05-1234567890", "#anon_048D05-1234567890"] {
            assert_eq!(object(text).unwrap().to_literal(), text);
        }
        for text in [
            "0",
            "#00",
            "#+1",
            "#048d05-1234567890",
            "$system",
            "*anonymous*",
        ] {
            assert!(object(text).is_err(), "{text}");
        }
    }
    #[test]
    fn sql_text_and_json_names_preserve_nul_without_collisions() {
        for text in ["", "Unicode Καλημέρα", "a\0b", "\"a\\u0000b\""] {
            let encoded = SqlText::encode(text);
            assert!(!encoded.text.contains('\0'));
            assert_eq!(
                SqlText::decode(&encoded.text, encoded.encoding).unwrap(),
                text
            );
            let encoded = serde_json::to_string(&DefinitionName::encode(Symbol::mk(text))).unwrap();
            assert!(!encoded.contains("\\u0000") || encoded.contains("\\\\u0000"));
            let decoded: DefinitionName = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded.decode().unwrap().as_str(), text);
        }
        assert!(SqlText::decode("\"plain\"", "json_string").is_err());
        assert!(SqlText::decode("a\0b", "utf8").is_err());
    }
    #[test]
    fn flags_preserve_unknown_and_obsolete_bits_and_validate_names() {
        for bits in [0, 0xffff, 0x8000, 0x48] {
            let flags = BitEnum::<ObjFlag>::from_u16(bits);
            assert_eq!(
                Flags::encode(flags).decode::<ObjFlag>().unwrap().to_u16(),
                bits
            );
        }
        let encoded = Flags::encode(BitEnum::<ObjFlag>::all_bits());
        assert!(encoded.names.contains(&"obsolete1".into()));
        assert!(encoded.names.contains(&"obsolete2".into()));
        assert!(
            Flags {
                names: vec!["wizard".into()],
                bits: 0
            }
            .decode::<ObjFlag>()
            .is_err()
        );
        assert!(
            Flags {
                names: vec!["unknown".into()],
                bits: 0x8000
            }
            .decode::<ObjFlag>()
            .is_err()
        );
    }
    #[test]
    fn definitions_keep_order_locations_flags_and_exact_names() {
        let props = PropDefs::from_items(&[
            PropDef::new(
                Uuid::from_u128(2),
                Obj::mk_id(4),
                Obj::mk_id(8),
                Symbol::mk("MiXeD\0Name"),
            ),
            PropDef::new(
                Uuid::from_u128(1),
                Obj::mk_id(3),
                Obj::mk_id(7),
                Symbol::mk("other"),
            ),
        ]);
        let encoded = encode_propdefs(&props).unwrap();
        let decoded = decode_propdefs(&encoded).unwrap();
        assert_eq!(props, decoded);
        assert_eq!(
            decoded.iter().next().unwrap().name().as_str(),
            "MiXeD\0Name"
        );
        let verbs = VerbDefs::from_items(&[VerbDef::new(
            Uuid::from_u128(3),
            Obj::mk_id(5),
            Obj::mk_id(6),
            &[Symbol::mk("VeRb*"), Symbol::mk("a\0b")],
            BitEnum::from_u16(0xffff),
            VerbArgsSpec::this_none_this(),
        )]);
        let decoded = decode_verbdefs(&encode_verbdefs(&verbs).unwrap()).unwrap();
        assert_eq!(verbs, decoded);
        assert_eq!(decoded.iter().next().unwrap().names()[0].as_str(), "VeRb*");
        assert!(decode_propdefs("[{}]").is_err());
        assert_eq!(decode_propdefs("[]").unwrap(), PropDefs::empty());
        assert_eq!(decode_verbdefs("[]").unwrap(), VerbDefs::empty());
    }
}
