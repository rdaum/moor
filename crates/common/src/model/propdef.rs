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
use byteview::ByteView;
use moor_var::{
    AsByteBuffer, Obj, Symbol,
    encode::{DecodingError, EncodingError},
};
use uuid::Uuid;
use zerocopy::{FromBytes, Immutable, IntoBytes};

#[derive(Debug, Eq, PartialEq, Hash, Encode, Decode, Clone, IntoBytes, FromBytes, Immutable)]
#[repr(C)]
pub struct PropDef {
    uuid: [u8; 16],
    definer: Obj,
    location: Obj,
    name: Symbol,
}

impl PropDef {
    #[must_use]
    pub fn new(uuid: Uuid, definer: Obj, location: Obj, name: Symbol) -> Self {
        Self {
            uuid: *uuid.as_bytes(),
            definer,
            location,
            name,
        }
    }

    #[must_use]
    pub fn definer(&self) -> Obj {
        self.definer
    }
    #[must_use]
    pub fn location(&self) -> Obj {
        self.location
    }

    #[must_use]
    pub fn name(&self) -> Symbol {
        self.name
    }
}

impl Named for PropDef {
    fn matches_name(&self, name: Symbol) -> bool {
        self.name == name
    }

    fn names(&self) -> &[Symbol] {
        std::slice::from_ref(&self.name)
    }
}

impl HasUuid for PropDef {
    fn uuid(&self) -> Uuid {
        Uuid::from_bytes(self.uuid)
    }
}

pub type PropDefs = Defs<PropDef>;

impl AsByteBuffer for PropDef {
    fn size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        // Zero-copy: direct access to the struct's bytes
        Ok(f(IntoBytes::as_bytes(self)))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        // Zero-copy to Vec
        Ok(IntoBytes::as_bytes(self).to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        let bytes = bytes.as_ref();
        if bytes.len() != std::mem::size_of::<Self>() {
            return Err(DecodingError::CouldNotDecode(format!(
                "Expected {} bytes for PropDef, got {}",
                size_of::<Self>(),
                bytes.len()
            )));
        }

        // Handle potentially unaligned ByteView data safely
        // Copy to properly aligned buffer, then transmute directly
        let mut aligned_buffer = [0u8; std::mem::size_of::<Self>()];
        aligned_buffer.copy_from_slice(bytes);

        // Safe transmute using zerocopy - no additional copy
        Self::read_from_bytes(&aligned_buffer)
            .map_err(|_| DecodingError::CouldNotDecode("Invalid bytes for PropDef".to_string()))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        // Zero-copy: create ByteView directly from struct bytes
        Ok(ByteView::from(IntoBytes::as_bytes(self)))
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{HasUuid, PropDef, PropDefs, ValSet};
    use moor_var::{Obj, Symbol};
    use uuid::Uuid;

    #[test]
    fn test_in_propdefs() {
        let test_pd1 = PropDef::new(Uuid::new_v4(), Obj::mk_id(1), Obj::mk_id(2), "test".into());

        let test_pd2 = PropDef::new(
            Uuid::new_v4(),
            Obj::mk_id(10),
            Obj::mk_id(12),
            "test2".into(),
        );

        let pds = PropDefs::empty().with_all_added(&[test_pd1.clone(), test_pd2.clone()]);
        let pd1 = pds.find_first_named(Symbol::mk("test")).unwrap();
        assert_eq!(pd1.uuid(), test_pd1.uuid());
    }

    #[test]
    fn test_clone_compare() {
        let pd = PropDef::new(Uuid::new_v4(), Obj::mk_id(1), Obj::mk_id(2), "test".into());
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
            PropDef::new(Uuid::new_v4(), Obj::mk_id(1), Obj::mk_id(2), "test1".into()),
            PropDef::new(
                Uuid::new_v4(),
                Obj::mk_id(10),
                Obj::mk_id(12),
                "test2".into(),
            ),
            PropDef::new(
                Uuid::new_v4(),
                Obj::mk_id(100),
                Obj::mk_id(120),
                "test3".into(),
            ),
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
