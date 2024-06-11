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

use binary_layout::{binary_layout, Field};
use std::fmt::{Display, Formatter};

use crate::{AsByteBuffer, DecodingError, EncodingError, NOTHING};
use bincode::{Decode, Encode};
use daumtils::SliceRef;
use enum_primitive_derive::Primitive;

use crate::util::BitEnum;
use crate::var::Objid;

#[derive(Debug, Ord, PartialOrd, Copy, Clone, Eq, PartialEq, Hash, Primitive, Encode, Decode)]
pub enum ObjFlag {
    User = 0,
    Programmer = 1,
    Wizard = 2,
    Obsolete1 = 3,
    Read = 4,
    Write = 5,
    Obsolete2 = 6,
    Fertile = 8,
}

// The set of built-in object attributes
#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash, Primitive, Decode, Encode)]
pub enum ObjAttr {
    Owner = 0,
    Name = 1,
    Parent = 2,
    Location = 3,
    Flags = 4,
}
impl Display for ObjAttr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owner => f.write_str("owner"),
            Self::Name => f.write_str("name"),
            Self::Parent => f.write_str("parent"),
            Self::Location => f.write_str("location"),
            Self::Flags => f.write_str("flags"),
        }
    }
}

binary_layout!(objattrs_buf, LittleEndian, {
    owner: Objid as i64,
    parent: Objid as i64,
    location: Objid as i64,
    flags: BitEnum<ObjFlag> as u16,
    name: [u8],
});

#[derive(Debug, Clone)]
pub struct ObjAttrs(SliceRef);

impl ObjAttrs {
    #[must_use]
    pub fn empty() -> Self {
        let mut buffer = vec![0; objattrs_buf::name::OFFSET];
        let mut objattrs_view = objattrs_buf::View::new(&mut buffer);
        objattrs_view
            .owner_mut()
            .try_write(NOTHING)
            .expect("Failed to encode owner");
        objattrs_view
            .parent_mut()
            .try_write(NOTHING)
            .expect("Failed to encode parent");
        objattrs_view
            .location_mut()
            .try_write(NOTHING)
            .expect("Failed to encode location");
        objattrs_view
            .flags_mut()
            .try_write(BitEnum::new())
            .expect("Failed to encode flags");

        Self(SliceRef::from_vec(buffer))
    }

    pub fn new(
        owner: Objid,
        parent: Objid,
        location: Objid,
        flags: BitEnum<ObjFlag>,
        name: &str,
    ) -> Self {
        let header_size = objattrs_buf::name::OFFSET;
        let name_bytes = name.as_bytes();
        let mut buf = vec![0; header_size + name_bytes.len()];
        let mut objattrs_view = objattrs_buf::View::new(&mut buf);
        objattrs_view
            .owner_mut()
            .try_write(owner)
            .expect("Failed to encode owner");
        objattrs_view
            .parent_mut()
            .try_write(parent)
            .expect("Failed to encode parent");
        objattrs_view
            .location_mut()
            .try_write(location)
            .expect("Failed to encode location");
        objattrs_view
            .flags_mut()
            .try_write(flags)
            .expect("Failed to encode flags");

        buf[header_size..].copy_from_slice(name_bytes);

        Self(SliceRef::from_vec(buf))
    }

    pub fn owner(&self) -> Option<Objid> {
        let objattrs_view = objattrs_buf::View::new(self.0.as_slice());
        let oid = objattrs_view.owner().try_read().unwrap();
        if oid == NOTHING {
            None
        } else {
            Some(oid)
        }
    }

    pub fn set_owner(&mut self, o: Objid) -> &mut Self {
        let mut buffer_as_vec = self.0.as_slice().to_vec();
        let mut objattrs_view = objattrs_buf::View::new(&mut buffer_as_vec);
        objattrs_view
            .owner_mut()
            .try_write(o)
            .expect("Failed to encode owner");
        self.0 = SliceRef::from_vec(buffer_as_vec);
        self
    }

    pub fn location(&self) -> Option<Objid> {
        let objattrs_view = objattrs_buf::View::new(self.0.as_slice());
        let oid = objattrs_view.location().try_read().unwrap();
        if oid == NOTHING {
            None
        } else {
            Some(oid)
        }
    }

    pub fn set_location(&mut self, o: Objid) -> &mut Self {
        let mut buffer_as_vec = self.0.as_slice().to_vec();
        let mut objattrs_view = objattrs_buf::View::new(&mut buffer_as_vec);
        objattrs_view
            .location_mut()
            .try_write(o)
            .expect("Failed to encode location");
        self.0 = SliceRef::from_vec(buffer_as_vec);
        self
    }

    pub fn parent(&self) -> Option<Objid> {
        let objattrs_view = objattrs_buf::View::new(self.0.as_slice());
        let oid = objattrs_view.parent().try_read().unwrap();
        if oid == NOTHING {
            None
        } else {
            Some(oid)
        }
    }

    pub fn set_parent(&mut self, o: Objid) -> &mut Self {
        let mut buffer_as_vec = self.0.as_slice().to_vec();
        let mut objattrs_view = objattrs_buf::View::new(&mut buffer_as_vec);
        objattrs_view
            .parent_mut()
            .try_write(o)
            .expect("Failed to encode parent");
        self.0 = SliceRef::from_vec(buffer_as_vec);
        self
    }

    pub fn flags(&self) -> BitEnum<ObjFlag> {
        let objattrs_view = objattrs_buf::View::new(self.0.as_slice());
        objattrs_view.flags().try_read().unwrap()
    }

    pub fn set_flags(&mut self, flags: BitEnum<ObjFlag>) -> &mut Self {
        let mut buffer_as_vec = self.0.as_slice().to_vec();
        let mut objattrs_view = objattrs_buf::View::new(&mut buffer_as_vec);
        objattrs_view
            .flags_mut()
            .try_write(flags)
            .expect("Failed to encode flags");
        self.0 = SliceRef::from_vec(buffer_as_vec);
        self
    }

    pub fn name(&self) -> Option<String> {
        if self.0.len() == objattrs_buf::name::OFFSET {
            return None;
        }
        let objattrs_view = objattrs_buf::View::new(self.0.as_slice());
        objattrs_view.name().to_vec();
        Some(String::from_utf8(objattrs_view.name().to_vec()).unwrap())
    }

    pub fn set_name(&mut self, s: &str) -> &mut Self {
        let mut buffer_as_vec = self.0.as_slice().to_vec();
        let name_as_vec = s.as_bytes().to_vec();
        buffer_as_vec.extend_from_slice(&name_as_vec);
        self.0 = SliceRef::from_vec(buffer_as_vec);
        self
    }
}

impl Default for ObjAttrs {
    fn default() -> Self {
        Self::empty()
    }
}

impl AsByteBuffer for ObjAttrs {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.0.as_slice()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.as_slice().to_vec())
    }

    fn from_sliceref(bytes: SliceRef) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        Ok(Self(bytes))
    }

    fn as_sliceref(&self) -> Result<SliceRef, EncodingError> {
        Ok(self.0.clone())
    }
}
