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

//! Create a world state DB, populate it with a pile of random objects and properties, and then
//! commit. Then reload it and verify the presence of all such properties.

#[cfg(test)]
mod tests {
    use moor_common::model::{ObjFlag, PropFlag, WorldStateSource};
    use moor_common::util::BitEnum;
    use moor_db::{DatabaseConfig, TxDB};
    use moor_var::{NOTHING, Obj, SYSTEM_OBJECT, Symbol, Var, v_int};
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Arc;

    fn test_db(path: &Path) -> Arc<TxDB> {
        Arc::new(TxDB::open(Some(path), DatabaseConfig::default()).0)
    }

    struct TestObject {
        properties: HashMap<Symbol, Var>,
    }

    fn generate_random_object_and_props(db: Arc<TxDB>) -> HashMap<Obj, TestObject> {
        let num_objects = 1000;
        let max_properties = 20;

        let mut objects = HashMap::new();

        let mut tx = db.new_world_state().unwrap();

        for _ in 0..num_objects {
            let o = tx
                .create_object(
                    &SYSTEM_OBJECT,
                    &NOTHING,
                    &SYSTEM_OBJECT,
                    BitEnum::new_with(ObjFlag::Read) | ObjFlag::Write,
                )
                .unwrap();

            let num_props = rand::random::<usize>() % max_properties;

            let mut props = HashMap::new();
            for _ in 0..num_props {
                let prop_name = format!("prop{}", rand::random::<u32>());
                let prop_value_i = rand::random::<i64>();
                let prop_value = v_int(prop_value_i);

                tx.define_property(
                    &SYSTEM_OBJECT,
                    &o,
                    &o,
                    Symbol::mk(&prop_name),
                    &SYSTEM_OBJECT,
                    BitEnum::new_with(PropFlag::Read),
                    Some(prop_value.clone()),
                )
                .unwrap();

                props.insert(Symbol::mk(&prop_name), prop_value);
            }
            let to = TestObject { properties: props };
            objects.insert(o, to);
        }

        tx.commit().unwrap();

        objects
    }

    /// Test that we can insert a bunch of objects and properties, and then retrieve them.
    /// Does so in the same physical DB instance.
    #[test]
    fn test_mass_insert_retrieve() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        let objects = generate_random_object_and_props(db.clone());

        let tx = db.new_world_state().unwrap();

        for (o, to) in objects.iter() {
            for (prop_name, prop_value) in to.properties.iter() {
                let info = tx.get_property_info(&SYSTEM_OBJECT, o, *prop_name).unwrap();
                assert_eq!(info.0.name(), prop_name.as_str());
                assert_eq!(info.0.location(), *o);
                assert_eq!(info.0.definer(), *o);
                assert_eq!(info.1.flags(), BitEnum::new_with(PropFlag::Read));

                let value = tx.retrieve_property(&SYSTEM_OBJECT, o, *prop_name).unwrap();
                assert_eq!(value, *prop_value);
            }
        }

        // Max object should be consistent
        assert_eq!(
            tx.max_object(&SYSTEM_OBJECT).unwrap().id().0,
            (objects.len() as i32) - 1
        );
    }

    /// Test that we can insert a bunch of objects and properties, and then retrieve them.
    /// Does so in a new physical DB instance.
    #[test]
    fn test_mass_insert_retrieve_new_db() {
        let tmpdir = tempfile::tempdir().unwrap();
        let objects = {
            let db = test_db(tmpdir.path());
            generate_random_object_and_props(db.clone())
        };
        let db2 = test_db(tmpdir.path());

        let tx = db2.new_world_state().unwrap();

        for (o, to) in objects.iter() {
            for (prop_name, prop_value) in to.properties.iter() {
                let info = tx.get_property_info(&SYSTEM_OBJECT, o, *prop_name).unwrap();
                assert_eq!(info.0.name(), prop_name.as_str());
                assert_eq!(info.0.location(), *o);
                assert_eq!(info.0.definer(), *o);
                assert_eq!(info.1.flags(), BitEnum::new_with(PropFlag::Read));

                let value = tx.retrieve_property(&SYSTEM_OBJECT, o, *prop_name).unwrap();
                assert_eq!(value, *prop_value);
            }
        }

        // Max object should be consistent
        assert_eq!(
            tx.max_object(&SYSTEM_OBJECT).unwrap().id().0,
            (objects.len() as i32) - 1
        );
    }
}
