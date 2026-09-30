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

//! Typed relation rows transported as JSON, with native columns as the stored representation.
use super::{
    PostgresError,
    codec::{self, Flags, SqlText, invalid},
};
use crate::{
    AnonymousObjectMetadata, EntityMetadataKey, ObjAndUUIDHolder, StringHolder, Timestamp,
};
use moor_common::{
    model::{ObjFlag, PropDefs, PropPerms, VerbDefs},
    util::BitEnum,
};
use moor_compiler::{
    PERSISTENT_LITERAL_VERSION, PERSISTENT_SOURCE_VERSION, SourceCodecError, SourceProfile,
    read_persistent_literal, read_persistent_source, write_persistent_literal,
    write_persistent_source,
};
use moor_var::{Obj, Symbol, Var, program::ProgramType};
use serde_json::{Map, Value, json};
use uuid::Uuid;

pub(super) type Row = Map<String, Value>;

pub(super) fn text<'a>(row: &'a Row, field: &'static str) -> Result<&'a str, PostgresError> {
    row.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(field, "missing or invalid text field"))
}
pub(super) fn number<T: std::str::FromStr>(
    row: &Row,
    field: &'static str,
) -> Result<T, PostgresError> {
    let value = row
        .get(field)
        .ok_or_else(|| invalid(field, "missing numeric field"))?;
    let parsed = match value {
        Value::String(value) => value.parse(),
        Value::Number(value) => value.to_string().parse(),
        _ => return Err(invalid(field, "invalid numeric field")),
    };
    parsed.map_err(|_| invalid(field, "numeric field out of range"))
}
fn json_field<T: serde::de::DeserializeOwned>(
    row: &Row,
    field: &'static str,
) -> Result<T, PostgresError> {
    serde_json::from_value(
        row.get(field)
            .ok_or_else(|| invalid(field, "missing JSON field"))?
            .clone(),
    )
    .map_err(|_| invalid(field, "invalid JSON field"))
}
fn uuid(row: &Row, field: &'static str) -> Result<Uuid, PostgresError> {
    Uuid::parse_str(text(row, field)?).map_err(|_| invalid(field, "invalid UUID"))
}
fn fields(value: Value) -> Row {
    let Value::Object(row) = value else {
        unreachable!("row codec constructs an object")
    };
    row
}

pub(super) fn parse_row(row: super::PostgresRow) -> Result<Value, PostgresError> {
    if row.columns.len() != 1 {
        return Err(invalid("row", "expected one JSON column"));
    }
    serde_json::from_slice(
        row.columns[0]
            .as_deref()
            .ok_or_else(|| invalid("row", "NULL JSON row"))?,
    )
    .map_err(|_| invalid("row", "invalid JSON row"))
}

/// Logical keys determine conflict targets; spelling payloads are not part of metadata identity.
pub(super) trait RowKey: Sized {
    fn encode_key(&self, relation: &'static str) -> Row;
    fn decode_key(row: &Row, relation: &'static str) -> Result<Self, PostgresError>;
}
impl RowKey for Obj {
    fn encode_key(&self, _: &'static str) -> Row {
        fields(json!({"object_ref":self.to_literal()}))
    }
    fn decode_key(row: &Row, _: &'static str) -> Result<Self, PostgresError> {
        codec::object(text(row, "object_ref")?)
    }
}
impl RowKey for ObjAndUUIDHolder {
    fn encode_key(&self, relation: &'static str) -> Row {
        let mut row = self.obj().encode_key(relation);
        row.insert(uuid_column(relation).into(), json!(self.uuid().to_string()));
        row
    }
    fn decode_key(row: &Row, relation: &'static str) -> Result<Self, PostgresError> {
        Ok(Self::new(
            &codec::object(text(row, "object_ref")?)?,
            uuid(row, uuid_column(relation))?,
        ))
    }
}
fn uuid_column(relation: &str) -> &'static str {
    if relation == "object_verbs" {
        "verb_uuid"
    } else {
        "property_uuid"
    }
}
impl RowKey for EntityMetadataKey {
    fn encode_key(&self, _: &'static str) -> Row {
        let key = self.key();
        let folded = SqlText::encode(&key.to_folded_case());
        let spelling = SqlText::encode(key.as_str());
        let kind = if self.is_object() {
            "object"
        } else if self.is_property() {
            "property"
        } else {
            "verb"
        };
        fields(
            json!({"object_ref":self.obj().to_literal(),"entity_kind":kind,"entity_uuid":self.uuid().unwrap_or_else(Uuid::nil).to_string(),"key_encoding":folded.encoding,"key_folded":folded.text,"key_spelling_encoding":spelling.encoding,"key_spelling":spelling.text}),
        )
    }
    fn decode_key(row: &Row, _: &'static str) -> Result<Self, PostgresError> {
        let object = codec::object(text(row, "object_ref")?)?;
        let spelling = SqlText::decode(
            text(row, "key_spelling")?,
            text(row, "key_spelling_encoding")?,
        )?;
        let folded = SqlText::decode(text(row, "key_folded")?, text(row, "key_encoding")?)?;
        let name = Symbol::mk(&spelling);
        if folded != name.to_folded_case() {
            return Err(invalid(
                "key_folded",
                "metadata key disagrees with stored spelling",
            ));
        }
        let uuid = uuid(row, "entity_uuid")?;
        match text(row, "entity_kind")? {
            "object" if uuid.is_nil() => Ok(Self::object(object, name)),
            "property" => Ok(Self::property(object, uuid, name)),
            "verb" => Ok(Self::verb(object, uuid, name)),
            _ => Err(invalid("entity_kind", "invalid metadata entity")),
        }
    }
}

pub(super) trait RowValue: Sized {
    fn encode_value(
        &self,
        relation: &'static str,
        profile: &SourceProfile,
    ) -> Result<Row, PostgresError>;
    fn decode_value(
        row: &Row,
        relation: &'static str,
        profile: &SourceProfile,
    ) -> Result<Self, PostgresError>;
}
fn object_column(relation: &'static str) -> Result<&'static str, PostgresError> {
    match relation {
        "object_parent" => Ok("parent_ref"),
        "object_location" => Ok("location_ref"),
        "object_owner" => Ok("owner_ref"),
        _ => Err(invalid("relation", "not an object-valued relation")),
    }
}
impl RowValue for Obj {
    fn encode_value(
        &self,
        relation: &'static str,
        _: &SourceProfile,
    ) -> Result<Row, PostgresError> {
        Ok(
            [(object_column(relation)?.into(), json!(self.to_literal()))]
                .into_iter()
                .collect(),
        )
    }
    fn decode_value(
        row: &Row,
        relation: &'static str,
        _: &SourceProfile,
    ) -> Result<Self, PostgresError> {
        codec::object(text(row, object_column(relation)?)?)
    }
}
impl RowValue for StringHolder {
    fn encode_value(&self, _: &'static str, _: &SourceProfile) -> Result<Row, PostgresError> {
        let name = SqlText::encode(&self.0);
        Ok(fields(
            json!({"name":name.text,"name_encoding":name.encoding}),
        ))
    }
    fn decode_value(row: &Row, _: &'static str, _: &SourceProfile) -> Result<Self, PostgresError> {
        Ok(Self(SqlText::decode(
            text(row, "name")?,
            text(row, "name_encoding")?,
        )?))
    }
}
impl RowValue for BitEnum<ObjFlag> {
    fn encode_value(&self, _: &'static str, _: &SourceProfile) -> Result<Row, PostgresError> {
        let flags = Flags::encode(*self);
        Ok(fields(
            json!({"flag_names":flags.names,"flag_bits":flags.bits}),
        ))
    }
    fn decode_value(row: &Row, _: &'static str, _: &SourceProfile) -> Result<Self, PostgresError> {
        Flags {
            names: json_field(row, "flag_names")?,
            bits: number(row, "flag_bits")?,
        }
        .decode()
    }
}
impl RowValue for PropPerms {
    fn encode_value(&self, _: &'static str, _: &SourceProfile) -> Result<Row, PostgresError> {
        let flags = Flags::encode(self.flags());
        Ok(fields(
            json!({"owner_ref":self.owner().to_literal(),"flag_names":flags.names,"flag_bits":flags.bits}),
        ))
    }
    fn decode_value(row: &Row, _: &'static str, _: &SourceProfile) -> Result<Self, PostgresError> {
        Ok(Self::new(
            codec::object(text(row, "owner_ref")?)?,
            Flags {
                names: json_field(row, "flag_names")?,
                bits: number(row, "flag_bits")?,
            }
            .decode()?,
        ))
    }
}
macro_rules! definitions {
    ($type:ty,$encode:ident,$decode:ident) => {
        impl RowValue for $type {
            fn encode_value(&self, _: &'static str, _: &SourceProfile) -> Result<Row,PostgresError> {
                let definitions=serde_json::from_str::<Value>(&codec::$encode(self)?).map_err(|_|invalid("definitions","cannot encode definitions"))?;
                Ok(fields(json!({"definitions":definitions})))
            }
            fn decode_value(row: &Row, _: &'static str, _: &SourceProfile) -> Result<Self,PostgresError> {
                let definitions=row.get("definitions").ok_or_else(||invalid("definitions","missing definitions"))?;
                codec::$decode(&definitions.to_string())
            }
        }
    };
}
definitions!(PropDefs, encode_propdefs, decode_propdefs);
definitions!(VerbDefs, encode_verbdefs, decode_verbdefs);
impl RowValue for Var {
    fn encode_value(&self, _: &'static str, profile: &SourceProfile) -> Result<Row, PostgresError> {
        let mut literal = String::new();
        write_persistent_literal(self, profile, &mut literal).map_err(|error| {
            PostgresError::Codec {
                field: "value_literal",
                detail: error.to_string(),
            }
        })?;
        Ok(fields(
            json!({"value_literal":literal,"literal_format":PERSISTENT_LITERAL_VERSION}),
        ))
    }
    fn decode_value(
        row: &Row,
        _: &'static str,
        profile: &SourceProfile,
    ) -> Result<Self, PostgresError> {
        if number::<u32>(row, "literal_format")? != PERSISTENT_LITERAL_VERSION {
            return Err(invalid("literal_format", "unsupported literal format"));
        }
        read_persistent_literal(text(row, "value_literal")?, profile).map_err(|error| {
            PostgresError::Codec {
                field: "value_literal",
                detail: error.to_string(),
            }
        })
    }
}
fn source_error(error: SourceCodecError) -> PostgresError {
    let detail = match error {
        SourceCodecError::Compile(error) => error.to_string(),
        error => error.to_string(),
    };
    PostgresError::Codec {
        field: "source",
        detail,
    }
}

impl RowValue for ProgramType {
    fn encode_value(&self, _: &'static str, profile: &SourceProfile) -> Result<Row, PostgresError> {
        let mut source = String::new();
        write_persistent_source(self, profile, &mut source).map_err(source_error)?;
        Ok(fields(
            json!({"source":source,"source_format":PERSISTENT_SOURCE_VERSION,"compiler_profile":super::schema::PROFILE_ID}),
        ))
    }
    fn decode_value(
        row: &Row,
        _: &'static str,
        profile: &SourceProfile,
    ) -> Result<Self, PostgresError> {
        if number::<u32>(row, "source_format")? != PERSISTENT_SOURCE_VERSION
            || text(row, "compiler_profile")? != super::schema::PROFILE_ID
        {
            return Err(invalid(
                "source_format",
                "unsupported source format or compiler profile",
            ));
        }
        read_persistent_source(text(row, "source")?, profile).map_err(source_error)
    }
}
impl RowValue for AnonymousObjectMetadata {
    fn encode_value(&self, _: &'static str, _: &SourceProfile) -> Result<Row, PostgresError> {
        let (created, accessed) = self.micros();
        Ok(fields(
            json!({"created_micros":created.to_string(),"last_accessed_micros":accessed.to_string()}),
        ))
    }
    fn decode_value(row: &Row, _: &'static str, _: &SourceProfile) -> Result<Self, PostgresError> {
        Ok(Self::from_micros(
            number(row, "created_micros")?,
            number(row, "last_accessed_micros")?,
        ))
    }
}

pub(super) fn encode<K: RowKey, V: RowValue>(
    relation: &'static str,
    timestamp: Timestamp,
    key: &K,
    value: &V,
    profile: &SourceProfile,
) -> Result<Value, PostgresError> {
    let mut row = key.encode_key(relation);
    row.insert("logical_timestamp".into(), json!(timestamp.0.to_string()));
    let encoded = value
        .encode_value(relation, profile)
        .map_err(|error| contextual(relation, &row, error))?;
    row.extend(encoded);
    Ok(Value::Object(row))
}
pub(super) fn decode<K: RowKey, V: RowValue>(
    relation: &'static str,
    row: Value,
    profile: &SourceProfile,
) -> Result<(Timestamp, K, V), PostgresError> {
    let row = row
        .as_object()
        .ok_or_else(|| invalid("row", "expected row object"))?;
    (|| {
        Ok((
            Timestamp(number(row, "logical_timestamp")?),
            K::decode_key(row, relation)?,
            V::decode_value(row, relation, profile)?,
        ))
    })()
    .map_err(|error| contextual(relation, row, error))
}

/// Keep relation and canonical identity in diagnostics without logging a row's payload.
pub(super) fn contextual(
    relation: &'static str,
    row: &Row,
    source: PostgresError,
) -> PostgresError {
    let object = text(row, "object_ref")
        .ok()
        .and_then(|text| codec::object(text).ok())
        .map(|obj| obj.to_literal())
        .unwrap_or_else(|| "<invalid object>".into());
    let uuid = ["property_uuid", "verb_uuid", "entity_uuid"]
        .into_iter()
        .find_map(|field| {
            row.get(field)
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
        });
    let key = uuid.map_or_else(|| object.clone(), |uuid| format!("{object}/{uuid}"));
    PostgresError::Row {
        relation,
        key,
        source: Box::new(source),
    }
}

/// Produce lossless text for wide SQL numeric fields before the JSON parser sees them.
pub(super) fn select_row(relation: &str) -> String {
    let extra = if relation == "anonymous_object_metadata" {
        ", 'created_micros', t.created_micros::text, 'last_accessed_micros', t.last_accessed_micros::text"
    } else {
        ""
    };
    format!(
        "(to_jsonb(t) || jsonb_build_object('logical_timestamp', t.logical_timestamp::text{extra}))::text"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_common::model::PropFlag;
    use moor_var::{v_int, v_list, v_str};
    #[test]
    fn metadata_identity_includes_holder_and_preserves_spelling() {
        let profile = SourceProfile::default();
        let uuid = Uuid::from_u128(7);
        let key = EntityMetadataKey::property(Obj::mk_id(4), uuid, Symbol::mk("Straße\0"));
        let row = encode(
            "entity_metadata",
            Timestamp(u64::MAX),
            &key,
            &v_str("x\0y"),
            &profile,
        )
        .unwrap();
        let (timestamp, decoded, value): (_, EntityMetadataKey, Var) =
            decode("entity_metadata", row.clone(), &profile).unwrap();
        assert_eq!(timestamp, Timestamp(u64::MAX));
        assert_eq!(decoded, key);
        assert_eq!(decoded.key().as_str(), "Straße\0");
        assert_eq!(value, v_str("x\0y"));
        let other = EntityMetadataKey::property(Obj::mk_id(5), uuid, key.key());
        assert_ne!(
            row["object_ref"],
            other.encode_key("entity_metadata")["object_ref"]
        );
        let mut broken = row;
        broken["key_folded"] = json!("wrong");
        assert!(decode::<EntityMetadataKey, Var>("entity_metadata", broken, &profile).is_err());
    }
    #[test]
    fn wide_numbers_and_native_permissions_round_trip() {
        let profile = SourceProfile::default();
        let key = Obj::mk_id(-1);
        let value = AnonymousObjectMetadata::from_micros(u128::MAX, u128::MAX - 1);
        let row = encode(
            "anonymous_object_metadata",
            Timestamp(u64::MAX),
            &key,
            &value,
            &profile,
        )
        .unwrap();
        let (_, _, decoded): (_, Obj, AnonymousObjectMetadata) =
            decode("anonymous_object_metadata", row, &profile).unwrap();
        assert_eq!(value, decoded);
        let key = ObjAndUUIDHolder::new(&key, Uuid::from_u128(7));
        let value = PropPerms::new(Obj::mk_id(-1), BitEnum::<PropFlag>::from_u16(0xffff));
        let row = encode("object_propflags", Timestamp(12), &key, &value, &profile).unwrap();
        let (_, decoded_key, decoded): (_, ObjAndUUIDHolder, PropPerms) =
            decode("object_propflags", row, &profile).unwrap();
        assert_eq!(decoded_key, key);
        assert_eq!(decoded, value);
    }
    #[test]
    fn literals_reject_unsupported_formats_and_keep_none_distinct_from_sql_null() {
        let profile = SourceProfile::default();
        let key = Obj::mk_id(1);
        for value in [Var::mk_none(), v_list(&[v_int(1), v_str("hi")])] {
            let row = encode("object_last_move", Timestamp(1), &key, &value, &profile).unwrap();
            let (_, _, decoded): (_, Obj, Var) =
                decode("object_last_move", row.clone(), &profile).unwrap();
            assert_eq!(value, decoded);
            let mut broken = row;
            broken["literal_format"] = json!(99);
            assert!(decode::<Obj, Var>("object_last_move", broken, &profile).is_err());
        }
    }
}
