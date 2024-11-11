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

use crate::fjall_relation::RelationPartitions;
use crate::FjallTransaction;
use fjall::{Config, PartitionCreateOptions, TxKeyspace, TxPartitionHandle};
use moor_db::WorldStateTable;
use std::fmt::Display;
use std::marker::PhantomData;
use std::path::Path;
use strum::{EnumProperty, IntoEnumIterator};
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use crate::fjall_db::FjallDb;
    use moor_values::model::WorldStateSource;

    #[test]
    fn test_fjall_db_open_close() {
        let (db, fresh) = FjallDb::open(None);
        assert!(fresh);
        db.checkpoint().unwrap();
    }
}

pub struct FjallDb<Relation>
where
    Relation: Send + Sync + Display + Into<usize> + Copy,
{
    pub(crate) keyspace: TxKeyspace,
    sequences_partition: TxPartitionHandle,
    relations_partitions: Vec<RelationPartitions>,
    phantom_data: PhantomData<Relation>,

    /// If this is a temporary database, this will be Some(TempDir) that will be cleaned up when
    /// the database is dropped.
    _tmpdir: Option<TempDir>,
}

impl<Relation> FjallDb<Relation>
where
    Relation: Send + Sync + Display + Into<usize> + Copy,
{
    pub fn open(path: Option<&Path>) -> (Self, bool) {
        let tmpdir = if path.is_none() {
            Some(TempDir::new().unwrap())
        } else {
            None
        };

        let path = path.unwrap_or_else(|| tmpdir.as_ref().unwrap().path());
        let keyspace = Config::new(path).open_transactional().unwrap();
        let sequences_partition = keyspace
            .open_partition("sequences", PartitionCreateOptions::default())
            .unwrap();
        let mut relations_partitions = Vec::new();

        // If the partitions count in the keyspaces is not equal to the count of relations in the
        // WorldStateTable, we're "fresh"
        let fresh = keyspace.partition_count() != WorldStateTable::iter().count();
        for relation in WorldStateTable::iter() {
            let partition = keyspace
                .open_partition(&relation.to_string(), PartitionCreateOptions::default())
                .unwrap();
            let has_secondary = relation
                .get_str("SecondaryIndexed")
                .map(|it| it == "true")
                .unwrap_or(false);
            let secondary = (has_secondary).then(|| {
                keyspace
                    .open_partition(
                        &format!("{}_secondary", relation.to_string()),
                        PartitionCreateOptions::default(),
                    )
                    .unwrap()
            });
            relations_partitions.push(RelationPartitions {
                primary: partition,
                secondary,
            });
        }
        (
            Self {
                keyspace,
                sequences_partition,
                relations_partitions,
                phantom_data: Default::default(),
                _tmpdir: tmpdir,
            },
            fresh,
        )
    }

    pub fn new_transaction(&self) -> FjallTransaction<Relation> {
        let tx = self.keyspace.write_tx().unwrap();
        FjallTransaction::new(tx, &self.sequences_partition, &self.relations_partitions)
    }
}
