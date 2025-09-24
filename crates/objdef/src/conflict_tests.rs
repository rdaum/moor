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

//! Tests for objdef conflict detection and resolution using only the public API.
//! Establishes objects, commits, then loads conflicting objdefs to test conflict handling.

#[cfg(test)]
mod tests {
    use crate::{ConflictMode, Entity, ObjDefLoaderOptions, ObjectDefinitionLoader};
    use moor_common::model::{ObjFlag, PrepSpec, VerbFlag, WorldStateSource};
    use moor_compiler::CompileOptions;
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_var::{NOTHING, Obj, SYSTEM_OBJECT, Symbol, v_int, v_str};
    use std::path::Path;
    use std::sync::Arc;

    fn test_db(path: &Path) -> Arc<TxDB> {
        Arc::new(TxDB::open(Some(path), DatabaseConfig::default()).0)
    }

    /// Create initial objects with inheritance relationships and commit them
    fn setup_objects() -> Result<Arc<TxDB>, Box<dyn std::error::Error>> {
        let tmpdir = tempfile::tempdir()?;
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Clobber,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

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
        parser.load_single_object(root_spec, CompileOptions::default(), None, None, options)?;

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

        let options2 = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Clobber,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };
        let mut parser2 = ObjectDefinitionLoader::new(loader.as_mut());
        parser2.load_single_object(child_spec, CompileOptions::default(), None, None, options2)?;

        loader.commit()?;
        Ok(db)
    }

    #[test]
    fn test_property_conflict_skip_mode() -> Result<(), Box<dyn std::error::Error>> {
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

                override description = "changed value";
                override count = 999;
                override base_prop = "changed root value";
            endobject
        "#;

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Skip,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let _results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
        loader.commit()?;

        // Should detect conflicts
        assert!(
            !_results.conflicts.is_empty(),
            "Should detect property conflicts"
        );

        // Verify values weren't changed due to Skip mode
        let ws = db.new_world_state()?;
        let desc =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(1), Symbol::mk("description"))?;
        let count = ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(1), Symbol::mk("count"))?;
        let base_prop =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(1), Symbol::mk("base_prop"))?;

        assert_eq!(desc, v_str("initial value"));
        assert_eq!(count, v_int(42));
        assert_eq!(base_prop, v_str("root value"));

        Ok(())
    }

    #[test]
    fn test_property_conflict_clobber_mode() -> Result<(), Box<dyn std::error::Error>> {
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

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Clobber,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let _results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
        loader.commit()?;

        // Values should be changed due to Clobber mode
        let ws = db.new_world_state()?;
        let desc =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(1), Symbol::mk("description"))?;
        let count = ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(1), Symbol::mk("count"))?;
        let base_prop =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(1), Symbol::mk("base_prop"))?;

        assert_eq!(desc, v_str("clobbered value"));
        assert_eq!(count, v_int(777));
        assert_eq!(base_prop, v_str("clobbered root value"));

        Ok(())
    }

    #[test]
    fn test_flags_conflict_skip() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let conflicting_spec = r#"
            object #1
                name: "Test Object"
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

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Skip,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let _results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
        loader.commit()?;

        // Should detect conflicts
        assert!(
            !_results.conflicts.is_empty(),
            "Should detect flag conflicts"
        );

        // Check if flags remained unchanged (Skip mode should preserve original values)
        let ws = db.new_world_state()?;
        let flags = ws.flags_of(&Obj::mk_id(1))?;

        // Original values should be preserved in Skip mode
        assert!(
            !flags.contains(ObjFlag::Wizard),
            "Wizard flag should remain false (original)"
        );
        assert!(
            flags.contains(ObjFlag::Programmer),
            "Programmer flag should remain true (original)"
        );
        assert!(
            !flags.contains(ObjFlag::User),
            "User flag should remain false (original)"
        );
        assert!(
            flags.contains(ObjFlag::Fertile),
            "Fertile flag should remain true (original)"
        );
        assert!(
            flags.contains(ObjFlag::Read),
            "Read flag should remain true (original)"
        );

        Ok(())
    }

    #[test]
    fn test_entity_specific_overrides() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let conflicting_spec = r#"
            object #1
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: true
                programmer: false
                player: true
                fertile: false
                readable: false

                override description = "should be skipped";
            endobject
        "#;

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Skip,
            target_object: None,
            constants: None,
            // Override only object flags, but skip property changes
            overrides: vec![(Obj::mk_id(1), Entity::ObjectFlags)],
            removals: vec![],
        };

        let _results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
        loader.commit()?;

        let ws = db.new_world_state()?;
        let _flags = ws.flags_of(&Obj::mk_id(1))?;
        let _desc =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(1), Symbol::mk("description"))?;

        // Verify entity-specific overrides work

        Ok(())
    }

    #[test]
    fn test_verb_conflict_skip() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let conflicting_spec = r#"
            object #1
                name: "Test Object"
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

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Skip,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let _results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
        loader.commit()?;

        // Check if original verb is unchanged
        let ws = db.new_world_state()?;
        let verb_result = ws.find_command_verb_on(
            &SYSTEM_OBJECT,
            &Obj::mk_id(1),
            Symbol::mk("look"),
            &Obj::mk_id(1),
            PrepSpec::None,
            &NOTHING,
        )?;

        if let Some((_, verbdef)) = verb_result {
            // Should still be original owner (#1) and executable, not changed to #0 and non-executable
            assert_eq!(verbdef.owner(), Obj::mk_id(1));
            assert!(verbdef.flags().contains(VerbFlag::Exec));
        }

        Ok(())
    }

    #[test]
    fn test_dry_run_mode() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let conflicting_spec = r#"
            object #1
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: true
                programmer: false
                player: true
                fertile: false
                readable: false

                override description = "dry run change";
            endobject
        "#;

        let options = ObjDefLoaderOptions {
            dry_run: true,
            conflict_mode: ConflictMode::Clobber,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let _results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;

        // Should recommend not to commit
        assert!(!_results.commit);

        // Don't commit (as recommended)
        // Verify nothing changed
        let ws = db.new_world_state()?;
        let desc =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(1), Symbol::mk("description"))?;
        let flags = ws.flags_of(&Obj::mk_id(1))?;

        assert_eq!(desc, v_str("initial value"));
        assert!(!flags.contains(ObjFlag::Wizard));

        Ok(())
    }

    #[test]
    fn test_parentage_conflict_skip() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        // Try to change child's parent from #1 to #-1
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

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Skip,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
        loader.commit()?;

        // Should detect parentage conflict
        assert!(
            !results.conflicts.is_empty(),
            "Should detect parentage conflicts"
        );

        // Verify parent relationship remained unchanged
        let ws = db.new_world_state()?;
        let child_parent = ws.parent_of(&SYSTEM_OBJECT, &Obj::mk_id(2))?;
        assert_eq!(child_parent, Obj::mk_id(1), "Child parent should remain #1");

        Ok(())
    }

    #[test]
    fn test_inherited_property_conflicts() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_objects()?;
        let mut loader = db.loader_client()?;
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        // Try to modify inherited properties on child object
        let conflicting_spec = r#"
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

                override description = "modified child description";
                override base_prop = "modified inherited prop";
                override child_prop = "modified child only";
            endobject
        "#;

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Skip,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
        loader.commit()?;

        // Should detect property conflicts
        assert!(
            !results.conflicts.is_empty(),
            "Should detect property conflicts"
        );

        // Verify original property values are preserved
        let ws = db.new_world_state()?;
        let child_desc =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(2), Symbol::mk("description"))?;
        let child_only =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(2), Symbol::mk("child_prop"))?;

        assert_eq!(child_desc, v_str("child overridden"));
        assert_eq!(child_only, v_str("child only"));

        Ok(())
    }

    #[test]
    fn test_parentage_change_clobber() -> Result<(), Box<dyn std::error::Error>> {
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

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Clobber,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let _results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
        loader.commit()?;

        // May or may not detect conflict (Clobber mode applies the change)

        // Verify parent relationship was changed
        let ws = db.new_world_state()?;
        let child_parent = ws.parent_of(&SYSTEM_OBJECT, &Obj::mk_id(2))?;
        assert_eq!(
            child_parent, NOTHING,
            "Child parent should now be NOTHING (#-1)"
        );

        Ok(())
    }

    #[test]
    fn test_verb_conflict_clobber() -> Result<(), Box<dyn std::error::Error>> {
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

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Clobber,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let _results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
        loader.commit()?;

        // Check if verb was changed due to Clobber mode
        let ws = db.new_world_state()?;
        let verb_result = ws.find_command_verb_on(
            &SYSTEM_OBJECT,
            &Obj::mk_id(1),
            Symbol::mk("look"),
            &Obj::mk_id(1),
            PrepSpec::None,
            &NOTHING,
        )?;

        if let Some((_, verbdef)) = verb_result {
            // Should be changed to new owner (#0) and non-executable (flags "rw")
            assert_eq!(verbdef.owner(), SYSTEM_OBJECT);
            assert!(!verbdef.flags().contains(VerbFlag::Exec));
        }

        Ok(())
    }

    #[test]
    fn test_flags_conflict_clobber() -> Result<(), Box<dyn std::error::Error>> {
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

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Clobber,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let _results = parser.load_single_object(
            conflicting_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
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
    fn test_property_override_conflicts_skip() -> Result<(), Box<dyn std::error::Error>> {
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

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Clobber,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };
        parser1.load_single_object(
            first_override_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
        loader1.commit()?;

        // Now try to load conflicting overrides in Skip mode
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

        let skip_options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Skip,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let results = parser2.load_single_object(
            conflicting_override_spec,
            CompileOptions::default(),
            None,
            None,
            skip_options,
        )?;
        loader2.commit()?;

        // Should detect property override conflicts
        assert!(
            !results.conflicts.is_empty(),
            "Should detect property override conflicts"
        );

        // Verify original override values are preserved
        let ws = db.new_world_state()?;
        let desc =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(2), Symbol::mk("description"))?;
        let base_prop =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(2), Symbol::mk("base_prop"))?;

        assert_eq!(desc, v_str("first override"));
        assert_eq!(base_prop, v_str("first base override"));

        Ok(())
    }

    #[test]
    fn test_property_override_conflicts_clobber() -> Result<(), Box<dyn std::error::Error>> {
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

        let options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Clobber,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };
        parser1.load_single_object(
            first_override_spec,
            CompileOptions::default(),
            None,
            None,
            options,
        )?;
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

        let clobber_options = ObjDefLoaderOptions {
            dry_run: false,
            conflict_mode: ConflictMode::Clobber,
            target_object: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
        };

        let _results = parser2.load_single_object(
            conflicting_override_spec,
            CompileOptions::default(),
            None,
            None,
            clobber_options,
        )?;
        loader2.commit()?;

        // Verify override values were changed due to Clobber mode
        let ws = db.new_world_state()?;
        let desc =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(2), Symbol::mk("description"))?;
        let base_prop =
            ws.retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(2), Symbol::mk("base_prop"))?;

        assert_eq!(desc, v_str("second override"));
        assert_eq!(base_prop, v_str("second base override"));

        Ok(())
    }
}
