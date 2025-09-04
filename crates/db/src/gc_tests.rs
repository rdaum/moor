//! Tests for garbage collection operations on anonymous objects

#[cfg(test)]
mod tests {
    use crate::{AnonymousObjectMetadata, Database, DatabaseConfig, TxDB};
    use moor_common::model::{CommitResult, ObjAttrs, ObjectKind, WorldStateSource};
    use moor_common::util::BitEnum;
    use moor_var::Obj;
    use moor_var::Symbol;
    use moor_var::{NOTHING, v_int, v_list, v_map, v_obj, v_str};

    const WIZARD: Obj = Obj::mk_id(2);

    fn test_db() -> TxDB {
        let db = TxDB::open(None, DatabaseConfig::default()).0;
        let mut loader = db.loader_client().unwrap();
        let wizard = Obj::mk_id(2);
        loader
            .create_object(
                Some(wizard),
                &ObjAttrs::new(WIZARD, NOTHING, NOTHING, BitEnum::all(), "Wizard"),
            )
            .unwrap();
        loader.commit().unwrap();
        db
    }

    #[test]
    fn test_anonymous_object_metadata_storage() {
        let db = test_db();

        // Create some anonymous objects
        let mut tx = db.new_world_state().unwrap();
        let anon1 = tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon2 = tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // NOW get the GC interface and do GC operations
        let mut gc = db.gc_interface().unwrap();

        // Test storing metadata
        let metadata1 = AnonymousObjectMetadata::new(0).unwrap(); // young generation
        let metadata2 = AnonymousObjectMetadata::new(1).unwrap(); // old generation

        gc.store_anonymous_object_metadata(&anon1, metadata1.clone())
            .unwrap();
        gc.store_anonymous_object_metadata(&anon2, metadata2.clone())
            .unwrap();

        // Test retrieving metadata
        let retrieved1 = gc.get_anonymous_object_metadata(&anon1).unwrap().unwrap();
        let retrieved2 = gc.get_anonymous_object_metadata(&anon2).unwrap().unwrap();

        assert_eq!(retrieved1.generation(), 0);
        assert_eq!(retrieved2.generation(), 1);
        assert!(retrieved1.is_young_generation());
        assert!(retrieved2.is_old_generation());

        // Test retrieving non-existent metadata
        let mut tx2 = db.new_world_state().unwrap();
        let anon3 = tx2
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(tx2.commit(), Ok(CommitResult::Success));
        let retrieved3 = gc.get_anonymous_object_metadata(&anon3).unwrap();
        assert!(retrieved3.is_none());
    }

    #[test]
    fn test_simple_gc_generation_operations() {
        let db = test_db();

        // Create anonymous objects in different generations
        let mut tx = db.new_world_state().unwrap();
        let anon_young = tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon_old = tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // NOW get the GC interface and do GC operations
        let mut gc = db.gc_interface().unwrap();

        // anon_young already has generation 0 automatically assigned, so only update anon_old
        gc.store_anonymous_object_metadata(&anon_old, AnonymousObjectMetadata::new(1).unwrap())
            .unwrap();

        // Commit the changes
        let commit_result = gc.commit().unwrap();
        assert_eq!(commit_result, CommitResult::Success);

        // Get a new GC interface for queries after commit
        let mut gc = db.gc_interface().unwrap();

        // First verify metadata retrieval works
        let meta_young = gc.get_anonymous_object_metadata(&anon_young).unwrap();
        let meta_old = gc.get_anonymous_object_metadata(&anon_old).unwrap();

        assert!(
            meta_young.is_some(),
            "Should have metadata for young object"
        );
        assert!(meta_old.is_some(), "Should have metadata for old object");

        if let Some(meta) = meta_young {
            assert_eq!(
                meta.generation(),
                0,
                "Young object should have generation 0"
            );
        }

        if let Some(meta) = meta_old {
            assert_eq!(meta.generation(), 1, "Old object should have generation 1");
        }

        // Test getting objects by generation
        let young_objects = gc.get_anonymous_objects_by_generation(0).unwrap();
        assert_eq!(young_objects.len(), 1);
        assert!(young_objects.contains(&anon_young));

        let old_objects = gc.get_anonymous_objects_by_generation(1).unwrap();
        println!("Found {} old objects", old_objects.len());
        assert_eq!(old_objects.len(), 1);
        assert!(old_objects.contains(&anon_old));

        // Test promotion
        let promoted = gc
            .promote_anonymous_objects(&[anon_young, anon_old])
            .unwrap();
        assert_eq!(promoted, 1); // Only anon_young should be promoted

        // Verify promotion
        let meta_young = gc
            .get_anonymous_object_metadata(&anon_young)
            .unwrap()
            .unwrap();
        let meta_old = gc
            .get_anonymous_object_metadata(&anon_old)
            .unwrap()
            .unwrap();

        assert!(meta_young.is_old_generation());
        assert!(meta_old.is_old_generation()); // Was already old
    }

    #[test]
    fn test_anonymous_objects_in_object_relationships() {
        let db = test_db();

        // Create anonymous objects
        let mut create_tx = db.new_world_state().unwrap();
        let anon_parent = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon_location = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon_child = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(create_tx.commit(), Ok(CommitResult::Success));

        // Store metadata
        {
            let mut gc = db.gc_interface().unwrap();
            gc.store_anonymous_object_metadata(
                &anon_parent,
                AnonymousObjectMetadata::new(0).unwrap(),
            )
            .unwrap();
            gc.store_anonymous_object_metadata(
                &anon_location,
                AnonymousObjectMetadata::new(0).unwrap(),
            )
            .unwrap();
            gc.store_anonymous_object_metadata(
                &anon_child,
                AnonymousObjectMetadata::new(1).unwrap(),
            )
            .unwrap();
        }

        let mut tx = db.new_world_state().unwrap();

        // Create regular object with anonymous parent and location
        let obj = tx
            .create_object(
                &WIZARD,
                &anon_parent,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();

        tx.move_object(&WIZARD, &obj, &anon_location).unwrap();

        // Create another object with anonymous child (anon_child located in obj)
        tx.move_object(&WIZARD, &anon_child, &obj).unwrap();

        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // Test reference scanning - this should find anonymous objects in object relationships
        let references = {
            let mut gc = db.gc_interface().unwrap();
            gc.scan_anonymous_object_references().unwrap()
        };

        // Should find references from our regular object (obj has anon_parent as parent, anon_location as location)
        let obj_refs = references
            .iter()
            .find(|(ref_obj, _)| *ref_obj == obj)
            .expect("Should find references from our test object");

        // Should find both anon_parent and anon_location referenced from obj
        assert!(
            obj_refs.1.contains(&anon_parent),
            "Should find anonymous parent reference"
        );
        assert!(
            obj_refs.1.contains(&anon_location),
            "Should find anonymous location reference"
        );

        // We won't find anon_child referenced FROM obj, because anon_child is located IN obj, not referenced by obj
        // But we might find it if there are reverse indices, but that's not part of this test
    }

    #[test]
    fn test_nested_anonymous_object_references() {
        let db = test_db();

        // Create anonymous objects first
        let mut create_tx = db.new_world_state().unwrap();
        let anon1 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon2 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon3 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(create_tx.commit(), Ok(CommitResult::Success));

        // Create regular object
        let mut tx = db.new_world_state().unwrap();
        let obj = tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();

        // Store metadata
        {
            let mut gc = db.gc_interface().unwrap();
            gc.store_anonymous_object_metadata(&anon1, AnonymousObjectMetadata::new(0).unwrap())
                .unwrap();
            gc.store_anonymous_object_metadata(&anon2, AnonymousObjectMetadata::new(0).unwrap())
                .unwrap();
            gc.store_anonymous_object_metadata(&anon3, AnonymousObjectMetadata::new(1).unwrap())
                .unwrap();
        }

        // Create nested structure: list containing maps containing anonymous objects
        let inner_map1_pairs = vec![(v_str("anon"), v_obj(anon1)), (v_obj(anon2), v_int(123))];

        let inner_map2_pairs = vec![(v_str("deep_anon"), v_obj(anon3))];

        let nested_list = v_list(&[
            v_map(&inner_map1_pairs),
            v_map(&inner_map2_pairs),
            v_list(&[v_obj(anon1), v_str("not_anon")]),
        ]);

        tx.define_property(
            &WIZARD,
            &obj,
            &obj,
            Symbol::mk("deeply_nested"),
            &NOTHING,
            BitEnum::new(),
            Some(nested_list),
        )
        .unwrap();

        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // Test reference scanning finds all nested references
        let references = {
            let mut gc = db.gc_interface().unwrap();
            gc.scan_anonymous_object_references().unwrap()
        };

        let obj_refs = references
            .iter()
            .find(|(ref_obj, _)| *ref_obj == obj)
            .expect("Should find references from our test object");

        // Should find all anonymous objects, including duplicates
        assert!(obj_refs.1.contains(&anon1));
        assert!(obj_refs.1.contains(&anon2));
        assert!(obj_refs.1.contains(&anon3));

        // anon1 appears twice, so total references should be 4
        assert_eq!(obj_refs.1.len(), 4);
    }

    #[test]
    fn test_gc_collection_operations() {
        let db = test_db();

        // Create anonymous objects first
        let mut create_tx = db.new_world_state().unwrap();
        let anon1 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon2 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon3 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon4 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(create_tx.commit(), Ok(CommitResult::Success));

        // Create a regular object that references some of them
        let mut tx = db.new_world_state().unwrap();
        let obj = tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();

        // Only reference anon1 and anon3, leaving anon2 and anon4 unreachable
        tx.define_property(
            &WIZARD,
            &obj,
            &obj,
            Symbol::mk("ref1"),
            &WIZARD,
            BitEnum::new(),
            Some(v_obj(anon1)),
        )
        .unwrap();

        tx.define_property(
            &WIZARD,
            &obj,
            &obj,
            Symbol::mk("ref2"),
            &WIZARD,
            BitEnum::new(),
            Some(v_obj(anon3)),
        )
        .unwrap();

        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // Store metadata and test collection with single GC interface instance
        {
            let mut gc = db.gc_interface().unwrap();
            gc.store_anonymous_object_metadata(&anon1, AnonymousObjectMetadata::new(0).unwrap())
                .unwrap();
            gc.store_anonymous_object_metadata(&anon2, AnonymousObjectMetadata::new(0).unwrap())
                .unwrap();
            gc.store_anonymous_object_metadata(&anon3, AnonymousObjectMetadata::new(1).unwrap())
                .unwrap();
            gc.store_anonymous_object_metadata(&anon4, AnonymousObjectMetadata::new(1).unwrap())
                .unwrap();

            let unreachable = vec![anon2, anon4];
            let collected_count = gc
                .collect_unreachable_anonymous_objects(&unreachable)
                .unwrap();
            assert_eq!(collected_count, 2);

            // Verify metadata for collected objects is removed
            assert!(gc.get_anonymous_object_metadata(&anon2).unwrap().is_none());
            assert!(gc.get_anonymous_object_metadata(&anon4).unwrap().is_none());

            // Verify metadata for reachable objects still exists
            assert!(gc.get_anonymous_object_metadata(&anon1).unwrap().is_some());
            assert!(gc.get_anonymous_object_metadata(&anon3).unwrap().is_some());
        }
    }

    #[test]
    fn test_gc_interface_operations() {
        let db = test_db();

        // Create anonymous objects first
        let mut create_tx = db.new_world_state().unwrap();
        let anon1 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon2 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(create_tx.commit(), Ok(CommitResult::Success));

        // Create regular object and references
        let mut tx = db.new_world_state().unwrap();
        let obj = tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();

        tx.define_property(
            &WIZARD,
            &obj,
            &obj,
            Symbol::mk("anon_ref"),
            &WIZARD,
            BitEnum::new(),
            Some(v_list(&[v_obj(anon1), v_obj(anon2)])),
        )
        .unwrap();

        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // Store metadata and test all GC operations with single interface instance
        {
            let mut gc = db.gc_interface().unwrap();
            gc.store_anonymous_object_metadata(&anon1, AnonymousObjectMetadata::new(0).unwrap())
                .unwrap();
            gc.store_anonymous_object_metadata(&anon2, AnonymousObjectMetadata::new(1).unwrap())
                .unwrap();

            // Test metadata retrieval through GC interface
            let meta1 = gc.get_anonymous_object_metadata(&anon1).unwrap().unwrap();
            let meta2 = gc.get_anonymous_object_metadata(&anon2).unwrap().unwrap();
            assert_eq!(meta1.generation(), 0);
            assert_eq!(meta2.generation(), 1);

            // Test reference scanning through GC interface
            let references = gc.scan_anonymous_object_references().unwrap();
            let obj_refs = references
                .iter()
                .find(|(ref_obj, _)| *ref_obj == obj)
                .expect("Should find references from our test object");

            assert!(obj_refs.1.contains(&anon1));
            assert!(obj_refs.1.contains(&anon2));
            assert_eq!(obj_refs.1.len(), 2);
        }
    }

    #[test]
    fn test_gc_metadata_generation_tracking() {
        let db = test_db();

        // Create anonymous object first
        let mut create_tx = db.new_world_state().unwrap();
        let anon = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(create_tx.commit(), Ok(CommitResult::Success));

        // Start with young generation
        let mut metadata = AnonymousObjectMetadata::new(0).unwrap();
        assert!(metadata.is_young_generation());
        assert!(!metadata.is_old_generation());

        // Store young generation metadata and commit
        {
            let mut gc = db.gc_interface().unwrap();
            gc.store_anonymous_object_metadata(&anon, metadata.clone())
                .unwrap();
        }

        // Promote to old generation and verify with new transaction
        metadata.set_generation(1);
        assert!(!metadata.is_young_generation());
        assert!(metadata.is_old_generation());

        {
            let mut gc = db.gc_interface().unwrap();
            gc.store_anonymous_object_metadata(&anon, metadata).unwrap();

            // Verify promotion worked in same transaction
            let retrieved = gc.get_anonymous_object_metadata(&anon).unwrap().unwrap();
            assert_eq!(retrieved.generation(), 1);
            assert!(retrieved.is_old_generation());
        }
    }

    #[test]
    fn test_empty_reference_scan() {
        let db = test_db();

        // Test scanning when there are no anonymous object references
        let references = {
            let mut gc = db.gc_interface().unwrap();
            gc.scan_anonymous_object_references().unwrap()
        };
        assert!(references.is_empty());

        // Create objects with no anonymous references
        let mut tx = db.new_world_state().unwrap();
        let obj1 = tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();
        let obj2 = tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();

        tx.define_property(
            &WIZARD,
            &obj1,
            &obj1,
            Symbol::mk("regular_ref"),
            &WIZARD,
            BitEnum::new(),
            Some(v_obj(obj2)),
        )
        .unwrap();

        tx.define_property(
            &WIZARD,
            &obj1,
            &obj1,
            Symbol::mk("simple_data"),
            &WIZARD,
            BitEnum::new(),
            Some(v_list(&[v_int(1), v_str("test"), v_obj(obj2)])),
        )
        .unwrap();

        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // Should still be empty since no anonymous objects are referenced
        let references = {
            let mut gc = db.gc_interface().unwrap();
            gc.scan_anonymous_object_references().unwrap()
        };
        assert!(references.is_empty());
    }

    #[test]
    fn test_generation_specific_reference_scanning() {
        let db = test_db();

        // Create anonymous objects first
        let mut create_tx = db.new_world_state().unwrap();
        let anon_young1 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon_young2 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon_old1 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon_old2 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(create_tx.commit(), Ok(CommitResult::Success));

        // Create a regular object that references both generations
        {
            let mut tx = db.new_world_state().unwrap();
            let obj = tx
                .create_object(
                    &WIZARD,
                    &NOTHING,
                    &WIZARD,
                    BitEnum::new(),
                    ObjectKind::NextObjid,
                )
                .unwrap();

            tx.define_property(
                &WIZARD,
                &obj,
                &obj,
                Symbol::mk("young_refs"),
                &WIZARD,
                BitEnum::new(),
                Some(v_list(&[v_obj(anon_young1), v_obj(anon_young2)])),
            )
            .unwrap();

            tx.define_property(
                &WIZARD,
                &obj,
                &obj,
                Symbol::mk("old_refs"),
                &WIZARD,
                BitEnum::new(),
                Some(v_list(&[v_obj(anon_old1), v_obj(anon_old2)])),
            )
            .unwrap();

            tx.commit().unwrap();
        }

        // Store metadata and test generation-specific scanning with single GC interface instance
        {
            let mut gc = db.gc_interface().unwrap();
            gc.store_anonymous_object_metadata(
                &anon_young1,
                AnonymousObjectMetadata::new(0).unwrap(),
            )
            .unwrap();
            gc.store_anonymous_object_metadata(
                &anon_young2,
                AnonymousObjectMetadata::new(0).unwrap(),
            )
            .unwrap();
            gc.store_anonymous_object_metadata(
                &anon_old1,
                AnonymousObjectMetadata::new(1).unwrap(),
            )
            .unwrap();
            gc.store_anonymous_object_metadata(
                &anon_old2,
                AnonymousObjectMetadata::new(1).unwrap(),
            )
            .unwrap();

            // Scan only young generation references
            let young_refs = gc.scan_anonymous_object_references_generation(0).unwrap();
            assert_eq!(young_refs.len(), 1); // One object with young refs
            let (_obj, refs) = &young_refs[0];
            assert_eq!(refs.len(), 2);
            assert!(refs.contains(&anon_young1));
            assert!(refs.contains(&anon_young2));

            // Scan only old generation references
            let old_refs = gc.scan_anonymous_object_references_generation(1).unwrap();
            assert_eq!(old_refs.len(), 1); // One object with old refs
            let (_obj, refs) = &old_refs[0];
            assert_eq!(refs.len(), 2);
            assert!(refs.contains(&anon_old1));
            assert!(refs.contains(&anon_old2));
        }
    }

    #[test]
    fn test_get_anonymous_objects_by_generation() {
        let db = test_db();

        // Create anonymous objects first
        let mut create_tx = db.new_world_state().unwrap();
        let anon_young1 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon_young2 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon_old1 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(create_tx.commit(), Ok(CommitResult::Success));

        // Store metadata and test retrieval with single GC interface instance
        {
            let mut gc = db.gc_interface().unwrap();
            // anon_young1 and anon_young2 already have generation 0 automatically assigned
            // Only update anon_old1 to generation 1
            gc.store_anonymous_object_metadata(
                &anon_old1,
                AnonymousObjectMetadata::new(1).unwrap(),
            )
            .unwrap();

            // Commit the changes
            let commit_result = gc.commit().unwrap();
            assert_eq!(commit_result, CommitResult::Success);

            // Get a new GC interface for queries after commit
            let gc = db.gc_interface().unwrap();

            let young_objects = gc.get_anonymous_objects_by_generation(0).unwrap();
            assert_eq!(young_objects.len(), 2);
            assert!(young_objects.contains(&anon_young1));
            assert!(young_objects.contains(&anon_young2));

            let old_objects = gc.get_anonymous_objects_by_generation(1).unwrap();
            assert_eq!(old_objects.len(), 1);
            assert!(old_objects.contains(&anon_old1));
        }
    }

    #[test]
    fn test_promote_anonymous_objects() {
        let db = test_db();

        // Create anonymous objects first
        let mut create_tx = db.new_world_state().unwrap();
        let anon1 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon2 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        let anon3 = create_tx
            .create_object(
                &WIZARD,
                &NOTHING,
                &WIZARD,
                BitEnum::new(),
                ObjectKind::Anonymous,
            )
            .unwrap();
        assert_eq!(create_tx.commit(), Ok(CommitResult::Success));

        // Store metadata, test promotion, and verify results with single GC interface instance
        {
            let mut gc = db.gc_interface().unwrap();
            gc.store_anonymous_object_metadata(&anon1, AnonymousObjectMetadata::new(0).unwrap())
                .unwrap();
            gc.store_anonymous_object_metadata(&anon2, AnonymousObjectMetadata::new(0).unwrap())
                .unwrap();
            gc.store_anonymous_object_metadata(&anon3, AnonymousObjectMetadata::new(1).unwrap()) // Already old
                .unwrap();

            let promoted = gc
                .promote_anonymous_objects(&[anon1, anon2, anon3])
                .unwrap();
            assert_eq!(promoted, 2); // Only anon1 and anon2 should be promoted

            // Verify promotions
            let meta1 = gc.get_anonymous_object_metadata(&anon1).unwrap().unwrap();
            let meta2 = gc.get_anonymous_object_metadata(&anon2).unwrap().unwrap();
            let meta3 = gc.get_anonymous_object_metadata(&anon3).unwrap().unwrap();

            assert!(meta1.is_old_generation());
            assert!(meta2.is_old_generation());
            assert!(meta3.is_old_generation()); // Was already old
        }
    }
}
