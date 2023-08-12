use std::fmt::{Display, Formatter};

use bincode::{Decode, Encode};

pub const SYSTEM_OBJECT: Objid = Objid(0);
pub const NOTHING: Objid = Objid(-1);
pub const AMBIGUOUS: Objid = Objid(-2);

pub const FAILED_MATCH: Objid = Objid(-3);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct Objid(pub i64);

impl Display for Objid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#{}", self.0))
    }
}

impl Objid {
    pub fn to_literal(&self) -> String {
        format!("#{}", self.0)
    }
}

/// When we want to refer to a set of object ids, use this type.
// (Mainly this is for encapsulation its storage and retrieval)
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct ObjSet(Vec<Objid>);

impl ObjSet {
    pub fn new() -> Self {
        ObjSet(Vec::new())
    }

    pub fn from(oids: Vec<Objid>) -> Self {
        ObjSet(oids)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Objid> {
        self.0.iter()
    }

    pub fn insert(&mut self, oid: Objid) {
        if !self.0.contains(&oid) {
            self.0.push(oid);
        }
    }

    pub fn contains(&self, oid: Objid) -> bool {
        self.0.contains(&oid)
    }

    pub fn append(&mut self, other: Self) {
        for oid in other.0 {
            self.insert(oid);
        }
    }
}

impl Default for ObjSet {
    fn default() -> Self {
        ObjSet::new()
    }
}
