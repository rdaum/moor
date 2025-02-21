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

use crate::Obj;
use crate::Symbol;
use crate::encode::{DecodingError, EncodingError};
use crate::model::defset::{Defs, HasUuid, Named};
use crate::{AsByteBuffer, DATA_LAYOUT_VERSION};
use binary_layout::{Field, binary_layout};
use bytes::Bytes;
use bytes::{Buf, BufMut};
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PropDef(Bytes);

binary_layout!(propdef, LittleEndian, {
    data_version: u8,
    uuid: [u8; 16],
    definer: Obj as i32,
    location: Obj as i32,
    name: [u8],
});

impl PropDef {
    fn from_bytes(bytes: Bytes) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn new(uuid: Uuid, definer: Obj, location: Obj, name: &str) -> Self {
        let header_size = propdef::name::OFFSET;
        let mut buf = vec![0; header_size + name.len() + 8];
        let mut propdef_view = propdef::View::new(&mut buf);
        propdef_view.data_version_mut().write(DATA_LAYOUT_VERSION);
        propdef_view.uuid_mut().copy_from_slice(uuid.as_bytes());
        propdef_view
            .definer_mut()
            .try_write(definer)
            .expect("Failed to encode definer");
        propdef_view
            .location_mut()
            .try_write(location)
            .expect("Failed to encode location");

        let mut name_buf = propdef_view.name_mut();
        name_buf.put_u8(name.len() as u8);
        name_buf.put_slice(name.as_bytes());

        Self(Bytes::from(buf))
    }

    fn get_layout_view(&self) -> propdef::View<&[u8]> {
        let view = propdef::View::new(self.0.as_ref());
        assert_eq!(
            view.data_version().read(),
            DATA_LAYOUT_VERSION,
            "Unsupported data layout version: {}",
            view.data_version().read()
        );
        view
    }

    #[must_use]
    pub fn definer(&self) -> Obj {
        self.get_layout_view()
            .definer()
            .try_read()
            .expect("Failed to decode definer")
    }
    #[must_use]
    pub fn location(&self) -> Obj {
        self.get_layout_view()
            .location()
            .try_read()
            .expect("Failed to decode location")
    }

    #[must_use]
    pub fn name(&self) -> &str {
        let names_offset = propdef::name::OFFSET;
        let mut names_buf = &self.0.as_ref()[names_offset..];
        let name_len = names_buf.get_u8() as usize;
        let name_slice = names_buf.get(..name_len).unwrap();
        std::str::from_utf8(name_slice).unwrap()
    }
}

impl AsByteBuffer for PropDef {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.0.as_ref()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.as_ref().to_vec())
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError> {
        // TODO: Validate propdef on decode
        Ok(Self::from_bytes(bytes))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(self.0.clone())
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
        let view = propdef::View::new(self.0.as_ref());
        Uuid::from_bytes(*view.uuid())
    }
}

pub type PropDefs = Defs<PropDef>;

#[cfg(test)]
mod tests {
    use crate::AsByteBuffer;
    use crate::Obj;
    use crate::Symbol;
    use crate::model::ValSet;
    use crate::model::defset::HasUuid;
    use crate::model::propdef::{PropDef, PropDefs};
    use bytes::Bytes;
    use uuid::Uuid;

    #[test]
    fn test_create_reconstitute() {
        let uuid = Uuid::new_v4();
        let test_pd = PropDef::new(uuid, Obj::mk_id(1), Obj::mk_id(2), "test");

        let re_pd = PropDef::from_bytes(test_pd.0);
        assert_eq!(re_pd.uuid(), uuid);
        assert_eq!(re_pd.definer(), Obj::mk_id(1));
        assert_eq!(re_pd.location(), Obj::mk_id(2));
        assert_eq!(re_pd.name(), "test");
    }

    #[test]
    fn test_create_reconstitute_as_byte_buffer() {
        let uuid = Uuid::new_v4();
        let test_pd = PropDef::new(uuid, Obj::mk_id(1), Obj::mk_id(2), "test");

        let bytes = test_pd.with_byte_buffer(<[u8]>::to_vec).unwrap();
        let re_pd = PropDef::from_bytes(bytes.into());
        assert_eq!(re_pd.uuid(), uuid);
        assert_eq!(re_pd.definer(), Obj::mk_id(1));
        assert_eq!(re_pd.location(), Obj::mk_id(2));
        assert_eq!(re_pd.name(), "test");
    }

    #[test]
    fn test_in_propdefs() {
        let test_pd1 = PropDef::new(Uuid::new_v4(), Obj::mk_id(1), Obj::mk_id(2), "test");

        let test_pd2 = PropDef::new(Uuid::new_v4(), Obj::mk_id(10), Obj::mk_id(12), "test2");

        let pds = PropDefs::empty().with_all_added(&[test_pd1.clone(), test_pd2.clone()]);
        let pd1 = pds.find_first_named(Symbol::mk("test")).unwrap();
        assert_eq!(pd1.uuid(), test_pd1.uuid());

        let byte_vec = pds.with_byte_buffer(<[u8]>::to_vec).unwrap();
        let pds2 = PropDefs::from_bytes(Bytes::from(byte_vec)).unwrap();
        let pd2 = pds2.find_first_named(Symbol::mk("test2")).unwrap();
        assert_eq!(pd2.uuid(), test_pd2.uuid());

        assert_eq!(pd2.name(), "test2");
        assert_eq!(pd2.definer(), Obj::mk_id(10));
        assert_eq!(pd2.location(), Obj::mk_id(12));
    }
}
