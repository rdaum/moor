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

use crate::encode::{DecodingError, EncodingError};
use crate::model::defset::{Defs, HasUuid, Named};
use crate::model::{uuid_fb, values_flatbuffers};
use crate::Objid;
use crate::Symbol;
use crate::{AsByteBuffer, DATA_LAYOUT_VERSION};
use bytes::Bytes;
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PropDef(Bytes);

impl PropDef {
    fn from_bytes(bytes: Bytes) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn new(uuid: Uuid, definer: Objid, location: Objid, name: &str) -> Self {
        let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(256);
        let name = builder.create_string(name);
        let mut pbuilder = values_flatbuffers::moor::values::PropDefBuilder::new(&mut builder);
        pbuilder.add_data_version(DATA_LAYOUT_VERSION);
        pbuilder.add_definer(definer.0);
        pbuilder.add_location(location.0);
        pbuilder.add_name(name);
        pbuilder.add_uuid(&uuid_fb(&uuid));
        let root = pbuilder.finish();
        builder.finish_minimal(root);
        let (vec, start) = builder.collapse();
        let b = Bytes::from(vec);
        let b = b.slice(start..);
        Self(b)
    }

    fn get_flatbuffer(&self) -> values_flatbuffers::moor::values::PropDef {
        let pd = flatbuffers::root::<values_flatbuffers::moor::values::PropDef>(self.0.as_ref())
            .unwrap();
        assert_eq!(pd.data_version(), DATA_LAYOUT_VERSION);
        pd
    }

    #[must_use]
    pub fn definer(&self) -> Objid {
        Objid(self.get_flatbuffer().definer())
    }

    #[must_use]
    pub fn location(&self) -> Objid {
        Objid(self.get_flatbuffer().location())
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.get_flatbuffer().name().unwrap()
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
        let fb = self.get_flatbuffer();
        let uuid = fb.uuid().unwrap();
        Uuid::from_bytes(uuid.0)
    }
}

pub type PropDefs = Defs<PropDef>;

#[cfg(test)]
mod tests {
    use crate::model::defset::HasUuid;
    use crate::model::propdef::{PropDef, PropDefs};
    use crate::model::ValSet;
    use crate::AsByteBuffer;
    use crate::Objid;
    use crate::Symbol;
    use bytes::Bytes;
    use uuid::Uuid;

    #[test]
    fn test_create_reconstitute() {
        let uuid = Uuid::new_v4();
        let test_pd = PropDef::new(uuid, Objid(1), Objid(2), "test");

        let re_pd = PropDef::from_bytes(test_pd.0);
        assert_eq!(re_pd.uuid(), uuid);
        assert_eq!(re_pd.definer(), Objid(1));
        assert_eq!(re_pd.location(), Objid(2));
        assert_eq!(re_pd.name(), "test");
    }

    #[test]
    fn test_create_reconstitute_as_byte_buffer() {
        let uuid = Uuid::new_v4();
        let test_pd = PropDef::new(uuid, Objid(1), Objid(2), "test");

        let bytes = test_pd.with_byte_buffer(<[u8]>::to_vec).unwrap();
        let re_pd = PropDef::from_bytes(bytes.into());
        assert_eq!(re_pd.uuid(), uuid);
        assert_eq!(re_pd.definer(), Objid(1));
        assert_eq!(re_pd.location(), Objid(2));
        assert_eq!(re_pd.name(), "test");
    }

    #[test]
    fn test_in_propdefs() {
        let test_pd1 = PropDef::new(Uuid::new_v4(), Objid(1), Objid(2), "test");

        let test_pd2 = PropDef::new(Uuid::new_v4(), Objid(10), Objid(12), "test2");

        let pds = PropDefs::empty().with_all_added(&[test_pd1.clone(), test_pd2.clone()]);
        let pd1 = pds.find_first_named(Symbol::mk("test")).unwrap();
        assert_eq!(pd1.uuid(), test_pd1.uuid());

        let byte_vec = pds.with_byte_buffer(<[u8]>::to_vec).unwrap();
        let pds2 = PropDefs::from_bytes(Bytes::from(byte_vec)).unwrap();
        let pd2 = pds2.find_first_named(Symbol::mk("test2")).unwrap();
        assert_eq!(pd2.uuid(), test_pd2.uuid());

        assert_eq!(pd2.name(), "test2");
        assert_eq!(pd2.definer(), Objid(10));
        assert_eq!(pd2.location(), Objid(12));
    }
}
