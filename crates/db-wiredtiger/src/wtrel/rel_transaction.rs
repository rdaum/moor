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

use std::sync::atomic::AtomicI64;
use std::sync::Arc;

use moor_db::{RelationalError, RelationalTransaction};
use moor_values::model::{CommitResult, ValSet};
use moor_values::AsByteBuffer;

use crate::bindings::FormatType::RawByte;
use crate::bindings::{CursorConfig, Datum, Error, Pack, Session};
use crate::wtrel::rel_db::MAX_NUM_SEQUENCES;
use crate::wtrel::relation::WiredTigerRelation;
use crate::wtrel::{from_datum, to_datum};

fn cursor_options() -> CursorConfig {
    CursorConfig::new().raw(true)
}

pub struct WiredTigerRelTransaction<TableType: WiredTigerRelation> {
    session: Session,
    sequences: Arc<[AtomicI64; MAX_NUM_SEQUENCES]>,
    _phantom: std::marker::PhantomData<TableType>,
}

type Result<T> = std::result::Result<T, RelationalError>;

impl<Tables> WiredTigerRelTransaction<Tables>
where
    Tables: WiredTigerRelation + Send,
{
    pub(crate) fn new(session: Session, sequences: Arc<[AtomicI64; MAX_NUM_SEQUENCES]>) -> Self {
        WiredTigerRelTransaction {
            session,
            sequences,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    fn composite_key_for<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer + Sized,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer + Sized,
    >(
        &self,
        o: &DomainA,
        u: &DomainB,
    ) -> Datum {
        let (a, b) = (o.as_bytes().unwrap(), u.as_bytes().unwrap());

        let mut pack = Pack::new(
            &self.session,
            &[RawByte(Some(a.len())), RawByte(Some(b.len()))],
            a.len() + b.len(),
        );
        pack.push_item(a.as_ref());
        pack.push_item(b.as_ref());
        pack.pack()
    }
}

fn err_map(e: Error) -> RelationalError {
    match e {
        Error::Rollback => RelationalError::ConflictRetry,
        Error::NotFound => RelationalError::NotFound,
        Error::DuplicateKey => RelationalError::Duplicate("Duplicate key".to_string()),
        _ => {
            panic!("Unexpected error: {:?}", e)
        }
    }
}

impl<Tables> RelationalTransaction<Tables> for WiredTigerRelTransaction<Tables>
where
    Tables: WiredTigerRelation + Send,
{
    fn commit(self) -> CommitResult {
        match self.session.commit() {
            Ok(_) => CommitResult::Success,
            Err(Error::Rollback) => CommitResult::ConflictRetry,
            Err(e) => {
                panic!("Unexpected error: {:?}", e)
            }
        }
    }

    fn rollback(self) {
        self.session
            .rollback_transaction()
            .expect("Failed to rollback transaction")
    }

    fn increment_sequence<S: Into<u8>>(&self, seq: S) -> i64 {
        self.sequences[seq.into() as usize].fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
    }

    /// Update the given sequence to `value` iff `value` is greater than the current value.
    fn update_sequence_max<S: Into<u8>>(&self, seq: S, value: i64) -> i64 {
        let sequence = &self.sequences[seq.into() as usize];
        loop {
            let current = sequence.load(std::sync::atomic::Ordering::SeqCst);
            let max = std::cmp::max(current, value);
            if max <= current {
                return current;
            }
            if sequence
                .compare_exchange(
                    current,
                    max,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
            {
                return current;
            }
        }
    }

    fn get_sequence<S: Into<u8>>(&self, seq: S) -> Option<i64> {
        Some(self.sequences[seq.into() as usize].load(std::sync::atomic::Ordering::Relaxed))
    }

    fn remove_by_domain<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Tables,
        domain: &Domain,
    ) -> Result<()> {
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options()))
            .map_err(err_map)?;

        let domain_datum = to_datum(&self.session, domain);
        cursor.set_key(domain_datum).map_err(err_map)?;
        cursor.remove().map_err(err_map)?;
        Ok(())
    }

    fn remove_by_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        domain_a: &DomainA,
        domain_b: &DomainB,
    ) -> Result<()> {
        let key_bytes = self.composite_key_for(domain_a, domain_b);
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options()))
            .map_err(err_map)?;
        cursor.set_key(key_bytes).map_err(err_map)?;
        if let Err(Error::NotFound) = cursor.search() {
            return Ok(());
        }
        cursor.remove().map_err(err_map)?;
        Ok(())
    }

    fn remove_by_codomain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        codomain: &Codomain,
    ) -> Result<()> {
        let table = rel.get_secondary_index();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options()))
            .map_err(err_map)?;
        let codomain_sr = to_datum(&self.session, codomain);
        cursor.set_key(codomain_sr).map_err(err_map)?;
        if let Err(Error::NotFound) = cursor.search() {
            return Ok(());
        }
        cursor.remove().map_err(err_map)?;
        loop {
            match cursor.next() {
                Ok(_) => {
                    let codomain_scan =
                        from_datum::<Codomain>(&self.session, cursor.get_value().map_err(err_map)?);
                    if codomain_scan.ne(codomain) {
                        break;
                    }
                    cursor.remove().map_err(err_map)?;
                }
                Err(Error::NotFound) => break,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }
        Ok(())
    }

    fn upsert<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        domain: &Domain,
        codomain: &Codomain,
    ) -> Result<()> {
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options().overwrite(true)))
            .map_err(err_map)?;
        cursor
            .set_key(to_datum(&self.session, domain))
            .map_err(err_map)?;
        cursor
            .set_value(to_datum(&self.session, codomain))
            .map_err(err_map)?;
        cursor.insert().map_err(err_map)?;
        Ok(())
    }
    fn insert_tuple<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        domain: &Domain,
        codomain: &Codomain,
    ) -> Result<()> {
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options().overwrite(false)))
            .map_err(err_map)?;
        cursor
            .set_key(to_datum(&self.session, domain))
            .map_err(err_map)?;
        cursor
            .set_value(to_datum(&self.session, codomain))
            .map_err(err_map)?;

        match cursor.insert() {
            Ok(_) => {}
            Err(Error::DuplicateKey) => {
                return Err(RelationalError::Duplicate(format!(
                    "Duplicate key for relation {}",
                    rel
                )));
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
        Ok(())
    }

    /// Full scan an entire relation where and return the codomains matching the predicate.
    fn scan_with_predicate<P, Domain, Codomain>(
        &self,
        rel: Tables,
        pred: P,
    ) -> Result<Vec<(Domain, Codomain)>>
    where
        P: Fn(&Domain, &Codomain) -> bool,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
    {
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options().readonly(true)))
            .map_err(err_map)?;
        cursor.reset().map_err(err_map)?;
        let mut results = vec![];
        loop {
            match cursor.next() {
                Ok(_) => {
                    let domain =
                        from_datum::<Domain>(&self.session, cursor.get_key().map_err(err_map)?);
                    let codatum = cursor.get_value().map_err(err_map)?;
                    let codomain = from_datum::<Codomain>(&self.session, codatum);
                    if pred(&domain, &codomain) {
                        results.push((domain.clone(), codomain.clone()));
                    }
                }
                Err(Error::NotFound) => break,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }
        Ok(results)
    }
    fn seek_unique_by_domain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        domain: &Domain,
    ) -> Result<Option<Codomain>> {
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options().readonly(true)))
            .map_err(err_map)?;
        cursor
            .set_key(to_datum(&self.session, domain))
            .map_err(err_map)?;
        match cursor.search() {
            Ok(_) => Ok(Some(from_datum(
                &self.session,
                cursor.get_value().map_err(err_map)?,
            ))),
            Err(Error::NotFound) => Ok(None),
            Err(e) => Err(err_map(e)),
        }
    }
    fn tuple_size_for_unique_domain<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Tables,
        domain: &Domain,
    ) -> Result<Option<usize>> {
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options().readonly(true)))
            .map_err(err_map)?;
        cursor
            .set_key(to_datum(&self.session, domain))
            .map_err(err_map)?;
        match cursor.search() {
            Ok(_) => Ok(Some(cursor.get_value().map_err(err_map)?.len())),
            Err(Error::NotFound) => Ok(None),
            Err(e) => Err(err_map(e)),
        }
    }
    fn tuple_size_for_unique_codomain<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Tables,
        codomain: &Codomain,
    ) -> Result<Option<usize>> {
        // Requires there be a secondary index on the relation named 'codomain'
        let index = rel.get_secondary_index();
        let cursor = self
            .session
            .open_cursor(&index, Some(cursor_options().readonly(true)))
            .map_err(err_map)?;
        cursor
            .set_key(to_datum(&self.session, codomain))
            .map_err(err_map)?;
        match cursor.search() {
            Ok(_) => Ok(Some(cursor.get_value().map_err(err_map)?.len())),
            Err(Error::NotFound) => Ok(None),
            Err(e) => Err(err_map(e)),
        }
    }
    fn seek_unique_by_codomain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        codomain: &Codomain,
    ) -> Result<Domain> {
        // Requires there be a secondary index on the relation named 'codomain'
        let index = rel.get_secondary_index();
        let cursor = self.session.open_cursor(&index, None).map_err(err_map)?;
        let codomain_datum = to_datum(&self.session, codomain);
        cursor.set_key(codomain_datum).map_err(err_map)?;
        cursor.search().map_err(err_map)?;
        Ok(from_datum(
            &self.session,
            cursor.get_value().map_err(err_map)?,
        ))
    }

    fn seek_by_codomain<
        Domain: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
        ResultSet: ValSet<Domain>,
    >(
        &self,
        rel: Tables,
        codomain: &Codomain,
    ) -> Result<ResultSet> {
        // Requires there be a secondary index on the relation named 'codomain'
        let index = rel.get_secondary_index();
        let cursor = self.session.open_cursor(&index, None).map_err(err_map)?;
        let codomain_sr = to_datum(&self.session, codomain);
        cursor.set_key(codomain_sr).map_err(err_map)?;
        match cursor.search() {
            Ok(_) => {}
            Err(Error::NotFound) => {
                return Ok(ResultSet::empty());
            }
            Err(e) => return Err(err_map(e)),
        }
        let mut items = vec![];
        loop {
            let val = cursor.get_value().map_err(err_map)?;
            items.push(from_datum(&self.session, val));
            match cursor.next() {
                Ok(_) => {
                    let codomain_scan: Codomain =
                        from_datum::<Codomain>(&self.session, cursor.get_key().map_err(err_map)?);
                    if codomain_scan.ne(codomain) {
                        break;
                    }
                }
                Err(Error::NotFound) => {
                    break;
                }
                Err(e) => return Err(err_map(e)),
            }
        }
        Ok(ResultSet::from_items(&items))
    }

    fn seek_by_unique_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        domain_a: &DomainA,
        domain_b: &DomainB,
    ) -> Result<Option<Codomain>> {
        let key_bytes = self.composite_key_for(domain_a, domain_b);
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options().readonly(true)))
            .map_err(err_map)?;
        cursor.set_key(key_bytes).map_err(err_map)?;
        match cursor.search() {
            Ok(_) => Ok(Some(from_datum(
                &self.session,
                cursor.get_value().map_err(err_map)?,
            ))),
            Err(Error::NotFound) => Ok(None),
            Err(e) => Err(err_map(e)),
        }
    }
    fn tuple_size_by_composite_domain<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        domain_a: &DomainA,
        domain_b: &DomainB,
    ) -> Result<Option<usize>> {
        let key_bytes = self.composite_key_for(domain_a, domain_b);
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options().readonly(true)))
            .map_err(err_map)?;
        cursor.set_key(key_bytes).map_err(err_map)?;
        match cursor.search() {
            Ok(_) => Ok(Some(cursor.get_value().map_err(err_map)?.len())),
            Err(Error::NotFound) => Ok(None),
            Err(e) => Err(err_map(e)),
        }
    }
    fn insert_composite_domain_tuple<
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        domain_a: &DomainA,
        domain_b: &DomainB,
        codomain: &Codomain,
    ) -> Result<()> {
        let key_bytes = self.composite_key_for(domain_a, domain_b);
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options().overwrite(false)))
            .map_err(err_map)?;
        cursor.set_key(key_bytes).map_err(err_map)?;
        cursor
            .set_value(to_datum(&self.session, codomain))
            .map_err(err_map)?;
        match cursor.insert() {
            Ok(_) => Ok(()),
            Err(Error::DuplicateKey) => {
                Err(RelationalError::Duplicate("Duplicate key".to_string()))
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    fn delete_composite_if_exists<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        domain_a: &DomainA,
        domain_b: &DomainB,
    ) -> Result<()> {
        let key_bytes = self.composite_key_for(domain_a, domain_b);
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options()))
            .map_err(err_map)?;
        cursor.set_key(key_bytes).map_err(err_map)?;
        match cursor.remove() {
            Ok(_) => Ok(()),
            Err(Error::NotFound) => Ok(()),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    fn upsert_composite<
        DomainA: Clone + Eq + PartialEq + AsByteBuffer,
        DomainB: Clone + Eq + PartialEq + AsByteBuffer,
        Codomain: Clone + Eq + PartialEq + AsByteBuffer,
    >(
        &self,
        rel: Tables,
        domain_a: &DomainA,
        domain_b: &DomainB,
        value: &Codomain,
    ) -> Result<()> {
        let key_bytes = self.composite_key_for(domain_a, domain_b);
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options().overwrite(true)))
            .map_err(err_map)?;

        cursor.set_key(key_bytes).map_err(err_map)?;
        cursor
            .set_value(to_datum(&self.session, value))
            .map_err(err_map)?;
        cursor.insert().map_err(err_map)?;
        Ok(())
    }

    #[allow(dead_code)]
    fn delete_if_exists<Domain: Clone + Eq + PartialEq + AsByteBuffer>(
        &self,
        rel: Tables,
        domain: &Domain,
    ) -> Result<()> {
        let table = rel.into();
        let cursor = self
            .session
            .open_cursor(&table, Some(cursor_options()))
            .map_err(err_map)?;
        cursor
            .set_key(to_datum(&self.session, domain))
            .map_err(err_map)?;
        match cursor.remove() {
            Ok(_) => Ok(()),
            Err(Error::NotFound) => Ok(()),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use strum::{AsRefStr, Display, EnumCount, EnumIter, EnumProperty};

    use moor_db::RelationalTransaction;
    use moor_values::model::{ObjSet, ValSet};
    use moor_values::Objid;
    use TestRelation::{CompositeToOne, OneToOne, OneToOneSecondaryIndexed, Sequences};

    use crate::wtrel::rel_db::WiredTigerRelDb;

    use crate::wtrel::relation::WiredTigerRelation;

    #[repr(u8)]
    #[derive(Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount)]
    pub enum TestSequences {
        MaximumSequenceAction = 0,
    }

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
    impl WiredTigerRelation for TestRelation {}

    fn test_db(path: &Path) -> Arc<WiredTigerRelDb<TestRelation>> {
        let db = WiredTigerRelDb::new(path, Sequences, true);
        db.create_tables();
        db.load_sequences();
        db
    }
    #[test]
    fn test_insert_seek_unique() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let tx = db.clone().start_tx();

        tx.insert_tuple(OneToOne, &Objid::mk_id(1), &Objid::mk_id(2))
            .unwrap();
        tx.insert_tuple(OneToOne, &Objid::mk_id(2), &Objid::mk_id(3))
            .unwrap();
        tx.insert_tuple(OneToOne, &Objid::mk_id(3), &Objid::mk_id(4))
            .unwrap();
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOne, &Objid::mk_id(1))
                .unwrap(),
            Some(Objid::mk_id(2))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOne, &Objid::mk_id(2))
                .unwrap(),
            Some(Objid::mk_id(3))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOne, &Objid::mk_id(3))
                .unwrap(),
            Some(Objid::mk_id(4))
        );
    }

    #[test]
    fn test_composite_insert_seek_unique() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let tx = db.clone().start_tx();
        tx.insert_composite_domain_tuple(
            CompositeToOne,
            &Objid::mk_id(1),
            &Objid::mk_id(2),
            &Objid::mk_id(3),
        )
        .unwrap();
        tx.insert_composite_domain_tuple(
            CompositeToOne,
            &Objid::mk_id(2),
            &Objid::mk_id(3),
            &Objid::mk_id(4),
        )
        .unwrap();
        tx.insert_composite_domain_tuple(
            CompositeToOne,
            &Objid::mk_id(3),
            &Objid::mk_id(4),
            &Objid::mk_id(5),
        )
        .unwrap();

        assert_eq!(
            tx.seek_by_unique_composite_domain::<Objid, Objid, Objid>(
                CompositeToOne,
                &Objid::mk_id(1),
                &Objid::mk_id(2)
            )
            .unwrap(),
            Some(Objid::mk_id(3))
        );
        assert_eq!(
            tx.seek_by_unique_composite_domain::<Objid, Objid, Objid>(
                CompositeToOne,
                &Objid::mk_id(2),
                &Objid::mk_id(3)
            )
            .unwrap(),
            Some(Objid::mk_id(4))
        );
        assert_eq!(
            tx.seek_by_unique_composite_domain::<Objid, Objid, Objid>(
                CompositeToOne,
                &Objid::mk_id(3),
                &Objid::mk_id(4)
            )
            .unwrap(),
            Some(Objid::mk_id(5))
        );

        // Now upsert an existing value...
        tx.upsert_composite(
            CompositeToOne,
            &Objid::mk_id(1),
            &Objid::mk_id(2),
            &Objid::mk_id(4),
        )
        .unwrap();
        assert_eq!(
            tx.seek_by_unique_composite_domain::<Objid, Objid, Objid>(
                CompositeToOne,
                &Objid::mk_id(1),
                &Objid::mk_id(2)
            )
            .unwrap(),
            Some(Objid::mk_id(4))
        );

        // And insert a new using upsert
        tx.upsert_composite(
            CompositeToOne,
            &Objid::mk_id(4),
            &Objid::mk_id(5),
            &Objid::mk_id(6),
        )
        .unwrap();
        assert_eq!(
            tx.seek_by_unique_composite_domain::<Objid, Objid, Objid>(
                CompositeToOne,
                &Objid::mk_id(4),
                &Objid::mk_id(5)
            )
            .unwrap(),
            Some(Objid::mk_id(6))
        );
    }

    #[test]
    fn test_codomain_index() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let tx = db.clone().start_tx();
        tx.insert_tuple(OneToOneSecondaryIndexed, &Objid::mk_id(3), &Objid::mk_id(2))
            .unwrap();
        tx.insert_tuple(OneToOneSecondaryIndexed, &Objid::mk_id(2), &Objid::mk_id(1))
            .unwrap();
        tx.insert_tuple(OneToOneSecondaryIndexed, &Objid::mk_id(1), &Objid::mk_id(0))
            .unwrap();
        tx.insert_tuple(OneToOneSecondaryIndexed, &Objid::mk_id(4), &Objid::mk_id(0))
            .unwrap();

        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid::mk_id(3))
                .unwrap(),
            Some(Objid::mk_id(2))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid::mk_id(2))
                .unwrap(),
            Some(Objid::mk_id(1))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid::mk_id(1))
                .unwrap(),
            Some(Objid::mk_id(0))
        );

        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(0))
                .unwrap(),
            ObjSet::from_items(&[Objid::mk_id(1), Objid::mk_id(4)])
        );
        assert_eq!(
            tx.seek_unique_by_codomain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid::mk_id(1))
                .unwrap(),
            Objid::mk_id(2)
        );
        assert_eq!(
            tx.seek_unique_by_codomain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid::mk_id(2))
                .unwrap(),
            Objid::mk_id(3)
        );

        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(3))
                .unwrap(),
            ObjSet::empty()
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(0))
                .unwrap(),
            ObjSet::from_items(&[Objid::mk_id(1), Objid::mk_id(4)])
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(1))
                .unwrap(),
            ObjSet::from_items(&[Objid::mk_id(2)])
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(2))
                .unwrap(),
            ObjSet::from_items(&[Objid::mk_id(3)])
        );

        // Now commit and re-verify.
        assert_eq!(tx.commit(), super::CommitResult::Success);
        let tx = db.start_tx();

        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid::mk_id(3))
                .unwrap(),
            Some(Objid::mk_id(2))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid::mk_id(2))
                .unwrap(),
            Some(Objid::mk_id(1))
        );
        assert_eq!(
            tx.seek_unique_by_domain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid::mk_id(1))
                .unwrap(),
            Some(Objid::mk_id(0))
        );

        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(3))
                .unwrap(),
            ObjSet::empty(),
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(2))
                .unwrap(),
            ObjSet::from_items(&[Objid::mk_id(3)])
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(1))
                .unwrap(),
            ObjSet::from_items(&[Objid::mk_id(2)])
        );
        assert_eq!(
            tx.seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(0))
                .unwrap(),
            ObjSet::from_items(&[Objid::mk_id(1), Objid::mk_id(4)])
        );

        // And then update a value and verify.
        tx.upsert::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid::mk_id(1), &Objid::mk_id(2))
            .unwrap();
        assert_eq!(
            tx.seek_unique_by_codomain::<Objid, Objid>(OneToOneSecondaryIndexed, &Objid::mk_id(1))
                .unwrap(),
            Objid::mk_id(2)
        );
        // Verify that the secondary index is updated... First check for new value.
        let children: ObjSet = tx
            .seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(2))
            .unwrap();
        assert_eq!(children.len(), 2);
        assert!(
            children.contains(Objid::mk_id(1)),
            "Expected children of 2 to contain 1"
        );
        assert!(
            !children.contains(Objid::mk_id(0)),
            "Expected children of 2 to not contain 0"
        );
        // Now check the old value.
        let children = tx
            .seek_by_codomain::<Objid, Objid, ObjSet>(OneToOneSecondaryIndexed, &Objid::mk_id(0))
            .unwrap();
        assert_eq!(children, ObjSet::from_items(&[Objid::mk_id(4)]));
    }
}
