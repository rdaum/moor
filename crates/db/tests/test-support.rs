use moor_db::rdb::{RelBox, RelationInfo};
use std::path::PathBuf;
use std::sync::Arc;

/// Build a test database with a bunch of relations
#[allow(dead_code)]
pub fn test_db(dir: PathBuf) -> Arc<RelBox> {
    // Generate 10 test relations that we'll use for testing.
    let relations = (0..100)
        .map(|i| RelationInfo {
            name: format!("relation_{}", i),
            domain_type_id: 0,
            codomain_type_id: 0,
            secondary_indexed: false,
        })
        .collect::<Vec<_>>();

    RelBox::new(1 << 24, Some(dir), &relations, 0)
}

#[derive(Debug, serde::Deserialize, Copy, Clone, PartialEq)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub enum Type {
    invoke,
    ok,
    fail,
}

impl Type {
    #[allow(dead_code)]
    pub fn as_keyword(&self) -> &str {
        match self {
            Type::invoke => "invoke",
            Type::ok => "ok",
            Type::fail => "fail",
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
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
#[allow(dead_code)]
pub enum Value {
    append(String, i64, i64),
    r(String, i64, Option<Vec<i64>>),
}
