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

use crate::util::BitEnum;
use bincode::{Decode, Encode};
use enum_primitive_derive::Primitive;
use moor_var::Var;
use moor_var::{BincodeAsByteBufferExt, Obj};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Primitive, Encode, Decode)]
pub enum PropFlag {
    Read = 0,
    Write = 1,
    Chown = 2,
}

impl PropFlag {
    pub fn parse_str(s: &str) -> Option<BitEnum<PropFlag>> {
        let mut flags: u8 = 0;
        for c in s.chars() {
            if c == 'r' {
                flags |= 1 << PropFlag::Read as u8;
            } else if c == 'w' {
                flags |= 1 << PropFlag::Write as u8;
            } else if c == 'c' {
                flags |= 1 << PropFlag::Chown as u8;
            } else {
                return None;
            }
        }

        Some(BitEnum::from_u8(flags))
    }

    pub fn rcw() -> BitEnum<PropFlag> {
        BitEnum::new_with(PropFlag::Read) | BitEnum::new_with(PropFlag::Write)
    }

    pub fn rc() -> BitEnum<PropFlag> {
        BitEnum::new_with(PropFlag::Read) | BitEnum::new_with(PropFlag::Chown)
    }

    pub fn rw() -> BitEnum<PropFlag> {
        BitEnum::new_with(PropFlag::Write) | BitEnum::new_with(PropFlag::Chown)
    }

    pub fn r() -> BitEnum<PropFlag> {
        BitEnum::new_with(PropFlag::Read)
    }
}

pub fn prop_flags_string(flags: BitEnum<PropFlag>) -> String {
    let mut s = String::new();
    if flags.contains(PropFlag::Read) {
        s.push('r');
    }
    if flags.contains(PropFlag::Write) {
        s.push('w');
    }
    if flags.contains(PropFlag::Chown) {
        s.push('c');
    }
    s
}

#[derive(Debug, Clone, Copy, Primitive)]
pub enum PropAttr {
    Value = 0,
    Location = 1,
    Owner = 2,
    Flags = 3,
    Clear = 4,
}

#[derive(Clone, Debug)]
pub struct PropAttrs {
    pub name: Option<String>,
    pub value: Option<Var>,
    pub location: Option<Obj>,
    pub owner: Option<Obj>,
    pub flags: Option<BitEnum<PropFlag>>,
}

impl PropAttrs {
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: None,
            name: None,
            location: None,
            owner: None,
            flags: None,
        }
    }
}

impl Default for PropAttrs {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PropPerms {
    owner: Obj,
    flags: BitEnum<PropFlag>,
}

impl PropPerms {
    #[must_use]
    pub fn new(owner: Obj, flags: BitEnum<PropFlag>) -> Self {
        Self { owner, flags }
    }

    #[must_use]
    pub fn owner(&self) -> Obj {
        self.owner.clone()
    }

    #[must_use]
    pub fn flags(&self) -> BitEnum<PropFlag> {
        self.flags.clone()
    }

    pub fn with_owner(self, owner: Obj) -> Self {
        Self::new(owner, self.flags())
    }

    pub fn with_flags(self, flags: BitEnum<PropFlag>) -> Self {
        Self::new(self.owner(), flags)
    }
}

impl BincodeAsByteBufferExt for PropPerms {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_make_get() {
        let pperms = PropPerms::new(Obj::mk_id(1), PropFlag::rc());
        assert_eq!(pperms.owner(), Obj::mk_id(1));
        assert_eq!(pperms.flags(), PropFlag::rc());
    }
}
