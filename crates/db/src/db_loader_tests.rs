// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Tests for LoaderInterface and batch mutation functionality

#[cfg(test)]
mod tests {
    use crate::{Database, DatabaseConfig, TxDB};
    use moor_common::model::{
        loader::batch_mutate,
        mutations::ObjectMutation,
        ObjAttrs, ObjFlag, PropFlag, VerbArgsSpec, VerbFlag, WorldStateSource,
    };
    use moor_common::util::BitEnum;
    use moor_var::{
        program::{program::Program, ProgramType},
        v_int, v_str, Obj, Symbol, NOTHING, SYSTEM_OBJECT,
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
                None,
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
        loader.delete_property(&obj, Symbol::mk("test_prop")).unwrap();

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
                None,
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
                None,
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

    #[test]
    fn test_batch_mutate_define_property() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object
        let obj = loader
            .create_object(
                None,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        // Define a property via mutation
        let mutations = vec![ObjectMutation::DefineProperty {
            name: Symbol::mk("test_prop"),
            owner: obj,
            flags: BitEnum::new_with(PropFlag::Read),
            value: Some(v_str("hello")),
        }];

        let result = batch_mutate(loader.as_mut(), &obj, &mutations);
        assert!(result.all_succeeded());

        // Verify property was created
        let prop_value = loader
            .get_existing_property_value(&obj, Symbol::mk("test_prop"))
            .unwrap();
        assert!(prop_value.is_some());
        let (value, _) = prop_value.unwrap();
        assert_eq!(value, v_str("hello"));

        loader.commit().unwrap();
    }

    #[test]
    fn test_batch_mutate_delete_property() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object with a property
        let obj = loader
            .create_object(
                None,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        loader
            .define_property(
                &obj,
                &obj,
                Symbol::mk("test_prop"),
                &obj,
                BitEnum::new(),
                Some(v_int(42)),
            )
            .unwrap();

        // Delete it via mutation
        let mutations = vec![ObjectMutation::DeleteProperty {
            name: Symbol::mk("test_prop"),
        }];

        let result = batch_mutate(loader.as_mut(), &obj, &mutations);
        assert!(result.all_succeeded());

        // Verify property was deleted
        let prop_value = loader
            .get_existing_property_value(&obj, Symbol::mk("test_prop"))
            .unwrap();
        assert!(prop_value.is_none());

        loader.commit().unwrap();
    }

    #[test]
    fn test_batch_mutate_set_property_value() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object with a property
        let obj = loader
            .create_object(
                None,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        loader
            .define_property(
                &obj,
                &obj,
                Symbol::mk("test_prop"),
                &obj,
                BitEnum::new(),
                Some(v_int(42)),
            )
            .unwrap();

        // Change value via mutation
        let mutations = vec![ObjectMutation::SetPropertyValue {
            name: Symbol::mk("test_prop"),
            value: v_int(99),
        }];

        let result = batch_mutate(loader.as_mut(), &obj, &mutations);
        assert!(result.all_succeeded());

        // Verify value changed
        let prop_value = loader
            .get_existing_property_value(&obj, Symbol::mk("test_prop"))
            .unwrap()
            .unwrap();
        assert_eq!(prop_value.0, v_int(99));

        loader.commit().unwrap();
    }

    #[test]
    fn test_batch_mutate_define_verb() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object
        let obj = loader
            .create_object(
                None,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        // Define a verb via mutation
        let mutations = vec![ObjectMutation::DefineVerb {
            names: vec![Symbol::mk("test_verb")],
            owner: obj,
            flags: BitEnum::new_with(VerbFlag::Exec),
            argspec: VerbArgsSpec::this_none_this(),
            program: ProgramType::MooR(Program::new()),
        }];

        let result = batch_mutate(loader.as_mut(), &obj, &mutations);
        assert!(result.all_succeeded());

        // Verify verb was created
        let verb = loader
            .get_existing_verb_by_names(&obj, &[Symbol::mk("test_verb")])
            .unwrap();
        assert!(verb.is_some());

        loader.commit().unwrap();
    }

    #[test]
    fn test_batch_mutate_delete_verb() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object with a verb
        let obj = loader
            .create_object(
                None,
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

        // Delete it via mutation
        let mutations = vec![ObjectMutation::DeleteVerb {
            names: vec![Symbol::mk("test_verb")],
        }];

        let result = batch_mutate(loader.as_mut(), &obj, &mutations);
        assert!(result.all_succeeded());

        // Verify verb was deleted
        let verb = loader
            .get_existing_verb_by_names(&obj, &[Symbol::mk("test_verb")])
            .unwrap();
        assert!(verb.is_none());

        loader.commit().unwrap();
    }

    #[test]
    fn test_batch_mutate_multiple_operations() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object
        let obj = loader
            .create_object(
                None,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        // Apply multiple mutations at once
        let mutations = vec![
            ObjectMutation::DefineProperty {
                name: Symbol::mk("prop1"),
                owner: obj,
                flags: BitEnum::new(),
                value: Some(v_int(1)),
            },
            ObjectMutation::DefineProperty {
                name: Symbol::mk("prop2"),
                owner: obj,
                flags: BitEnum::new(),
                value: Some(v_int(2)),
            },
            ObjectMutation::DefineVerb {
                names: vec![Symbol::mk("verb1")],
                owner: obj,
                flags: BitEnum::new_with(VerbFlag::Exec),
                argspec: VerbArgsSpec::this_none_this(),
                program: ProgramType::MooR(Program::new()),
            },
            ObjectMutation::SetObjectFlags {
                flags: BitEnum::new_with(ObjFlag::Read),
            },
        ];

        let result = batch_mutate(loader.as_mut(), &obj, &mutations);
        assert!(result.all_succeeded());
        assert_eq!(result.succeeded_count(), 4);
        assert_eq!(result.failed_count(), 0);

        // Verify all mutations were applied
        assert!(loader
            .get_existing_property_value(&obj, Symbol::mk("prop1"))
            .unwrap()
            .is_some());
        assert!(loader
            .get_existing_property_value(&obj, Symbol::mk("prop2"))
            .unwrap()
            .is_some());
        assert!(loader
            .get_existing_verb_by_names(&obj, &[Symbol::mk("verb1")])
            .unwrap()
            .is_some());

        loader.commit().unwrap();

        // Verify in new transaction
        let tx = db.new_world_state().unwrap();
        let flags = tx.flags_of(&obj).unwrap();
        assert!(flags.contains(ObjFlag::Read));
    }

    #[test]
    fn test_batch_mutate_with_failure() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object
        let obj = loader
            .create_object(
                None,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        // Try to delete a non-existent property
        let mutations = vec![
            ObjectMutation::DefineProperty {
                name: Symbol::mk("good_prop"),
                owner: obj,
                flags: BitEnum::new(),
                value: Some(v_int(1)),
            },
            ObjectMutation::DeleteProperty {
                name: Symbol::mk("nonexistent"),
            },
            ObjectMutation::DefineProperty {
                name: Symbol::mk("another_prop"),
                owner: obj,
                flags: BitEnum::new(),
                value: Some(v_int(2)),
            },
        ];

        let result = batch_mutate(loader.as_mut(), &obj, &mutations);
        assert!(!result.all_succeeded());
        assert_eq!(result.succeeded_count(), 2);
        assert_eq!(result.failed_count(), 1);

        // Check that the first mutation succeeded but the second failed
        assert!(result.results[0].result.is_ok());
        assert!(result.results[1].result.is_err());
        assert!(result.results[2].result.is_ok());

        // Get the error
        let (failed_idx, _err) = result.first_error().unwrap();
        assert_eq!(failed_idx, 1);
    }

    #[test]
    fn test_batch_mutate_update_verb_program() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object with a verb
        let obj = loader
            .create_object(
                None,
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

        // Update the verb program
        let new_program = ProgramType::MooR(Program::new());
        let mutations = vec![ObjectMutation::UpdateVerbProgram {
            names: vec![Symbol::mk("test_verb")],
            program: new_program.clone(),
        }];

        let result = batch_mutate(loader.as_mut(), &obj, &mutations);
        assert!(result.all_succeeded());

        // Verify program was updated
        let (uuid, _) = loader
            .get_existing_verb_by_names(&obj, &[Symbol::mk("test_verb")])
            .unwrap()
            .unwrap();
        let retrieved_program = loader.get_verb_program(&obj, uuid).unwrap();
        assert_eq!(retrieved_program, new_program);

        loader.commit().unwrap();
    }

    #[test]
    fn test_batch_mutate_update_verb_metadata() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        // Create an object with a verb
        let obj = loader
            .create_object(
                None,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        loader
            .add_verb(
                &obj,
                &[Symbol::mk("old_name")],
                &obj,
                BitEnum::new_with(VerbFlag::Exec),
                VerbArgsSpec::this_none_this(),
                ProgramType::MooR(Program::new()),
            )
            .unwrap();

        // Rename the verb
        let mutations = vec![ObjectMutation::UpdateVerbMetadata {
            names: vec![Symbol::mk("old_name")],
            new_names: Some(vec![Symbol::mk("new_name")]),
            owner: None,
            flags: None,
            argspec: None,
        }];

        let result = batch_mutate(loader.as_mut(), &obj, &mutations);
        assert!(result.all_succeeded());

        // Verify old name doesn't work
        let verb = loader
            .get_existing_verb_by_names(&obj, &[Symbol::mk("old_name")])
            .unwrap();
        assert!(verb.is_none());

        // Verify new name works
        let verb = loader
            .get_existing_verb_by_names(&obj, &[Symbol::mk("new_name")])
            .unwrap();
        assert!(verb.is_some());

        loader.commit().unwrap();
    }
}
