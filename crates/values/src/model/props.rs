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

use crate::util::BitEnum;
use crate::var::Objid;
use crate::var::Var;
use crate::{AsByteBuffer, DecodingError, EncodingError};
use binary_layout::binary_layout;
use bincode::{Decode, Encode};
use daumtils::SliceRef;
use enum_primitive_derive::Primitive;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Primitive, Encode, Decode)]
pub enum PropFlag {
    Read = 0,
    Write = 1,
    Chown = 2,
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
    pub location: Option<Objid>,
    pub owner: Option<Objid>,
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
    owner: Objid as i64,
    flags: BitEnum<PropFlag> as u16,
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropPerms(SliceRef);

impl PropPerms {
    #[must_use]
    pub fn new(owner: Objid, flags: BitEnum<PropFlag>) -> Self {
        let mut buf = vec![0; prop_perms_buf::SIZE.unwrap()];
        let mut view = prop_perms_buf::View::new(&mut buf);
        view.owner_mut()
            .try_write(owner)
            .expect("Failed to encode owner");
        view.flags_mut()
            .try_write(flags)
            .expect("Failed to encode flags");
        Self(SliceRef::from_vec(buf))
    }

    #[must_use]
    pub fn owner(&self) -> Objid {
        let view = prop_perms_buf::View::new(self.0.as_slice());
        view.owner().try_read().expect("Failed to decode owner")
    }

    #[must_use]
    pub fn flags(&self) -> BitEnum<PropFlag> {
        let view = prop_perms_buf::View::new(self.0.as_slice());
        view.flags().try_read().expect("Failed to decode flags")
    }

    pub fn with_owner(self, owner: Objid) -> Self {
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
        Ok(f(self.0.as_slice()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.as_slice().to_vec())
    }

    fn from_sliceref(bytes: SliceRef) -> Result<Self, DecodingError> {
        Ok(Self(bytes))
    }

    fn as_sliceref(&self) -> Result<SliceRef, EncodingError> {
        Ok(self.0.clone())
    }
}
