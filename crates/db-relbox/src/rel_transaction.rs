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

use daumtils::SliceRef;
use moor_db::{RelationalError, RelationalTransaction};
use moor_values::model::{CommitResult, ValSet};
use moor_values::{AsByteBuffer, EncodingError};
use relbox::{RelationError, Transaction};
use std::fmt::Debug;

pub struct RelboxTransaction<T> {
    tx: Transaction,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> RelboxTransaction<T> {
    pub fn new(tx: Transaction) -> Self {
        Self {
            tx,
            _phantom: std::marker::PhantomData,
        }
    }
}
type Result<T> = std::result::Result<T, RelationalError>;

fn err_map(e: RelationError) -> RelationalError {
    match e {
        RelationError::TupleNotFound => RelationalError::NotFound,
        _ => panic!("Unexpected error: {:?}", e),
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Composite<DomainA: AsByteBuffer, DomainB: AsByteBuffer> {
    bytes: SliceRef,
    _phantom: std::marker::PhantomData<(DomainA, DomainB)>,
}

impl<DomainA: AsByteBuffer, DomainB: AsByteBuffer> Composite<DomainA, DomainB> {
    fn new(domain_a: DomainA, domain_b: DomainB) -> Self {
        let (a_bytes, b_bytes) = (
            domain_a.as_sliceref().unwrap(),
            domain_b.as_sliceref().unwrap(),
        );
        let mut bytes =
            Vec::with_capacity(a_bytes.len() + b_bytes.len() + std::mem::size_of::<usize>());
        bytes.extend_from_slice(&a_bytes.len().to_le_bytes());
        bytes.extend_from_slice(a_bytes.as_slice());
        bytes.extend_from_slice(b_bytes.as_slice());
        let bytes = SliceRef::from_vec(bytes);
        Self {
            bytes,
            _phantom: std::marker::PhantomData,
        }
    }

    #[allow(dead_code)]
    fn domain_a(&self) -> DomainA {
        let bytes = self.bytes.as_slice();
        let len = usize::from_le_bytes(bytes[..std::mem::size_of::<usize>()].try_into().unwrap());
        let sr = self.bytes.slice(std::mem::size_of::<usize>()..len);
        DomainA::from_sliceref(sr).expect("Failed to convert domain")
    }

    #[allow(dead_code)]
    fn domain_b(&self) -> DomainB {
        let bytes = self.bytes.as_slice();

        let len = usize::from_le_bytes(bytes[..std::mem::size_of::<usize>()].try_into().unwrap());
        let sr = self.bytes.slice(len + std::mem::size_of::<usize>()..);
        DomainB::from_sliceref(sr).expect("Failed to convert domain")
    }

    fn as_sliceref(&self) -> std::result::Result<SliceRef, EncodingError> {
        Ok(self.bytes.clone())
    }
}

impl<T> RelationalTransaction<T> for RelboxTransaction<T>
where
    T: Into<usize>,
{
    fn commit(&self) -> CommitResult {
        if self.tx.commit().is_err() {
            return CommitResult::ConflictRetry;
        }
        CommitResult::Success
    }

    fn rollback(&self) {
        self.tx.rollback().expect("Failed to rollback transaction");
    }

    fn increment_sequence<S: Into<u8>>(&self, seq: S) -> i64 {
        self.tx.increment_sequence(seq.into() as usize) as i64
    }

    fn update_sequence_max<S: Into<u8>>(&self, seq: S, value: i64) -> i64 {
        let seq_num = seq.into() as usize;
        self.tx.update_sequence_max(seq_num, value as u64);
        self.tx.sequence_current(seq_num) as i64
    }

    fn get_sequence<S: Into<u8>>(&self, seq: S) -> i64 {
        self.tx.sequence_current(seq.into() as usize) as i64
    }

    fn remove_by_domain<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: T,
        domain: Domain,
    ) -> Result<()> {
        self.tx
            .relation(relbox::RelationId(rel.into()))
            .remove_by_domain(domain.as_sliceref().unwrap())
            .map_err(err_map)
    }

    fn remove_by_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: T,
        domain_a: DomainA,
        domain_b: DomainB,
    ) -> Result<()> {
        let composite = Composite::new(domain_a, domain_b);
        self.tx
            .relation(relbox::RelationId(rel.into()))
            .remove_by_domain(composite.as_sliceref().unwrap())
            .map_err(err_map)
    }

    fn remove_by_codomain<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        _rel: T,
        _codomain: Codomain,
    ) -> Result<()> {
        unimplemented!("remove_by_codomain")
    }

    fn upsert<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: T,
        domain: Domain,
        codomain: Codomain,
    ) -> Result<()> {
        self.tx
            .relation(relbox::RelationId(rel.into()))
            .upsert_by_domain(
                domain.as_sliceref().unwrap(),
                codomain.as_sliceref().unwrap(),
            )
            .map_err(err_map)
    }

    fn insert_tuple<
        Domain: Clone + Eq + PartialEq + AsByteBuffer + Debug,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer + Debug,
    >(
        &self,
        rel: T,
        domain: Domain,
        codomain: Codomain,
    ) -> Result<()> {
        self.tx
            .relation(relbox::RelationId(rel.into()))
            .insert_tuple(
                domain.as_sliceref().unwrap(),
                codomain.as_sliceref().unwrap(),
            )
            .map_err(err_map)
    }

    fn scan_with_predicate<P, Domain, Codomain>(
        &self,
        rel: T,
        pred: P,
    ) -> Result<Vec<(Domain, Codomain)>>
    where
        P: Fn(&Domain, &Codomain) -> bool,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
    {
        let results = self
            .tx
            .relation(relbox::RelationId(rel.into()))
            .predicate_scan(&|t| {
                let domain = Domain::from_sliceref(t.domain()).expect("Failed to convert domain");
                let codomain =
                    Codomain::from_sliceref(t.codomain()).expect("Failed to convert codomain");
                pred(&domain, &codomain)
            })
            .map_err(err_map)?;
        Ok(results
            .iter()
            .map(|tr| {
                (
                    Domain::from_sliceref(tr.domain()).expect("Failed to convert domain"),
                    Codomain::from_sliceref(tr.codomain()).expect("Failed to convert codomain"),
                )
            })
            .collect())
    }

    fn seek_unique_by_domain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: T,
        domain: Domain,
    ) -> Result<Option<Codomain>> {
        match self
            .tx
            .relation(relbox::RelationId(rel.into()))
            .seek_unique_by_domain(domain.as_sliceref().unwrap())
            .map(|t| Codomain::from_sliceref(t.codomain()).expect("Failed to convert codomain"))
        {
            Ok(o) => Ok(Some(
                Codomain::from_sliceref(o.as_sliceref().unwrap())
                    .expect("Failed to convert domain"),
            )),
            Err(RelationError::TupleNotFound) => Ok(None),
            Err(e) => Err(err_map(e)),
        }
    }

    fn tuple_size_for_unique_domain<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: T,
        domain: Domain,
    ) -> Result<Option<usize>> {
        match self
            .tx
            .relation(relbox::RelationId(rel.into()))
            .seek_unique_by_domain(domain.as_sliceref().unwrap())
        {
            Ok(o) => Ok(Some(o.slot_buffer().len())),
            Err(RelationError::TupleNotFound) => Ok(None),
            Err(e) => Err(err_map(e)),
        }
    }

    fn tuple_size_for_unique_codomain<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: T,
        codomain: Codomain,
    ) -> Result<Option<usize>> {
        match self
            .tx
            .relation(relbox::RelationId(rel.into()))
            .seek_by_codomain(codomain.as_sliceref().unwrap())
        {
            Ok(o) => {
                if o.is_empty() {
                    return Ok(None);
                }
                if o.len() > 1 {
                    return Err(RelationalError::Duplicate(
                        "Multiple tuples found for codomain".into(),
                    ));
                }
                let o = o.into_iter().next().unwrap();
                Ok(Some(o.codomain().len()))
            }
            Err(RelationError::TupleNotFound) => Ok(None),
            Err(e) => Err(err_map(e)),
        }
    }

    fn seek_unique_by_codomain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: T,
        codomain: Codomain,
    ) -> Result<Domain> {
        match self
            .tx
            .relation(relbox::RelationId(rel.into()))
            .seek_by_codomain(codomain.as_sliceref().unwrap())
        {
            Ok(o) => {
                if o.is_empty() {
                    return Err(RelationalError::NotFound);
                }
                if o.len() > 1 {
                    return Err(RelationalError::Duplicate(
                        "Multiple tuples found for codomain".into(),
                    ));
                }
                let o = o.into_iter().next().unwrap();
                Ok(Domain::from_sliceref(o.domain()).expect("Failed to convert domain"))
            }
            Err(RelationError::TupleNotFound) => Err(RelationalError::NotFound),
            Err(e) => Err(err_map(e)),
        }
    }

    fn seek_by_codomain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer + Debug,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer + Debug,
        ResultSet: ValSet<Domain>,
    >(
        &self,
        rel: T,
        codomain: Codomain,
    ) -> Result<ResultSet> {
        let results = self
            .tx
            .relation(relbox::RelationId(rel.into()))
            .seek_by_codomain(codomain.as_sliceref().unwrap())
            .map_err(err_map)?;
        Ok(ResultSet::from_iter(results.iter().map(|tr| {
            Domain::from_sliceref(tr.domain()).expect("Failed to convert domain")
        })))
    }

    fn seek_by_unique_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: T,
        domain_a: DomainA,
        domain_b: DomainB,
    ) -> Result<Option<Codomain>> {
        let composite = Composite::new(domain_a, domain_b);
        match self
            .tx
            .relation(relbox::RelationId(rel.into()))
            .seek_unique_by_domain(composite.as_sliceref().unwrap())
            .map(|t| Codomain::from_sliceref(t.codomain()).expect("Failed to convert codomain"))
        {
            Ok(o) => Ok(Some(
                Codomain::from_sliceref(o.as_sliceref().unwrap())
                    .expect("Failed to convert domain"),
            )),
            Err(RelationError::TupleNotFound) => Ok(None),
            Err(e) => Err(err_map(e)),
        }
    }

    fn tuple_size_by_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: T,
        domain_a: DomainA,
        domain_b: DomainB,
    ) -> Result<Option<usize>> {
        let composite = Composite::new(domain_a, domain_b);
        match self
            .tx
            .relation(relbox::RelationId(rel.into()))
            .seek_unique_by_domain(composite.as_sliceref().unwrap())
        {
            Ok(o) => Ok(Some(o.slot_buffer().len())),
            Err(RelationError::TupleNotFound) => Ok(None),
            Err(e) => Err(err_map(e)),
        }
    }

    fn insert_composite_domain_tuple<
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: T,
        domain_a: DomainA,
        domain_b: DomainB,
        codomain: Codomain,
    ) -> Result<()> {
        let composite = Composite::new(domain_a, domain_b);
        self.tx
            .relation(relbox::RelationId(rel.into()))
            .insert_tuple(
                composite.as_sliceref().unwrap(),
                codomain.as_sliceref().unwrap(),
            )
            .map_err(err_map)
    }

    fn delete_composite_if_exists<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: T,
        domain_a: DomainA,
        domain_b: DomainB,
    ) -> Result<()> {
        let composite = Composite::new(domain_a, domain_b);
        self.tx
            .relation(relbox::RelationId(rel.into()))
            .remove_by_domain(composite.as_sliceref().unwrap())
            .map_err(err_map)
    }

    fn upsert_composite<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: T,
        domain_a: DomainA,
        domain_b: DomainB,
        value: Codomain,
    ) -> Result<()> {
        let composite = Composite::new(domain_a, domain_b);
        self.tx
            .relation(relbox::RelationId(rel.into()))
            .upsert_by_domain(
                composite.as_sliceref().unwrap(),
                value.as_sliceref().unwrap(),
            )
            .map_err(err_map)
    }

    fn delete_if_exists<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: T,
        domain: Domain,
    ) -> Result<()> {
        self.tx
            .relation(relbox::RelationId(rel.into()))
            .remove_by_domain(domain.as_sliceref().unwrap())
            .map_err(err_map)
    }
}
