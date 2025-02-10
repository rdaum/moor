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
use byteview::ByteView;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq)]
pub struct PropDef(ByteView);

binary_layout!(propdef, LittleEndian, {
    data_version: u8,
    uuid: [u8; 16],
    definer: Obj as i32,
    location: Obj as i32,
    name: [u8],
});

impl Clone for PropDef {
    fn clone(&self) -> Self {
        Self(self.0.to_detached().clone())
    }
}

impl PropDef {
    fn from_bytes(bytes: ByteView) -> Self {
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

        let name_buf = propdef_view.name_mut();
        name_buf[0] = name.len() as u8;
        name_buf[1..1 + name.len()].copy_from_slice(name.as_bytes());

        Self(ByteView::from(buf))
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
        let buf_len = self.0.len();
        assert!(buf_len >= names_offset);
        let names_buf = &self.0.as_ref()[names_offset..];
        let name_len = names_buf[0] as usize;
        let name_slice = names_buf[1..].get(..name_len).unwrap();
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

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        // TODO: Validate propdef on decode
        Ok(Self::from_bytes(bytes))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        Ok(self.0.to_detached())
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
    use crate::model::{HasUuid, PropDef, PropDefs, ValSet};
    use byteview::ByteView;
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
        let pds2 = PropDefs::from_bytes(ByteView::from(byte_vec)).unwrap();
        let pd2 = pds2.find_first_named(Symbol::mk("test2")).unwrap();
        assert_eq!(pd2.uuid(), test_pd2.uuid());

        assert_eq!(pd2.name(), "test2");
        assert_eq!(pd2.definer(), Obj::mk_id(10));
        assert_eq!(pd2.location(), Obj::mk_id(12));
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

        // Now write out and reconsistute.
        let pds_as_bytes = pds.as_bytes().unwrap();
        let re_pds = PropDefs::from_bytes(pds_as_bytes).unwrap();
        let re_pd_uuids: Vec<_> = re_pds.iter().map(|pd| pd.uuid()).collect();
        assert_eq!(re_pd_uuids, pvec_uuids);
    }
}
