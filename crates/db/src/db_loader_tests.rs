// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Tests for LoaderInterface functionality

#[cfg(test)]
mod tests {
    use crate::{Database, DatabaseConfig, TxDB};
    use moor_common::model::{ObjAttrs, ObjectKind, VerbArgsSpec, VerbFlag};
    use moor_common::util::BitEnum;
    use moor_var::{
        NOTHING, SYSTEM_OBJECT, Symbol,
        program::{ProgramType, program::Program},
        v_str,
    };
    use std::{path::Path, sync::Arc};

    fn test_db(path: &Path) -> Arc<TxDB> {
        Arc::new(TxDB::open(Some(path), DatabaseConfig::default()).0)
    }

    #[test]
    fn test_loader_delete_property() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object and define a property
        let obj = loader
            .create_object(
                ObjectKind::NextObjid,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        loader
            .define_property(
                &obj,
                &obj,
                Symbol::mk("test_prop"),
                &SYSTEM_OBJECT,
                BitEnum::new(),
                Some(v_str("test value")),
            )
            .unwrap();

        // Verify property exists
        let prop_value = loader
            .get_existing_property_value(&obj, Symbol::mk("test_prop"))
            .unwrap();
        assert!(prop_value.is_some());

        // Delete the property
        loader
            .delete_property(&obj, Symbol::mk("test_prop"))
            .unwrap();

        // Verify property no longer exists
        let prop_value = loader
            .get_existing_property_value(&obj, Symbol::mk("test_prop"))
            .unwrap();
        assert!(prop_value.is_none());

        loader.commit().unwrap();
    }

    #[test]
    fn test_loader_remove_verb() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object and add a verb
        let obj = loader
            .create_object(
                ObjectKind::NextObjid,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        loader
            .add_verb(
                &obj,
                &[Symbol::mk("test_verb")],
                &obj,
                BitEnum::new_with(VerbFlag::Exec),
                VerbArgsSpec::this_none_this(),
                ProgramType::MooR(Program::new()),
            )
            .unwrap();

        // Verify verb exists
        let verb = loader
            .get_existing_verb_by_names(&obj, &[Symbol::mk("test_verb")])
            .unwrap();
        assert!(verb.is_some());
        let (uuid, _) = verb.unwrap();

        // Remove the verb
        loader.remove_verb(&obj, uuid).unwrap();

        // Verify verb no longer exists
        let verb = loader
            .get_existing_verb_by_names(&obj, &[Symbol::mk("test_verb")])
            .unwrap();
        assert!(verb.is_none());

        loader.commit().unwrap();
    }

    #[test]
    fn test_loader_get_verb_program() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object and add a verb
        let obj = loader
            .create_object(
                ObjectKind::NextObjid,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let program = ProgramType::MooR(Program::new());
        loader
            .add_verb(
                &obj,
                &[Symbol::mk("test_verb")],
                &obj,
                BitEnum::new_with(VerbFlag::Exec),
                VerbArgsSpec::this_none_this(),
                program.clone(),
            )
            .unwrap();

        // Get verb UUID
        let (uuid, _) = loader
            .get_existing_verb_by_names(&obj, &[Symbol::mk("test_verb")])
            .unwrap()
            .unwrap();

        // Get verb program
        let retrieved_program = loader.get_verb_program(&obj, uuid).unwrap();
        assert_eq!(retrieved_program, program);

        loader.commit().unwrap();
    }
}
