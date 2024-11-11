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

//! A set of common tests for any world state implementation.

use crate::worldstate_transaction::WorldStateTransaction;
use crate::{RelationalTransaction, RelationalWorldStateTransaction, WorldStateTable};
use moor_values::model::VerbArgsSpec;
use moor_values::model::{BinaryType, VerbAttrs};
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::model::{HasUuid, Named};
use moor_values::model::{ObjAttrs, PropFlag, ValSet};
use moor_values::model::{ObjSet, ObjectRef};
use moor_values::util::BitEnum;
use moor_values::Objid;
use moor_values::Symbol;
use moor_values::NOTHING;
use moor_values::{v_int, v_str};

pub fn perform_test_create_object<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();
    let oid = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
        )
        .unwrap();
    assert_eq!(oid, Objid(0));
    assert!(tx.object_valid(oid).unwrap());
    assert_eq!(tx.get_object_owner(oid).unwrap(), oid);
    assert_eq!(tx.get_object_parent(oid).unwrap(), NOTHING);
    assert_eq!(tx.get_object_location(oid).unwrap(), NOTHING);
    assert_eq!(tx.get_object_name(oid).unwrap(), "test");
    assert_eq!(tx.commit(), Ok(CommitResult::Success));

    // Verify existence in a new transaction.
    let tx = begin_tx();
    assert!(tx.object_valid(oid).unwrap());
    assert_eq!(tx.get_object_owner(oid).unwrap(), oid);
}

pub fn perform_test_create_object_fixed_id<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();

    // Force at 1.
    let oid = tx
        .create_object(Some(Objid(1)), ObjAttrs::default())
        .unwrap();
    assert_eq!(oid, Objid(1));
    // Now verify the next will be 2.
    let oid2 = tx.create_object(None, ObjAttrs::default()).unwrap();
    assert_eq!(oid2, Objid(2));
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

pub fn perform_test_parent_children<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();

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

    assert_eq!(tx.get_object_parent(b).unwrap(), a);
    assert!(tx
        .get_object_children(a)
        .unwrap()
        .is_same(ObjSet::from_items(&[b])));

    assert_eq!(tx.get_object_parent(a).unwrap(), NOTHING);
    assert_eq!(tx.get_object_children(b).unwrap(), ObjSet::empty());

    // Add a second child
    let c = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test3"),
        )
        .unwrap();

    assert_eq!(tx.get_object_parent(c).unwrap(), a);
    assert!(tx
        .get_object_children(a)
        .unwrap()
        .is_same(ObjSet::from_items(&[b, c])));

    assert_eq!(tx.get_object_parent(a).unwrap(), NOTHING);
    assert_eq!(tx.get_object_children(b).unwrap(), ObjSet::empty());

    // Create new obj and reparent one child
    let d = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test4"),
        )
        .unwrap();

    tx.set_object_parent(b, d).unwrap();
    assert_eq!(tx.get_object_parent(b).unwrap(), d);
    assert!(tx
        .get_object_children(a)
        .unwrap()
        .is_same(ObjSet::from_items(&[c])));
    assert!(tx
        .get_object_children(d)
        .unwrap()
        .is_same(ObjSet::from_items(&[b])));
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

pub fn perform_test_descendants<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();

    let a = tx
        .create_object(
            Some(Objid(0)),
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
        )
        .unwrap();
    assert_eq!(a, Objid(0));

    let b = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
        )
        .unwrap();
    assert_eq!(b, Objid(1));

    let c = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test3"),
        )
        .unwrap();
    assert_eq!(c, Objid(2));

    let d = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, c, NOTHING, BitEnum::new(), "test4"),
        )
        .unwrap();
    assert_eq!(d, Objid(3));

    assert!(tx
        .descendants(a)
        .unwrap()
        .is_same(ObjSet::from_items(&[b, c, d])));
    assert_eq!(tx.descendants(b).unwrap(), ObjSet::empty());
    assert_eq!(tx.descendants(c).unwrap(), ObjSet::from_items(&[d]));

    // Now reparent d to b
    tx.set_object_parent(d, b).unwrap();
    assert!(tx
        .get_object_children(a)
        .unwrap()
        .is_same(ObjSet::from_items(&[b, c])));
    assert_eq!(tx.get_object_children(b).unwrap(), ObjSet::from_items(&[d]));
    assert_eq!(tx.get_object_children(c).unwrap(), ObjSet::empty());
    assert!(tx
        .descendants(a)
        .unwrap()
        .is_same(ObjSet::from_items(&[b, c, d])));
    assert_eq!(tx.descendants(b).unwrap(), ObjSet::from_items(&[d]));
    assert_eq!(tx.descendants(c).unwrap(), ObjSet::empty());
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

pub fn perform_test_location_contents<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();

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

    assert_eq!(tx.get_object_location(b).unwrap(), a);
    assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::from_items(&[b]));

    assert_eq!(tx.get_object_location(a).unwrap(), NOTHING);
    assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());

    let c = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test3"),
        )
        .unwrap();

    tx.set_object_location(b, c).unwrap();
    assert_eq!(tx.get_object_location(b).unwrap(), c);
    assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::empty());
    assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::from_items(&[b]));

    let d = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test4"),
        )
        .unwrap();
    tx.set_object_location(d, c).unwrap();
    assert!(tx
        .get_object_contents(c)
        .unwrap()
        .is_same(ObjSet::from_items(&[b, d])));
    assert_eq!(tx.get_object_location(d).unwrap(), c);

    tx.set_object_location(a, c).unwrap();
    assert!(tx
        .get_object_contents(c)
        .unwrap()
        .is_same(ObjSet::from_items(&[b, d, a])));
    assert_eq!(tx.get_object_location(a).unwrap(), c);

    // Validate recursive move detection.
    match tx.set_object_location(c, b).err() {
        Some(WorldStateError::RecursiveMove(_, _)) => {}
        _ => {
            panic!("Expected recursive move error");
        }
    }

    // Move b one level deeper, and then check recursive move detection again.
    tx.set_object_location(b, d).unwrap();
    match tx.set_object_location(c, b).err() {
        Some(WorldStateError::RecursiveMove(_, _)) => {}
        _ => {
            panic!("Expected recursive move error");
        }
    }

    // The other way around, d to c should be fine.
    tx.set_object_location(d, c).unwrap();
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

/// Test data integrity of object moves between commits.
pub fn perform_test_object_move_commits<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();

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

    tx.set_object_location(b, a).unwrap();
    tx.set_object_location(c, a).unwrap();
    assert_eq!(tx.get_object_location(b).unwrap(), a);
    assert_eq!(tx.get_object_location(c).unwrap(), a);
    assert!(tx
        .get_object_contents(a)
        .unwrap()
        .is_same(ObjSet::from_items(&[b, c])));
    assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());
    assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::empty());

    assert_eq!(tx.commit(), Ok(CommitResult::Success));

    let mut tx = begin_tx();
    assert_eq!(tx.get_object_location(b).unwrap(), a);
    assert_eq!(tx.get_object_location(c).unwrap(), a);
    let contents = tx.get_object_contents(a).expect("Unable to get contents");
    assert!(
        contents.is_same(ObjSet::from_items(&[b, c])),
        "Contents of a are not as expected: {:?} vs {:?}",
        contents,
        ObjSet::from_items(&[b, c])
    );
    assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());
    assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::empty());

    tx.set_object_location(b, c).unwrap();
    assert_eq!(tx.get_object_location(b).unwrap(), c);
    assert_eq!(tx.get_object_location(c).unwrap(), a);
    assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::from_items(&[c]));
    assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());
    assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::from_items(&[b]));
    assert_eq!(tx.commit(), Ok(CommitResult::Success));

    let tx = begin_tx();
    assert_eq!(tx.get_object_location(c).unwrap(), a);
    assert_eq!(tx.get_object_location(b).unwrap(), c);
    assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::from_items(&[c]));
    assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());
    assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::from_items(&[b]));
}

pub fn perform_test_simple_property<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();

    let oid = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
        )
        .unwrap();

    tx.define_property(
        oid,
        oid,
        Symbol::mk_case_insensitive("test"),
        NOTHING,
        BitEnum::new(),
        Some(v_str("test")),
    )
    .unwrap();
    let (prop, v, perms, is_clear) = tx
        .resolve_property(oid, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
    assert_eq!(v, v_str("test"));
    assert_eq!(perms.owner(), NOTHING);
    assert!(!is_clear);
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

/// Regression test for updating-verbs failing.
pub fn perform_test_verb_add_update<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();
    let oid = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
        )
        .unwrap();
    tx.add_object_verb(
        oid,
        oid,
        vec![Symbol::mk_case_insensitive("test")],
        vec![],
        BinaryType::LambdaMoo18X,
        BitEnum::new(),
        VerbArgsSpec::this_none_this(),
    )
    .unwrap();
    // resolve the verb to its vh.
    let vh = tx
        .resolve_verb(oid, Symbol::mk_case_insensitive("test"), None)
        .unwrap();
    assert_eq!(vh.names(), vec!["test"]);
    // Verify it's actually on the object when we get verbs.
    let verbs = tx.get_verbs(oid).unwrap();
    assert_eq!(verbs.len(), 1);
    assert!(verbs.contains(vh.uuid()));
    // update the verb using its uuid, renaming it.
    tx.update_verb(
        oid,
        vh.uuid(),
        VerbAttrs {
            definer: None,
            owner: None,
            names: Some(vec![Symbol::mk_case_insensitive("test2")]),
            flags: None,
            args_spec: None,
            binary_type: None,
            binary: None,
        },
    )
    .unwrap();
    // resolve with the new name.
    let vh = tx
        .resolve_verb(oid, Symbol::mk_case_insensitive("test2"), None)
        .unwrap();
    assert_eq!(vh.names(), vec!["test2"]);

    // Now commit, and try to resolve again.
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
    let mut tx = begin_tx();
    let vh = tx
        .resolve_verb(oid, Symbol::mk_case_insensitive("test2"), None)
        .unwrap();
    assert_eq!(vh.names(), vec!["test2"]);
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

pub fn perform_test_transitive_property_resolution<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();

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
        a,
        a,
        Symbol::mk_case_insensitive("test"),
        NOTHING,
        BitEnum::new(),
        Some(v_str("test_value")),
    )
    .unwrap();
    let (prop, v, perms, is_clear) = tx
        .resolve_property(b, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
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

    tx.set_object_parent(b, c).unwrap();

    let result = tx.resolve_property(b, Symbol::mk_case_insensitive("test"));
    assert_eq!(
        result.err().unwrap(),
        WorldStateError::PropertyNotFound(b, "test".to_string())
    );
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

pub fn perform_test_transitive_property_resolution_clear_property<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();

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
        a,
        a,
        Symbol::mk_case_insensitive("test"),
        NOTHING,
        BitEnum::new(),
        Some(v_str("test_value")),
    )
    .unwrap();
    let (prop, v, perms, is_clear) = tx
        .resolve_property(b, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
    assert_eq!(v, v_str("test_value"));
    assert_eq!(perms.owner(), NOTHING);
    assert!(is_clear);

    // Set the property on the child to a new value.
    tx.set_property(b, prop.uuid(), v_int(666)).unwrap();

    // Verify the new value is present.
    let (prop, v, perms, is_clear) = tx
        .resolve_property(b, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
    assert_eq!(v, v_int(666));
    assert_eq!(perms.owner(), NOTHING);
    assert!(!is_clear);

    // Now clear, and we should get the old value, but with clear status.
    tx.clear_property(b, prop.uuid()).unwrap();
    let (prop, v, perms, is_clear) = tx
        .resolve_property(b, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
    assert_eq!(v, v_str("test_value"));
    assert_eq!(perms.owner(), NOTHING);
    assert!(is_clear);

    // Changing flags or owner should have nothing to do with the clarity of the property value.
    tx.update_property_info(
        b,
        prop.uuid(),
        Some(b),
        Some(BitEnum::new_with(PropFlag::Read)),
        None,
    )
    .unwrap();
    let (prop, v, perms, is_clear) = tx
        .resolve_property(b, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
    assert_eq!(v, v_str("test_value"));
    assert_eq!(perms.owner(), b);
    assert_eq!(perms.flags(), BitEnum::new_with(PropFlag::Read));
    assert!(is_clear);

    // Setting the value again makes it not clear
    tx.set_property(b, prop.uuid(), v_int(666)).unwrap();
    let (prop, v, perms, is_clear) = tx
        .resolve_property(b, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
    assert_eq!(v, v_int(666));
    assert_eq!(perms.owner(), b);
    assert_eq!(perms.flags(), BitEnum::new_with(PropFlag::Read));
    assert!(!is_clear);

    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

pub fn perform_test_rename_property<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let tx = begin_tx();
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
            a,
            a,
            Symbol::mk_case_insensitive("test"),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();

    // I can update the name on the parent...
    tx.update_property_info(a, uuid, None, None, Some("a_new_name".to_string()))
        .unwrap();

    // And now resolve that new name on the child.
    let (prop, v, perms, is_clear) = tx
        .resolve_property(b, Symbol::mk_case_insensitive("a_new_name"))
        .unwrap();
    assert_eq!(prop.name(), "a_new_name");
    assert_eq!(v, v_str("test_value"));
    assert_eq!(perms.owner(), NOTHING);
    assert!(is_clear);

    // But it's illegal to try to rename it on the child who doesn't define it.
    assert!(tx
        .update_property_info(b, uuid, None, None, Some("a_new_name".to_string()))
        .is_err())
}

/// Test regression where parent properties were present via `properties()` on children.
pub fn perform_test_regression_properties<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let tx = begin_tx();

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
        a,
        a,
        Symbol::mk_case_insensitive("test"),
        NOTHING,
        BitEnum::new(),
        Some(v_str("test_value")),
    )
    .unwrap();
    let (prop, v, perms, is_clear) = tx
        .resolve_property(b, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
    assert_eq!(v, v_str("test_value"));
    assert_eq!(perms.owner(), NOTHING);
    assert!(is_clear);

    // And another on child
    let child_prop = tx
        .define_property(
            b,
            b,
            Symbol::mk_case_insensitive("test2"),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value2")),
        )
        .unwrap();

    let props = tx.get_properties(b).unwrap();

    // Our prop should be there
    assert!(props.find(&child_prop).is_some());

    // Listing the set of properties on the child should include only the child's properties
    assert_eq!(props.len(), 1);
}

pub fn perform_test_verb_resolve<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();

    let a = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
        )
        .unwrap();

    tx.add_object_verb(
        a,
        a,
        vec![Symbol::mk_case_insensitive("test")],
        vec![],
        BinaryType::LambdaMoo18X,
        BitEnum::new(),
        VerbArgsSpec::this_none_this(),
    )
    .unwrap();

    assert_eq!(
        tx.resolve_verb(a, Symbol::mk_case_insensitive("test"), None)
            .unwrap()
            .names(),
        vec!["test"]
    );

    assert_eq!(
        tx.resolve_verb(
            a,
            Symbol::mk_case_insensitive("test"),
            Some(VerbArgsSpec::this_none_this())
        )
        .unwrap()
        .names(),
        vec!["test"]
    );

    let v_uuid = tx
        .resolve_verb(a, Symbol::mk_case_insensitive("test"), None)
        .unwrap()
        .uuid();
    assert_eq!(tx.get_verb_binary(a, v_uuid).unwrap(), vec![]);

    // Add a second verb with a different name
    tx.add_object_verb(
        a,
        a,
        vec![Symbol::mk_case_insensitive("test2")],
        vec![],
        BinaryType::LambdaMoo18X,
        BitEnum::new(),
        VerbArgsSpec::this_none_this(),
    )
    .unwrap();

    // Verify we can get it
    assert_eq!(
        tx.resolve_verb(a, Symbol::mk_case_insensitive("test2"), None)
            .unwrap()
            .names(),
        vec!["test2"]
    );
    assert_eq!(tx.commit(), Ok(CommitResult::Success));

    // Verify existence in a new transaction.
    let mut tx = begin_tx();
    assert_eq!(
        tx.resolve_verb(a, Symbol::mk_case_insensitive("test"), None)
            .unwrap()
            .names(),
        vec!["test"]
    );
    assert_eq!(
        tx.resolve_verb(a, Symbol::mk_case_insensitive("test2"), None)
            .unwrap()
            .names(),
        vec!["test2"]
    );
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

pub fn perform_test_verb_resolve_inherited<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();

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
        a,
        a,
        vec![Symbol::mk_case_insensitive("test")],
        vec![],
        BinaryType::LambdaMoo18X,
        BitEnum::new(),
        VerbArgsSpec::this_none_this(),
    )
    .unwrap();

    assert_eq!(
        tx.resolve_verb(b, Symbol::mk_case_insensitive("test"), None)
            .unwrap()
            .names(),
        vec!["test"]
    );

    assert_eq!(
        tx.resolve_verb(
            b,
            Symbol::mk_case_insensitive("test"),
            Some(VerbArgsSpec::this_none_this())
        )
        .unwrap()
        .names(),
        vec!["test"]
    );

    let v_uuid = tx
        .resolve_verb(b, Symbol::mk_case_insensitive("test"), None)
        .unwrap()
        .uuid();
    assert_eq!(tx.get_verb_binary(a, v_uuid).unwrap(), vec![]);
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

pub fn perform_test_verb_resolve_wildcard<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let mut tx = begin_tx();
    let a = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
        )
        .unwrap();

    let verb_names = vec!["dname*c", "iname*c"];
    tx.add_object_verb(
        a,
        a,
        verb_names
            .iter()
            .map(|s| Symbol::mk_case_insensitive(s))
            .collect(),
        vec![],
        BinaryType::LambdaMoo18X,
        BitEnum::new(),
        VerbArgsSpec::this_none_this(),
    )
    .unwrap();

    assert_eq!(
        tx.resolve_verb(a, Symbol::mk_case_insensitive("dname"), None)
            .unwrap()
            .names(),
        verb_names
    );

    assert_eq!(
        tx.resolve_verb(a, Symbol::mk_case_insensitive("dnamec"), None)
            .unwrap()
            .names(),
        verb_names
    );

    assert_eq!(
        tx.resolve_verb(a, Symbol::mk_case_insensitive("iname"), None)
            .unwrap()
            .names(),
        verb_names
    );

    assert_eq!(
        tx.resolve_verb(a, Symbol::mk_case_insensitive("inamec"), None)
            .unwrap()
            .names(),
        verb_names
    );
    assert_eq!(tx.commit(), Ok(CommitResult::Success));
}

pub fn perform_reparent_props<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let tx = begin_tx();
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
        a,
        a,
        Symbol::mk_case_insensitive("test"),
        NOTHING,
        BitEnum::new(),
        Some(v_str("test_value")),
    )
    .unwrap();

    // Verify it's on B & C
    let (prop, v, perms, is_clear) = tx
        .resolve_property(b, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
    assert_eq!(v, v_str("test_value"));
    assert_eq!(perms.owner(), NOTHING);
    assert!(is_clear);

    let (prop, v, perms, is_clear) = tx
        .resolve_property(c, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
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

    tx.set_object_parent(b, d).unwrap();

    // Verify the property is no longer on B
    let result = tx.resolve_property(b, Symbol::mk_case_insensitive("test"));
    assert_eq!(
        result.err().unwrap(),
        WorldStateError::PropertyNotFound(b, "test".to_string())
    );

    // Or C.
    let result = tx.resolve_property(c, Symbol::mk_case_insensitive("test"));
    assert_eq!(
        result.err().unwrap(),
        WorldStateError::PropertyNotFound(c, "test".to_string())
    );

    // Now add new property on D
    tx.define_property(
        d,
        d,
        Symbol::mk_case_insensitive("test2"),
        NOTHING,
        BitEnum::new(),
        Some(v_str("test_value2")),
    )
    .unwrap();

    // Verify it's on B and C
    let (prop, v, perms, is_clear) = tx
        .resolve_property(b, Symbol::mk_case_insensitive("test2"))
        .unwrap();
    assert_eq!(prop.name(), "test2");
    assert_eq!(v, v_str("test_value2"));
    assert_eq!(perms.owner(), NOTHING);
    assert!(is_clear);

    let (prop, v, perms, is_clear) = tx
        .resolve_property(c, Symbol::mk_case_insensitive("test2"))
        .unwrap();
    assert_eq!(prop.name(), "test2");
    assert_eq!(v, v_str("test_value2"));
    assert_eq!(perms.owner(), NOTHING);
    assert!(is_clear);

    // And now reparent C to A again, and verify it's back to having the first property.
    tx.set_object_parent(c, a).unwrap();
    let (prop, v, perms, is_clear) = tx
        .resolve_property(c, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
    assert_eq!(v, v_str("test_value"));
    assert_eq!(perms.owner(), NOTHING);
    assert!(is_clear);
}

pub fn perform_test_recycle_object<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    // Simple: property-less, #-1 located, #-1 parented object.
    let tx = begin_tx();
    let tobj = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
        )
        .unwrap();

    tx.recycle_object(tobj).expect("Unable to recycle object");

    // Verify it's gone.
    let result = tx.get_object_name(tobj);
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
    tx.recycle_object(b).expect("Unable to recycle object");
    let result = tx.get_object_name(b);
    assert_eq!(
        result.err().unwrap(),
        WorldStateError::ObjectNotFound(ObjectRef::Id(b))
    );

    // Verify that children list is empty for the parent.
    assert!(tx.get_object_children(a).unwrap().is_empty());

    // Create another one, add a property to the root, and then verify we can recycle the child.
    let c = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test3"),
        )
        .unwrap();
    tx.define_property(
        a,
        a,
        Symbol::mk_case_insensitive("test"),
        NOTHING,
        BitEnum::new(),
        Some(v_str("test_value")),
    )
    .unwrap();

    // Verify root's children list contains our object.
    assert!(tx
        .get_object_children(a)
        .unwrap()
        .is_same(ObjSet::from_items(&[c])));

    tx.recycle_object(c).expect("Unable to recycle object");
    let result = tx.get_object_name(c);
    assert_eq!(
        result.err().unwrap(),
        WorldStateError::ObjectNotFound(ObjectRef::Id(c))
    );

    // Verify the property is still there.
    let (prop, v, perms, _) = tx
        .resolve_property(a, Symbol::mk_case_insensitive("test"))
        .unwrap();
    assert_eq!(prop.name(), "test");
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
        a,
        a,
        Symbol::mk_case_insensitive("test2"),
        NOTHING,
        BitEnum::new(),
        Some(v_str("test_value2")),
    )
    .unwrap();

    tx.recycle_object(a).expect("Unable to recycle object");
    let result = tx.get_object_name(a);
    assert_eq!(
        result.err().unwrap(),
        WorldStateError::ObjectNotFound(ObjectRef::Id(a))
    );

    // Verify the child object is still there despite its parent being destroyed.
    let result = tx.get_object_name(d);
    assert_eq!(result.unwrap(), "test4");

    // Verify the object's parent is now NOTHING.
    assert_eq!(tx.get_object_parent(d).unwrap(), NOTHING);

    // We should not have the property, it came from our parent.
    let result = tx.resolve_property(d, Symbol::mk_case_insensitive("test2"));
    assert_eq!(
        result.err().unwrap(),
        WorldStateError::PropertyNotFound(d, "test2".to_string())
    );
}

// Verify that 'max_object' is the highest object id in the database, not one higher.
pub fn perform_test_max_object<F, TX>(begin_tx: F)
where
    F: Fn() -> RelationalWorldStateTransaction<TX>,
    TX: RelationalTransaction<WorldStateTable>,
{
    let tx = begin_tx();
    // Max object in a virgin DB should return #-1
    let max_obj = tx.get_max_object().unwrap();
    assert_eq!(max_obj, Objid(-1));
    let obj = tx
        .create_object(
            None,
            ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
        )
        .unwrap();
    assert_eq!(tx.get_max_object().unwrap(), obj);
}
