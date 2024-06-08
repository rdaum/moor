use moor_db::{RelationalError, RelationalTransaction};
use moor_values::model::{CommitResult, ValSet};
use moor_values::AsByteBuffer;
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
        let mut composite = domain_a.make_copy_as_vec().unwrap();
        composite.extend_from_slice(&domain_b.make_copy_as_vec().unwrap());
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
        let mut composite = domain_a.make_copy_as_vec().unwrap();
        composite.extend_from_slice(&domain_b.make_copy_as_vec().unwrap());
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
        let mut composite = domain_a.make_copy_as_vec().unwrap();
        composite.extend_from_slice(&domain_b.make_copy_as_vec().unwrap());
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
        let mut composite = domain_a.make_copy_as_vec().unwrap();
        composite.extend_from_slice(&domain_b.make_copy_as_vec().unwrap());
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
        let mut composite = domain_a.make_copy_as_vec().unwrap();
        composite.extend_from_slice(&domain_b.make_copy_as_vec().unwrap());
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
        let mut composite = domain_a.make_copy_as_vec().unwrap();
        composite.extend_from_slice(&domain_b.make_copy_as_vec().unwrap());
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
