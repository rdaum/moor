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

use crate::model::defset::{Defs, HasUuid, Named};
use bincode::{Decode, Encode};
use moor_var::BincodeAsByteBufferExt;
use moor_var::Obj;
use moor_var::Symbol;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Encode, Decode, Clone)]
pub struct PropDef {
    #[bincode(with_serde)]
    uuid: Uuid,
    definer: Obj,
    location: Obj,
    name: Symbol,
}

impl PropDef {
    #[must_use]
    pub fn new(uuid: Uuid, definer: Obj, location: Obj, name: &str) -> Self {
        Self {
            uuid,
            definer,
            location,
            name: Symbol::mk(name),
        }
    }

    #[must_use]
    pub fn definer(&self) -> Obj {
        self.definer.clone()
    }
    #[must_use]
    pub fn location(&self) -> Obj {
        self.location.clone()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl Named for PropDef {
    fn matches_name(&self, name: Symbol) -> bool {
        self.name().to_lowercase() == name.as_str()
    }

    fn names(&self) -> Vec<&str> {
        vec![self.name()]
    }
}

impl HasUuid for PropDef {
    fn uuid(&self) -> Uuid {
        self.uuid
    }
}

pub type PropDefs = Defs<PropDef>;

impl BincodeAsByteBufferExt for PropDef {}

#[cfg(test)]
mod tests {
    use crate::model::{HasUuid, PropDef, PropDefs, ValSet};
    use moor_var::{Obj, Symbol};
    use uuid::Uuid;

    #[test]
    fn test_in_propdefs() {
        let test_pd1 = PropDef::new(Uuid::new_v4(), Obj::mk_id(1), Obj::mk_id(2), "test");

        let test_pd2 = PropDef::new(Uuid::new_v4(), Obj::mk_id(10), Obj::mk_id(12), "test2");

        let pds = PropDefs::empty().with_all_added(&[test_pd1.clone(), test_pd2.clone()]);
        let pd1 = pds.find_first_named(Symbol::mk("test")).unwrap();
        assert_eq!(pd1.uuid(), test_pd1.uuid());
    }

    #[test]
    fn test_clone_compare() {
        let pd = PropDef::new(Uuid::new_v4(), Obj::mk_id(1), Obj::mk_id(2), "test");
        let pd2 = pd.clone();
        assert_eq!(pd, pd2);
        assert_eq!(pd.uuid(), pd2.uuid());
        assert_eq!(pd.definer(), pd2.definer());
        assert_eq!(pd.location(), pd2.location());
        assert_eq!(pd.name(), pd2.name());
    }

    #[test]
    fn test_propdefs_iter() {
        let pds_vec = vec![
            PropDef::new(Uuid::new_v4(), Obj::mk_id(1), Obj::mk_id(2), "test1"),
            PropDef::new(Uuid::new_v4(), Obj::mk_id(10), Obj::mk_id(12), "test2"),
            PropDef::new(Uuid::new_v4(), Obj::mk_id(100), Obj::mk_id(120), "test3"),
        ];

        let pds = PropDefs::from_items(&pds_vec);
        let mut pd_iter = pds.iter();
        let pd1 = pd_iter.next().unwrap();
        assert_eq!(pd1.uuid(), pds_vec[0].uuid());
        let pd2 = pd_iter.next().unwrap();
        assert_eq!(pd2.uuid(), pds_vec[1].uuid());
        let pd3 = pd_iter.next().unwrap();
        assert_eq!(pd3.uuid(), pds_vec[2].uuid());
        assert!(pd_iter.next().is_none());

        let pd_uuids: Vec<_> = pds.iter().map(|pd| pd.uuid()).collect();
        let pvec_uuids: Vec<_> = pds_vec.iter().map(|pd| pd.uuid()).collect();
        assert_eq!(pd_uuids, pvec_uuids);
    }
}
