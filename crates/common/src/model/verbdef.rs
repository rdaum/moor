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

use crate::{
    model::{
        defset::{Defs, HasUuid, Named},
        r#match::VerbArgsSpec,
        verbs::VerbFlag,
    },
    util::{BitEnum, verbcasecmp},
};
use moor_var::{ByteSized, Obj, Symbol};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VerbDef {
    inner: Arc<VerbDefInner>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct VerbDefInner {
    uuid: Uuid,
    location: Obj,
    owner: Obj,
    flags: BitEnum<VerbFlag>,
    args: VerbArgsSpec,
    names: Vec<Symbol>,
}

impl VerbDef {
    #[must_use]
    pub fn new(
        uuid: Uuid,
        location: Obj,
        owner: Obj,
        names: &[Symbol],
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Self {
        Self {
            inner: Arc::new(VerbDefInner {
                uuid,
                location,
                owner,
                flags,
                args,
                names: names.to_vec(),
            }),
        }
    }

    #[must_use]
    pub fn location(&self) -> Obj {
        self.inner.location
    }

    #[must_use]
    pub fn owner(&self) -> Obj {
        self.inner.owner
    }
    #[must_use]
    pub fn flags(&self) -> BitEnum<VerbFlag> {
        self.inner.flags
    }

    #[must_use]
    pub fn args(&self) -> VerbArgsSpec {
        self.inner.args
    }

    pub fn matches_spec(
        &self,
        argspec: &Option<VerbArgsSpec>,
        flagspec: &Option<BitEnum<VerbFlag>>,
    ) -> bool {
        if let Some(argspec) = argspec
            && !self.args().matches(argspec)
        {
            return false;
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
            .any(|verb| verbcasecmp(&verb.as_arc_str(), &name.as_arc_str()))
    }

    fn names(&self) -> &[Symbol] {
        &self.inner.names
    }
}

impl HasUuid for VerbDef {
    fn uuid(&self) -> Uuid {
        self.inner.uuid
    }
}

pub type VerbDefs = Defs<VerbDef>;

impl ByteSized for VerbDef {
    fn size_bytes(&self) -> usize {
        size_of::<Uuid>()
            + self.inner.location.size_bytes()
            + self.inner.owner.size_bytes()
            + self.inner.flags.size_bytes()
            + size_of::<BitEnum<VerbFlag>>()
            + size_of::<VerbArgsSpec>()
            + self.inner.names.len() * size_of::<Symbol>()
    }
}

#[cfg(test)]
mod tests {
    use crate::{model::verbs::VerbFlag, util::BitEnum};

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
