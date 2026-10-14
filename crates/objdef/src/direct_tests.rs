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

//! Tests for direct objdef merge using the public API.
//! Establishes objects, commits, then verifies explicit updates and inherited overrides.

#[cfg(test)]
mod tests {
    use crate::{ObjDefLoaderOptions, ObjectDefinitionLoader};
    use moor_common::model::{
        HasUuid, ObjFlag, PrepSpec, TaskPermissions, VerbFlag, VerbLookup, WorldStateSource,
        command_verb_argspec,
    };
    use moor_common::util::BitEnum;
    use moor_compiler::{CompileOptions, compile};
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_var::{NOTHING, Obj, SYSTEM_OBJECT, Symbol, program::ProgramType, v_int, v_str};
    use std::{path::Path, sync::Arc};

    fn test_db(path: &Path) -> Arc<TxDB> {
        Arc::new(
            TxDB::try_open_fjall(path, DatabaseConfig::default())
                .unwrap()
                .0,
        )
    }

    fn system_permissions() -> TaskPermissions {
        TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new())
    }

    /// Create initial objects with inheritance relationships and commit them
    fn setup_objects() -> Result<Arc<TxDB>, Box<dyn std::error::Error>> {
        let tmpdir = tempfile::tempdir()?;
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let options = ObjDefLoaderOptions::default();

        // Create root object #1
        let root_spec = r#"
            object #1
                name: "Root Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: false
                programmer: true
                player: false
                fertile: true
                readable: true

                property description (owner: #1, flags: "rc") = "initial value";
                property count (owner: #1, flags: "rw") = 42;
                property base_prop (owner: #1, flags: "rc") = "root value";

                verb "look" (this none none) owner: #1 flags: "rxd"
                    return "original look";
                endverb
            endobject
        "#;
        parser.load_single_object(root_spec, CompileOptions::default(), options)?;

        // Create child object #2 inheriting from #1
        let child_spec = r#"
            object #2
                name: "Child Object"
                owner: #1
                parent: #1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: true

                override description = "child overridden";
                property child_prop (owner: #2, flags: "rw") = "child only";

                verb "child_verb" (this none none) owner: #2 flags: "rxd"
                    return "child implementation";
                endverb
            endobject
        "#;

        let options2 = ObjDefLoaderOptions::default();
        let mut parser2 = ObjectDefinitionLoader::new(loader.as_mut());
        parser2.load_single_object(child_spec, CompileOptions::default(), options2)?;

        loader.commit()?;
        Ok(db)
    }

    #[test]
    fn explicit_clear_and_replacement_restore_clear_values() {
        let db = setup_objects().unwrap();
        let child = Obj::mk_id(2);
        let names = [Symbol::mk("description"), Symbol::mk("child_prop")];
        let source = r#"
            object #2
                parent: #1
                owner: #1
                override description (owner: #1, flags: "rc") [note -> "clear"] clear;
                property child_prop (owner: #2, flags: "rw") [note -> "clear"] clear;
            endobject
        "#;
        // Repeated imports must recognize an existing clear property, not redefine it.
        for _ in 0..2 {
            let mut loader = db.loader_client().unwrap();
            ObjectDefinitionLoader::new(loader.as_mut())
                .load_single_object(source, CompileOptions::default(), Default::default())
                .unwrap();
            loader.commit().unwrap();
            let ws = db.new_world_state().unwrap();
            for name in names {
                assert!(
                    ws.is_property_clear(&system_permissions(), &child, name)
                        .unwrap()
                );
                assert_eq!(
                    ws.get_property_metadata(
                        &system_permissions(),
                        &child,
                        name,
                        Symbol::mk("note")
                    )
                    .unwrap(),
                    Some(v_str("clear"))
                );
            }
            assert_eq!(
                ws.retrieve_property(&system_permissions(), &child, names[0])
                    .unwrap(),
                v_str("initial value")
            );
        }

        // A legacy declaration with no value also restores clear during replacement.
        let mut loader = db.loader_client().unwrap();
        loader
            .set_property(&child, names[1], None, None, Some(v_int(99)))
            .unwrap();
        ObjectDefinitionLoader::new(loader.as_mut())
            .reload_single_object(
                r#"object #2 parent: #1 property child_prop (owner: #2, flags: "rw"); endobject"#,
                CompileOptions::default(),
                None,
                None,
            )
            .unwrap();
        loader.commit().unwrap();
        let ws = db.new_world_state().unwrap();
        assert!(
            ws.is_property_clear(&system_permissions(), &child, names[1])
                .unwrap()
        );
    }

    #[test]
    fn omitted_property_values_preserve_local_values() {
        let db = setup_objects().unwrap();
        let mut loader = db.loader_client().unwrap();
        ObjectDefinitionLoader::new(loader.as_mut())
            .load_single_object(
                r#"
                object #2
                    parent: #1
                    override description;
                    override description (owner: #1, flags: "r") [note -> 1];
                    property child_prop (owner: #2, flags: "r") [note -> 1];
                endobject
            "#,
                CompileOptions::default(),
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();
        let ws = db.new_world_state().unwrap();
        for (name, expected) in [
            ("description", "child overridden"),
            ("child_prop", "child only"),
        ] {
            let name = Symbol::mk(name);
            assert_eq!(
                ws.retrieve_property(&system_permissions(), &Obj::mk_id(2), name)
                    .unwrap(),
                v_str(expected)
            );
            assert_eq!(
                ws.get_property_metadata(
                    &system_permissions(),
                    &Obj::mk_id(2),
                    name,
                    Symbol::mk("note")
                )
                .unwrap(),
                Some(v_int(1))
            );
        }
    }

    #[test]
    fn test_property_merge() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let conflicting_spec = r#"
            object #1
                name: "Root Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: false
                programmer: true
                player: false
                fertile: true
                readable: true

                override description = "clobbered value";
                override count = 777;
                override base_prop = "clobbered root value";
            endobject
        "#;

        let options = ObjDefLoaderOptions::default();

        let _results =
            parser.load_single_object(conflicting_spec, CompileOptions::default(), options)?;
        loader.commit()?;

        // Values should be changed due to Clobber mode
        let ws = db.new_world_state()?;
        let desc = ws.retrieve_property(
            &system_permissions(),
            &Obj::mk_id(1),
            Symbol::mk("description"),
        )?;
        let count =
            ws.retrieve_property(&system_permissions(), &Obj::mk_id(1), Symbol::mk("count"))?;
        let base_prop = ws.retrieve_property(
            &system_permissions(),
            &Obj::mk_id(1),
            Symbol::mk("base_prop"),
        )?;

        assert_eq!(desc, v_str("clobbered value"));
        assert_eq!(count, v_int(777));
        assert_eq!(base_prop, v_str("clobbered root value"));

        Ok(())
    }

    #[test]
    fn test_parentage_change() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        // Try to change child's parent from #1 to #-1 (NOTHING) in Clobber mode
        let conflicting_spec = r#"
            object #2
                name: "Child Object"
                owner: #1
                parent: #-1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: true
            endobject
        "#;

        let options = ObjDefLoaderOptions::default();

        let _results =
            parser.load_single_object(conflicting_spec, CompileOptions::default(), options)?;
        loader.commit()?;

        let ws = db.new_world_state()?;
        let child_parent = ws.parent_of(&system_permissions(), &Obj::mk_id(2))?;
        assert_eq!(
            child_parent, NOTHING,
            "Child parent should now be NOTHING (#-1)"
        );

        Ok(())
    }

    #[test]
    fn test_verb_merge() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let conflicting_spec = r#"
            object #1
                name: "Root Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: false
                programmer: true
                player: false
                fertile: true
                readable: true

                verb "look" (this none none) owner: #0 flags: "rw"
                    return "modified look";
                endverb
            endobject
        "#;

        let options = ObjDefLoaderOptions::default();

        let _results =
            parser.load_single_object(conflicting_spec, CompileOptions::default(), options)?;
        loader.commit()?;

        // Check if verb was changed due to Clobber mode
        let ws = db.new_world_state()?;
        let target = Obj::mk_id(1);
        let argspec = command_verb_argspec(&target, &target, PrepSpec::None, &NOTHING);
        let verb_result = ws.lookup_verb(
            &system_permissions(),
            VerbLookup::command(&target, Symbol::mk("look"), argspec),
        )?;

        let verbdef = verb_result.expect("look verb should remain after clobber");
        assert_eq!(verbdef.owner(), SYSTEM_OBJECT);
        assert!(!verbdef.flags().contains(VerbFlag::Exec));
        let (program, _) = ws.retrieve_verb(&system_permissions(), &target, verbdef.uuid())?;
        assert_eq!(
            program,
            ProgramType::MooR(compile(
                "return \"modified look\";",
                CompileOptions::default()
            )?),
            "Clobber mode should install the incoming verb body"
        );

        Ok(())
    }

    #[test]
    fn test_flags_merge() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let conflicting_spec = r#"
            object #1
                name: "Root Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: true
                programmer: false
                player: true
                fertile: false
                readable: false
            endobject
        "#;

        let options = ObjDefLoaderOptions::default();

        let _results =
            parser.load_single_object(conflicting_spec, CompileOptions::default(), options)?;
        loader.commit()?;

        // Check if flags were changed due to Clobber mode
        let ws = db.new_world_state()?;
        let flags = ws.flags_of(&Obj::mk_id(1))?;

        // Should be changed to new values
        assert!(
            flags.contains(ObjFlag::Wizard),
            "Wizard flag should be true (clobbered)"
        );
        assert!(
            !flags.contains(ObjFlag::Programmer),
            "Programmer flag should be false (clobbered)"
        );
        assert!(
            flags.contains(ObjFlag::User),
            "User flag should be true (clobbered)"
        );
        assert!(
            !flags.contains(ObjFlag::Fertile),
            "Fertile flag should be false (clobbered)"
        );
        assert!(
            !flags.contains(ObjFlag::Read),
            "Read flag should be false (clobbered)"
        );

        Ok(())
    }

    #[test]
    fn test_property_override_merge() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;

        // First, load an objdef that overrides some properties
        let mut loader1 = db.loader_client()?;
        let mut parser1 = ObjectDefinitionLoader::new(loader1.as_mut());
        let first_override_spec = r#"
            object #2
                name: "Child Object"
                owner: #1
                parent: #1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: true

                override description = "first override";
                override base_prop = "first base override";
            endobject
        "#;

        let options = ObjDefLoaderOptions::default();
        parser1.load_single_object(first_override_spec, CompileOptions::default(), options)?;
        loader1.commit()?;

        // Now try to load conflicting overrides in Clobber mode
        let mut loader2 = db.loader_client()?;
        let mut parser2 = ObjectDefinitionLoader::new(loader2.as_mut());
        let conflicting_override_spec = r#"
            object #2
                name: "Child Object"
                owner: #1
                parent: #1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: true

                override description = "second override";
                override base_prop = "second base override";
            endobject
        "#;

        let clobber_options = ObjDefLoaderOptions::default();

        let _results = parser2.load_single_object(
            conflicting_override_spec,
            CompileOptions::default(),
            clobber_options,
        )?;
        loader2.commit()?;

        // Verify override values were changed due to Clobber mode
        let ws = db.new_world_state()?;
        let desc = ws.retrieve_property(
            &system_permissions(),
            &Obj::mk_id(2),
            Symbol::mk("description"),
        )?;
        let base_prop = ws.retrieve_property(
            &system_permissions(),
            &Obj::mk_id(2),
            Symbol::mk("base_prop"),
        )?;

        assert_eq!(desc, v_str("second override"));
        assert_eq!(base_prop, v_str("second base override"));

        Ok(())
    }
}
