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

//! Fjall-backed persistence provider with per-type encoding strategies.
//!
//! This adapter binds logical mutations to Fjall keyspaces. Each stored type has an
//! encoding through `EncodeFor`. Snapshot reads use `fjall_reader`; CRUD helpers
//! are available only to unit-test fixtures.
//!
//! ## Encoding Strategies
//!
//! Different types use different encoding approaches for performance and efficiency:
//!
//! - **Zerocopy types** (`Obj`, `BitEnum`, etc.): Direct byte representation using the `zerocopy`
//!   crate's `IntoBytes`/`FromBytes` traits. No serialization overhead.
//!
//! - **ByteView wrappers** (`ObjSet`, `PropPerms`): Zero-copy passthrough - these types already
//!   hold a `ByteView` internally, so encoding just extracts the view via `AsRef<ByteView>` and
//!   decoding uses `From<ByteView>`.
//!
//! - **FlatBuffer types** (`ProgramType`, `Var`, `VerbDefs`, `PropDefs`): Uses FlatBuffers via
//!   the `planus` crate for efficient schema-based serialization with forward/backward
//!   compatibility. `Var` is written directly from its borrowed representation.
//!
//! - **UTF-8 types** (`StringHolder`): Direct UTF-8 byte encoding without additional framing.
//!
//! ## Transaction persistence
//!
//! The database commit pipeline collects operations from every relation into one
//! transaction batch. A single background writer commits those batches to Fjall
//! atomically and in publication order.

#[cfg(test)]
use crate::{db_counters, provider::Provider};
use crate::{
    provider::batch_writer::{BatchEncoder, BatchValue},
    tx::{EncodeFor, Error, RelationCodomain, RelationDomain, Timestamp},
};
use ahash::AHashMap;
use byteview::ByteView;
use fjall::Slice;
#[cfg(test)]
use moor_common::model::WorldStateTimerOp;
use moor_var::Var;
use planus::{ReadAsRoot, WriteAsOffset};
use std::{any::Any, marker::PhantomData};

/// Fjall backing provider used for startup reads and encoding transaction writes.
#[derive(Clone)]
pub(crate) struct FjallProvider<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    relation_name: &'static str,
    fjall_keyspace: fjall::Keyspace,
    marker: PhantomData<fn() -> (Domain, Codomain)>,
}

const TIMESTAMP_BYTES: usize = std::mem::size_of::<u64>();

type SeededPropertyValueIndex = (
    Box<dyn crate::tx::RelationIndex<crate::ObjAndUUIDHolder, Var>>,
    Timestamp,
    AHashMap<crate::ObjAndUUIDHolder, super::property_value_store::PropertyValueChain>,
);

fn frame_value(payload: &[u8], timestamp: Timestamp) -> Slice {
    let mut framed = Vec::with_capacity(payload.len() + TIMESTAMP_BYTES);
    framed.extend_from_slice(payload);
    framed.extend_from_slice(&timestamp.0.to_le_bytes());
    framed.into()
}

pub(crate) fn decode_fjall_value<Codomain>(
    user_value: Slice,
) -> Result<(Timestamp, Codomain), Error>
where
    FjallCodec: EncodeFor<Codomain, Stored = ByteView>,
{
    let (timestamp, payload) = split_fjall_value(user_value)?;
    let codomain = FjallCodec.decode(payload)?;
    Ok((timestamp, codomain))
}

pub(crate) fn split_fjall_value(user_value: Slice) -> Result<(Timestamp, ByteView), Error> {
    let stored = ByteView::from(user_value);
    let Some(payload_len) = stored.len().checked_sub(TIMESTAMP_BYTES) else {
        return Err(Error::EncodingFailure);
    };
    let timestamp = Timestamp(u64::from_le_bytes(
        stored[payload_len..]
            .try_into()
            .map_err(|_| Error::EncodingFailure)?,
    ));
    let payload = stored.slice(..payload_len);
    Ok((timestamp, payload))
}

pub(crate) trait EncodeFjallValue<T>: EncodeFor<T, Stored = ByteView> {
    fn encode_fjall_value(
        &self,
        value: &T,
        timestamp: Timestamp,
        encoder: &mut BatchEncoder,
    ) -> Result<Slice, Error>;
}

fn encode_flatbuffer<T>(root: impl WriteAsOffset<T>) -> ByteView {
    let mut builder = planus::Builder::new();
    let bytes = builder.finish(root, None);
    ByteView::from(bytes)
}

fn frame_flatbuffer<T>(root: impl WriteAsOffset<T>, timestamp: Timestamp) -> Slice {
    let mut builder = planus::Builder::new();
    let bytes = builder.finish(root, None);
    frame_value(bytes, timestamp)
}

struct FjallBatchValue<Codomain>
where
    Codomain: RelationCodomain,
{
    codomain: Codomain,
}

impl<Codomain> BatchValue for FjallBatchValue<Codomain>
where
    Codomain: RelationCodomain,
    FjallCodec: EncodeFjallValue<Codomain>,
{
    fn encode(
        self: Box<Self>,
        timestamp: Timestamp,
        encoder: &mut BatchEncoder,
    ) -> Result<Slice, Error> {
        FjallCodec.encode_fjall_value(&self.codomain, timestamp, encoder)
    }
}

impl<Domain, Codomain> FjallProvider<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    pub fn new(relation_name: &'static str, fjall_keyspace: fjall::Keyspace) -> Self {
        Self {
            relation_name,
            fjall_keyspace,
            marker: PhantomData,
        }
    }

    pub fn partition(&self) -> &fjall::Keyspace {
        &self.fjall_keyspace
    }
}

impl FjallProvider<crate::ObjAndUUIDHolder, Var> {
    pub(crate) fn encode_property_value_changes(
        &self,
        changes: Vec<super::logical::PreparedPropertyValueOp>,
    ) -> Vec<super::batch_writer::BatchOp> {
        use super::batch_writer::{BatchOp, BatchOpSource, BatchOpType};

        changes
            .into_iter()
            .map(|prepared| {
                let object = prepared.property.obj();
                let uuid = prepared.property.uuid();
                BatchOp {
                    partition: self.fjall_keyspace.clone(),
                    op_type: BatchOpType::PropertyValue(prepared),
                    source: BatchOpSource::Property {
                        relation: self.relation_name,
                        object,
                        uuid,
                    },
                }
            })
            .collect()
    }

    pub(crate) fn seeded_property_value_index(
        &self,
        snapshot: &fjall::Snapshot,
        relation: &crate::tx::Relation<crate::ObjAndUUIDHolder, Var>,
    ) -> Result<SeededPropertyValueIndex, Error> {
        use super::property_value_store::{PROPERTY_VALUE_CHAIN_LIMITS, PropertyValueScan};
        let mut chains = AHashMap::new();
        let tuples = PropertyValueScan::new(
            fjall::Readable::iter(snapshot, &self.fjall_keyspace),
            PROPERTY_VALUE_CHAIN_LIMITS,
        )
        .map(|entry| {
            let (property, reconstructed) =
                entry.map_err(|error| Error::RetrievalFailure(error.to_string()))?;
            chains.insert(property.clone(), reconstructed.chain);
            Ok((
                reconstructed.logical_timestamp,
                property,
                reconstructed.value,
            ))
        });
        let (index, max_timestamp) = relation.seeded_index(tuples)?;
        Ok((index, max_timestamp, chains))
    }
}

impl<Domain, Codomain> FjallProvider<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    Self: EncodeFor<Domain, Stored = ByteView> + EncodeFor<Codomain, Stored = ByteView>,
    FjallCodec: EncodeFjallValue<Codomain>,
{
    /// Consume a published working set into operations for the BatchWriter.
    pub(crate) fn encode_changes(
        &self,
        changes: crate::tx::WorkingSetTuples<Domain, Codomain>,
    ) -> Result<Vec<super::batch_writer::BatchOp>, Error> {
        use super::batch_writer::{BatchOp, BatchOpType};
        use crate::tx::OpType;

        let mut batch_ops = Vec::with_capacity(changes.len());
        for (domain, op) in changes {
            match op.operation {
                OpType::Insert(codomain) | OpType::Update(codomain) => {
                    let key_bytes = <Self as EncodeFor<Domain>>::encode(self, &domain)?;
                    let source = self.batch_op_source(&domain);
                    let batch_value: Box<dyn super::batch_writer::BatchValue> =
                        Box::new(FjallBatchValue { codomain });
                    batch_ops.push(BatchOp {
                        partition: self.fjall_keyspace.clone(),
                        op_type: BatchOpType::Insert {
                            key: key_bytes.into(),
                            value: batch_value,
                        },
                        source,
                    });
                }
                OpType::Delete => {
                    let key_bytes = <Self as EncodeFor<Domain>>::encode(self, &domain)?;
                    batch_ops.push(BatchOp {
                        partition: self.fjall_keyspace.clone(),
                        op_type: BatchOpType::Delete {
                            key: key_bytes.into(),
                        },
                        source: self.batch_op_source(&domain),
                    });
                }
            }
        }
        Ok(batch_ops)
    }

    fn batch_op_source(&self, domain: &Domain) -> super::batch_writer::BatchOpSource {
        use super::batch_writer::BatchOpSource;

        if matches!(self.relation_name, "object_propvalues" | "object_propflags")
            && let Some(holder) =
                (domain as &dyn Any).downcast_ref::<crate::model::ObjAndUUIDHolder>()
        {
            return BatchOpSource::Property {
                relation: self.relation_name,
                object: holder.obj(),
                uuid: holder.uuid(),
            };
        }

        BatchOpSource::Relation(self.relation_name)
    }
}

#[cfg(test)]
impl<Domain, Codomain> Provider<Domain, Codomain> for FjallProvider<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    Self: EncodeFor<Domain, Stored = ByteView> + EncodeFor<Codomain, Stored = ByteView>,
    FjallCodec: EncodeFjallValue<Codomain>,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::ProviderTupleCheck);

        // Hit backing store
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::ProviderTupleLoad);
        let key_stored = <Self as EncodeFor<Domain>>::encode(self, domain)?;
        let Some(result) = self
            .fjall_keyspace
            .get(key_stored)
            .map_err(|e| Error::RetrievalFailure(e.to_string()))?
        else {
            return Ok(None);
        };
        Ok(Some(decode_fjall_value(result)?))
    }

    fn put(&self, timestamp: Timestamp, domain: &Domain, codomain: &Codomain) -> Result<(), Error> {
        let key_bytes = <Self as EncodeFor<Domain>>::encode(self, domain)?;
        let value = FjallCodec.encode_fjall_value(codomain, timestamp, &mut BatchEncoder::new())?;
        self.fjall_keyspace
            .insert(key_bytes, value)
            .map_err(|error| Error::StorageFailure(error.to_string()))
    }

    fn del(&self, _timestamp: Timestamp, domain: &Domain) -> Result<(), Error> {
        let key_bytes = <Self as EncodeFor<Domain>>::encode(self, domain)?;
        self.fjall_keyspace
            .remove(key_bytes)
            .map_err(|error| Error::StorageFailure(error.to_string()))
    }

    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        let mut result = Vec::new();

        // Scan backing store
        for entry in self.fjall_keyspace.iter() {
            let (key, value) = entry
                .into_inner()
                .map_err(|e| Error::RetrievalFailure(e.to_string()))?;
            let domain = <Self as EncodeFor<Domain>>::decode(self, ByteView::from(key))?;

            let (timestamp, codomain) = decode_fjall_value(value)?;
            if predicate(&domain, &codomain) {
                result.push((timestamp, domain, codomain));
            }
        }

        Ok(result)
    }

    fn stop(&self) -> Result<(), Error> {
        // No-op: providers no longer have their own background threads.
        // The BatchWriter is stopped at the MoorDB level.
        Ok(())
    }
}

// ============================================================================
// Fjall Codec - Shared encoding logic for FjallProvider and SnapshotLoader
// ============================================================================

/// Zero-sized type that provides encoding/decoding for all Fjall-stored types.
/// This allows both FjallProvider and SnapshotLoader to share the same encoding logic.
#[derive(Clone, Copy)]
pub(crate) struct FjallCodec;

// ============================================================================
// Per-Type Encoding Implementations for FjallCodec
// ============================================================================
// Each type gets its own EncodeFor impl, allowing custom encoding logic

use crate::{AnonymousObjectMetadata, EntityMetadataKey, ObjAndUUIDHolder, StringHolder};
use moor_common::{
    model::{ObjFlag, ObjSet, PropDefs, PropPerms, VerbDefs},
    util::BitEnum,
};
use moor_schema::convert::{encode_db_var, stored_to_program, var_from_db_flatbuffer_ref};
use moor_schema::convert_program::encode_program_to_fb;
use moor_var::{Obj, Symbol, program::ProgramType};
// Per-type encoding implementations
// Each type can be encoded regardless of whether it's used as Domain or Codomain
// We use a blanket impl for all FjallProvider<Domain, Codomain> combinations

/// Encoding for zerocopy types (IntoBytes + FromBytes) - zero-copy serialization
macro_rules! impl_zerocopy_encode {
    ($type:ty) => {
        impl EncodeFor<$type> for FjallCodec {
            type Stored = ByteView;

            fn encode(&self, value: &$type) -> Result<Self::Stored, Error> {
                use zerocopy::IntoBytes;
                Ok(ByteView::from(IntoBytes::as_bytes(value)))
            }

            fn decode(&self, stored: Self::Stored) -> Result<$type, Error> {
                use zerocopy::FromBytes;
                let bytes = stored.as_ref();
                if bytes.len() != std::mem::size_of::<$type>() {
                    return Err(Error::EncodingFailure);
                }

                // Handle potentially unaligned data safely
                let mut aligned_buffer = vec![0u8; std::mem::size_of::<$type>()];
                aligned_buffer.copy_from_slice(bytes);

                <$type>::read_from_bytes(&aligned_buffer).map_err(|_| Error::EncodingFailure)
            }
        }
    };
}

/// Encoding for types that wrap ByteView - zero-copy passthrough
macro_rules! impl_byteview_wrapper_encode {
    ($type:ty) => {
        impl EncodeFor<$type> for FjallCodec {
            type Stored = ByteView;

            fn encode(&self, value: &$type) -> Result<Self::Stored, Error> {
                Ok(AsRef::<ByteView>::as_ref(value).clone())
            }

            fn decode(&self, stored: Self::Stored) -> Result<$type, Error> {
                Ok(<$type>::from(stored))
            }
        }
    };
}

// Zerocopy types - direct byte access, no serialization overhead
impl_zerocopy_encode!(Obj);
impl_zerocopy_encode!(ObjAndUUIDHolder);
impl_zerocopy_encode!(AnonymousObjectMetadata);
impl_zerocopy_encode!(BitEnum<ObjFlag>);

// ByteView wrappers - zero-copy passthrough
impl_byteview_wrapper_encode!(ObjSet);
impl_byteview_wrapper_encode!(PropPerms);

// Keep the object prefix in the same byte order as other relations for snapshot export.
// Symbol IDs are process-local; the suffix stores their case-folded UTF-8 spelling.
impl EncodeFor<EntityMetadataKey> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, value: &EntityMetadataKey) -> Result<Self::Stored, Error> {
        use zerocopy::IntoBytes;

        let name = value.key().to_folded_case();
        let mut bytes = Vec::with_capacity(std::mem::size_of::<Obj>() + 17 + name.len());
        bytes.extend_from_slice(value.obj().as_bytes());
        let tag = if value.is_object() {
            0
        } else if value.is_property() {
            1
        } else if value.is_verb() {
            2
        } else {
            return Err(Error::EncodingFailure);
        };
        bytes.push(tag);
        bytes.extend_from_slice(value.uuid().unwrap_or_default().as_bytes());
        bytes.extend_from_slice(name.as_bytes());
        Ok(ByteView::from(bytes))
    }

    fn decode(&self, stored: Self::Stored) -> Result<EntityMetadataKey, Error> {
        use zerocopy::FromBytes;

        const OBJ_BYTES: usize = std::mem::size_of::<Obj>();
        const HEADER_BYTES: usize = OBJ_BYTES + 17;
        if stored.len() < HEADER_BYTES {
            return Err(Error::EncodingFailure);
        }
        let obj = Obj::read_from_bytes(&stored[..OBJ_BYTES]).map_err(|_| Error::EncodingFailure)?;
        let uuid = uuid::Uuid::from_slice(&stored[OBJ_BYTES + 1..HEADER_BYTES])
            .map_err(|_| Error::EncodingFailure)?;
        let name =
            std::str::from_utf8(&stored[HEADER_BYTES..]).map_err(|_| Error::EncodingFailure)?;
        match stored[OBJ_BYTES] {
            0 if uuid.is_nil() => Ok(EntityMetadataKey::object(obj, Symbol::mk(name))),
            1 => Ok(EntityMetadataKey::property(obj, uuid, Symbol::mk(name))),
            2 => Ok(EntityMetadataKey::verb(obj, uuid, Symbol::mk(name))),
            _ => Err(Error::EncodingFailure),
        }
    }
}

// Var - FlatBuffer encoding for DB storage (allows lambdas and anonymous objects)
impl EncodeFor<Var> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, value: &Var) -> Result<Self::Stored, Error> {
        let mut builder = planus::Builder::new();
        let encoded = encode_db_var(&mut builder, value).map_err(|_| Error::EncodingFailure)?;
        Ok(ByteView::from(encoded))
    }

    fn decode(&self, stored: Self::Stored) -> Result<Var, Error> {
        // Parse FlatBuffer and convert directly from ref (avoids intermediate owned struct copy)
        let fb_ref =
            moor_schema::var::VarRef::read_as_root(&stored).map_err(|_| Error::EncodingFailure)?;
        var_from_db_flatbuffer_ref(fb_ref).map_err(|_| Error::EncodingFailure)
    }
}

// FlatBuffer types - VerbDefs and PropDefs
impl EncodeFor<VerbDefs> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, value: &VerbDefs) -> Result<Self::Stored, Error> {
        let fb_verbdefs = moor_schema::convert::verbdefs_to_flatbuffer(value)
            .map_err(|_| Error::EncodingFailure)?;
        Ok(encode_flatbuffer(fb_verbdefs))
    }

    fn decode(&self, stored: Self::Stored) -> Result<VerbDefs, Error> {
        let fb_ref = moor_schema::common::VerbDefsRef::read_as_root(&stored)
            .map_err(|_| Error::EncodingFailure)?;
        fb_ref
            .verbs()
            .map_err(|_| Error::EncodingFailure)?
            .iter()
            .map(|verb| {
                let verb = verb.map_err(|_| Error::EncodingFailure)?;
                moor_schema::convert::verbdef_from_ref(verb).map_err(|_| Error::EncodingFailure)
            })
            .collect()
    }
}

impl EncodeFor<PropDefs> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, value: &PropDefs) -> Result<Self::Stored, Error> {
        let fb_propdefs = moor_schema::convert::propdefs_to_flatbuffer(value)
            .map_err(|_| Error::EncodingFailure)?;
        Ok(encode_flatbuffer(fb_propdefs))
    }

    fn decode(&self, stored: Self::Stored) -> Result<PropDefs, Error> {
        let fb_ref = moor_schema::common::PropDefsRef::read_as_root(&stored)
            .map_err(|_| Error::EncodingFailure)?;
        fb_ref
            .props()
            .map_err(|_| Error::EncodingFailure)?
            .iter()
            .map(|prop| {
                let prop = prop.map_err(|_| Error::EncodingFailure)?;
                moor_schema::convert::propdef_from_ref(prop).map_err(|_| Error::EncodingFailure)
            })
            .collect()
    }
}

// StringHolder - direct UTF-8 encoding
impl EncodeFor<StringHolder> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, value: &StringHolder) -> Result<Self::Stored, Error> {
        Ok(ByteView::from(value.0.as_bytes()))
    }

    fn decode(&self, stored: Self::Stored) -> Result<StringHolder, Error> {
        let s = String::from_utf8(stored.to_vec()).map_err(|_| Error::EncodingFailure)?;
        Ok(StringHolder(s))
    }
}

// ProgramType uses flatbuffer encoding - see below

// ============================================================================
// ProgramType - uses FlatBuffer encoding via program_convert
// ============================================================================

impl EncodeFor<ProgramType> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, program: &ProgramType) -> Result<Self::Stored, Error> {
        match program {
            ProgramType::MooR(prog) => {
                let stored = encode_program_to_fb(prog)
                    .map_err(|e| Error::StorageFailure(format!("Failed to encode program: {e}")))?;
                Ok(encode_flatbuffer(stored))
            }
        }
    }

    fn decode(&self, stored: Self::Stored) -> Result<ProgramType, Error> {
        use moor_var::program::stored_program::StoredProgram;

        let stored_program = StoredProgram::from(stored);

        // Read the FlatBuffer and extract the language union
        use moor_schema::program as fb;
        use planus::ReadAsRoot;

        let fb_program = fb::StoredProgramRef::read_as_root(stored_program.as_bytes())
            .map_err(|e| Error::StorageFailure(format!("Failed to read program: {e}")))?;

        let language = fb_program
            .language()
            .map_err(|e| Error::StorageFailure(format!("Failed to read language union: {e}")))?;

        // Match on language variant and construct appropriate ProgramType
        match language {
            fb::StoredProgramLanguageRef::StoredMooRProgram(_moor_ref) => {
                // Decode the full program using the existing function
                let program = stored_to_program(&stored_program).map_err(|e| {
                    Error::StorageFailure(format!("Failed to decode MooR program: {e}"))
                })?;
                Ok(ProgramType::MooR(program))
            }
        }
    }
}

macro_rules! impl_fjall_value_from_bytes {
    ($type:ty) => {
        impl EncodeFjallValue<$type> for FjallCodec {
            fn encode_fjall_value(
                &self,
                value: &$type,
                timestamp: Timestamp,
                _encoder: &mut BatchEncoder,
            ) -> Result<Slice, Error> {
                use zerocopy::IntoBytes;
                Ok(frame_value(IntoBytes::as_bytes(value), timestamp))
            }
        }
    };
}

macro_rules! impl_fjall_value_from_byteview {
    ($type:ty) => {
        impl EncodeFjallValue<$type> for FjallCodec {
            fn encode_fjall_value(
                &self,
                value: &$type,
                timestamp: Timestamp,
                _encoder: &mut BatchEncoder,
            ) -> Result<Slice, Error> {
                Ok(frame_value(AsRef::<ByteView>::as_ref(value), timestamp))
            }
        }
    };
}

impl_fjall_value_from_bytes!(Obj);
impl_fjall_value_from_bytes!(BitEnum<ObjFlag>);
impl_fjall_value_from_bytes!(AnonymousObjectMetadata);
impl_fjall_value_from_byteview!(ObjSet);
impl_fjall_value_from_byteview!(PropPerms);

impl EncodeFjallValue<StringHolder> for FjallCodec {
    fn encode_fjall_value(
        &self,
        value: &StringHolder,
        timestamp: Timestamp,
        _encoder: &mut BatchEncoder,
    ) -> Result<Slice, Error> {
        Ok(frame_value(value.0.as_bytes(), timestamp))
    }
}

impl EncodeFjallValue<Var> for FjallCodec {
    fn encode_fjall_value(
        &self,
        value: &Var,
        timestamp: Timestamp,
        encoder: &mut BatchEncoder,
    ) -> Result<Slice, Error> {
        let encoded =
            encode_db_var(encoder.var_builder(), value).map_err(|_| Error::EncodingFailure)?;
        Ok(frame_value(encoded, timestamp))
    }
}

impl EncodeFjallValue<VerbDefs> for FjallCodec {
    fn encode_fjall_value(
        &self,
        value: &VerbDefs,
        timestamp: Timestamp,
        _encoder: &mut BatchEncoder,
    ) -> Result<Slice, Error> {
        let fb_verbdefs = moor_schema::convert::verbdefs_to_flatbuffer(value)
            .map_err(|_| Error::EncodingFailure)?;
        Ok(frame_flatbuffer(fb_verbdefs, timestamp))
    }
}

impl EncodeFjallValue<PropDefs> for FjallCodec {
    fn encode_fjall_value(
        &self,
        value: &PropDefs,
        timestamp: Timestamp,
        _encoder: &mut BatchEncoder,
    ) -> Result<Slice, Error> {
        let fb_propdefs = moor_schema::convert::propdefs_to_flatbuffer(value)
            .map_err(|_| Error::EncodingFailure)?;
        Ok(frame_flatbuffer(fb_propdefs, timestamp))
    }
}

impl EncodeFjallValue<ProgramType> for FjallCodec {
    fn encode_fjall_value(
        &self,
        program: &ProgramType,
        timestamp: Timestamp,
        _encoder: &mut BatchEncoder,
    ) -> Result<Slice, Error> {
        match program {
            ProgramType::MooR(program) => {
                let stored = encode_program_to_fb(program).map_err(|error| {
                    Error::StorageFailure(format!("Failed to encode program: {error}"))
                })?;
                Ok(frame_flatbuffer(stored, timestamp))
            }
        }
    }
}

// ============================================================================
// Blanket impl: FjallProvider delegates to FjallCodec for all types
// ============================================================================

impl<Domain, Codomain, T> EncodeFor<T> for FjallProvider<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    FjallCodec: EncodeFor<T, Stored = ByteView>,
{
    type Stored = ByteView;

    fn encode(&self, value: &T) -> Result<Self::Stored, Error> {
        FjallCodec.encode(value)
    }

    fn decode(&self, stored: Self::Stored) -> Result<T, Error> {
        FjallCodec.decode(stored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Provider;
    use fjall::KeyspaceCreateOptions;
    use moor_var::v_str;

    fn metadata_keys(variant: bool) -> Vec<EntityMetadataKey> {
        let names = if variant {
            ["RCS_ID", "STRASSE", "ς"]
        } else {
            ["rcs_id", "Straße", "σ"]
        };
        let obj = Obj::mk_id(7);
        let uuid = uuid::Uuid::from_u128(42);
        names
            .into_iter()
            .flat_map(|name| {
                let name = Symbol::mk(name);
                [
                    EntityMetadataKey::object(obj, name),
                    EntityMetadataKey::property(obj, uuid, name),
                    EntityMetadataKey::verb(obj, uuid, name),
                ]
            })
            .collect()
    }

    #[test]
    fn metadata_key_encoding_matches_symbol_equality() {
        for (key, variant) in metadata_keys(false).into_iter().zip(metadata_keys(true)) {
            assert_eq!(key, variant);
            let encoded = FjallCodec.encode(&key).unwrap();
            assert_eq!(encoded, FjallCodec.encode(&variant).unwrap());
            let decoded: EntityMetadataKey = FjallCodec.decode(encoded).unwrap();
            assert_eq!(decoded, key);
        }
    }

    #[test]
    fn metadata_key_encoding_preserves_object_prefix_and_entity_identity() {
        use zerocopy::IntoBytes;

        for obj in [Obj::mk_id(-1), Obj::mk_id(42), Obj::mk_uuobjid_generated()] {
            let uuid = uuid::Uuid::new_v4();
            let name = Symbol::mk("rcs_id");
            let keys = [
                EntityMetadataKey::object(obj, name),
                EntityMetadataKey::property(obj, uuid, name),
                EntityMetadataKey::verb(obj, uuid, name),
            ];
            let mut encoded_keys = std::collections::HashSet::new();
            for key in keys {
                let encoded = FjallCodec.encode(&key).unwrap();
                assert!(encoded.starts_with(obj.as_bytes()));
                assert!(encoded.ends_with(b"rcs_id"));
                assert!(encoded_keys.insert(encoded.clone()));
                let decoded: EntityMetadataKey = FjallCodec.decode(encoded).unwrap();
                assert_eq!(decoded, key);
            }
        }
    }

    #[test]
    fn metadata_key_decoder_rejects_malformed_keys() {
        let key = EntityMetadataKey::object(Obj::mk_id(7), Symbol::mk("rcs_id"));
        let encoded = FjallCodec.encode(&key).unwrap();
        let decode = |bytes: Vec<u8>| {
            <FjallCodec as EncodeFor<EntityMetadataKey>>::decode(&FjallCodec, bytes.into())
        };
        for len in 0..std::mem::size_of::<Obj>() + 17 {
            assert!(decode(encoded[..len].to_vec()).is_err());
        }
        let mut invalid_tag = encoded.to_vec();
        invalid_tag[std::mem::size_of::<Obj>()] = 3;
        assert!(decode(invalid_tag).is_err());
        let mut object_with_uuid = encoded.to_vec();
        object_with_uuid[std::mem::size_of::<Obj>() + 1] = 1;
        assert!(decode(object_with_uuid).is_err());
        let mut invalid_utf8 = encoded.to_vec();
        *invalid_utf8.last_mut().unwrap() = 0xff;
        assert!(decode(invalid_utf8).is_err());
    }

    /// Each phase runs in a fresh process with a different symbol interning order.
    #[test]
    fn metadata_keys_survive_process_restart() {
        const PHASE_ENV: &str = "MOOR_METADATA_KEY_TEST_PHASE";
        const PATH_ENV: &str = "MOOR_METADATA_KEY_TEST_PATH";
        let Ok(phase) = std::env::var(PHASE_ENV) else {
            let directory = tempfile::tempdir().unwrap();
            for phase in 0..4 {
                let output = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "provider::fjall_provider::tests::metadata_keys_survive_process_restart",
                        "--nocapture",
                    ])
                    .env(PHASE_ENV, phase.to_string())
                    .env(PATH_ENV, directory.path())
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "metadata restart phase {phase} failed:\n{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
            }
            return;
        };
        let phase: usize = phase.parse().unwrap();
        for index in 0..phase * 100 {
            Symbol::mk(&format!("unrelated-{phase}-{index}"));
        }
        let database = fjall::Database::builder(std::env::var_os(PATH_ENV).unwrap())
            .open()
            .unwrap();
        let partition = database
            .keyspace("metadata", KeyspaceCreateOptions::default)
            .unwrap();
        let provider = FjallProvider::<EntityMetadataKey, Var>::new("metadata", partition);
        let keys = metadata_keys(phase == 1 || phase == 3);
        let initial = v_str("release:one");
        let updated = v_str("release:two");
        if phase == 0 {
            for key in &keys {
                provider.put(Timestamp(1), key, &initial).unwrap();
            }
            return;
        }
        let rows = provider.scan(&|_, _| true).unwrap();
        if phase == 3 {
            assert!(rows.is_empty());
            for key in &keys {
                assert_eq!(provider.get(key).unwrap(), None);
            }
            return;
        }
        assert_eq!(rows.len(), keys.len());
        let expected = if phase == 1 { initial } else { updated.clone() };
        for key in &keys {
            assert_eq!(
                provider.get(key).unwrap(),
                Some((Timestamp(phase as u64), expected.clone()))
            );
            assert!(
                rows.iter()
                    .any(|(_, stored_key, value)| stored_key == key && value == &expected)
            );
            if phase == 1 {
                provider.put(Timestamp(2), key, &updated).unwrap();
            } else {
                provider.del(Timestamp(3), key).unwrap();
            }
        }
    }

    #[test]
    fn a_previous_miss_does_not_mask_a_later_value() {
        let tempdir = tempfile::tempdir().unwrap();
        let database = fjall::Database::builder(tempdir.path()).open().unwrap();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let provider = FjallProvider::<Obj, Obj>::new("values", partition.clone());
        let key = Obj::mk_id(7);
        let value = Obj::mk_id(8);

        assert_eq!(provider.get(&key).unwrap(), None);

        provider.put(Timestamp(17), &key, &value).unwrap();

        assert_eq!(provider.get(&key).unwrap(), Some((Timestamp(17), value)));
    }

    #[test]
    fn stored_values_suffix_the_timestamp_outside_the_codec_payload() {
        let tempdir = tempfile::tempdir().unwrap();
        let database = fjall::Database::builder(tempdir.path()).open().unwrap();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let provider = FjallProvider::<Obj, Obj>::new("values", partition.clone());
        let key = Obj::mk_id(7);
        let value = Obj::mk_id(8);

        provider.put(Timestamp(123), &key, &value).unwrap();

        let encoded_key = FjallCodec.encode(&key).unwrap();
        let encoded_value = FjallCodec.encode(&value).unwrap();
        let stored = partition.get(encoded_key).unwrap().unwrap();
        let payload_len = stored.len() - TIMESTAMP_BYTES;
        assert_eq!(&stored[..payload_len], &*encoded_value);
        assert_eq!(&stored[payload_len..], &123_u64.to_le_bytes());
        assert_eq!(provider.get(&key).unwrap(), Some((Timestamp(123), value)));
    }

    #[test]
    fn flatbuffer_payload_remains_valid_inside_timestamp_frame() {
        let tempdir = tempfile::tempdir().unwrap();
        let database = fjall::Database::builder(tempdir.path()).open().unwrap();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let provider = FjallProvider::<Obj, Var>::new("values", partition.clone());
        let key = Obj::mk_id(7);
        let value = v_str(&"context".repeat(1_024));

        provider.put(Timestamp(456), &key, &value).unwrap();

        let encoded_key = FjallCodec.encode(&key).unwrap();
        let encoded_value = FjallCodec.encode(&value).unwrap();
        let stored = partition.get(encoded_key).unwrap().unwrap();
        let payload_len = stored.len() - TIMESTAMP_BYTES;
        assert_eq!(&stored[..payload_len], &*encoded_value);
        assert_eq!(&stored[payload_len..], &456_u64.to_le_bytes());
        assert_eq!(provider.get(&key).unwrap(), Some((Timestamp(456), value)));
    }
}
