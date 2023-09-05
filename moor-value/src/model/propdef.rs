use crate::model::defset::{Defs, HasUuid, Named};
use crate::model::props::PropFlag;
use crate::util::bitenum::BitEnum;
use crate::util::slice_ref::SliceRef;
use crate::var::objid::Objid;
use crate::{AsByteBuffer, DATA_LAYOUT_VERSION};
use binary_layout::{define_layout, Field};
use bytes::{Buf, BufMut};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PropDef(SliceRef);

define_layout!(propdef, LittleEndian, {
    data_version: u8,
    uuid: [u8; 16],
    definer: Objid as i64,
    location: Objid as i64,
    owner: Objid as i64,
    flags: BitEnum<PropFlag> as u16,
    name: [u8],
});

impl PropDef {
    fn from_bytes(bytes: SliceRef) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn new(
        uuid: Uuid,
        definer: Objid,
        location: Objid,
        name: &str,
        flags: BitEnum<PropFlag>,
        owner: Objid,
    ) -> Self {
        let header_size = propdef::name::OFFSET;
        let mut buf = vec![0; header_size + name.len() + 8];
        let mut propdef_view = propdef::View::new(&mut buf);
        propdef_view.data_version_mut().write(DATA_LAYOUT_VERSION);
        propdef_view.uuid_mut().copy_from_slice(uuid.as_bytes());
        propdef_view.definer_mut().write(definer);
        propdef_view.location_mut().write(location);
        propdef_view.owner_mut().write(owner);
        propdef_view.flags_mut().write(flags);

        let mut name_buf = propdef_view.name_mut();
        name_buf.put_u8(name.len() as u8);
        name_buf.put_slice(name.as_bytes());

        Self(SliceRef::from_vec(buf))
    }

    fn get_layout_view(&self) -> propdef::View<&[u8]> {
        let view = propdef::View::new(self.0.as_slice());
        assert_eq!(
            view.data_version().read(),
            DATA_LAYOUT_VERSION,
            "Unsupported data layout version: {}",
            view.data_version().read()
        );
        view
    }

    #[must_use]
    pub fn definer(&self) -> Objid {
        self.get_layout_view().definer().read()
    }
    #[must_use]
    pub fn location(&self) -> Objid {
        self.get_layout_view().location().read()
    }
    #[must_use]
    pub fn owner(&self) -> Objid {
        self.get_layout_view().owner().read()
    }
    #[must_use]
    pub fn flags(&self) -> BitEnum<PropFlag> {
        self.get_layout_view().flags().read()
    }
    #[must_use]
    pub fn name(&self) -> &str {
        let names_offset = propdef::name::OFFSET;
        let mut names_buf = &self.0.as_slice()[names_offset..];
        let name_len = names_buf.get_u8() as usize;
        let name_slice = names_buf.get(..name_len).unwrap();
        return std::str::from_utf8(name_slice).unwrap();
    }
}

impl AsByteBuffer for PropDef {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> R {
        f(self.0.as_slice())
    }

    fn make_copy_as_vec(&self) -> Vec<u8> {
        self.0.as_slice().to_vec()
    }

    fn from_sliceref(bytes: SliceRef) -> Self {
        Self::from_bytes(bytes)
    }
}

impl Named for PropDef {
    fn matches_name(&self, name: &str) -> bool {
        self.name().to_lowercase() == name.to_lowercase().as_str()
    }
}

impl HasUuid for PropDef {
    fn uuid(&self) -> Uuid {
        let view = propdef::View::new(self.0.as_slice());
        Uuid::from_bytes(*view.uuid())
    }
}

pub type PropDefs = Defs<PropDef>;

#[cfg(test)]
mod tests {
    use crate::model::defset::HasUuid;
    use crate::model::propdef::{PropDef, PropDefs};
    use crate::util::bitenum::BitEnum;
    use crate::util::slice_ref::SliceRef;
    use crate::var::objid::Objid;
    use crate::AsByteBuffer;
    use uuid::Uuid;

    #[test]
    fn test_create_reconstitute() {
        let uuid = Uuid::new_v4();
        let test_pd = PropDef::new(
            uuid,
            Objid(1),
            Objid(2),
            "test",
            BitEnum::from_u8(0b101),
            Objid(3),
        );

        let re_pd = PropDef::from_bytes(test_pd.0);
        assert_eq!(re_pd.uuid(), uuid);
        assert_eq!(re_pd.definer(), Objid(1));
        assert_eq!(re_pd.location(), Objid(2));
        assert_eq!(re_pd.owner(), Objid(3));
        assert_eq!(re_pd.flags(), BitEnum::from_u8(0b101));
        assert_eq!(re_pd.name(), "test");
    }

    #[test]
    fn test_create_reconstitute_as_byte_buffer() {
        let uuid = Uuid::new_v4();
        let test_pd = PropDef::new(
            uuid,
            Objid(1),
            Objid(2),
            "test",
            BitEnum::from_u8(0b101),
            Objid(3),
        );

        let bytes = test_pd.with_byte_buffer(<[u8]>::to_vec);
        let re_pd = PropDef::from_sliceref(SliceRef::from_bytes(&bytes));
        assert_eq!(re_pd.uuid(), uuid);
        assert_eq!(re_pd.definer(), Objid(1));
        assert_eq!(re_pd.location(), Objid(2));
        assert_eq!(re_pd.owner(), Objid(3));
        assert_eq!(re_pd.flags(), BitEnum::from_u8(0b101));
        assert_eq!(re_pd.name(), "test");
    }

    #[test]
    fn test_in_propdefs() {
        let test_pd1 = PropDef::new(
            Uuid::new_v4(),
            Objid(1),
            Objid(2),
            "test",
            BitEnum::from_u8(0b101),
            Objid(3),
        );

        let test_pd2 = PropDef::new(
            Uuid::new_v4(),
            Objid(10),
            Objid(12),
            "test2",
            BitEnum::from_u8(0b101),
            Objid(13),
        );

        let pds = PropDefs::empty().with_all_added(&[test_pd1.clone(), test_pd2.clone()]);
        let pd1 = pds.find_named("test").unwrap();
        assert_eq!(pd1.uuid(), test_pd1.uuid());

        let byte_vec = pds.with_byte_buffer(<[u8]>::to_vec);
        let pds2 = PropDefs::from_sliceref(SliceRef::from_vec(byte_vec));
        let pd2 = pds2.find_named("test2").unwrap();
        assert_eq!(pd2.uuid(), test_pd2.uuid());

        assert_eq!(pd2.name(), "test2");
        assert_eq!(pd2.definer(), Objid(10));
        assert_eq!(pd2.location(), Objid(12));
        assert_eq!(pd2.owner(), Objid(13));
        assert_eq!(pd2.flags(), BitEnum::from_u8(0b101));
    }
}
