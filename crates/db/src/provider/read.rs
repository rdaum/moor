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

//! Snapshot-bound logical reads. Cursors decode at most one tuple per advance.
//!
//! All scans use object reference byte order, then UUID bytes. Metadata names within
//! each entity use a stable backend order; the shared loader sorts visible names.
//! This order is shared across relations for streaming export joins. Callers must
//! not assume numeric object order. Runtime predicate scans use resident indexes.
use crate::{
    AnonymousObjectMetadata, EntityMetadataKey, ObjAndUUIDHolder, StringHolder,
    tx::{Error, Timestamp},
};
use moor_common::{
    model::{ObjFlag, PropDefs, PropPerms, VerbDefs},
    util::BitEnum,
};
use moor_var::{Obj, Var, program::ProgramType};
use std::sync::Arc;
use uuid::Uuid;

pub(crate) type TupleCursor<K, V> =
    Box<dyn Iterator<Item = Result<(Timestamp, K, V), Error>> + Send>;
pub(crate) trait ScanRequest {
    fn all() -> Self;
}
#[derive(Clone, Copy)]
pub(crate) enum ObjectScan {
    All,
}
impl ScanRequest for ObjectScan {
    fn all() -> Self {
        Self::All
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ObjectUuidScan {
    All,
    #[allow(dead_code)] // Available for object-scoped administrative scans.
    Object(Obj),
}
impl ScanRequest for ObjectUuidScan {
    fn all() -> Self {
        Self::All
    }
}
#[derive(Clone, Copy)]
pub(crate) enum MetadataEntity {
    Object,
    Property(Uuid),
    Verb(Uuid),
}
#[derive(Clone, Copy)]
pub(crate) enum MetadataScan {
    All,
    #[allow(dead_code)] // Available for object-scoped administrative scans.
    Object(Obj),
    Entity(Obj, MetadataEntity),
}
impl ScanRequest for MetadataScan {
    fn all() -> Self {
        Self::All
    }
}
pub(crate) trait ReadKey: Send + Sync + 'static {
    type Scan: ScanRequest;
    fn object(&self) -> Obj;
}
impl ReadKey for Obj {
    type Scan = ObjectScan;
    fn object(&self) -> Obj {
        *self
    }
}
impl ReadKey for ObjAndUUIDHolder {
    type Scan = ObjectUuidScan;
    fn object(&self) -> Obj {
        self.obj()
    }
}
impl ReadKey for EntityMetadataKey {
    type Scan = MetadataScan;
    fn object(&self) -> Obj {
        self.obj()
    }
}

/// A reader owns its snapshot lease. An absent key means absence in that snapshot.
/// Cursors must retain the lease, including when the reader itself is dropped.
pub(crate) trait RelationReader<K: ReadKey, V>: Send + Sync {
    fn get(&self, key: &K) -> Result<Option<(Timestamp, V)>, Error>;
    fn scan(&self, request: K::Scan) -> Result<TupleCursor<K, V>, Error>;
}
macro_rules! define_readers {
    ($( $field:ident $category:ident $policy:ident $arrow:tt $domain:ty,$codomain:ty ),* $(,)?) => {
        /// Typed readers for one common storage snapshot.
        pub(crate) struct SnapshotReaders {
            $( pub(crate) $field: Arc<dyn RelationReader<$domain, $codomain>>, )*
        }
    };
}
crate::relation_registry::relation_registry!(define_readers);
