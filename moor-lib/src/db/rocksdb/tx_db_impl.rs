use rocksdb::ColumnFamily;

use moor_value::model::WorldStateError;
use moor_value::var::objid::{ObjSet, Objid, NOTHING};
use moor_value::AsByteBuffer;

use crate::db::rocksdb::ColumnFamilies;

pub(crate) fn oid_key(o: Objid) -> Vec<u8> {
    o.0.to_be_bytes().to_vec()
}

pub(crate) fn composite_key(o: Objid, uuid: &uuid::Bytes) -> Vec<u8> {
    let mut key = oid_key(o);
    key.extend_from_slice(&uuid[..]);
    key
}

pub(crate) fn get_oid_value<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
) -> Result<Objid, anyhow::Error> {
    let ok = oid_key(o);
    let ov = tx.get_cf(cf, ok).unwrap();
    let ov = ov.ok_or(WorldStateError::ObjectNotFound(o))?;
    let ov = u64::from_be_bytes(ov.try_into().unwrap());
    let ov = Objid(ov as i64);
    Ok(ov)
}

pub(crate) fn get_oid_or_nothing<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
) -> Result<Objid, anyhow::Error> {
    let ok = oid_key(o);
    let ov = tx.get_cf(cf, ok).unwrap();
    let Some(ov) = ov else {
        return Ok(NOTHING);
    };
    let ov = u64::from_be_bytes(ov.try_into().unwrap());
    let ov = Objid(ov as i64);
    Ok(ov)
}

pub(crate) fn set_oid_value<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
    v: Objid,
) -> Result<(), anyhow::Error> {
    let ok = oid_key(o);
    let ov = oid_key(v);
    tx.put_cf(cf, ok, ov).unwrap();
    Ok(())
}

pub(crate) fn get_objset<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
) -> Result<ObjSet, anyhow::Error> {
    let ok = oid_key(o);
    let bytes = tx.get_cf(cf, ok)?;
    let bytes = bytes.ok_or(WorldStateError::ObjectNotFound(o))?;
    let ov = ObjSet::from_byte_vector(bytes);
    Ok(ov)
}

pub(crate) fn set_objset<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
    v: ObjSet,
) -> Result<(), WorldStateError> {
    let ok = oid_key(o);
    tx.put_cf(cf, ok, v.as_byte_buffer()).unwrap();
    Ok(())
}

pub(crate) fn cf_for<'a>(cf_handles: &[&'a ColumnFamily], cf: ColumnFamilies) -> &'a ColumnFamily {
    cf_handles[(cf as u8) as usize]
}

pub(crate) fn err_is_objnjf(e: &anyhow::Error) -> bool {
    if let Some(WorldStateError::ObjectNotFound(_)) = e.downcast_ref::<WorldStateError>() {
        return true;
    }
    false
}

pub(crate) struct RocksDbTx<'a> {
    pub(crate) tx: rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    pub(crate) cf_handles: Vec<&'a ColumnFamily>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rocksdb::OptimisticTransactionDB;
    use strum::VariantNames;
    use tempdir::TempDir;
    use uuid::Uuid;

    use moor_value::model::objects::ObjAttrs;
    use moor_value::model::r#match::VerbArgsSpec;
    use moor_value::model::verbs::BinaryType;
    use moor_value::model::WorldStateError;
    use moor_value::util::bitenum::BitEnum;
    use moor_value::var::objid::{ObjSet, Objid, NOTHING};
    use moor_value::var::v_str;

    use crate::db::rocksdb::tx_db_impl::RocksDbTx;
    use crate::db::rocksdb::ColumnFamilies;

    struct TestDb {
        db: Arc<OptimisticTransactionDB>,
    }

    impl TestDb {
        fn tx(&self) -> RocksDbTx {
            let cf_handles = ColumnFamilies::VARIANTS
                .iter()
                .enumerate()
                .map(|cf| self.db.cf_handle(cf.1).unwrap())
                .collect();
            let rtx = self.db.transaction();

            RocksDbTx {
                tx: rtx,
                cf_handles,
            }
        }
    }

    fn mk_test_db() -> TestDb {
        let tmp_dir = TempDir::new("test_db").unwrap();
        let db_path = tmp_dir.path().join("test_db");
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let column_families = ColumnFamilies::VARIANTS;
        let db: Arc<OptimisticTransactionDB> =
            Arc::new(OptimisticTransactionDB::open_cf(&options, db_path, column_families).unwrap());

        TestDb { db: db.clone() }
    }

    #[test]
    fn test_create_object() {
        let db = mk_test_db();
        let tx = db.tx();
        let oid = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();
        assert_eq!(oid, Objid(0));
        assert!(tx.object_valid(oid).unwrap());
        assert_eq!(tx.get_object_owner(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_parent(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_location(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_name(oid).unwrap(), "test");
    }

    #[test]
    fn test_parent_children() {
        let db = mk_test_db();
        let tx = db.tx();

        // Single parent/child relationship.
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(tx.get_object_parent(b).unwrap(), a);
        assert_eq!(tx.get_object_children(a).unwrap(), ObjSet::from(vec![b]));

        assert_eq!(tx.get_object_parent(a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(b).unwrap(), ObjSet::new());

        // Add a second child
        let c = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(tx.get_object_parent(c).unwrap(), a);
        assert_eq!(tx.get_object_children(a).unwrap(), ObjSet::from(vec![b, c]));

        assert_eq!(tx.get_object_parent(a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(b).unwrap(), ObjSet::new());

        // Create new obj and reparent one child
        let d = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test3".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.set_object_parent(b, d).unwrap();
        assert_eq!(tx.get_object_parent(b).unwrap(), d);
        assert_eq!(tx.get_object_children(a).unwrap(), ObjSet::from(vec![c]));
        assert_eq!(tx.get_object_children(d).unwrap(), ObjSet::from(vec![b]));
    }

    #[test]
    fn test_descendants() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let c = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let d = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test4".into()),
                    parent: Some(c),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(tx.descendants(a).unwrap(), ObjSet::from(vec![b, c, d]));
        assert_eq!(tx.descendants(b).unwrap(), ObjSet::new());
        assert_eq!(tx.descendants(c).unwrap(), ObjSet::from(vec![d]));

        // Now reparent d to b
        tx.set_object_parent(d, b).unwrap();
        assert_eq!(tx.get_object_children(a).unwrap(), ObjSet::from(vec![b, c]));
        assert_eq!(tx.get_object_children(b).unwrap(), ObjSet::from(vec![d]));
        assert_eq!(tx.get_object_children(c).unwrap(), ObjSet::new());
        assert_eq!(tx.descendants(a).unwrap(), ObjSet::from(vec![b, c, d]));
        assert_eq!(tx.descendants(b).unwrap(), ObjSet::from(vec![d]));
        assert_eq!(tx.descendants(c).unwrap(), ObjSet::new());
    }

    #[test]
    fn test_location_contents() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(NOTHING),
                    location: Some(a),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(tx.get_object_location(b).unwrap(), a);
        assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::from(vec![b]));

        assert_eq!(tx.get_object_location(a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::new());

        let c = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test3".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.set_object_location(b, c).unwrap();
        assert_eq!(tx.get_object_location(b).unwrap(), c);
        assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::new());
        assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::from(vec![b]));

        let d = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test4".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();
        tx.set_object_location(d, c).unwrap();
        assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::from(vec![b, d]));
        assert_eq!(tx.get_object_location(d).unwrap(), c);

        tx.set_object_location(a, c).unwrap();
        assert_eq!(
            tx.get_object_contents(c).unwrap(),
            ObjSet::from(vec![b, d, a])
        );
        assert_eq!(tx.get_object_location(a).unwrap(), c);

        // Validate recursive move detection.
        match tx
            .set_object_location(c, b)
            .err()
            .unwrap()
            .downcast_ref::<WorldStateError>()
        {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // Move b one level deeper, and then check recursive move detection again.
        tx.set_object_location(b, d).unwrap();
        match tx
            .set_object_location(c, b)
            .err()
            .unwrap()
            .downcast_ref::<WorldStateError>()
        {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // The other way around, d to c should be fine.
        tx.set_object_location(d, c).unwrap();
    }

    #[test]
    fn test_simple_property() {
        let db = mk_test_db();
        let tx = db.tx();
        let oid = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.define_property(
            oid,
            oid,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test")),
        )
        .unwrap();
        let (prop, v) = tx.resolve_property(oid, "test".into()).unwrap();
        assert_eq!(prop.name, "test");
        assert_eq!(v, v_str("test"));
    }

    #[test]
    fn test_transitive_property_resolution() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.define_property(
            a,
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();
        let (prop, v) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name, "test");
        assert_eq!(v, v_str("test_value"));

        // Verify we *don't* get this property for an unrelated, unhinged object by reparenting b
        // to new parent c.  This should remove the defs for a's properties from b.
        let c = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test3".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.set_object_parent(b, c).unwrap();

        let result = tx.resolve_property(b, "test".into());
        assert_eq!(
            result
                .err()
                .unwrap()
                .downcast_ref::<WorldStateError>()
                .unwrap(),
            &WorldStateError::PropertyNotFound(b, "test".into())
        );
    }

    #[test]
    fn test_transitive_property_resolution_clear_property() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.define_property(
            a,
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();
        let (prop, v) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name, "test");
        assert_eq!(v, v_str("test_value"));

        // Define the property again, but on the object 'b',
        // This should raise an error because the child already *has* this property.
        // MOO will not let this happen. The right way to handle overloading is to set the value
        // on the child.
        let result = tx.define_property(a, b, "test".into(), NOTHING, BitEnum::new(), None);
        assert!(
            matches!(result, Err(e) if matches!(e.downcast_ref::<WorldStateError>(),
                Some(WorldStateError::DuplicatePropertyDefinition(_, _))))
        );
    }

    #[test]
    fn test_verb_resolve() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.add_object_verb(
            a,
            a,
            vec!["test".into()],
            vec![],
            BinaryType::LambdaMoo18X,
            BitEnum::new(),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();

        assert_eq!(
            tx.resolve_verb(a, "test".into(), None).unwrap().names,
            vec!["test"]
        );

        assert_eq!(
            tx.resolve_verb(a, "test".into(), Some(VerbArgsSpec::this_none_this()))
                .unwrap()
                .names,
            vec!["test"]
        );

        let v_uuid = tx.resolve_verb(a, "test".into(), None).unwrap().uuid;
        let v_uuid = Uuid::from_bytes(v_uuid);
        assert_eq!(tx.get_binary(a, v_uuid).unwrap(), vec![]);
    }

    #[test]
    fn test_verb_resolve_wildcard() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let verb_names = vec!["dname*c".into(), "iname*c".into()];
        tx.add_object_verb(
            a,
            a,
            verb_names.clone(),
            vec![],
            BinaryType::LambdaMoo18X,
            BitEnum::new(),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();

        assert_eq!(
            tx.resolve_verb(a, "dname".into(), None).unwrap().names,
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "dnamec".into(), None).unwrap().names,
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "iname".into(), None).unwrap().names,
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "inamec".into(), None).unwrap().names,
            verb_names
        );
    }
}
