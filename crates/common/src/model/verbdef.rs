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
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{BinaryType, VerbFlag};
use crate::util::BitEnum;
use crate::util::verbname_cmp;
use bincode::{Decode, Encode};
use moor_var::BincodeAsByteBufferExt;
use moor_var::Obj;
use moor_var::Symbol;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Encode, Decode)]
pub struct VerbDef {
    #[bincode(with_serde)]
    uuid: Uuid,
    location: Obj,
    owner: Obj,
    flags: BitEnum<VerbFlag>,
    binary_type: BinaryType,
    args: VerbArgsSpec,
    names: Vec<Symbol>,
}

impl VerbDef {
    #[must_use]
    pub fn new(
        uuid: Uuid,
        location: Obj,
        owner: Obj,
        names: &[&str],
        flags: BitEnum<VerbFlag>,
        binary_type: BinaryType,
        args: VerbArgsSpec,
    ) -> Self {
        Self {
            uuid,
            location,
            owner,
            flags,
            binary_type,
            args,
            names: names.iter().map(|s| (*s).into()).collect(),
        }
    }

    #[must_use]
    pub fn location(&self) -> Obj {
        self.location.clone()
    }

    #[must_use]
    pub fn owner(&self) -> Obj {
        self.owner.clone()
    }
    #[must_use]
    pub fn flags(&self) -> BitEnum<VerbFlag> {
        self.flags
    }

    #[must_use]
    pub fn binary_type(&self) -> BinaryType {
        self.binary_type
    }
    #[must_use]
    pub fn args(&self) -> VerbArgsSpec {
        self.args
    }

    pub fn matches_spec(
        &self,
        argspec: &Option<VerbArgsSpec>,
        flagspec: &Option<BitEnum<VerbFlag>>,
    ) -> bool {
        if let Some(argspec) = argspec {
            if !self.args().matches(argspec) {
                return false;
            }
        }
        if let Some(flagspec) = flagspec {
            return self.flags().contains_all(*flagspec);
        }

        true
    }
}

impl Named for VerbDef {
    fn matches_name(&self, name: Symbol) -> bool {
        self.names()
            .iter()
            .any(|verb| verbname_cmp(verb.to_lowercase().as_str(), name.as_str()))
    }

    #[must_use]
    fn names(&self) -> Vec<&str> {
        self.names.iter().map(|s| s.as_str()).collect()
    }
}

impl HasUuid for VerbDef {
    fn uuid(&self) -> Uuid {
        self.uuid
    }
}

impl BincodeAsByteBufferExt for VerbDef {}

pub type VerbDefs = Defs<VerbDef>;

#[cfg(test)]
mod tests {
    use crate::model::verbs::VerbFlag;
    use crate::util::BitEnum;

    #[test]
    fn test_bitflags() {
        // Just a basic sanity test
        assert!(VerbFlag::r().contains(VerbFlag::Read));
        assert!(VerbFlag::w().contains(VerbFlag::Write));
        assert!(VerbFlag::x().contains(VerbFlag::Exec));
        assert!(VerbFlag::d().contains(VerbFlag::Debug));

        assert_eq!(
            VerbFlag::rwx(),
            BitEnum::new() | VerbFlag::Read | VerbFlag::Write | VerbFlag::Exec
        );
        assert_eq!(
            VerbFlag::rwxd(),
            BitEnum::new() | VerbFlag::Read | VerbFlag::Write | VerbFlag::Exec | VerbFlag::Debug
        );
    }
}
