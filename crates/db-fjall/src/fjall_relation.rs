// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::value_set::ValueSet;
use bytes::Bytes;
use fjall::{TxPartitionHandle, WriteTransaction};
use moor_db::RelationalError::NotFound;
use moor_db::{RelationalError, RelationalTransaction};
use moor_values::model::{CommitResult, ValSet};
use moor_values::{AsByteBuffer, DecodingError, EncodingError};
use std::fmt::{Debug, Display};
use std::mem::MaybeUninit;

#[derive(Clone)]
pub(crate) struct RelationPartitions {
    // Domain -> Codomain
    pub(crate) primary: TxPartitionHandle,
    // Codomain -> Domains (Vector)
    pub(crate) secondary: Option<TxPartitionHandle>,
}

pub type OpResult<T> = std::result::Result<T, RelationalError>;

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct CompositeDomain<DomainA, DomainB>
where
    DomainA: Clone + Eq + PartialEq + AsByteBuffer,
    DomainB: Clone + Eq + PartialEq + AsByteBuffer,
{
    data: Bytes,
    phantom: std::marker::PhantomData<(DomainA, DomainB)>,
}

impl<DomainA, DomainB> Debug for CompositeDomain<DomainA, DomainB>
where
    DomainA: Clone + Eq + PartialEq + AsByteBuffer,
    DomainB: Clone + Eq + PartialEq + AsByteBuffer,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CompositeDomain").field(&self.data).finish()
    }
}

impl<DomainA, DomainB> CompositeDomain<DomainA, DomainB>
where
    DomainA: Clone + Eq + PartialEq + AsByteBuffer,
    DomainB: Clone + Eq + PartialEq + AsByteBuffer,
{
    fn new(data: Bytes) -> Self {
        Self {
            data,
            phantom: Default::default(),
        }
    }

    fn composite_key(domain_a: &DomainA, domain_b: &DomainB) -> CompositeDomain<DomainA, DomainB> {
        let mut key = Vec::new();
        key.extend_from_slice(&domain_a.as_bytes().unwrap());
        key.extend_from_slice(&domain_b.as_bytes().unwrap());
        CompositeDomain {
            data: Bytes::from(key),
            phantom: Default::default(),
        }
    }
}

impl<DomainA, DomainB> AsByteBuffer for CompositeDomain<DomainA, DomainB>
where
    DomainA: Clone + Eq + PartialEq + AsByteBuffer,
    DomainB: Clone + Eq + PartialEq + AsByteBuffer,
{
    fn size_bytes(&self) -> usize {
        self.data.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.data.as_ref()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.data.to_vec())
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        Ok(Self::new(bytes))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(self.data.clone())
    }
}

pub struct FjallTransaction<Relation>
where
    Relation: Send + Sync + Display,
{
    tx: MaybeUninit<WriteTransaction>,
    sequences_partition: TxPartitionHandle,
    relations_partitions: Vec<RelationPartitions>,
    phantom: std::marker::PhantomData<Relation>,
}

impl<Relation> FjallTransaction<Relation>
where
    Relation: Send + Sync + Display + Into<usize> + Copy,
{
    pub(crate) fn new(
        tx: WriteTransaction,
        sequences_partition: &TxPartitionHandle,
        relations_partitions: &Vec<RelationPartitions>,
    ) -> Self {
        Self {
            tx: MaybeUninit::new(tx),
            sequences_partition: sequences_partition.clone(),
            relations_partitions: relations_partitions.clone(),
            phantom: Default::default(),
        }
    }

    #[allow(clippy::mut_from_ref)]
    fn write_tx(&self) -> &mut WriteTransaction {
        // Dirty tricks to make mutable. Ugh.
        // TODO: We have to do this because I made the choice somewhere up the pipe to have the
        //   world state transaction be !mut. There were... reasons... but I will need to revisit
        //   this.
        unsafe { &mut *(self.tx.as_ptr() as *mut WriteTransaction) }
    }

    #[allow(clippy::mut_from_ref)]
    fn take_tx(self) -> WriteTransaction {
        unsafe { self.tx.assume_init() }
    }

    fn primary_partition(&self, rel: Relation) -> TxPartitionHandle {
        let idx: usize = rel.into();
        self.relations_partitions[idx].primary.clone()
    }

    fn secondary_partition(&self, rel: Relation) -> Option<TxPartitionHandle> {
        let idx: usize = rel.into();
        self.relations_partitions[idx].secondary.clone()
    }
}

impl<Relation> FjallTransaction<Relation>
where
    Relation: Send + Sync + Display + Into<usize> + Copy,
{
    // Lookup in a codomain index and a get a Vec<Domain>
    fn codomain_lookup<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        tx: &mut WriteTransaction,
        table: TxPartitionHandle,
        codomain: &Codomain,
    ) -> OpResult<ValueSet<Domain>> {
        let result = tx.get(&table, codomain.as_bytes().unwrap()).unwrap();
        if result.is_none() {
            return Err(RelationalError::NotFound);
        }
        let bytes = result.unwrap();
        let cset = ValueSet::new(bytes.into());
        Ok(cset)
    }

    // Remove a domain from a codomain index.
    fn codomain_remove<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        tx: &mut WriteTransaction,
        table: TxPartitionHandle,
        codomain_bytes: Bytes,
        domain_bytes: Bytes,
    ) -> OpResult<()> {
        let result = tx.get(&table, codomain_bytes.clone()).unwrap();
        let cset: ValueSet<Domain> = match result {
            Some(value) => ValueSet::new(value.into()),
            None => {
                return Err(RelationalError::NotFound);
            }
        };

        let new_cset = cset.without_bytes(&domain_bytes);
        tx.insert(&table, codomain_bytes, &new_cset.data());
        Ok(())
    }

    // Insert a domain into a codomain index.
    fn codomain_insert<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        tx: &mut WriteTransaction,
        table: TxPartitionHandle,
        codomain: &Codomain,
        domain: &Domain,
    ) -> OpResult<()> {
        let result = tx.get(&table, codomain.as_bytes().unwrap()).unwrap();
        let cset = match result {
            Some(value) => ValueSet::new(value.into()),
            None => ValueSet::new(Bytes::from(vec![0, 0, 0, 0])),
        };
        let new_cset = cset.append(domain.clone());
        tx.insert(&table, codomain.as_bytes().unwrap(), &new_cset.data());

        Ok(())
    }
}

impl<Relation> RelationalTransaction<Relation> for FjallTransaction<Relation>
where
    Relation: Send + Sync + Display + Into<usize> + Copy,
{
    fn commit(self) -> CommitResult {
        let tx = self.take_tx();
        match tx.commit() {
            Ok(Ok(())) => CommitResult::Success,
            Ok(Err(_)) => CommitResult::ConflictRetry,
            Err(e) => {
                // This is a fundamental database error, so we should panic.
                panic!("Error committing transaction: {:?}", e);
            }
        }
    }

    fn rollback(self) {
        let tx = self.take_tx();
        tx.rollback();
    }

    fn increment_sequence<S: Into<u8>>(&self, seq: S) -> i64 {
        let tx = self.write_tx();
        let seq_num = seq.into();
        let seq_name = format!("seq_{}", seq_num);
        let prev = tx
            .get(&self.sequences_partition, &seq_name)
            .unwrap()
            .map(|v| {
                let mut bytes = [0; 8];
                bytes.copy_from_slice(&v);
                i64::from_le_bytes(bytes)
            })
            .unwrap_or(-1);
        let next = prev + 1;
        let mut next_bytes = [0; 8];
        next_bytes.copy_from_slice(&next.to_le_bytes());
        tx.insert(&self.sequences_partition, &seq_name, &next_bytes);
        next
    }

    fn update_sequence_max<S: Into<u8>>(&self, seq: S, value: i64) -> i64 {
        let tx = self.write_tx();
        let seq_num = seq.into();
        let seq_name = format!("seq_{}", seq_num);
        let prev = tx
            .get(&self.sequences_partition, &seq_name)
            .unwrap()
            .map(|v| {
                let mut bytes = [0; 8];
                bytes.copy_from_slice(&v);
                i64::from_le_bytes(bytes)
            })
            .unwrap_or(0);
        let next = prev.max(value as i64);
        let mut next_bytes = [0; 8];
        next_bytes.copy_from_slice(&next.to_le_bytes());
        tx.insert(&self.sequences_partition, &seq_name, &next_bytes);
        next
    }

    fn get_sequence<S: Into<u8>>(&self, seq: S) -> Option<i64> {
        let tx = self.write_tx();
        let seq_num = seq.into();
        let seq_name = format!("seq_{}", seq_num);
        let Some(seq_val) = tx.get(&self.sequences_partition, &seq_name).unwrap() else {
            return None;
        };
        let mut bytes = [0; 8];
        bytes.copy_from_slice(&seq_val);
        Some(u64::from_le_bytes(bytes) as i64)
    }

    fn remove_by_domain<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Relation,
        domain: &Domain,
    ) -> OpResult<()> {
        let tx = self.write_tx();
        let table = self.primary_partition(rel);
        let domain_bytes = domain.as_bytes().unwrap();
        let result = tx.take(&table, domain_bytes.clone()).unwrap();
        if result.is_none() {
            return Err(RelationalError::NotFound);
        }

        // Remove from secondary index if it exists.
        if let Some(secondary) = self.secondary_partition(rel) {
            let codomain_bytes = Bytes::from(result.unwrap());
            self.codomain_remove::<Domain>(tx, secondary, codomain_bytes, domain_bytes)?;
        }

        Ok(())
    }

    fn remove_by_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: &DomainA,
        domain_b: &DomainB,
    ) -> OpResult<()> {
        let tx = self.write_tx();
        let table = &self.primary_partition(rel);
        let key = CompositeDomain::composite_key(domain_a, domain_b);
        let result = tx.take(&table, key.as_bytes().unwrap()).unwrap();
        if result.is_none() {
            return Err(RelationalError::NotFound);
        }
        // Remove from secondary index if it exists.
        if let Some(secondary) = self.secondary_partition(rel) {
            let codomain_bytes = Bytes::from(result.unwrap());
            self.codomain_remove::<CompositeDomain<DomainA, DomainB>>(
                tx,
                secondary,
                codomain_bytes,
                key.as_bytes().unwrap(),
            )?;
        }

        Ok(())
    }

    fn remove_by_codomain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        codomain: &Codomain,
    ) -> OpResult<()> {
        // Seek the codomain index first to find the domain.
        // If we find it, remove it from both places, otherwise return NotFound.
        let tx = self.write_tx();
        let secondary = self.secondary_partition(rel).expect("No secondary index");
        let result = tx.get(&secondary, codomain.as_bytes().unwrap()).unwrap();
        if result.is_none() {
            return Err(RelationalError::NotFound);
        }

        let domain_bytes = Bytes::from(result.unwrap());
        let primary = self.primary_partition(rel);
        let result = tx.get(&primary, &domain_bytes).unwrap();
        if result.is_none() {
            return Err(RelationalError::NotFound);
        }

        self.codomain_remove::<Domain>(tx, secondary, codomain.as_bytes().unwrap(), domain_bytes)
    }

    fn upsert<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain: &Domain,
        codomain: &Codomain,
    ) -> OpResult<()> {
        let tx = self.write_tx();
        let table = self.primary_partition(rel);
        let key = domain.as_bytes().unwrap();
        let value = codomain.as_bytes().unwrap();

        // Check for an old value.
        let old_value = tx.get(&table, &key).unwrap();
        tx.insert(&table, &key, &value);
        if let Some(secondary) = self.secondary_partition(rel) {
            // Remove the old value from the secondary index.
            if let Some(old_value) = old_value {
                self.codomain_remove::<Domain>(tx, secondary.clone(), old_value.into(), key)?;
            }
            self.codomain_insert(tx, secondary, codomain, domain)?;
        }
        Ok(())
    }

    fn insert_tuple<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain: &Domain,
        codomain: &Codomain,
    ) -> OpResult<()> {
        // Have to check if the tuple already exists.
        let tx = self.write_tx();
        let table = self.primary_partition(rel);
        let key = domain.as_bytes().unwrap();
        if tx.get(&table, &key).unwrap().is_some() {
            return Err(RelationalError::Duplicate(
                "Tuple already exists".to_string(),
            ));
        }
        let value = codomain.as_bytes().unwrap();
        tx.insert(&table, &key, &value);
        if let Some(secondary) = self.secondary_partition(rel) {
            self.codomain_insert(tx, secondary, codomain, domain)?;
        }
        Ok(())
    }

    fn scan_with_predicate<P, Domain, Codomain>(
        &self,
        rel: Relation,
        pred: P,
    ) -> OpResult<Vec<(Domain, Codomain)>>
    where
        P: Fn(&Domain, &Codomain) -> bool,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
    {
        let tx = self.write_tx();
        let table = self.primary_partition(rel);
        let mut results = Vec::new();

        for entry in tx.iter(&table) {
            let (key, value) = entry.unwrap();
            let domain = Domain::from_bytes(key.into()).unwrap();
            let codomain = Codomain::from_bytes(value.into()).unwrap();
            if pred(&domain, &codomain) {
                results.push((domain, codomain));
            }
        }
        Ok(results)
    }

    fn seek_unique_by_domain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain: &Domain,
    ) -> OpResult<Option<Codomain>> {
        let tx = self.write_tx();
        let table = self.primary_partition(rel);
        let key = domain.as_bytes().unwrap();
        let result = tx.get(&table, &key).unwrap();
        match result {
            Some(value) => {
                let result = Codomain::from_bytes(value.into());
                match result {
                    Ok(value) => Ok(Some(value)),
                    Err(e) => {
                        panic!("Error decoding codomain: {:?} in relation {}", e, rel);
                    }
                }
            }
            None => Ok(None),
        }
    }

    fn tuple_size_for_unique_domain<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Relation,
        domain: &Domain,
    ) -> OpResult<Option<usize>> {
        let tx = self.write_tx();
        let table = self.primary_partition(rel);
        let key = domain.as_bytes().unwrap();
        let result = tx.get(&table, &key).unwrap();
        match result {
            Some(value) => Ok(Some(value.len())),
            None => Ok(None),
        }
    }

    fn tuple_size_for_unique_codomain<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Relation,
        codomain: &Codomain,
    ) -> OpResult<Option<usize>> {
        let tx = self.write_tx();
        let table = self.secondary_partition(rel).unwrap();
        let key = codomain.as_bytes().unwrap();
        let result = tx.get(&table, &key).unwrap();
        match result {
            Some(value) => {
                let cset = ValueSet::new(value.into());
                Ok(cset
                    .find(codomain)
                    .map(|idx| cset.at(idx).unwrap().size_bytes()))
            }
            None => Ok(None),
        }
    }

    fn seek_unique_by_codomain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        codomain: &Codomain,
    ) -> OpResult<Domain> {
        let cset: ValueSet<Domain> = self.codomain_lookup(
            self.write_tx(),
            self.secondary_partition(rel).unwrap(),
            codomain,
        )?;
        let len = cset.len();
        if len == 0 {
            return Err(RelationalError::NotFound);
        }
        if cset.len() != 1 {
            return Err(RelationalError::Duplicate(format!(
                "Multiple tuples found for codomain. {} values present",
                cset.len()
            )));
        }

        cset.at(0).map(Ok).unwrap()
    }

    fn seek_by_codomain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
        ResultSet: ValSet<Domain>,
    >(
        &self,
        rel: Relation,
        codomain: &Codomain,
    ) -> OpResult<ResultSet> {
        let cset = match self.codomain_lookup(
            self.write_tx(),
            self.secondary_partition(rel).unwrap(),
            codomain,
        ) {
            Ok(cset) => cset,
            Err(NotFound) => {
                return Ok(ResultSet::empty());
            }
            Err(e) => {
                return Err(e);
            }
        };

        let cset_iter = cset.iter();
        Ok(ResultSet::from_iter(cset_iter))
    }

    fn seek_by_unique_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: &DomainA,
        domain_b: &DomainB,
    ) -> OpResult<Option<Codomain>> {
        let key = CompositeDomain::composite_key(domain_a, domain_b);
        self.seek_unique_by_domain(rel, &key)
    }

    fn tuple_size_by_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: &DomainA,
        domain_b: &DomainB,
    ) -> OpResult<Option<usize>> {
        let key = CompositeDomain::composite_key(domain_a, domain_b);
        self.tuple_size_for_unique_domain(rel, &key)
    }

    fn insert_composite_domain_tuple<
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: &DomainA,
        domain_b: &DomainB,
        codomain: &Codomain,
    ) -> OpResult<()> {
        let key = CompositeDomain::composite_key(domain_a, domain_b);
        self.insert_tuple(rel, &key, codomain)
    }

    fn delete_composite_if_exists<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: &DomainA,
        domain_b: &DomainB,
    ) -> OpResult<()> {
        let key = CompositeDomain::composite_key(domain_a, domain_b);
        let result = self.remove_by_domain(rel, &key);
        match result {
            Ok(()) => Ok(()),
            Err(NotFound) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn upsert_composite<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: &DomainA,
        domain_b: &DomainB,
        value: &Codomain,
    ) -> OpResult<()> {
        let key = CompositeDomain::composite_key(domain_a, domain_b);
        self.upsert(rel, &key, value)
    }

    fn delete_if_exists<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Relation,
        domain: &Domain,
    ) -> OpResult<()> {
        let result = self.remove_by_domain(rel, domain);
        match result {
            Ok(()) => Ok(()),
            Err(NotFound) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::fjall_db::FjallDb;
    use crate::fjall_relation::tests::TestRelation::{
        CompositeToOne, OneToOne, OneToOneSecondaryIndexed,
    };
    use moor_db::RelationalTransaction;
    use moor_values::model::{ObjSet, ValSet};
    use moor_values::Objid;
    use strum::{AsRefStr, Display, EnumCount, EnumIter, EnumProperty};

    /// The set of binary relations that are used to represent the world state in the moor system.
    #[repr(usize)]
    #[derive(
        Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount, Display, EnumProperty, AsRefStr,
    )]
    pub enum TestRelation {
        /// Object<->Parent
        OneToOne = 0,
        /// Object<->Location
        #[strum(props(SecondaryIndexed = "true"))]
        OneToOneSecondaryIndexed = 1,
        /// (Object, UUID)->PropertyValue (Var)
        #[strum(props(CompositeDomain = "true", Domain_A_Size = "8", Domain_B_Size = "16"))]
        CompositeToOne = 8,
        /// Set of sequences sequence_id -> current_value
        Sequences = 9,
    }

    impl Into<usize> for TestRelation {
        fn into(self) -> usize {
            self as usize
        }
    }

    fn test_db() -> FjallDb<TestRelation> {
        let (db, _) = FjallDb::open(None);
        db
    }

    #[test]
    fn test_insert_seek_unique() {
        let db = test_db();
        let tx = db.new_transaction();

        tx.insert_tuple(OneToOne, &Objid(1), &Objid(2)).unwrap();
        tx.insert_tuple(OneToOne, &Objid(2), &Objid(3)).unwrap();
        tx.insert_tuple(OneToOne, &Objid(3), &Objid(4)).unwrap();
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOne, &Objid(1))
                .unwrap(),
            Some(Objid(2))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOne, &Objid(2))
                .unwrap(),
            Some(Objid(3))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOne, &Objid(3))
                .unwrap(),
            Some(Objid(4))
        );
    }

    #[test]
    fn test_composite_insert_seek_unique() {
        let db = test_db();
        let tx = db.new_transaction();

        tx.insert_composite_domain_tuple(CompositeToOne, &Objid(1), &Objid(2), &Objid(3))
            .unwrap();
        tx.insert_composite_domain_tuple(CompositeToOne, &Objid(2), &Objid(3), &Objid(4))
            .unwrap();
        tx.insert_composite_domain_tuple(CompositeToOne, &Objid(3), &Objid(4), &Objid(5))
            .unwrap();

        assert_eq!(
            tx.seek_by_unique_composite_domain::<Objid, Objid, Objid>(
                CompositeToOne,
                &Objid(1),
                &Objid(2)
            )
            .unwrap(),
            Some(Objid(3))
        );
        assert_eq!(
            tx.seek_by_unique_composite_domain::<Objid, Objid, Objid>(
                CompositeToOne,
                &Objid(2),
                &Objid(3)
            )
            .unwrap(),
            Some(Objid(4))
        );
        assert_eq!(
            tx.seek_by_unique_composite_domain::<Objid, Objid, Objid>(
                CompositeToOne,
                &Objid(3),
                &Objid(4)
            )
            .unwrap(),
            Some(Objid(5))
        );

        // Now upsert an existing value...
        tx.upsert_composite(CompositeToOne, &Objid(1), &Objid(2), &Objid(4))
            .unwrap();
        assert_eq!(
            tx.seek_by_unique_composite_domain::<Objid, Objid, Objid>(
                CompositeToOne,
                &Objid(1),
                &Objid(2)
            )
            .unwrap(),
            Some(Objid(4))
        );

        // And insert a new using upsert
        tx.upsert_composite(CompositeToOne, &Objid(4), &Objid(5), &Objid(6))
            .unwrap();
        assert_eq!(
            tx.seek_by_unique_composite_domain::<Objid, Objid, Objid>(
                CompositeToOne,
                &Objid(4),
                &Objid(5)
            )
            .unwrap(),
            Some(Objid(6))
        );
    }

    #[test]
    fn test_codomain_index() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db();
        let tx = db.new_transaction();
        tx.insert_tuple(OneToOneSecondaryIndexed, &Objid(3), &Objid(2))
            .unwrap();
        tx.insert_tuple(OneToOneSecondaryIndexed, &Objid(2), &Objid(1))
            .unwrap();
        tx.insert_tuple(OneToOneSecondaryIndexed, &Objid(1), &Objid(0))
            .unwrap();
        tx.insert_tuple(OneToOneSecondaryIndexed, &Objid(4), &Objid(0))
            .unwrap();

        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid(3))
                .unwrap(),
            Some(Objid(2))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid(2))
                .unwrap(),
            Some(Objid(1))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid(1))
                .unwrap(),
            Some(Objid(0))
        );

        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(0))
                .unwrap(),
            ObjSet::from_items(&[Objid(1), Objid(4)])
        );
        assert_eq!(
            tx.seek_unique_by_codomain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid(1))
                .unwrap(),
            Objid(2)
        );
        assert_eq!(
            tx.seek_unique_by_codomain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid(2))
                .unwrap(),
            Objid(3)
        );

        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(3))
                .unwrap(),
            ObjSet::empty()
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(0))
                .unwrap(),
            ObjSet::from_items(&[Objid(1), Objid(4)])
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(1))
                .unwrap(),
            ObjSet::from_items(&[Objid(2)])
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(2))
                .unwrap(),
            ObjSet::from_items(&[Objid(3)])
        );

        // Now commit and re-verify.
        assert_eq!(tx.commit(), super::CommitResult::Success);
        let tx = db.new_transaction();

        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid(3))
                .unwrap(),
            Some(Objid(2))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid(2))
                .unwrap(),
            Some(Objid(1))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid(1))
                .unwrap(),
            Some(Objid(0))
        );

        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(3))
                .unwrap(),
            ObjSet::empty(),
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(2))
                .unwrap(),
            ObjSet::from_items(&[Objid(3)])
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(1))
                .unwrap(),
            ObjSet::from_items(&[Objid(2)])
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(0))
                .unwrap(),
            ObjSet::from_items(&[Objid(1), Objid(4)])
        );

        // And then update a value and verify.
        tx.upsert::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid(1), &Objid(2))
            .unwrap();
        assert_eq!(
            tx.seek_unique_by_codomain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid(1))
                .unwrap(),
            Objid(2)
        );
        // Verify that the secondary index is updated... First check for new value.
        let children: ObjSet = tx
            .seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(2))
            .unwrap();
        assert_eq!(children.len(), 2);
        assert!(
            children.contains(Objid(1)),
            "Expected children of 2 to contain 1"
        );
        assert!(
            !children.contains(Objid(0)),
            "Expected children of 2 to not contain 0"
        );
        // Now check the old value.
        let children = tx
            .seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid(0))
            .unwrap();
        assert_eq!(children, ObjSet::from_items(&[Objid(4)]));
    }
}
