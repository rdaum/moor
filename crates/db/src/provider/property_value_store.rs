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

//! Physical record codec and bounded reconstruction for incremental property values.

use crate::{ObjAndUUIDHolder, db_counters, tx::Timestamp};
use moor_common::{
    model::{WorldStateCountOp, WorldStateTimerOp},
    util::Instant,
};
use moor_schema::convert::{encode_db_var, var_from_db_flatbuffer_ref};
use moor_var::{List, Var};
use planus::ReadAsRoot;
use smallvec::{SmallVec, smallvec};
use zerocopy::{FromBytes, IntoBytes};

#[cfg(test)]
use super::logical::PreparedPropertyValueMutation;
#[cfg(test)]
use super::logical::prepare_property_value_mutation;
#[cfg(test)]
use crate::tx::OpType;

const RECORD_MAGIC: [u8; 4] = *b"MPRV";
const RECORD_FORMAT_VERSION: u8 = 1;
const RECORD_HEADER_BYTES: usize = 16;
const PROPERTY_KEY_BYTES: usize = std::mem::size_of::<ObjAndUUIDHolder>();
pub(crate) const PROPERTY_RECORD_KEY_BYTES: usize = PROPERTY_KEY_BYTES + size_of::<u64>();

const FULL_RECORD_KIND: u8 = 0;
const LIST_APPEND_RECORD_KIND: u8 = 1;

pub(crate) const PROPERTY_VALUE_CHAIN_LIMITS: PropertyValueChainLimits =
    PropertyValueChainLimits::new(64, 4 * 1024 * 1024);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PropertyValueRecordKind {
    Full,
    ListAppend,
}

impl PropertyValueRecordKind {
    fn as_byte(self) -> u8 {
        match self {
            Self::Full => FULL_RECORD_KIND,
            Self::ListAppend => LIST_APPEND_RECORD_KIND,
        }
    }

    fn from_byte(value: u8) -> Result<Self, PropertyValueRecordError> {
        match value {
            FULL_RECORD_KIND => Ok(Self::Full),
            LIST_APPEND_RECORD_KIND => Ok(Self::ListAppend),
            _ => Err(PropertyValueRecordError::UnknownRecordKind(value)),
        }
    }
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PropertyValueRecordError {
    #[error("failed to read property-value storage: {0}")]
    Storage(String),
    #[error("property-value record key has an invalid length")]
    InvalidKeyLength,
    #[error("property-value record has a truncated header")]
    TruncatedHeader,
    #[error("property-value record has an invalid magic value")]
    InvalidMagic,
    #[error("property-value record uses unsupported format version {0}")]
    UnsupportedFormatVersion(u8),
    #[error("property-value record uses unknown kind {0}")]
    UnknownRecordKind(u8),
    #[error("property-value record has nonzero reserved header bytes")]
    InvalidReservedBytes,
    #[error("property-value record contains an invalid Var payload")]
    InvalidVarPayload,
    #[error("property-value chain is empty")]
    EmptyChain,
    #[error("property-value chain does not start with a complete value")]
    MissingFullRecord,
    #[error("property-value chain contains a second complete value")]
    UnexpectedFullRecord,
    #[error("property-value records are not in record-version order")]
    RecordOrder,
    #[error("property-value list append has an empty suffix")]
    EmptyListAppend,
    #[error("property-value list append does not contain a list")]
    InvalidListAppend,
    #[error("property-value list append follows a non-list value")]
    AppendToNonList,
    #[error("property-value chain exceeds its append-record limit")]
    AppendRecordLimit,
    #[error("property-value chain exceeds its append-byte limit")]
    AppendByteLimit,
    #[error("property-value range contains records for more than one property")]
    MultipleProperties,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PropertyValueChainLimits {
    /// Maximum number of records, including the initial complete record.
    pub max_records: usize,
    /// Maximum encoded payload bytes across all append records.
    pub max_append_bytes: usize,
}

impl PropertyValueChainLimits {
    pub const fn new(max_records: usize, max_append_bytes: usize) -> Self {
        Self {
            max_records,
            max_append_bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PropertyValueRecordKey {
    pub property: ObjAndUUIDHolder,
    pub record_version: u64,
}

pub(crate) fn encode_property_value_record_key(
    property: &ObjAndUUIDHolder,
    record_version: u64,
) -> [u8; PROPERTY_RECORD_KEY_BYTES] {
    let mut key = [0; PROPERTY_RECORD_KEY_BYTES];
    key[..PROPERTY_KEY_BYTES].copy_from_slice(property.as_bytes());
    key[PROPERTY_KEY_BYTES..].copy_from_slice(&record_version.to_be_bytes());
    key
}

pub(crate) fn decode_property_value_record_key(
    key: &[u8],
) -> Result<PropertyValueRecordKey, PropertyValueRecordError> {
    if key.len() != PROPERTY_RECORD_KEY_BYTES {
        return Err(PropertyValueRecordError::InvalidKeyLength);
    }

    let property = ObjAndUUIDHolder::read_from_bytes(&key[..PROPERTY_KEY_BYTES])
        .map_err(|_| PropertyValueRecordError::InvalidKeyLength)?;
    let record_version = u64::from_be_bytes(
        key[PROPERTY_KEY_BYTES..]
            .try_into()
            .map_err(|_| PropertyValueRecordError::InvalidKeyLength)?,
    );
    Ok(PropertyValueRecordKey {
        property,
        record_version,
    })
}

pub(crate) fn property_value_record_bounds(
    property: &ObjAndUUIDHolder,
) -> (
    [u8; PROPERTY_RECORD_KEY_BYTES],
    [u8; PROPERTY_RECORD_KEY_BYTES],
) {
    (
        encode_property_value_record_key(property, 0),
        encode_property_value_record_key(property, u64::MAX),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PropertyValueRecordRef<'a> {
    pub logical_timestamp: Timestamp,
    pub kind: PropertyValueRecordKind,
    pub payload: &'a [u8],
}

pub(crate) fn encode_full_record(
    builder: &mut planus::Builder,
    value: &Var,
    logical_timestamp: Timestamp,
) -> Result<Vec<u8>, PropertyValueRecordError> {
    let payload =
        encode_db_var(builder, value).map_err(|_| PropertyValueRecordError::InvalidVarPayload)?;
    Ok(frame_record(
        PropertyValueRecordKind::Full,
        logical_timestamp,
        payload,
    ))
}

pub(crate) fn encode_list_append_record(
    builder: &mut planus::Builder,
    suffix: &List,
    logical_timestamp: Timestamp,
) -> Result<Vec<u8>, PropertyValueRecordError> {
    if suffix.is_empty() {
        return Err(PropertyValueRecordError::EmptyListAppend);
    }
    let suffix = Var::from(suffix.clone());
    let payload =
        encode_db_var(builder, &suffix).map_err(|_| PropertyValueRecordError::InvalidVarPayload)?;
    Ok(frame_record(
        PropertyValueRecordKind::ListAppend,
        logical_timestamp,
        payload,
    ))
}

fn frame_record(
    kind: PropertyValueRecordKind,
    logical_timestamp: Timestamp,
    payload: &[u8],
) -> Vec<u8> {
    let mut record = Vec::with_capacity(RECORD_HEADER_BYTES + payload.len());
    record.extend_from_slice(&RECORD_MAGIC);
    record.push(RECORD_FORMAT_VERSION);
    record.push(kind.as_byte());
    record.extend_from_slice(&[0, 0]);
    record.extend_from_slice(&logical_timestamp.0.to_le_bytes());
    record.extend_from_slice(payload);
    record
}

pub(crate) fn decode_property_value_record(
    record: &[u8],
) -> Result<PropertyValueRecordRef<'_>, PropertyValueRecordError> {
    if record.len() < RECORD_HEADER_BYTES {
        return Err(PropertyValueRecordError::TruncatedHeader);
    }
    if record[..4] != RECORD_MAGIC {
        return Err(PropertyValueRecordError::InvalidMagic);
    }
    if record[4] != RECORD_FORMAT_VERSION {
        return Err(PropertyValueRecordError::UnsupportedFormatVersion(
            record[4],
        ));
    }
    let kind = PropertyValueRecordKind::from_byte(record[5])?;
    if record[6..8] != [0, 0] {
        return Err(PropertyValueRecordError::InvalidReservedBytes);
    }
    let logical_timestamp = Timestamp(u64::from_le_bytes(
        record[8..16]
            .try_into()
            .map_err(|_| PropertyValueRecordError::TruncatedHeader)?,
    ));
    Ok(PropertyValueRecordRef {
        logical_timestamp,
        kind,
        payload: &record[RECORD_HEADER_BYTES..],
    })
}

pub(crate) fn property_value_record_payload_bytes(
    record: &[u8],
) -> Result<usize, PropertyValueRecordError> {
    Ok(decode_property_value_record(record)?.payload.len())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PropertyValueChain {
    record_versions: SmallVec<[u64; 1]>,
    append_bytes: usize,
}

impl PropertyValueChain {
    pub fn full(record_version: u64) -> Self {
        Self {
            record_versions: smallvec![record_version],
            append_bytes: 0,
        }
    }

    #[cfg(test)]
    pub fn full_version(&self) -> u64 {
        self.record_versions[0]
    }

    #[cfg(test)]
    pub fn append_versions(&self) -> &[u64] {
        &self.record_versions[1..]
    }

    #[cfg(test)]
    pub fn append_count(&self) -> usize {
        self.record_versions.len() - 1
    }

    #[cfg(test)]
    pub fn append_bytes(&self) -> usize {
        self.append_bytes
    }

    pub fn record_versions(&self) -> impl Iterator<Item = u64> + '_ {
        self.record_versions.iter().copied()
    }

    pub fn reaches_limit(&self, additional_bytes: usize, limits: PropertyValueChainLimits) -> bool {
        self.record_versions.len() >= limits.max_records
            || self.append_bytes.saturating_add(additional_bytes) > limits.max_append_bytes
    }

    pub fn push_append(&mut self, record_version: u64, append_bytes: usize) {
        self.record_versions.push(record_version);
        self.append_bytes = self.append_bytes.saturating_add(append_bytes);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReconstructedPropertyValue {
    pub logical_timestamp: Timestamp,
    pub value: Var,
    pub chain: PropertyValueChain,
}

pub(crate) struct PropertyValueReconstructor {
    limits: PropertyValueChainLimits,
    value: Option<Var>,
    logical_timestamp: Timestamp,
    record_versions: SmallVec<[u64; 1]>,
    append_bytes: usize,
    last_version: Option<u64>,
    record_count: usize,
    started_at: Instant,
}

impl PropertyValueReconstructor {
    pub fn new(limits: PropertyValueChainLimits) -> Self {
        Self {
            limits,
            value: None,
            logical_timestamp: Timestamp(0),
            record_versions: SmallVec::new(),
            append_bytes: 0,
            last_version: None,
            record_count: 0,
            started_at: Instant::now(),
        }
    }

    pub fn push(
        &mut self,
        record_version: u64,
        record: &[u8],
    ) -> Result<(), PropertyValueRecordError> {
        let record = decode_property_value_record(record)?;
        self.push_decoded(
            record_version,
            record.kind,
            record.logical_timestamp,
            record.payload.len(),
            decode_var(record.payload)?,
        )
    }

    /// Apply a decoded record with backend-specific payload size accounting.
    pub(crate) fn push_decoded(
        &mut self,
        record_version: u64,
        kind: PropertyValueRecordKind,
        logical_timestamp: Timestamp,
        payload_bytes: usize,
        decoded: Var,
    ) -> Result<(), PropertyValueRecordError> {
        if self
            .last_version
            .is_some_and(|last_version| record_version <= last_version)
        {
            return Err(PropertyValueRecordError::RecordOrder);
        }

        match (&self.value, kind) {
            (None, PropertyValueRecordKind::Full) => {
                self.value = Some(decoded);
                self.record_versions.push(record_version);
            }
            (None, PropertyValueRecordKind::ListAppend) => {
                return Err(PropertyValueRecordError::MissingFullRecord);
            }
            (Some(_), PropertyValueRecordKind::Full) => {
                return Err(PropertyValueRecordError::UnexpectedFullRecord);
            }
            (Some(value), PropertyValueRecordKind::ListAppend) => {
                if self.record_versions.len() >= self.limits.max_records {
                    return Err(PropertyValueRecordError::AppendRecordLimit);
                }
                let append_bytes = self
                    .append_bytes
                    .checked_add(payload_bytes)
                    .ok_or(PropertyValueRecordError::AppendByteLimit)?;
                if append_bytes > self.limits.max_append_bytes {
                    return Err(PropertyValueRecordError::AppendByteLimit);
                }

                let suffix = decoded;
                let Some(suffix) = suffix.as_list() else {
                    return Err(PropertyValueRecordError::InvalidListAppend);
                };
                if suffix.is_empty() {
                    return Err(PropertyValueRecordError::EmptyListAppend);
                }
                let Some(value) = value.as_list() else {
                    return Err(PropertyValueRecordError::AppendToNonList);
                };
                let appended = value
                    .clone()
                    .append_owned(&Var::from(suffix.clone()))
                    .map_err(|_| PropertyValueRecordError::InvalidListAppend)?
                    .with_cleared_hint();
                self.value = Some(appended);
                self.record_versions.push(record_version);
                self.append_bytes = append_bytes;
            }
        }

        self.logical_timestamp = logical_timestamp;
        self.last_version = Some(record_version);
        self.record_count += 1;
        Ok(())
    }

    pub fn finish(self) -> Result<ReconstructedPropertyValue, PropertyValueRecordError> {
        let Some(value) = self.value else {
            return Err(PropertyValueRecordError::EmptyChain);
        };
        db_counters().timers_rare.record_elapsed(
            WorldStateTimerOp::PropertyValueReconstruct,
            self.started_at.elapsed(),
        );
        db_counters().counters.add(
            WorldStateCountOp::PropertyValueReconstructedRecords,
            isize::try_from(self.record_count).unwrap_or(isize::MAX),
        );
        Ok(ReconstructedPropertyValue {
            logical_timestamp: self.logical_timestamp,
            value,
            chain: PropertyValueChain {
                record_versions: self.record_versions,
                append_bytes: self.append_bytes,
            },
        })
    }
}

struct ActiveProperty {
    property: ObjAndUUIDHolder,
    reconstructor: PropertyValueReconstructor,
}

impl ActiveProperty {
    fn finish(
        self,
    ) -> Result<(ObjAndUUIDHolder, ReconstructedPropertyValue), PropertyValueRecordError> {
        Ok((self.property, self.reconstructor.finish()?))
    }
}

enum PropertyScanState {
    Idle,
    Active(ActiveProperty),
    Terminal,
}

pub(crate) struct PropertyValueScan {
    records: fjall::Iter,
    limits: PropertyValueChainLimits,
    state: PropertyScanState,
    pending: Option<(fjall::Slice, fjall::Slice)>,
}

impl PropertyValueScan {
    pub fn new(records: fjall::Iter, limits: PropertyValueChainLimits) -> Self {
        Self {
            records,
            limits,
            state: PropertyScanState::Idle,
            pending: None,
        }
    }

    fn fail(
        &mut self,
        error: PropertyValueRecordError,
    ) -> Option<Result<(ObjAndUUIDHolder, ReconstructedPropertyValue), PropertyValueRecordError>>
    {
        self.state = PropertyScanState::Terminal;
        self.pending = None;
        Some(Err(error))
    }
}

impl Iterator for PropertyValueScan {
    type Item = Result<(ObjAndUUIDHolder, ReconstructedPropertyValue), PropertyValueRecordError>;

    fn next(&mut self) -> Option<Self::Item> {
        if matches!(self.state, PropertyScanState::Terminal) {
            return None;
        }

        loop {
            let record = if let Some(record) = self.pending.take() {
                record
            } else {
                let Some(record) = self.records.next() else {
                    return match std::mem::replace(&mut self.state, PropertyScanState::Terminal) {
                        PropertyScanState::Active(active) => Some(active.finish()),
                        PropertyScanState::Idle | PropertyScanState::Terminal => None,
                    };
                };
                match record.into_inner() {
                    Ok(record) => record,
                    Err(error) => {
                        return self.fail(PropertyValueRecordError::Storage(error.to_string()));
                    }
                }
            };
            let (key, value) = record;
            let decoded_key = match decode_property_value_record_key(&key) {
                Ok(key) => key,
                Err(error) => return self.fail(error),
            };

            let mut active = match std::mem::replace(&mut self.state, PropertyScanState::Terminal) {
                PropertyScanState::Idle => ActiveProperty {
                    property: decoded_key.property,
                    reconstructor: PropertyValueReconstructor::new(self.limits),
                },
                PropertyScanState::Active(active) if active.property != decoded_key.property => {
                    let finished = match active.finish() {
                        Ok(finished) => finished,
                        Err(error) => return self.fail(error),
                    };
                    self.pending = Some((key, value));
                    self.state = PropertyScanState::Idle;
                    return Some(Ok(finished));
                }
                PropertyScanState::Active(active) => active,
                PropertyScanState::Terminal => return None,
            };
            if let Err(error) = active
                .reconstructor
                .push(decoded_key.record_version, &value)
            {
                return self.fail(error);
            }
            self.state = PropertyScanState::Active(active);
        }
    }
}

impl std::iter::FusedIterator for PropertyValueScan {}

pub(crate) fn reconstruct_property_value(
    records: fjall::Iter,
    limits: PropertyValueChainLimits,
) -> Result<Option<(ObjAndUUIDHolder, ReconstructedPropertyValue)>, PropertyValueRecordError> {
    let mut scan = PropertyValueScan::new(records, limits);
    let first = scan.next().transpose()?;
    if scan.next().transpose()?.is_some() {
        return Err(PropertyValueRecordError::MultipleProperties);
    }
    Ok(first)
}

fn decode_var(payload: &[u8]) -> Result<Var, PropertyValueRecordError> {
    let value = moor_schema::var::VarRef::read_as_root(payload)
        .map_err(|_| PropertyValueRecordError::InvalidVarPayload)?;
    var_from_db_flatbuffer_ref(value).map_err(|_| PropertyValueRecordError::InvalidVarPayload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{Obj, v_int, v_list, v_str};
    use uuid::Uuid;

    const LIMITS: PropertyValueChainLimits = PropertyValueChainLimits::new(4, 4096);

    fn holder() -> ObjAndUUIDHolder {
        ObjAndUUIDHolder::new(&Obj::mk_id(42), Uuid::from_u128(0x1234))
    }

    fn scan_partition() -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
        let directory = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(directory.path()).open().unwrap();
        let partition = db
            .keyspace("values", fjall::KeyspaceCreateOptions::default)
            .unwrap();
        (directory, db, partition)
    }

    #[test]
    fn scan_finishes_empty_single_and_multiple_properties_once() {
        let (_directory, _db, partition) = scan_partition();
        let mut empty = PropertyValueScan::new(partition.iter(), LIMITS);
        assert!(empty.next().is_none());
        assert!(empty.next().is_none());

        let first = holder();
        let second = ObjAndUUIDHolder::new(&Obj::mk_id(42), Uuid::from_u128(0x1235));
        let mut builder = planus::Builder::new();
        let full = encode_full_record(&mut builder, &v_list(&[v_int(1)]), Timestamp(1)).unwrap();
        partition
            .insert(encode_property_value_record_key(&first, 1), full)
            .unwrap();
        let append =
            encode_list_append_record(&mut builder, &List::mk_list(&[v_int(2)]), Timestamp(2))
                .unwrap();
        partition
            .insert(encode_property_value_record_key(&first, 2), append)
            .unwrap();
        let mut single = PropertyValueScan::new(partition.iter(), LIMITS);
        let (property, value) = single.next().unwrap().unwrap();
        assert_eq!(property, first);
        assert_eq!(value.value, v_list(&[v_int(1), v_int(2)]));
        assert_eq!(value.logical_timestamp, Timestamp(2));
        assert_eq!(
            value.chain.record_versions().collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(single.next().is_none());
        assert!(single.next().is_none());

        let full = encode_full_record(&mut builder, &v_int(3), Timestamp(3)).unwrap();
        partition
            .insert(encode_property_value_record_key(&second, 3), full)
            .unwrap();
        let mut scan = PropertyValueScan::new(partition.iter(), LIMITS);
        assert_eq!(scan.next().unwrap().unwrap().0, first);
        let (property, value) = scan.next().unwrap().unwrap();
        assert_eq!(property, second);
        assert_eq!(value.value, v_int(3));
        assert_eq!(value.chain, PropertyValueChain::full(3));
        assert!(scan.next().is_none());
        assert!(scan.next().is_none());
    }

    #[test]
    fn scan_terminates_after_key_or_pending_record_decode_error() {
        let (_directory, _db, partition) = scan_partition();
        partition.insert(b"bad", b"record").unwrap();
        let mut scan = PropertyValueScan::new(partition.iter(), LIMITS);
        assert_eq!(
            scan.next(),
            Some(Err(PropertyValueRecordError::InvalidKeyLength))
        );
        assert!(scan.next().is_none());
        assert!(scan.next().is_none());
        partition.remove(b"bad").unwrap();

        let first = holder();
        let second = ObjAndUUIDHolder::new(&Obj::mk_id(42), Uuid::from_u128(0x1235));
        let third = ObjAndUUIDHolder::new(&Obj::mk_id(42), Uuid::from_u128(0x1236));
        let full =
            encode_full_record(&mut planus::Builder::new(), &v_int(1), Timestamp(1)).unwrap();
        partition
            .insert(encode_property_value_record_key(&first, 1), full.clone())
            .unwrap();
        partition
            .insert(encode_property_value_record_key(&second, 2), b"bad")
            .unwrap();
        partition
            .insert(encode_property_value_record_key(&third, 3), full)
            .unwrap();
        let mut scan = PropertyValueScan::new(partition.iter(), LIMITS);
        assert_eq!(scan.next().unwrap().unwrap().0, first);
        assert_eq!(
            scan.next(),
            Some(Err(PropertyValueRecordError::TruncatedHeader))
        );
        // A later valid property must not revive a failed scan.
        assert!(scan.next().is_none());
        assert!(scan.next().is_none());
    }

    #[test]
    fn prepares_a_shared_list_append() {
        let base = (0..4096).map(v_int).collect::<List>();
        let final_value = base.clone().push_owned(&v_int(4096));

        let prepared = prepare_property_value_mutation(
            Some(&Var::from(base)),
            OpType::Update(final_value.clone()),
        );

        let PreparedPropertyValueMutation::AppendList {
            suffix,
            final_value: prepared_value,
        } = prepared
        else {
            panic!("expected a prepared list append");
        };
        assert_eq!(suffix, List::mk_list(&[v_int(4096)]));
        assert_eq!(prepared_value, final_value);
    }

    #[test]
    fn prepares_replacements_and_deletes() {
        let replacement = prepare_property_value_mutation(None, OpType::Insert(v_int(7)));
        assert_eq!(
            replacement,
            PreparedPropertyValueMutation::Replace { value: v_int(7) }
        );

        let rebuilt_base = (0..4096).map(v_int).collect::<List>();
        let rebuilt_final = (0..4097).map(v_int).collect::<List>();
        let replacement = prepare_property_value_mutation(
            Some(&Var::from(rebuilt_base)),
            OpType::Update(Var::from_list_with_hint(
                rebuilt_final,
                moor_var::OP_HINT_LIST_APPEND,
            )),
        );
        assert!(matches!(
            replacement,
            PreparedPropertyValueMutation::Replace { .. }
        ));

        assert_eq!(
            prepare_property_value_mutation(None, OpType::Delete),
            PreparedPropertyValueMutation::Delete
        );
    }

    #[test]
    fn record_key_orders_record_versions() {
        let property = holder();
        let first = encode_property_value_record_key(&property, 1);
        let second = encode_property_value_record_key(&property, 256);

        assert_eq!(&first[..PROPERTY_KEY_BYTES], property.as_bytes());
        assert!(first < second);
        assert_eq!(
            decode_property_value_record_key(&second).unwrap(),
            PropertyValueRecordKey {
                property,
                record_version: 256,
            }
        );
    }

    #[test]
    fn record_bounds_cover_all_record_versions() {
        let property = holder();
        let (lower, upper) = property_value_record_bounds(&property);

        assert_eq!(lower, encode_property_value_record_key(&property, 0));
        assert_eq!(upper, encode_property_value_record_key(&property, u64::MAX));
    }

    #[test]
    fn record_header_has_stable_bytes() {
        let mut builder = planus::Builder::new();
        let record = encode_full_record(&mut builder, &v_int(7), Timestamp(0x0102)).unwrap();

        assert_eq!(
            &record[..RECORD_HEADER_BYTES],
            b"MPRV\x01\x00\x00\x00\x02\x01\x00\x00\x00\x00\x00\x00"
        );
    }

    #[test]
    fn reconstructs_a_bounded_append_chain() {
        let mut builder = planus::Builder::new();
        let full = encode_full_record(
            &mut builder,
            &v_list(&[v_str("one"), v_str("two")]),
            Timestamp(10),
        )
        .unwrap();
        let suffix = List::mk_list(&[v_str("three"), v_str("four")]);
        let append = encode_list_append_record(&mut builder, &suffix, Timestamp(11)).unwrap();
        let mut reconstructor = PropertyValueReconstructor::new(LIMITS);

        reconstructor.push(100, &full).unwrap();
        reconstructor.push(101, &append).unwrap();
        let value = reconstructor.finish().unwrap();

        assert_eq!(
            value.value,
            v_list(&[v_str("one"), v_str("two"), v_str("three"), v_str("four")])
        );
        assert_eq!(value.logical_timestamp, Timestamp(11));
        assert_eq!(value.value.op_hint(), 0);
        assert_eq!(value.chain.full_version(), 100);
        assert_eq!(value.chain.append_versions(), &[101]);
        assert_eq!(value.chain.append_count(), 1);
        assert_eq!(
            value.chain.append_bytes(),
            append.len() - RECORD_HEADER_BYTES
        );
    }

    #[test]
    fn rejects_an_append_without_a_full_record() {
        let mut builder = planus::Builder::new();
        let suffix = List::mk_list(&[v_int(1)]);
        let append = encode_list_append_record(&mut builder, &suffix, Timestamp(1)).unwrap();
        let mut reconstructor = PropertyValueReconstructor::new(LIMITS);

        assert_eq!(
            reconstructor.push(1, &append),
            Err(PropertyValueRecordError::MissingFullRecord)
        );
    }

    #[test]
    fn rejects_out_of_order_records() {
        let mut builder = planus::Builder::new();
        let full = encode_full_record(&mut builder, &v_list(&[]), Timestamp(1)).unwrap();
        let suffix = List::mk_list(&[v_int(1)]);
        let append = encode_list_append_record(&mut builder, &suffix, Timestamp(2)).unwrap();
        let mut reconstructor = PropertyValueReconstructor::new(LIMITS);

        reconstructor.push(2, &full).unwrap();
        assert_eq!(
            reconstructor.push(1, &append),
            Err(PropertyValueRecordError::RecordOrder)
        );
    }

    #[test]
    fn rejects_record_and_byte_limit_overflow() {
        let mut builder = planus::Builder::new();
        let full = encode_full_record(&mut builder, &v_list(&[]), Timestamp(1)).unwrap();
        let suffix = List::mk_list(&[v_str("suffix")]);
        let append = encode_list_append_record(&mut builder, &suffix, Timestamp(2)).unwrap();
        let mut count_limited =
            PropertyValueReconstructor::new(PropertyValueChainLimits::new(0, usize::MAX));
        count_limited.push(1, &full).unwrap();
        assert_eq!(
            count_limited.push(2, &append),
            Err(PropertyValueRecordError::AppendRecordLimit)
        );

        let mut byte_limited = PropertyValueReconstructor::new(PropertyValueChainLimits::new(2, 0));
        byte_limited.push(1, &full).unwrap();
        assert_eq!(
            byte_limited.push(2, &append),
            Err(PropertyValueRecordError::AppendByteLimit)
        );
    }

    #[test]
    fn chain_accepts_the_exact_append_byte_limit() {
        let mut chain = PropertyValueChain::full(1);
        chain.push_append(2, 40);

        assert!(!chain.reaches_limit(60, PropertyValueChainLimits::new(3, 100)));
        assert!(chain.reaches_limit(61, PropertyValueChainLimits::new(3, 100)));
    }

    #[test]
    fn rejects_corrupt_record_headers() {
        assert_eq!(
            decode_property_value_record(b"short"),
            Err(PropertyValueRecordError::TruncatedHeader)
        );

        let mut record = vec![0; RECORD_HEADER_BYTES];
        record[..4].copy_from_slice(&RECORD_MAGIC);
        record[4] = RECORD_FORMAT_VERSION;
        record[5] = 99;
        assert_eq!(
            decode_property_value_record(&record),
            Err(PropertyValueRecordError::UnknownRecordKind(99))
        );

        record[5] = FULL_RECORD_KIND;
        record[4] = RECORD_FORMAT_VERSION + 1;
        assert_eq!(
            decode_property_value_record(&record),
            Err(PropertyValueRecordError::UnsupportedFormatVersion(2))
        );

        record[4] = RECORD_FORMAT_VERSION;
        record[6] = 1;
        assert_eq!(
            decode_property_value_record(&record),
            Err(PropertyValueRecordError::InvalidReservedBytes)
        );
    }

    #[test]
    fn rejects_invalid_chain_shapes() {
        let mut builder = planus::Builder::new();
        let full = encode_full_record(&mut builder, &v_list(&[]), Timestamp(1)).unwrap();
        let second_full = encode_full_record(&mut builder, &v_list(&[]), Timestamp(2)).unwrap();
        let mut reconstructor = PropertyValueReconstructor::new(LIMITS);
        reconstructor.push(1, &full).unwrap();
        assert_eq!(
            reconstructor.push(2, &second_full),
            Err(PropertyValueRecordError::UnexpectedFullRecord)
        );

        let empty = PropertyValueReconstructor::new(LIMITS);
        assert_eq!(empty.finish(), Err(PropertyValueRecordError::EmptyChain));
    }

    #[test]
    fn rejects_invalid_append_payloads() {
        let mut builder = planus::Builder::new();
        assert_eq!(
            encode_list_append_record(&mut builder, &List::mk_list(&[]), Timestamp(1)),
            Err(PropertyValueRecordError::EmptyListAppend)
        );

        let full = encode_full_record(&mut builder, &v_int(1), Timestamp(1)).unwrap();
        let suffix = List::mk_list(&[v_int(2)]);
        let append = encode_list_append_record(&mut builder, &suffix, Timestamp(2)).unwrap();
        let mut reconstructor = PropertyValueReconstructor::new(LIMITS);
        reconstructor.push(1, &full).unwrap();
        assert_eq!(
            reconstructor.push(2, &append),
            Err(PropertyValueRecordError::AppendToNonList)
        );
    }
}
