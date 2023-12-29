use std::collections::HashMap;
use std::fmt::Debug;
#[derive(Debug, serde::Deserialize, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum Type {
    invoke,
    ok,
    fail,
}
impl Type {
    pub fn to_keyword(&self) -> &str {
        match self {
            Type::invoke => "invoke",
            Type::ok => "ok",
            Type::fail => "fail",
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct History {
    pub f: String,
    pub index: i64,
    pub process: i64,
    pub time: i64,
    pub r#type: Type,
    pub value: Vec<Value>,
}

// ["append",9,1]
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
#[allow(non_camel_case_types)]
pub enum Value {
    append(String, i64, i64),
    r(String, i64, Option<Vec<i64>>),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use moor_db::tuplebox::tb::{RelationInfo, TupleBox};
    use moor_db::tuplebox::{RelationId, Transaction};
    use moor_values::util::slice_ref::SliceRef;

    use super::*;

    /// Build a test database with a bunch of relations
    async fn test_db() -> Arc<TupleBox> {
        // Generate 10 test relations that we'll use for testing.
        let relations = (0..100)
            .map(|i| RelationInfo {
                name: format!("relation_{}", i),
                domain_type_id: 0,
                codomain_type_id: 0,
                secondary_indexed: false,
            })
            .collect::<Vec<_>>();

        
        TupleBox::new(1 << 24, 4096, None, &relations, 0).await
    }

    fn from_val(value: i64) -> SliceRef {
        SliceRef::from_bytes(&value.to_le_bytes()[..])
    }
    fn to_val(value: SliceRef) -> i64 {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(value.as_slice());
        i64::from_le_bytes(bytes)
    }

    async fn check_expected(
        process: i64,
        _db: Arc<TupleBox>,
        tx: &Transaction,
        relation: RelationId,
        expected_values: &Option<Vec<i64>>,
        action_type: Type,
    ) {
        // Expect to read these values from the relation in a scan.
        let tuples = tx
            .relation(relation)
            .await
            .predicate_scan(&|_| true)
            .await
            .unwrap();

        let got = tuples
            .iter()
            .map(|t| to_val(t.0.clone()))
            .collect::<BTreeSet<_>>();

        if let Some(values) = expected_values {
            let expected = values.iter().cloned().collect::<BTreeSet<_>>();

            assert!(
                expected.iter().all(|v| got.contains(v)),
                "T{} at {}, r {} expected {:?} but got {:?}",
                process,
                action_type.to_keyword(),
                relation.0,
                values,
                got
            );
        }
    }

    async fn check_completion(
        process: i64,
        db: Arc<TupleBox>,
        tx: &Transaction,
        values: Vec<Value>,
        action_type: Type,
    ) {
        for ev in values {
            match ev {
                Value::append(_, register, expect_val) => {
                    let relation = RelationId(register as usize);

                    // The value mentioned should have been added to the relation successfully
                    // (at invoke)
                    let (domain, _) = tx
                        .relation(relation)
                        .await
                        .seek_by_domain(from_val(expect_val))
                        .await
                        .unwrap();
                    let val = to_val(domain);
                    assert_eq!(
                        val,
                        expect_val,
                        "T{} at {}, expected {} to be {} after its insert",
                        process,
                        action_type.to_keyword(),
                        register,
                        expect_val
                    );
                }
                Value::r(_, register, expected_values) => {
                    let relation = RelationId(register as usize);

                    // Expect to read these values from the relation in a scan.
                    check_expected(
                        process,
                        db.clone(),
                        tx,
                        relation,
                        &expected_values,
                        action_type,
                    )
                    .await;
                }
            }
        }
    }

    #[tokio::test]
    async fn test_generate() {
        let db = test_db().await;

        let lines = include_str!("jepsen-dataset1.json")
            .lines()
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>();
        let events = lines
            .iter()
            .map(|l| serde_json::from_str::<History>(l).unwrap());
        let mut processes = HashMap::new();
        for e in events {
            match e.r#type {
                Type::invoke => {
                    // Start a transaction.
                    let tx = Arc::new(db.clone().start_tx());
                    let existing = processes.insert(e.process, tx.clone());
                    assert!(
                        existing.is_none(),
                        "T{} already exists uncommitted",
                        e.process
                    );
                    // Execute the actions
                    for ev in &e.value {
                        match ev {
                            Value::append(_, register, value) => {
                                // Insert the value into the relation.
                                let relation = RelationId(*register as usize);
                                tx.clone()
                                    .relation(relation)
                                    .await
                                    .insert_tuple(from_val(*value), from_val(*value))
                                    .await
                                    .unwrap();
                            }
                            Value::r(_, register, values) => {
                                let relation = RelationId(*register as usize);

                                check_expected(
                                    e.process,
                                    db.clone(),
                                    &tx,
                                    relation,
                                    values,
                                    e.r#type,
                                )
                                .await;
                            }
                        }
                    }
                }
                Type::ok => {
                    // Commit the transaction, expecting the values to be in the relation.
                    let tx = processes.remove(&e.process).unwrap();
                    check_completion(e.process, db.clone(), &tx, e.value, e.r#type).await;
                    tx.commit().await.unwrap();
                }
                Type::fail => {
                    // Rollback the transaction.
                    let tx = processes.remove(&e.process).unwrap();
                    check_completion(e.process, db.clone(), &tx, e.value, e.r#type).await;
                    tx.rollback().await.unwrap();
                }
            }
        }
    }
}
