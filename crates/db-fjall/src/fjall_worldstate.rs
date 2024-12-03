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

use crate::fjall_db::FjallDb;
use fjall::PersistMode;
use moor_db::db_worldstate::DbTxWorldState;
use moor_db::loader::LoaderInterface;
use moor_db::{Database, RelationalWorldStateTransaction, WorldStateTable};
use moor_values::model::{WorldState, WorldStateError, WorldStateSource};

impl WorldStateSource for FjallDb<WorldStateTable> {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let tx = self.new_transaction();
        let tx = RelationalWorldStateTransaction { tx };
        Ok(Box::new(DbTxWorldState { tx }))
    }

    fn checkpoint(&self) -> Result<(), WorldStateError> {
        self.keyspace.persist(PersistMode::SyncAll).unwrap();
        Ok(())
    }
}

impl Database for FjallDb<WorldStateTable> {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        let tx = self.new_transaction();
        let tx = RelationalWorldStateTransaction { tx };
        Ok(Box::new(DbTxWorldState { tx }))
    }
}

#[cfg(test)]
mod tests {
    use crate::fjall_db::FjallDb;
    use crate::FjallTransaction;
    use moor_db::{
        perform_reparent_props, perform_test_create_object, perform_test_create_object_fixed_id,
        perform_test_descendants, perform_test_location_contents, perform_test_max_object,
        perform_test_object_move_commits, perform_test_parent_children,
        perform_test_recycle_object, perform_test_regression_properties,
        perform_test_rename_property, perform_test_simple_property,
        perform_test_transitive_property_resolution,
        perform_test_transitive_property_resolution_clear_property, perform_test_verb_add_update,
        perform_test_verb_resolve, perform_test_verb_resolve_inherited,
        perform_test_verb_resolve_wildcard, RelationalWorldStateTransaction, WorldStateTable,
    };

    fn test_db() -> super::FjallDb<WorldStateTable> {
        let (db, _) = super::FjallDb::open(None);
        db
    }
    pub fn begin_tx(
        db: &FjallDb<WorldStateTable>,
    ) -> RelationalWorldStateTransaction<FjallTransaction<WorldStateTable>> {
        RelationalWorldStateTransaction {
            tx: db.new_transaction(),
        }
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

    #[test]
    fn test_recycle_object() {
        let db = test_db();
        perform_test_recycle_object(|| begin_tx(&db));
    }

    #[test]
    fn test_max_object() {
        let db = test_db();
        perform_test_max_object(|| begin_tx(&db));
    }
}
