// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::util::{BitEnum, BitFlag};
use byteview::ByteView;
use moor_var::{ByteSized, Obj, Symbol, Var};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[repr(u8)]
pub enum PropFlag {
    Read = 0,
    Write = 1,
    Chown = 2,
}

impl BitFlag for PropFlag {
    fn bit_index(self) -> u8 {
        self as u8
    }
}

impl PropFlag {
    #[must_use]
    pub fn all_flags() -> BitEnum<PropFlag> {
        BitEnum::new_with(PropFlag::Read) | PropFlag::Write | PropFlag::Chown
    }

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

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum PropAttr {
    Value = 0,
    Location = 1,
    Owner = 2,
    Flags = 3,
    Clear = 4,
}

#[derive(Clone, Debug)]
pub struct PropAttrs {
    pub name: Option<Symbol>,
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

const OWNER_OFFSET: usize = 0;
const FLAGS_OFFSET: usize = OWNER_OFFSET + 8;
const PROP_PERMS_SIZE: usize = FLAGS_OFFSET + 2;

fn read_owner(buf: &[u8]) -> Obj {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[OWNER_OFFSET..OWNER_OFFSET + 8]);
    Obj::from_bytes(&bytes).expect("Failed to decode owner")
}

fn write_owner(buf: &mut [u8], owner: Obj) {
    buf[OWNER_OFFSET..OWNER_OFFSET + 8].copy_from_slice(&owner.as_u64().to_le_bytes());
}

fn read_flags(buf: &[u8]) -> BitEnum<PropFlag> {
    let mut bytes = [0u8; 2];
    bytes.copy_from_slice(&buf[FLAGS_OFFSET..FLAGS_OFFSET + 2]);
    BitEnum::from_u16(u16::from_le_bytes(bytes))
}

fn write_flags(buf: &mut [u8], flags: BitEnum<PropFlag>) {
    buf[FLAGS_OFFSET..FLAGS_OFFSET + 2].copy_from_slice(&flags.to_u16().to_le_bytes());
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PropPerms(ByteView);

impl PropPerms {
    #[must_use]
    pub fn new(owner: Obj, flags: BitEnum<PropFlag>) -> Self {
        let mut buf = vec![0; PROP_PERMS_SIZE];
        write_owner(&mut buf, owner);
        write_flags(&mut buf, flags);
        Self(ByteView::from(buf))
    }

    #[must_use]
    pub fn owner(&self) -> Obj {
        read_owner(self.0.as_ref())
    }

    #[must_use]
    pub fn flags(&self) -> BitEnum<PropFlag> {
        read_flags(self.0.as_ref())
    }

    pub fn with_owner(self, owner: Obj) -> Self {
        Self::new(owner, self.flags())
    }

    pub fn with_flags(self, flags: BitEnum<PropFlag>) -> Self {
        Self::new(self.owner(), flags)
    }
}

impl AsRef<ByteView> for PropPerms {
    fn as_ref(&self) -> &ByteView {
        &self.0
    }
}

impl From<ByteView> for PropPerms {
    fn from(bytes: ByteView) -> Self {
        Self(bytes)
    }
}

impl ByteSized for PropPerms {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }
}

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
