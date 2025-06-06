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
use binary_layout::binary_layout;
use bincode::{Decode, Encode};
use byteview::ByteView;
use enum_primitive_derive::Primitive;
use moor_var::Var;
use moor_var::{AsByteBuffer, DecodingError, EncodingError};
use moor_var::{Obj, Symbol};

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

binary_layout!(prop_perms_buf, LittleEndian, {
    owner: Obj as u64,
    flags: BitEnum<PropFlag> as u16,
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropPerms(ByteView);

impl PropPerms {
    #[must_use]
    pub fn new(owner: Obj, flags: BitEnum<PropFlag>) -> Self {
        let mut buf = vec![0; prop_perms_buf::SIZE.unwrap()];
        let mut view = prop_perms_buf::View::new(&mut buf);
        view.owner_mut()
            .try_write(owner)
            .expect("Failed to encode owner");
        view.flags_mut()
            .try_write(flags)
            .expect("Failed to encode flags");
        Self(ByteView::from(buf))
    }

    #[must_use]
    pub fn owner(&self) -> Obj {
        let view = prop_perms_buf::View::new(self.0.as_ref());
        view.owner().try_read().expect("Failed to decode owner")
    }

    #[must_use]
    pub fn flags(&self) -> BitEnum<PropFlag> {
        let view = prop_perms_buf::View::new(self.0.as_ref());
        view.flags().try_read().expect("Failed to decode flags")
    }

    pub fn with_owner(self, owner: Obj) -> Self {
        Self::new(owner, self.flags())
    }

    pub fn with_flags(self, flags: BitEnum<PropFlag>) -> Self {
        Self::new(self.owner(), flags)
    }
}

impl AsByteBuffer for PropPerms {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.0.as_ref()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.as_ref().to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        Ok(Self(bytes))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        Ok(self.0.clone())
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
