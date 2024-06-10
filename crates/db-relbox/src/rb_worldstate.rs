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

use std::path::PathBuf;
use std::sync::Arc;

use strum::{EnumCount, IntoEnumIterator};

use moor_db::db_worldstate::DbTxWorldState;
use moor_db::loader::LoaderInterface;
use moor_db::{Database, RelationalWorldStateTransaction, WorldStateSequence, WorldStateTable};
use moor_values::model::WorldStateError;
use moor_values::model::{WorldState, WorldStateSource};
use moor_values::{AsByteBuffer, SYSTEM_OBJECT};
use relbox::relation_info_for;
use relbox::{RelBox, RelationInfo};

use crate::rel_transaction::RelboxTransaction;

/// An implementation of `WorldState` / `WorldStateSource` that uses the relbox as its backing
pub struct RelBoxWorldState {
    db: Arc<RelBox>,
}

impl RelBoxWorldState {
    pub fn open(path: Option<PathBuf>, memory_size: usize) -> (Self, bool) {
        let relations: Vec<RelationInfo> = WorldStateTable::iter().map(relation_info_for).collect();

        let db = RelBox::new(memory_size, path, &relations, WorldStateSequence::COUNT);

        // Check the db for sys (#0) object to see if this is a fresh DB or not.
        let fresh_db = {
            let canonical = db.copy_canonical();
            canonical[WorldStateTable::ObjectParent as usize]
                .seek_by_domain(SYSTEM_OBJECT.as_sliceref().unwrap())
                .expect("Could not seek for freshness check on DB")
                .is_empty()
        };
        (Self { db }, fresh_db)
    }
}

impl WorldStateSource for RelBoxWorldState {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let tx = self.db.clone().start_tx();
        let tx = RelboxTransaction::new(tx);
        let rel_tx = Box::new(RelationalWorldStateTransaction { tx: Some(tx) });
        Ok(Box::new(DbTxWorldState { tx: rel_tx }))
    }

    fn checkpoint(&self) -> Result<(), WorldStateError> {
        // noop
        Ok(())
    }
}

impl Database for RelBoxWorldState {
    fn loader_client(self: Arc<Self>) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        let tx = self.db.clone().start_tx();
        let tx = RelboxTransaction::new(tx);
        let rel_tx = Box::new(RelationalWorldStateTransaction { tx: Some(tx) });
        Ok(Box::new(DbTxWorldState { tx: rel_tx }))
    }

    fn world_state_source(self: Arc<Self>) -> Result<Arc<dyn WorldStateSource>, WorldStateError> {
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use strum::{EnumCount, IntoEnumIterator};

    use moor_db::{
        perform_reparent_props, perform_test_create_object, perform_test_create_object_fixed_id,
        perform_test_descendants, perform_test_location_contents, perform_test_object_move_commits,
        perform_test_parent_children, perform_test_regression_properties,
        perform_test_rename_property, perform_test_simple_property,
        perform_test_transitive_property_resolution,
        perform_test_transitive_property_resolution_clear_property, perform_test_verb_add_update,
        perform_test_verb_resolve, perform_test_verb_resolve_inherited,
        perform_test_verb_resolve_wildcard, RelationalWorldStateTransaction, WorldStateSequence,
        WorldStateTable,
    };
    use relbox::{relation_info_for, RelBox, RelationInfo};

    use crate::rel_transaction::RelboxTransaction;

    fn test_db() -> Arc<RelBox> {
        let relations: Vec<RelationInfo> = WorldStateTable::iter().map(relation_info_for).collect();

        RelBox::new(1 << 24, None, &relations, WorldStateSequence::COUNT)
    }

    pub fn begin_tx(
        db: &Arc<RelBox>,
    ) -> RelationalWorldStateTransaction<RelboxTransaction<WorldStateTable>> {
        let tx = RelboxTransaction::new(db.clone().start_tx());
        RelationalWorldStateTransaction { tx: Some(tx) }
    }

    #[test]
    fn test_create_object() {
        let db = test_db();
        perform_test_create_object(|| begin_tx(&db));
    }

    #[test]
    fn test_create_object_fixed_id() {
        let db = test_db();
        perform_test_create_object_fixed_id(|| begin_tx(&db));
    }

    #[test]
    fn test_parent_children() {
        let db = test_db();
        perform_test_parent_children(|| begin_tx(&db));
    }

    #[test]
    fn test_descendants() {
        let db = test_db();
        perform_test_descendants(|| begin_tx(&db));
    }

    #[test]
    fn test_location_contents() {
        let db = test_db();
        perform_test_location_contents(|| begin_tx(&db));
    }

    /// Test data integrity of object moves between commits.
    #[test]
    fn test_object_move_commits() {
        let db = test_db();
        perform_test_object_move_commits(|| begin_tx(&db));
    }

    #[test]
    fn test_simple_property() {
        let db = test_db();
        perform_test_simple_property(|| begin_tx(&db));
    }

    /// Regression test for updating-verbs failing.
    #[test]
    fn test_verb_add_update() {
        let db = test_db();
        perform_test_verb_add_update(|| begin_tx(&db));
    }

    #[test]
    fn test_transitive_property_resolution() {
        let db = test_db();
        perform_test_transitive_property_resolution(|| begin_tx(&db));
    }

    #[test]
    fn test_transitive_property_resolution_clear_property() {
        let db = test_db();
        perform_test_transitive_property_resolution_clear_property(|| begin_tx(&db));
    }

    #[test]
    fn test_rename_property() {
        let db = test_db();
        perform_test_rename_property(|| begin_tx(&db));
    }

    #[test]
    fn test_regression_properties() {
        let db = test_db();
        perform_test_regression_properties(|| begin_tx(&db));
    }

    #[test]
    fn test_verb_resolve() {
        let db = test_db();
        perform_test_verb_resolve(|| begin_tx(&db));
    }

    #[test]
    fn test_verb_resolve_inherited() {
        let db = test_db();
        perform_test_verb_resolve_inherited(|| begin_tx(&db));
    }

    #[test]
    fn test_verb_resolve_wildcard() {
        let db = test_db();
        perform_test_verb_resolve_wildcard(|| begin_tx(&db));
    }

    #[test]
    fn test_reparent() {
        let db = test_db();
        perform_reparent_props(|| begin_tx(&db));
    }
}
