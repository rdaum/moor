use crate::model::defset::{Defs, HasUuid, Named};
use crate::model::props::PropFlag;
use crate::util::bitenum::BitEnum;
use crate::util::slice_ref::SliceRef;
use crate::var::objid::Objid;
use crate::AsByteBuffer;
use bytes::BufMut;
use std::ops::Range;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PropDef(SliceRef);

const UUID_RANGE: Range<usize> = 0..16;
const DEFINER_RANGE: Range<usize> = 16..24;
const LOCATION_RANGE: Range<usize> = 24..32;
const OWNER_RANGE: Range<usize> = 32..40;

const FLAGS_POS: usize = 40;

const NAMESTART_POS: usize = 41;

impl PropDef {
    fn from_bytes(bytes: SliceRef) -> Self {
        Self(bytes)
    }

    pub fn new(
        uuid: Uuid,
        definer: Objid,
        location: Objid,
        name: &str,
        flags: BitEnum<PropFlag>,
        owner: Objid,
    ) -> Self {
        // Buffer composition:
        //    16 bytes uuid
        //    8 definer
        //    8 location
        //    8 owner
        //    1 flag
        //    name: dynamic, 8-bit length-prefixed.
        let mut buf = Vec::with_capacity(25 + name.len());
        buf.put_slice(uuid.as_bytes());
        buf.put_i64_le(definer.0);
        buf.put_i64_le(location.0);
        buf.put_i64_le(owner.0);
        buf.put_u8(flags.to_u16() as u8);

        assert!(name.len() < 256);
        buf.put_slice(name.as_bytes());
        Self(SliceRef::new(Arc::new(buf)))
    }

    pub fn definer(&self) -> Objid {
        Objid(i64::from_le_bytes(
            self.0.as_slice()[DEFINER_RANGE].try_into().unwrap(),
        ))
    }
    pub fn location(&self) -> Objid {
        Objid(i64::from_le_bytes(
            self.0.as_slice()[LOCATION_RANGE].try_into().unwrap(),
        ))
    }
    pub fn owner(&self) -> Objid {
        Objid(i64::from_le_bytes(
            self.0.as_slice()[OWNER_RANGE].try_into().unwrap(),
        ))
    }
    pub fn flags(&self) -> BitEnum<PropFlag> {
        BitEnum::from_u8(self.0.as_slice()[FLAGS_POS])
    }

    pub fn name(&self) -> &str {
        let slice = &self.0.as_slice()[NAMESTART_POS..];
        return std::str::from_utf8(slice).unwrap();
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
        PropDef::from_bytes(bytes)
    }
}

impl Named for PropDef {
    fn matches_name(&self, name: &str) -> bool {
        self.name().to_lowercase() == name.to_lowercase().as_str()
    }
}

impl HasUuid for PropDef {
    fn uuid(&self) -> Uuid {
        Uuid::from_bytes(self.0.as_slice()[UUID_RANGE].try_into().unwrap())
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

        let bytes = test_pd.with_byte_buffer(|b| b.to_vec());
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

        let byte_vec = pds.with_byte_buffer(|b| b.to_vec());
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
