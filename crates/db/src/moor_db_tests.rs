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

#[cfg(test)]
mod tests {
    use crate::DatabaseConfig;

    use crate::moor_db::MoorDB;
    use moor_common::model::{CommitResult, WorldStateError};
    use moor_common::model::{HasUuid, Named};
    use moor_common::model::{ObjAttrs, PropFlag, ValSet};
    use moor_common::model::{ObjFlag, VerbAttrs};
    use moor_common::model::{ObjSet, ObjectRef};
    use moor_common::model::{VerbArgsSpec, VerbFlag};
    use moor_common::util::BitEnum;
    use moor_var::Obj;
    use moor_var::Symbol;
    use moor_var::program::ProgramType;
    use moor_var::program::program::Program;
    use moor_var::{NOTHING, SYSTEM_OBJECT};
    use moor_var::{v_int, v_str};
    use std::sync::Arc;

    fn test_db() -> Arc<MoorDB> {
        MoorDB::open(None, DatabaseConfig::default()).0
    }

    #[test]
    fn test_create_object() {
        let db = test_db();
        let mut tx = db.start_transaction();
        let oid = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
        assert_eq!(oid, Obj::mk_id(0));
        assert!(tx.object_valid(&oid).unwrap());
        assert_eq!(tx.get_object_owner(&oid).unwrap(), oid);
        assert_eq!(tx.get_object_parent(&oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_location(&oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_name(&oid).unwrap(), "test");
        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // Verify existence in a new transaction.
        let tx = db.start_transaction();
        assert!(tx.object_valid(&oid).unwrap());
        assert_eq!(tx.get_object_owner(&oid).unwrap(), oid);

        assert_eq!(tx.get_objects().unwrap(), ObjSet::from_items(&[oid]));
    }

    #[test]
    pub fn test_create_object_fixed_id() {
        let db = test_db();
        let mut tx = db.start_transaction();

        // Force at 1.
        let oid = tx
            .create_object(Some(Obj::mk_id(1)), ObjAttrs::default())
            .unwrap();
        assert_eq!(oid, Obj::mk_id(1));
        // Now verify the next will be 2.
        let oid2 = tx.create_object(None, ObjAttrs::default()).unwrap();
        assert_eq!(oid2, Obj::mk_id(2));
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    pub fn test_parent_children() {
        let db = test_db();
        let mut tx = db.start_transaction();

        // Single parent/child relationship.
        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();

        assert_eq!(tx.get_object_parent(&b).unwrap(), a);
        assert!(
            tx.get_object_children(&a)
                .unwrap()
                .is_same(ObjSet::from_items(&[b]))
        );

        assert_eq!(tx.get_object_parent(&a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(&b).unwrap(), ObjSet::empty());

        // Add a second child
        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();

        assert_eq!(tx.get_object_parent(&c).unwrap(), a);
        assert!(
            tx.get_object_children(&a)
                .unwrap()
                .is_same(ObjSet::from_items(&[b, c]))
        );

        assert_eq!(tx.get_object_parent(&a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(&b).unwrap(), ObjSet::empty());

        // Create new obj and reparent one child
        let d = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test4"),
            )
            .unwrap();

        tx.set_object_parent(&b, &d).unwrap();
        assert_eq!(tx.get_object_parent(&b).unwrap(), d);
        assert!(
            tx.get_object_children(&a)
                .unwrap()
                .is_same(ObjSet::from_items(&[c]))
        );
        assert!(
            tx.get_object_children(&d)
                .unwrap()
                .is_same(ObjSet::from_items(&[b]))
        );

        let objects = tx.get_objects().unwrap();
        assert!(objects.is_same(ObjSet::from_items(&[a, b, c, d])));

        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    pub fn test_descendants() {
        let db = test_db();
        let mut tx = db.start_transaction();

        let a = tx
            .create_object(
                Some(Obj::mk_id(0)),
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
        assert_eq!(a, Obj::mk_id(0));

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();
        assert_eq!(b, Obj::mk_id(1));

        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();
        assert_eq!(c, Obj::mk_id(2));

        let d = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, c, NOTHING, BitEnum::new(), "test4"),
            )
            .unwrap();
        assert_eq!(d, Obj::mk_id(3));

        let desc = tx
            .descendants(&a, false)
            .expect("Could not retrieve descendants");
        assert!(
            desc.is_same(ObjSet::from_items(&[b, c, d])),
            "Descendants doesn't match expected is {desc:?}"
        );

        assert_eq!(tx.descendants(&b, false).unwrap(), ObjSet::empty());
        assert_eq!(tx.descendants(&c, false).unwrap(), ObjSet::from_items(&[d]));

        // Now reparent d to b
        tx.set_object_parent(&d, &b).unwrap();
        assert!(
            tx.get_object_children(&a)
                .unwrap()
                .is_same(ObjSet::from_items(&[b, c]))
        );
        assert_eq!(
            tx.get_object_children(&b).unwrap(),
            ObjSet::from_items(&[d])
        );
        assert_eq!(tx.get_object_children(&c).unwrap(), ObjSet::empty());
        assert!(
            tx.descendants(&a, false)
                .unwrap()
                .is_same(ObjSet::from_items(&[b, c, d]))
        );
        assert_eq!(tx.descendants(&b, false).unwrap(), ObjSet::from_items(&[d]));
        assert_eq!(tx.descendants(&c, false).unwrap(), ObjSet::empty());
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    pub fn test_location_contents() {
        let db = test_db();
        let mut tx = db.start_transaction();

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, a, BitEnum::new(), "test2"),
            )
            .unwrap();

        assert_eq!(tx.get_object_location(&b).unwrap(), a);
        assert_eq!(
            tx.get_object_contents(&a).unwrap(),
            ObjSet::from_items(&[b])
        );

        assert_eq!(tx.get_object_location(&a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_contents(&b).unwrap(), ObjSet::empty());

        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();

        tx.set_object_location(&b, &c).unwrap();
        assert_eq!(tx.get_object_location(&b).unwrap(), c);
        assert_eq!(tx.get_object_contents(&a).unwrap(), ObjSet::empty());
        assert_eq!(
            tx.get_object_contents(&c).unwrap(),
            ObjSet::from_items(&[b])
        );

        let d = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test4"),
            )
            .unwrap();
        tx.set_object_location(&d, &c).unwrap();
        assert!(
            tx.get_object_contents(&c)
                .unwrap()
                .is_same(ObjSet::from_items(&[b, d]))
        );
        assert_eq!(tx.get_object_location(&d).unwrap(), c);

        tx.set_object_location(&a, &c).unwrap();
        assert!(
            tx.get_object_contents(&c)
                .unwrap()
                .is_same(ObjSet::from_items(&[b, d, a]))
        );
        assert_eq!(tx.get_object_location(&a).unwrap(), c);

        // Validate recursive move detection.
        match tx.set_object_location(&c, &b).err() {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // Move b one level deeper, and then check recursive move detection again.
        tx.set_object_location(&b, &d).unwrap();
        match tx.set_object_location(&c, &b).err() {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // The other way around, d to c should be fine.
        tx.set_object_location(&d, &c).unwrap();
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    /// Test data integrity of object moves between commits.
    #[test]
    pub fn test_object_move_commits() {
        let db = test_db();
        let mut tx = db.start_transaction();

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, a, BitEnum::new(), "test2"),
            )
            .unwrap();

        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();

        tx.set_object_location(&b, &a).unwrap();
        tx.set_object_location(&c, &a).unwrap();
        assert_eq!(tx.get_object_location(&b).unwrap(), a);
        assert_eq!(tx.get_object_location(&c).unwrap(), a);
        assert!(
            tx.get_object_contents(&a)
                .unwrap()
                .is_same(ObjSet::from_items(&[b, c]))
        );
        assert_eq!(tx.get_object_contents(&b).unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(&c).unwrap(), ObjSet::empty());

        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        let mut tx = db.start_transaction();
        assert_eq!(tx.get_object_location(&b).unwrap(), a);
        assert_eq!(tx.get_object_location(&c).unwrap(), a);
        let contents = tx.get_object_contents(&a).expect("Unable to get contents");
        assert!(
            contents.is_same(ObjSet::from_items(&[b, c])),
            "Contents of a are not as expected: {:?} vs {:?}",
            contents,
            ObjSet::from_items(&[b, c])
        );
        assert_eq!(tx.get_object_contents(&b).unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(&c).unwrap(), ObjSet::empty());

        tx.set_object_location(&b, &c).unwrap();
        assert_eq!(tx.get_object_location(&b).unwrap(), c);
        assert_eq!(tx.get_object_location(&c).unwrap(), a);
        assert_eq!(
            tx.get_object_contents(&a).unwrap(),
            ObjSet::from_items(&[c])
        );
        assert_eq!(tx.get_object_contents(&b).unwrap(), ObjSet::empty());
        assert_eq!(
            tx.get_object_contents(&c).unwrap(),
            ObjSet::from_items(&[b])
        );
        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        let tx = db.start_transaction();
        assert_eq!(tx.get_object_location(&c).unwrap(), a);
        assert_eq!(tx.get_object_location(&b).unwrap(), c);
        assert_eq!(
            tx.get_object_contents(&a).unwrap(),
            ObjSet::from_items(&[c])
        );
        assert_eq!(tx.get_object_contents(&b).unwrap(), ObjSet::empty());
        assert_eq!(
            tx.get_object_contents(&c).unwrap(),
            ObjSet::from_items(&[b])
        );
    }

    #[test]
    pub fn test_simple_property() {
        let db = test_db();
        let mut tx = db.start_transaction();

        let oid = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        tx.define_property(
            &oid,
            &oid,
            Symbol::mk("test"),
            &NOTHING,
            BitEnum::new(),
            Some(v_str("test")),
        )
        .unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(&oid, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_str("test"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(!is_clear);
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    /// Regression test for updating-verbs failing.
    #[test]
    pub fn test_verb_add_update() {
        let db = test_db();
        let mut tx = db.start_transaction();

        let oid = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
        tx.add_object_verb(
            &oid,
            &oid,
            &[Symbol::mk("test")],
            ProgramType::MooR(Program::new()),
            BitEnum::new_with(VerbFlag::Exec),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();
        // resolve the verb to its vh.
        let vh = tx
            .resolve_verb(
                &oid,
                Symbol::mk("test"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec)),
            )
            .unwrap();
        assert_eq!(vh.names(), vec!["test".into()]);
        // Verify it's actually on the object when we get verbs.
        let verbs = tx.get_verbs(&oid).unwrap();
        assert_eq!(verbs.len(), 1);
        assert!(verbs.contains(vh.uuid()));
        // update the verb using its uuid, renaming it.
        tx.update_verb(
            &oid,
            vh.uuid(),
            VerbAttrs {
                definer: None,
                owner: None,
                names: Some(vec![Symbol::mk("test2")]),
                flags: None,
                args_spec: None,
                program: None,
            },
        )
        .unwrap();
        // resolve with the new name.
        let vh = tx
            .resolve_verb(
                &oid,
                Symbol::mk("test2"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec)),
            )
            .unwrap();
        assert_eq!(vh.names(), vec!["test2".into()]);

        // Now commit, and try to resolve again.
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
        let tx = db.start_transaction();
        let vh = tx
            .resolve_verb(
                &oid,
                Symbol::mk("test2"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec)),
            )
            .unwrap();
        assert_eq!(vh.names(), vec!["test2".into()]);
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    pub fn test_transitive_property_resolution() {
        let db = test_db();
        let mut tx = db.start_transaction();

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();

        tx.define_property(
            &a,
            &a,
            Symbol::mk("test"),
            &NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(&b, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // Verify we *don't* get this property for an unrelated, unhinged object by reparenting b
        // to new parent c.  This should remove the defs for a's properties from b.
        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();

        tx.set_object_parent(&b, &c).unwrap();

        let result = tx.resolve_property(&b, Symbol::mk("test"));
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::PropertyNotFound(b, "test".to_string())
        );
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    pub fn test_transitive_property_resolution_clear_property() {
        let db = test_db();
        let mut tx = db.start_transaction();

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();

        tx.define_property(
            &a,
            &a,
            Symbol::mk("test"),
            &NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(&b, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // Set the property on the child to a new value.
        tx.set_property(&b, prop.uuid(), v_int(666)).unwrap();

        // Verify the new value is present.
        let (prop, v, perms, is_clear) = tx.resolve_property(&b, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_int(666));
        assert_eq!(perms.owner(), NOTHING);
        assert!(!is_clear);

        // Now clear, and we should get the old value, but with clear status.
        tx.clear_property(&b, prop.uuid()).unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(&b, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // Changing flags or owner should have nothing to do with the clarity of the property value.
        tx.update_property_info(
            &b,
            prop.uuid(),
            Some(b),
            Some(BitEnum::new_with(PropFlag::Read)),
            None,
        )
        .unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(&b, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), b);
        assert_eq!(perms.flags(), BitEnum::new_with(PropFlag::Read));
        assert!(is_clear);

        // Setting the value again makes it not clear
        tx.set_property(&b, prop.uuid(), v_int(666)).unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(&b, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_int(666));
        assert_eq!(perms.owner(), b);
        assert_eq!(perms.flags(), BitEnum::new_with(PropFlag::Read));
        assert!(!is_clear);

        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    pub fn test_rename_property() {
        let db = test_db();
        let mut tx = db.start_transaction();
        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();

        let uuid = tx
            .define_property(
                &a,
                &a,
                Symbol::mk("test"),
                &NOTHING,
                BitEnum::new(),
                Some(v_str("test_value")),
            )
            .unwrap();

        // I can update the name on the parent...
        tx.update_property_info(&a, uuid, None, None, Some("a_new_name".into()))
            .unwrap();

        // And now resolve that new name on the child.
        let (prop, v, perms, is_clear) = tx.resolve_property(&b, Symbol::mk("a_new_name")).unwrap();
        assert_eq!(prop.name(), "a_new_name".into());
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // But it's illegal to try to rename it on the child who doesn't define it.
        assert!(
            tx.update_property_info(&b, uuid, None, None, Some("a_new_name".into()))
                .is_err()
        )
    }

    /// Test regression where parent properties were present via `properties()` on children.
    #[test]
    pub fn test_regression_properties() {
        let db = test_db();
        let mut tx = db.start_transaction();

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();

        // Define 1 property on parent
        tx.define_property(
            &a,
            &a,
            Symbol::mk("test"),
            &NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(&b, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // And another on child
        let child_prop = tx
            .define_property(
                &b,
                &b,
                Symbol::mk("test2"),
                &NOTHING,
                BitEnum::new(),
                Some(v_str("test_value2")),
            )
            .unwrap();

        let props = tx.get_properties(&b).unwrap();

        // Our prop should be there
        assert!(props.find(&child_prop).is_some());

        // Listing the set of properties on the child should include only the child's properties
        assert_eq!(props.len(), 1);
    }

    #[test]
    pub fn test_verb_resolve() {
        let db = test_db();
        let mut tx = db.start_transaction();

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        tx.add_object_verb(
            &a,
            &a,
            &[Symbol::mk("test")],
            ProgramType::MooR(Program::new()),
            BitEnum::new_with(VerbFlag::Exec),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();

        assert_eq!(
            tx.resolve_verb(
                &a,
                Symbol::mk("test"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            vec!["test".into()]
        );

        assert_eq!(
            tx.resolve_verb(
                &a,
                Symbol::mk("test"),
                Some(VerbArgsSpec::this_none_this()),
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            vec!["test".into()]
        );

        let v_uuid = tx
            .resolve_verb(
                &a,
                Symbol::mk("test"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec)),
            )
            .unwrap()
            .uuid();
        assert_eq!(
            tx.get_verb_program(&a, v_uuid).unwrap(),
            ProgramType::MooR(Program::new())
        );

        // Add a second verb with a different name
        tx.add_object_verb(
            &a,
            &a,
            &[Symbol::mk("test2")],
            ProgramType::MooR(Program::new()),
            BitEnum::new_with(VerbFlag::Exec),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();

        // Verify we can get it
        assert_eq!(
            tx.resolve_verb(
                &a,
                Symbol::mk("test2"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            vec!["test2".into()]
        );
        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // Verify existence in a new transaction.
        let tx = db.start_transaction();
        assert_eq!(
            tx.resolve_verb(
                &a,
                Symbol::mk("test"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            vec!["test".into()]
        );
        assert_eq!(
            tx.resolve_verb(
                &a,
                Symbol::mk("test2"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            vec!["test2".into()]
        );
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    pub fn test_verb_resolve_inherited() {
        let db = test_db();
        let mut tx = db.start_transaction();

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();

        tx.add_object_verb(
            &a,
            &a,
            &[Symbol::mk("test")],
            ProgramType::MooR(Program::new()),
            BitEnum::new_with(VerbFlag::Exec),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();

        assert_eq!(
            tx.resolve_verb(
                &b,
                Symbol::mk("test"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            vec!["test".into()]
        );

        assert_eq!(
            tx.resolve_verb(
                &b,
                Symbol::mk("test"),
                Some(VerbArgsSpec::this_none_this()),
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            vec!["test".into()]
        );

        let v_uuid = tx
            .resolve_verb(
                &b,
                Symbol::mk("test"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec)),
            )
            .unwrap()
            .uuid();
        assert_eq!(
            tx.get_verb_program(&a, v_uuid).unwrap(),
            ProgramType::MooR(Program::new())
        );
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    pub fn test_verb_resolve_wildcard() {
        let db = test_db();
        let mut tx = db.start_transaction();
        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let verb_names = vec![Symbol::mk("dname*c"), "iname*c".into()];
        tx.add_object_verb(
            &a,
            &a,
            &verb_names,
            ProgramType::MooR(Program::new()),
            BitEnum::new_with(VerbFlag::Exec),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();

        assert_eq!(
            tx.resolve_verb(
                &a,
                Symbol::mk("dname"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(
                &a,
                Symbol::mk("dnamec"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(
                &a,
                Symbol::mk("iname"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(
                &a,
                Symbol::mk("inamec"),
                None,
                Some(BitEnum::new_with(VerbFlag::Exec))
            )
            .unwrap()
            .names(),
            verb_names
        );
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    pub fn reparent_props() {
        let db = test_db();
        let mut tx = db.start_transaction();
        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();
        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, b, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();

        // Add a property on A
        tx.define_property(
            &a,
            &a,
            Symbol::mk("test"),
            &NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();

        // Verify it's on B & C
        let (prop, v, perms, is_clear) = tx.resolve_property(&b, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        let (prop, v, perms, is_clear) = tx.resolve_property(&c, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // Now make a new root and reparent B to it
        let d = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test4"),
            )
            .unwrap();

        tx.set_object_parent(&b, &d).unwrap();

        // Verify the property is no longer on B
        let result = tx.resolve_property(&b, Symbol::mk("test"));
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::PropertyNotFound(b, "test".to_string())
        );

        // Or C.
        let result = tx.resolve_property(&c, Symbol::mk("test"));
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::PropertyNotFound(c, "test".to_string())
        );

        // Now add new property on D
        tx.define_property(
            &d,
            &d,
            Symbol::mk("test2"),
            &NOTHING,
            BitEnum::new(),
            Some(v_str("test_value2")),
        )
        .unwrap();

        // Verify it's on B and C
        let (prop, v, perms, is_clear) = tx.resolve_property(&b, Symbol::mk("test2")).unwrap();
        assert_eq!(prop.name(), "test2".into());
        assert_eq!(v, v_str("test_value2"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        let (prop, v, perms, is_clear) = tx.resolve_property(&c, Symbol::mk("test2")).unwrap();
        assert_eq!(prop.name(), "test2".into());
        assert_eq!(v, v_str("test_value2"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // And now reparent C to A again, and verify it's back to having the first property.
        tx.set_object_parent(&c, &a).unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(&c, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);
    }

    #[test]
    pub fn test_recycle_object() {
        // Simple: property-less, #-1 located, #-1 parented object.
        let db = test_db();
        let mut tx = db.start_transaction();
        let tobj = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        tx.recycle_object(&tobj).expect("Unable to recycle object");

        // Verify it's gone.
        let result = tx.get_object_name(&tobj);
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::ObjectNotFound(ObjectRef::Id(tobj))
        );

        // Create two new objects and root the second off the first.
        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();

        // Recycle the child, and verify it's gone.
        tx.recycle_object(&b).expect("Unable to recycle object");
        let result = tx.get_object_name(&b);
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::ObjectNotFound(ObjectRef::Id(b))
        );

        // Verify that children list is empty for the parent.
        let children = tx.get_object_children(&a).unwrap();
        assert!(
            children.is_empty(),
            "Children list is not empty: {children:?}"
        );

        // Create another one, add a property to the root, and then verify we can recycle the child.
        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();
        tx.define_property(
            &a,
            &a,
            Symbol::mk("test"),
            &NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();

        // Verify root's children list contains our object.
        assert!(
            tx.get_object_children(&a)
                .unwrap()
                .is_same(ObjSet::from_items(&[c]))
        );

        tx.recycle_object(&c).expect("Unable to recycle object");
        let result = tx.get_object_name(&c);
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::ObjectNotFound(ObjectRef::Id(c))
        );

        // Verify the property is still there.
        let (prop, v, perms, _) = tx.resolve_property(&a, Symbol::mk("test")).unwrap();
        assert_eq!(prop.name(), "test".into());
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);

        // Create another, add a property, then recycle the root.
        let d = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test4"),
            )
            .unwrap();
        tx.define_property(
            &a,
            &a,
            Symbol::mk("test2"),
            &NOTHING,
            BitEnum::new(),
            Some(v_str("test_value2")),
        )
        .unwrap();

        tx.recycle_object(&a).expect("Unable to recycle object");
        let result = tx.get_object_name(&a);
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::ObjectNotFound(ObjectRef::Id(a))
        );

        // Verify the child object is still there despite its parent being destroyed.
        let result = tx.get_object_name(&d);
        assert_eq!(result.unwrap(), "test4");

        // Verify the object's parent is now NOTHING.
        assert_eq!(tx.get_object_parent(&d).unwrap(), NOTHING);

        // We should not have the property, it came from our parent.
        let result = tx.resolve_property(&d, Symbol::mk("test2"));
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::PropertyNotFound(d, "test2".to_string())
        );
    }

    // Verify that 'max_object' is the highest object id in the database, not one higher.
    #[test]
    pub fn test_max_object() {
        let db = test_db();
        let mut tx = db.start_transaction();
        // Max object in a virgin DB should return #-1
        let max_obj = tx.get_max_object().unwrap();
        assert_eq!(max_obj, NOTHING);
        let obj = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
        assert_eq!(tx.get_max_object().unwrap(), obj);
    }

    #[test]
    fn test_chown_property() {
        let db = test_db();
        let mut tx = db.start_transaction();
        let obj = tx.create_object(None, Default::default()).unwrap();
        let obj2 = tx.create_object(None, Default::default()).unwrap();
        let obj3 = tx.create_object(None, Default::default()).unwrap();
        let obj4 = tx.create_object(None, Default::default()).unwrap();
        tx.set_object_parent(&obj2, &obj).unwrap();
        tx.set_object_owner(&obj2, &obj4).unwrap();

        let uuid = tx
            .define_property(
                &obj,
                &obj,
                Symbol::mk("test"),
                &obj3,
                BitEnum::new_with(PropFlag::Chown),
                None,
            )
            .unwrap();

        // Property owner on obj should be obj3, but on obj2 it should be obj4, since that's
        // obj2's owner
        let o1perms = tx.retrieve_property_permissions(&obj, uuid).unwrap();
        assert_eq!(o1perms.owner(), obj3);
        let o2perms = tx.retrieve_property_permissions(&obj2, uuid).unwrap();
        assert_eq!(o2perms.owner(), obj4);

        // Now create a new object, descendant of obj, and the same logic should apply
        let obj5 = tx
            .create_object(
                None,
                ObjAttrs::new(
                    Obj::mk_id(obj4.id().0 + 1),
                    obj,
                    obj,
                    BitEnum::new_with(ObjFlag::Read),
                    "zoinks",
                ),
            )
            .unwrap();

        assert_eq!(tx.get_object_owner(&obj5).unwrap(), obj5);
        let o5perms = tx.retrieve_property_permissions(&obj5, uuid).unwrap();
        assert_eq!(o5perms.owner(), obj5);
    }

    /// Regression where property values of an ancestor shared with a new parent got lost.
    /// E.g. password lost when reparenting from "player" to "programmer", which also inherited
    /// from "player"
    #[test]
    fn test_property_subgraph_reparent() {
        let db = test_db();
        let mut tx = db.start_transaction();
        let player = tx.create_object(None, Default::default()).unwrap();
        let uuid = tx
            .define_property(
                &player,
                &player,
                Symbol::mk("password"),
                &player,
                BitEnum::new(),
                Some(v_int(666)),
            )
            .unwrap();
        let programmer = tx.create_object(None, Default::default()).unwrap();
        let builder = tx.create_object(None, Default::default()).unwrap();
        let user = tx.create_object(None, Default::default()).unwrap();
        tx.set_object_parent(&builder, &player).unwrap();
        tx.set_object_parent(&programmer, &builder).unwrap();
        tx.set_object_parent(&user, &player).unwrap();

        tx.set_property(&user, uuid, v_int(1234567890)).unwrap();
        assert_eq!(
            tx.retrieve_property(&user, uuid).unwrap().0.unwrap(),
            v_int(1234567890)
        );

        tx.set_object_parent(&user, &programmer).unwrap();
        assert_eq!(
            tx.retrieve_property(&user, uuid).unwrap().0.unwrap(),
            v_int(1234567890)
        );
    }

    #[test]
    fn test_regression_recycle_parent_lose_prop() {
        let db = test_db();
        let mut tx = db.start_transaction();
        let a = tx
            .create_object(
                None,
                ObjAttrs::new(Obj::mk_id(1), NOTHING, NOTHING, BitEnum::all(), "a"),
            )
            .unwrap();
        let b = tx
            .create_object(
                None,
                ObjAttrs::new(Obj::mk_id(1), a, NOTHING, BitEnum::all(), "b"),
            )
            .unwrap();
        tx.define_property(
            &b,
            &b,
            Symbol::mk("b"),
            &SYSTEM_OBJECT,
            BitEnum::new(),
            Some(v_str("b")),
        )
        .unwrap();

        assert_eq!(
            tx.resolve_property(&b, Symbol::mk("b")).unwrap().1,
            v_str("b")
        );

        tx.recycle_object(&a).unwrap();

        assert_eq!(
            tx.resolve_property(&b, Symbol::mk("b")).unwrap().1,
            v_str("b")
        );
    }

    #[test]
    fn test_regression_missing_child_propdef() {
        let db = test_db();
        let mut tx = db.start_transaction();
        let object_e = tx.create_object(None, Default::default()).unwrap();
        let object_c = tx.create_object(None, Default::default()).unwrap();
        tx.define_property(
            &object_c,
            &object_c,
            Symbol::mk("c"),
            &SYSTEM_OBJECT,
            BitEnum::new(),
            Some(v_str("c")),
        )
        .unwrap();
        tx.set_object_parent(&object_e, &object_c).unwrap();

        let c_uuid = tx
            .resolve_property(&object_c, Symbol::mk("c"))
            .unwrap()
            .0
            .uuid();
        tx.delete_property(&object_c, c_uuid).unwrap();
    }

    #[test]
    fn test_regression_verb_cache_accidental_miss() {
        let db = test_db();

        // Create a verb with no X bit and wrong arg spec, resolve it (fail), and then get_verb_by_name and it should
        // succeed. IT wasn't before because the verb cache was getting filled with a negative
        // miss.
        let mut tx = db.start_transaction();
        let a = tx.create_object(None, Default::default()).unwrap();
        tx.add_object_verb(
            &a,
            &a,
            &[Symbol::mk("test")],
            ProgramType::MooR(Program::new()),
            VerbFlag::rw(),
            VerbArgsSpec::none_none_none(),
        )
        .unwrap();
        let r = tx.resolve_verb(
            &a,
            Symbol::mk("test"),
            Some(VerbArgsSpec::this_none_this()),
            None,
        );
        assert!(r.is_err());
        let _ = tx
            .get_verb_by_name(&a, Symbol::mk("test"))
            .expect("Unable to get verb");
    }

    #[test]
    fn test_create_immediate_destroy() {
        // equiv of recycle(create($nothing));
        let db = test_db();
        let mut tx = db.start_transaction();
        let my_obj = tx
            .create_object(Some(Obj::mk_id(-1)), Default::default())
            .unwrap();
        tx.recycle_object(&my_obj).unwrap();
        let r = tx.commit().unwrap();
        assert_eq!(r, CommitResult::Success);
    }

    #[test]
    fn test_transaction_serialization_property_conflicts() {
        let db = test_db();

        // Create initial object with property
        let mut tx1 = db.start_transaction();
        let obj = tx1.create_object(None, Default::default()).unwrap();
        let prop_uuid = tx1
            .define_property(
                &obj,
                &obj,
                Symbol::mk("test"),
                &obj,
                BitEnum::new(),
                Some(v_str("initial")),
            )
            .unwrap();
        tx1.commit().unwrap();

        // Start two concurrent transactions
        let mut tx2 = db.start_transaction();
        let mut tx3 = db.start_transaction();

        // Modify property in tx2
        tx2.set_property(&obj, prop_uuid, v_str("tx2_value"))
            .unwrap();

        // Modify same property in tx3
        tx3.set_property(&obj, prop_uuid, v_str("tx3_value"))
            .unwrap();

        // First commit should succeed
        assert_eq!(tx2.commit().unwrap(), CommitResult::Success);

        // Second commit should fail due to conflict
        assert_eq!(tx3.commit().unwrap(), CommitResult::ConflictRetry);

        // Verify final value
        let tx4 = db.start_transaction();
        assert_eq!(
            tx4.retrieve_property(&obj, prop_uuid).unwrap().0.unwrap(),
            v_str("tx2_value")
        );
    }

    /// Regression test for cache corruption bug where get_verb_by_name records miss
    /// for inherited verbs, corrupting cache entries that resolve_verb can use
    #[test]
    fn test_get_verb_by_name_cache_corruption() {
        let db = test_db();

        // Set up inheritance hierarchy: child -> parent
        let (child_obj, parent_obj) = {
            let mut tx = db.start_transaction();

            let parent = tx
                .create_object(
                    None,
                    ObjAttrs::new(
                        NOTHING,
                        NOTHING,
                        NOTHING,
                        BitEnum::new_with(ObjFlag::Read) | ObjFlag::Write,
                        "parent",
                    ),
                )
                .unwrap();

            let child = tx
                .create_object(
                    None,
                    ObjAttrs::new(
                        NOTHING,
                        parent,
                        NOTHING,
                        BitEnum::new_with(ObjFlag::Read) | ObjFlag::Write,
                        "child",
                    ),
                )
                .unwrap();

            // Add verb to parent that child inherits
            tx.add_object_verb(
                &parent,
                &parent,
                &[Symbol::mk("test_verb")],
                ProgramType::MooR(Program::new()),
                BitEnum::new_with(VerbFlag::Read),
                VerbArgsSpec::none_none_none(),
            )
            .unwrap();

            tx.commit().unwrap();
            (child, parent)
        };

        // Step 1: resolve_verb finds inherited verb, populates cache
        {
            let tx = db.start_transaction();
            let result = tx.resolve_verb(&child_obj, Symbol::mk("test_verb"), None, None);

            // Should succeed - verb found via inheritance
            assert!(result.is_ok());
            let verb = result.unwrap();
            assert_eq!(verb.location(), parent_obj); // Verb is on parent
        }

        // Step 2: get_verb_by_name for same verb should NOT corrupt the cache
        {
            let tx = db.start_transaction();
            let result = tx.get_verb_by_name(&child_obj, Symbol::mk("test_verb"));

            // Should fail - get_verb_by_name only looks directly on object
            assert!(matches!(result, Err(WorldStateError::VerbNotFound(_, _))));
        }

        // Step 3: resolve_verb should still work (cache shouldn't be corrupted)
        {
            let tx = db.start_transaction();
            let result = tx.resolve_verb(&child_obj, Symbol::mk("test_verb"), None, None);

            // This is the regression test - should still succeed
            // If get_verb_by_name corrupted the cache by recording miss,
            // this will fail with E_VERBNF
            assert!(
                result.is_ok(),
                "resolve_verb failed after get_verb_by_name - cache corruption detected!"
            );
            let verb = result.unwrap();
            assert_eq!(verb.location(), parent_obj);
        }
    }

    #[test]
    fn test_concurrent_verb_lookup_regression() {
        // This test reproduces the exact scenario from the production logs:
        // - Transaction 1 calls resolve_verb and finds inherited verb
        // - Transaction 2 calls get_verb_by_name and should not corrupt cache
        // - Both should see consistent results despite being concurrent

        let db = test_db();

        // Set up inheritance hierarchy with verb on parent
        let (child_obj, parent_obj) = {
            let mut tx = db.start_transaction();

            let parent = tx
                .create_object(
                    None,
                    ObjAttrs::new(
                        NOTHING,
                        NOTHING,
                        NOTHING,
                        BitEnum::new_with(ObjFlag::Read) | ObjFlag::Write,
                        "parent",
                    ),
                )
                .unwrap();

            let child = tx
                .create_object(
                    None,
                    ObjAttrs::new(
                        NOTHING,
                        parent,
                        NOTHING,
                        BitEnum::new_with(ObjFlag::Read) | ObjFlag::Write,
                        "child",
                    ),
                )
                .unwrap();

            // Add "features" verb to parent (matching production logs)
            tx.add_object_verb(
                &parent,
                &parent,
                &[Symbol::mk("features")],
                ProgramType::MooR(Program::new()),
                BitEnum::new_with(VerbFlag::Read),
                VerbArgsSpec::none_none_none(),
            )
            .unwrap();

            tx.commit().unwrap();
            (child, parent)
        };

        // Simulate the exact sequence from logs using multiple transactions

        // Transaction 1: resolve_verb finds and caches inherited verb
        let tx1 = db.start_transaction();
        let resolve_result = tx1.resolve_verb(&child_obj, Symbol::mk("features"), None, None);
        assert!(
            resolve_result.is_ok(),
            "resolve_verb should find inherited verb"
        );
        let verb1 = resolve_result.unwrap();
        assert_eq!(verb1.location(), parent_obj);
        // Don't commit tx1 yet - it's still active when tx2 starts

        // Transaction 2: get_verb_by_name for same verb (concurrent with tx1)
        let tx2 = db.start_transaction();
        let get_result = tx2.get_verb_by_name(&child_obj, Symbol::mk("features"));
        // get_verb_by_name should fail (verb not directly on child)
        assert!(matches!(
            get_result,
            Err(WorldStateError::VerbNotFound(_, _))
        ));

        // Commit tx2 first (this is what happens in production logs)
        tx2.commit().unwrap();

        // Now commit tx1
        tx1.commit().unwrap();

        // Transaction 3: resolve_verb should still work after both commits
        // This is the critical test - if cache was corrupted, this will fail
        let tx3 = db.start_transaction();
        let final_resolve = tx3.resolve_verb(&child_obj, Symbol::mk("features"), None, None);

        assert!(
            final_resolve.is_ok(),
            "resolve_verb failed after concurrent get_verb_by_name - this reproduces the production bug!"
        );
        let verb3 = final_resolve.unwrap();
        assert_eq!(verb3.location(), parent_obj);
    }

    /// Test property inheritance, clearing, and properties() function behavior
    /// This is equivalent to the MOOT test in basic/property.moot
    /// Each step runs in a separate transaction like MOOT tests do
    #[test]
    fn test_property_inheritance_and_clearing() {
        // Run the test multiple times to help trigger inconsistent macOS bugs
        for iteration in 0..5 {
            let db = test_db();
            let parent;
            let child;

            // Step 1: Create parent object (use different IDs per iteration) - separate transaction
            {
                let mut tx = db.start_transaction();
                let parent_id = 1 + (iteration * 100); // Use different object IDs per iteration
                parent = tx
                    .create_object(
                        Some(Obj::mk_id(parent_id)),
                        ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "parent"),
                    )
                    .unwrap();
                assert_eq!(parent, Obj::mk_id(parent_id));
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
            }

            // Step 2: Create child object parented to parent - separate transaction
            {
                let mut tx = db.start_transaction();
                child = tx
                    .create_object(
                        None,
                        ObjAttrs::new(NOTHING, parent, NOTHING, BitEnum::new(), "child"),
                    )
                    .unwrap();
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
            }

            // Step 3: Define property "test" on parent with value 1 - separate transaction
            {
                let mut tx = db.start_transaction();
                tx.define_property(
                    &parent,
                    &parent,
                    Symbol::mk("test"),
                    &NOTHING,
                    BitEnum::new(),
                    Some(v_int(1)),
                )
                .unwrap();
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
            }

            // Step 4: Check parent property value - separate transaction
            {
                let tx = db.start_transaction();
                let (_prop, value, _perms, is_clear) =
                    tx.resolve_property(&parent, Symbol::mk("test")).unwrap();
                assert_eq!(value, v_int(1));
                assert!(!is_clear, "Property should not be clear on defining object");
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
            }

            // Step 5: Check child inherits property - separate transaction
            {
                let tx = db.start_transaction();
                let (_prop, value, _perms, is_clear) =
                    tx.resolve_property(&child, Symbol::mk("test")).unwrap();
                assert_eq!(value, v_int(1));
                assert!(is_clear, "Inherited property should be marked as clear");
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
            }

            // Step 6: Set property on child to value 2 - separate transaction
            let prop_uuid = {
                let mut tx = db.start_transaction();
                let (prop, _value, _perms, _is_clear) =
                    tx.resolve_property(&child, Symbol::mk("test")).unwrap();
                let uuid = prop.uuid();
                tx.set_property(&child, uuid, v_int(2)).unwrap();
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
                uuid
            };

            // Step 7: Check child now has value 2, not clear - separate transaction
            {
                let tx = db.start_transaction();
                let (_prop, value, _perms, is_clear) =
                    tx.resolve_property(&child, Symbol::mk("test")).unwrap();
                assert_eq!(value, v_int(2));
                assert!(!is_clear, "Property should not be clear after being set");
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
            }

            // Step 8: Check properties() function behavior - separate transaction
            {
                let tx = db.start_transaction();
                let parent_props = tx.get_properties(&parent).unwrap();
                assert_eq!(parent_props.len(), 1, "Parent should have 1 property");
                assert!(parent_props.find_first_named(Symbol::mk("test")).is_some());

                let child_props = tx.get_properties(&child).unwrap();
                assert_eq!(
                    child_props.len(),
                    0,
                    "Child should have 0 properties (inheritance not listed)"
                );
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
            }

            // Step 9: Clear property on child - separate transaction
            {
                let mut tx = db.start_transaction();

                tx.clear_property(&child, prop_uuid).unwrap();
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
            }

            // Step 10: Check child inherits parent's value again - separate transaction
            {
                let tx = db.start_transaction();

                let (_prop, value, _perms, is_clear) =
                    tx.resolve_property(&child, Symbol::mk("test")).unwrap();
                assert_eq!(
                    value,
                    v_int(1),
                    "Child should inherit parent's value (1) after clearing, but got {value:?}"
                );
                assert!(is_clear, "Property should be clear after clearing");
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
            }

            // Step 11: Check parent property unchanged - separate transaction
            {
                let tx = db.start_transaction();
                let (_prop, value, _perms, is_clear) =
                    tx.resolve_property(&parent, Symbol::mk("test")).unwrap();
                assert_eq!(value, v_int(1));
                assert!(!is_clear, "Parent property should remain unchanged");
                assert_eq!(tx.commit(), Ok(CommitResult::Success));
            }
        }
    }

    /// Test basic property persistence across transactions
    #[test]
    fn test_property_persistence_across_transactions() {
        let db = test_db();
        let parent;
        let child;
        let prop_uuid;

        // Create objects and property
        {
            let mut tx = db.start_transaction();
            parent = tx
                .create_object(
                    Some(Obj::mk_id(1)),
                    ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "parent"),
                )
                .unwrap();
            child = tx
                .create_object(
                    None,
                    ObjAttrs::new(NOTHING, parent, NOTHING, BitEnum::new(), "child"),
                )
                .unwrap();
            prop_uuid = tx
                .define_property(
                    &parent,
                    &parent,
                    Symbol::mk("test"),
                    &NOTHING,
                    BitEnum::new(),
                    Some(v_int(1)),
                )
                .unwrap();
            assert_eq!(tx.commit(), Ok(CommitResult::Success));
        }

        // Set property in one transaction
        {
            let mut tx = db.start_transaction();
            tx.set_property(&child, prop_uuid, v_int(2)).unwrap();
            assert_eq!(tx.commit(), Ok(CommitResult::Success));
        }

        // Read property in another transaction
        {
            let tx = db.start_transaction();
            let (value, _) = tx.retrieve_property(&child, prop_uuid).unwrap();
            assert_eq!(value, Some(v_int(2)));
        }

        // Clear property in one transaction
        {
            let mut tx = db.start_transaction();

            // Debug: Check what the underlying relation delete does
            tx.clear_property(&child, prop_uuid).unwrap();
            assert_eq!(tx.commit(), Ok(CommitResult::Success));
        }

        // Read property in another transaction
        {
            let tx = db.start_transaction();
            let (value, _) = tx.retrieve_property(&child, prop_uuid).unwrap();
            assert_eq!(value, None, "Property should be None after clearing");
        }
    }

    /// Test property operations across transactions using high-level APIs
    #[test]
    fn test_relation_delete_operation() {
        let db = test_db();
        let obj = Obj::mk_id(1);
        let prop_uuid;

        // Test 1: Create object and define/set property in one transaction, read in another
        {
            let mut tx = db.start_transaction();
            tx.create_object(
                Some(obj),
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
            let _prop_uuid = tx
                .define_property(
                    &obj,
                    &obj,
                    Symbol::mk("test_prop"),
                    &obj,
                    BitEnum::new(),
                    Some(v_int(42)),
                )
                .unwrap();
            assert_eq!(tx.commit(), Ok(CommitResult::Success));
        }

        // Read property by name in another transaction - this should work
        {
            let tx = db.start_transaction();
            let (prop, value, _perms, is_clear) =
                tx.resolve_property(&obj, Symbol::mk("test_prop")).unwrap();
            assert_eq!(
                value,
                v_int(42),
                "Read after write should see the written value"
            );
            assert!(!is_clear, "Property should not be clear");
            prop_uuid = prop.uuid(); // Get the actual UUID for later use
        }

        // Clear property in one transaction
        {
            let mut tx = db.start_transaction();
            tx.clear_property(&obj, prop_uuid).unwrap();
            assert_eq!(tx.commit(), Ok(CommitResult::Success));
        }

        // Read property in another transaction - should be None after clear
        {
            let tx = db.start_transaction();
            let (value, _) = tx.retrieve_property(&obj, prop_uuid).unwrap();
            assert_eq!(value, None, "Property should be None after clear");
        }
    }

    /// Test the same property behavior in a single transaction to verify the bug is transaction-related
    #[test]
    fn test_property_inheritance_and_clearing_single_tx() {
        let db = test_db();
        let mut tx = db.start_transaction();

        // Create parent object (#1 equivalent)
        let parent = tx
            .create_object(
                Some(Obj::mk_id(1)),
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "parent"),
            )
            .unwrap();

        // Create child object parented to parent
        let child = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, parent, NOTHING, BitEnum::new(), "child"),
            )
            .unwrap();

        // Define property "test" on parent with value 1
        tx.define_property(
            &parent,
            &parent,
            Symbol::mk("test"),
            &NOTHING,
            BitEnum::new(),
            Some(v_int(1)),
        )
        .unwrap();

        // Set property on child to value 2
        let (prop, _value, _perms, _is_clear) =
            tx.resolve_property(&child, Symbol::mk("test")).unwrap();
        let prop_uuid = prop.uuid();
        tx.set_property(&child, prop_uuid, v_int(2)).unwrap();

        // Verify child has value 2, not clear
        let (_prop, value, _perms, is_clear) =
            tx.resolve_property(&child, Symbol::mk("test")).unwrap();
        assert_eq!(value, v_int(2));
        assert!(!is_clear);

        // Clear property on child
        tx.clear_property(&child, prop_uuid).unwrap();

        // Check child inherits parent's value again
        let (_prop, value, _perms, is_clear) =
            tx.resolve_property(&child, Symbol::mk("test")).unwrap();
        assert_eq!(
            value,
            v_int(1),
            "Single TX: Child should inherit parent's value (1) after clearing, but got {value:?}"
        );
        assert!(
            is_clear,
            "Single TX: Property should be clear after clearing"
        );

        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    fn test_get_verb_by_name_cache_miss_after_flush_regression() {
        // This test reproduces the exact bug from production logs:
        // 1. Cache gets flushed (due to object creation/modification)
        // 2. get_verb_by_name is called for an inherited verb
        // 3. get_verb_by_name incorrectly records a miss (should not record anything)
        // 4. Subsequent resolve_verb calls fail due to the false cached miss

        let db = test_db();

        // Set up inheritance hierarchy: child inherits from parent
        let (child_obj, parent_obj) = {
            let mut tx = db.start_transaction();

            let parent = tx
                .create_object(
                    None,
                    ObjAttrs::new(
                        NOTHING,
                        NOTHING,
                        NOTHING,
                        BitEnum::new_with(ObjFlag::Read) | ObjFlag::Write,
                        "parent",
                    ),
                )
                .unwrap();

            let child = tx
                .create_object(
                    None,
                    ObjAttrs::new(
                        NOTHING,
                        parent,
                        NOTHING,
                        BitEnum::new_with(ObjFlag::Read) | ObjFlag::Write,
                        "child",
                    ),
                )
                .unwrap();

            // Add "features" verb to parent (child inherits it)
            tx.add_object_verb(
                &parent,
                &parent,
                &[Symbol::mk("features")],
                ProgramType::MooR(Program::new()),
                BitEnum::new_with(VerbFlag::Read),
                VerbArgsSpec::none_none_none(),
            )
            .unwrap();

            tx.commit().unwrap();
            (child, parent)
        };

        // Step 1: Verify the verb can be resolved via inheritance
        {
            let tx = db.start_transaction();
            let result = tx.resolve_verb(&child_obj, Symbol::mk("features"), None, None);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().location(), parent_obj);
        }

        // Step 2: Simulate cache flush by creating a new object (triggers flush)
        {
            let mut tx = db.start_transaction();
            tx.create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "temp_object"),
            )
            .unwrap();
            tx.commit().unwrap();
            // This commit causes cache flush due to object creation
        }

        // Step 3: In SAME transaction: Call get_verb_by_name then resolve_verb
        // This matches the production logs where both calls happen in tx 1643
        {
            let tx = db.start_transaction();

            // First call: get_verb_by_name (should fail but not corrupt cache)
            let get_result = tx.get_verb_by_name(&child_obj, Symbol::mk("features"));
            assert!(matches!(
                get_result,
                Err(WorldStateError::VerbNotFound(_, _))
            ));

            // Second call in SAME transaction: resolve_verb (should still work)
            let resolve_result = tx.resolve_verb(&child_obj, Symbol::mk("features"), None, None);

            // This is the critical test - if get_verb_by_name corrupted the cache,
            // resolve_verb will fail even within the same transaction
            assert!(
                resolve_result.is_ok(),
                "resolve_verb failed after get_verb_by_name in same transaction - reproduces production bug!"
            );
            assert_eq!(resolve_result.unwrap().location(), parent_obj);
        }
    }
}
