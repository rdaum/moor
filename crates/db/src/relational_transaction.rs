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

use moor_values::model::{CommitResult, ValSet};
use moor_values::AsByteBuffer;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

pub type Result<T> = std::result::Result<T, RelationalError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationalError {
    ConflictRetry,
    Duplicate(String),
    NotFound,
}

impl Display for RelationalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RelationalError::ConflictRetry => write!(f, "ConflictRetry"),
            RelationalError::Duplicate(s) => write!(f, "Duplicate: {}", s),
            RelationalError::NotFound => write!(f, "NotFound"),
        }
    }
}

impl Error for RelationalError {}

/// Traits defining a generic quasi binary-relational database transaction.
pub trait RelationalTransaction<Relation> {
    fn commit(&self) -> CommitResult;
    fn rollback(&self);

    fn increment_sequence<S: Into<u8>>(&self, seq: S) -> i64;
    fn update_sequence_max<S: Into<u8>>(&self, seq: S, value: i64) -> i64;
    fn get_sequence<S: Into<u8>>(&self, seq: S) -> i64;

    fn remove_by_domain<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Relation,
        domain: Domain,
    ) -> Result<()>;
    fn remove_by_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: DomainA,
        domain_b: DomainB,
    ) -> Result<()>;
    fn remove_by_codomain<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Relation,
        codomain: Codomain,
    ) -> Result<()>;
    fn upsert<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain: Domain,
        codomain: Codomain,
    ) -> Result<()>;
    fn insert_tuple<
        Domain: Clone + Eq + PartialEq + AsByteBuffer + Debug,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer + Debug,
    >(
        &self,
        rel: Relation,
        domain: Domain,
        codomain: Codomain,
    ) -> Result<()>;
    fn scan_with_predicate<P, Domain, Codomain>(
        &self,
        rel: Relation,
        pred: P,
    ) -> Result<Vec<(Domain, Codomain)>>
    where
        P: Fn(&Domain, &Codomain) -> bool,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
        Domain: Clone + Eq + PartialEq + AsByteBuffer;
    fn seek_unique_by_domain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain: Domain,
    ) -> Result<Option<Codomain>>;
    fn tuple_size_for_unique_domain<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Relation,
        domain: Domain,
    ) -> Result<Option<usize>>;
    fn tuple_size_for_unique_codomain<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Relation,
        codomain: Codomain,
    ) -> Result<Option<usize>>;
    fn seek_unique_by_codomain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        codomain: Codomain,
    ) -> Result<Domain>;

    fn seek_by_codomain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer + Debug,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer + Debug,
        ResultSet: ValSet<Domain>,
    >(
        &self,
        rel: Relation,
        codomain: Codomain,
    ) -> Result<ResultSet>;
    fn seek_by_unique_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: DomainA,
        domain_b: DomainB,
    ) -> Result<Option<Codomain>>;
    fn tuple_size_by_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: DomainA,
        domain_b: DomainB,
    ) -> Result<Option<usize>>;
    fn insert_composite_domain_tuple<
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: DomainA,
        domain_b: DomainB,
        codomain: Codomain,
    ) -> Result<()>;
    fn delete_composite_if_exists<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: DomainA,
        domain_b: DomainB,
    ) -> Result<()>;
    fn upsert_composite<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Relation,
        domain_a: DomainA,
        domain_b: DomainB,
        value: Codomain,
    ) -> Result<()>;
    #[allow(dead_code)]
    fn delete_if_exists<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Relation,
        domain: Domain,
    ) -> Result<()>;
}
