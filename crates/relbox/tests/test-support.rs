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

use relbox::index::{AttrType, IndexType};
use relbox::{RelBox, RelationInfo};
use std::path::PathBuf;
use std::sync::Arc;

/// Build a test database with a bunch of relations
#[allow(dead_code)]
pub fn test_db(dir: PathBuf) -> Arc<RelBox> {
    // Generate 10 test relations that we'll use for testing.
    let relations = (0..100)
        .map(|i| RelationInfo {
            name: format!("relation_{}", i),
            domain_type: AttrType::Integer,
            codomain_type: AttrType::Integer,
            secondary_indexed: false,
            unique_domain: true,
            index_type: IndexType::AdaptiveRadixTree,
            codomain_index_type: None,
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
