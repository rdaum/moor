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

//! Tests for garbage collection operations on anonymous objects

#[cfg(test)]
mod tests {
    use crate::{Database, DatabaseConfig, TxDB};
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
    fn test_anonymous_object_creation() {
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
        assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

        // Test that we can get anonymous objects using GC interface
        let gc = db.gc_interface().unwrap();
        let anonymous_objects = gc.get_anonymous_objects().unwrap();

        assert!(anonymous_objects.contains(&anon1));
        assert!(anonymous_objects.contains(&anon2));
        assert_eq!(anonymous_objects.len(), 2);

        // Test creating more anonymous objects
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
        assert!(matches!(tx2.commit(), Ok(CommitResult::Success { .. })));

        // Verify the new object appears in anonymous objects list
        // Need a new GC interface to see the changes from the second transaction
        let gc2 = db.gc_interface().unwrap();
        let updated_objects = gc2.get_anonymous_objects().unwrap();
        assert!(updated_objects.contains(&anon3));
        assert_eq!(updated_objects.len(), 3);
    }

    #[test]
    fn test_simple_gc_reference_scanning() {
        let db = test_db();

        // Create anonymous objects
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
        assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

        // Test reference scanning
        let mut gc = db.gc_interface().unwrap();

        // Verify both objects are detected as anonymous
        let anonymous_objects = gc.get_anonymous_objects().unwrap();
        assert_eq!(anonymous_objects.len(), 2);
        assert!(anonymous_objects.contains(&anon1));
        assert!(anonymous_objects.contains(&anon2));

        // Test reference scanning (should be empty since no references exist yet)
        let references = gc.scan_anonymous_object_references().unwrap();
        // The scan returns (referrer, referenced objects) pairs
        // Since we haven't created any references, this should be empty or only contain empty sets
        for (_referrer, referenced) in &references {
            assert!(
                referenced.is_empty()
                    || !referenced.iter().any(|obj| anonymous_objects.contains(obj))
            );
        }
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
        assert!(matches!(
            create_tx.commit(),
            Ok(CommitResult::Success { .. })
        ));

        // No metadata storage needed for mark & sweep

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

        assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

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
        assert!(matches!(
            create_tx.commit(),
            Ok(CommitResult::Success { .. })
        ));

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

        // No metadata storage needed for mark & sweep

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

        assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

        // Test reference scanning finds all nested references
        let references = {
            let mut gc = db.gc_interface().unwrap();
            gc.scan_anonymous_object_references().unwrap()
        };

        let obj_refs = references
            .iter()
            .find(|(ref_obj, _)| *ref_obj == obj)
            .expect("Should find references from our test object");

        // Should find all anonymous objects (HashSet automatically deduplicates)
        assert!(obj_refs.1.contains(&anon1));
        assert!(obj_refs.1.contains(&anon2));
        assert!(obj_refs.1.contains(&anon3));

        // anon1 appears twice, but HashSet deduplicates, so total unique references should be 3
        assert_eq!(obj_refs.1.len(), 3);
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
        assert!(matches!(
            create_tx.commit(),
            Ok(CommitResult::Success { .. })
        ));

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

        assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

        // Test collection with single GC interface instance
        {
            let mut gc = db.gc_interface().unwrap();

            // Verify all objects exist before collection
            let all_objects = gc.get_anonymous_objects().unwrap();
            assert!(all_objects.contains(&anon1));
            assert!(all_objects.contains(&anon2));
            assert!(all_objects.contains(&anon3));
            assert!(all_objects.contains(&anon4));
            assert_eq!(all_objects.len(), 4);

            // Simulate collection of unreachable objects (anon2 and anon4)
            let unreachable = std::collections::HashSet::from([anon2, anon4]);
            let collected_count = gc
                .collect_unreachable_anonymous_objects(&unreachable)
                .unwrap();
            assert_eq!(collected_count, 2);
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
        assert!(matches!(
            create_tx.commit(),
            Ok(CommitResult::Success { .. })
        ));

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

        assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

        // Test GC operations with single interface instance
        {
            let mut gc = db.gc_interface().unwrap();

            // Test getting anonymous objects
            let anonymous_objects = gc.get_anonymous_objects().unwrap();
            assert!(anonymous_objects.contains(&anon1));
            assert!(anonymous_objects.contains(&anon2));
            assert_eq!(anonymous_objects.len(), 2);

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

        assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

        // Should still be empty since no anonymous objects are referenced
        let references = {
            let mut gc = db.gc_interface().unwrap();
            gc.scan_anonymous_object_references().unwrap()
        };
        assert!(references.is_empty());
    }
}
