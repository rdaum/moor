use crate::model::defset::{Defs, HasUuid, Named};
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{BinaryType, VerbFlag};
use crate::util::bitenum::BitEnum;
use crate::util::slice_ref::SliceRef;
use crate::util::verbname_cmp;
use crate::var::objid::Objid;
use crate::AsByteBuffer;
use bytes::BufMut;
use num_traits::FromPrimitive;
use std::ops::Range;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbDef(SliceRef);

const UUID_RANGE: Range<usize> = 0..16;
const LOCATION_RANGE: Range<usize> = 16..24;
const OWNER_RANGE: Range<usize> = 24..32;
const FLAGS_POS: usize = 32;
const BINARY_TYPE_POS: usize = 33;
const ARGS_RANGE: Range<usize> = 34..38;
const NUM_NAMES_POSITION: usize = 38;
const NAMES_START: usize = 48;

impl VerbDef {
    fn from_bytes(bytes: SliceRef) -> Self {
        Self(bytes)
    }

    pub fn new(
        uuid: Uuid,
        location: Objid,
        owner: Objid,
        names: &[&str],
        flags: BitEnum<VerbFlag>,
        binary_type: BinaryType,
        args: VerbArgsSpec,
    ) -> Self {
        // We can't know the header length because of the dynamic portion at the end. The framework
        // won't let us.
        // So we have to calculate it ourselves:
        //    uuid: 16, location: 8, owner: 8, flags: 1, binary_type: 1, args: 4, num_names: 1
        // Which ends up being 39 bytes.
        // We'll pad that out to 48 bytes to make it a nice round number and reserve some room?
        let header_size = 48;
        let num_names = names.len();
        let names_region_size = names.iter().map(|_n| num_names).sum::<usize>() + num_names;
        let total_size = header_size + names_region_size;

        let mut buffer = Vec::with_capacity(total_size);
        buffer.put_slice(uuid.as_bytes());
        buffer.put_i64_le(location.0);
        buffer.put_i64_le(owner.0);
        buffer.put_u8(flags.to_u16() as u8);
        buffer.put_u8(binary_type as u8);
        buffer.put_slice(&args.to_bytes());
        buffer.put_u8(names.len() as u8);
        // Pad out the rest of the header, future expansion.
        buffer.put_slice(&[0; 9]);
        // Now append the names.
        for name in names {
            buffer.put_u8(name.len() as u8);
            buffer.put_slice(name.as_bytes());
        }

        Self(SliceRef::new(Arc::new(buffer)))
    }

    pub fn location(&self) -> Objid {
        let slice = &self.0.as_slice()[LOCATION_RANGE];
        Objid(i64::from_le_bytes(slice.try_into().unwrap()))
    }
    pub fn owner(&self) -> Objid {
        let slice = &self.0.as_slice()[OWNER_RANGE];
        Objid(i64::from_le_bytes(slice.try_into().unwrap()))
    }
    pub fn flags(&self) -> BitEnum<VerbFlag> {
        let flag_byte = &self.0.as_slice()[FLAGS_POS];
        BitEnum::from_u8(*flag_byte)
    }
    pub fn binary_type(&self) -> BinaryType {
        let binary_type_byte = &self.0.as_slice()[BINARY_TYPE_POS];
        BinaryType::from_u8(*binary_type_byte).unwrap()
    }
    pub fn args(&self) -> VerbArgsSpec {
        let args_slice = &self.0.as_slice()[ARGS_RANGE];
        VerbArgsSpec::from_bytes(args_slice.try_into().unwrap())
    }

    pub fn names(&self) -> Vec<&str> {
        let num_names = self.0.as_slice()[NUM_NAMES_POSITION] as usize;

        let slice = &self.0.as_slice()[NAMES_START..];
        let mut position = 0;
        let mut names = Vec::with_capacity(num_names);
        for _ in 0..num_names {
            let length = slice[position];
            position += 1;
            let name_slice = &slice[position..position + length as usize];
            position += length as usize;
            names.push(std::str::from_utf8(name_slice).unwrap());
        }
        names
    }
}

impl Named for VerbDef {
    fn matches_name(&self, name: &str) -> bool {
        self.names()
            .iter()
            .any(|verb| verbname_cmp(verb.to_lowercase().as_str(), name.to_lowercase().as_str()))
    }
}

impl HasUuid for VerbDef {
    fn uuid(&self) -> Uuid {
        let uuid_bytes = &self.0.as_slice()[UUID_RANGE];
        Uuid::from_bytes(uuid_bytes.try_into().unwrap())
    }
}

impl AsByteBuffer for VerbDef {
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
        VerbDef::from_bytes(bytes)
    }
}

pub type VerbDefs = Defs<VerbDef>;

#[cfg(test)]
mod tests {
    use crate::model::defset::HasUuid;
    use crate::model::r#match::VerbArgsSpec;
    use crate::model::verbdef::{VerbDef, VerbDefs};
    use crate::model::verbs::VerbFlag;
    use crate::util::bitenum::BitEnum;
    use crate::util::slice_ref::SliceRef;
    use crate::var::objid::Objid;
    use crate::AsByteBuffer;
    use std::sync::Arc;

    #[test]
    fn test_bitflags() {
        // Just a basic sanity test
        assert!(VerbFlag::r().contains(VerbFlag::Read));
        assert!(VerbFlag::w().contains(VerbFlag::Write));
        assert!(VerbFlag::x().contains(VerbFlag::Exec));
        assert!(VerbFlag::d().contains(VerbFlag::Debug));

        assert_eq!(
            VerbFlag::rwx(),
            BitEnum::new() | VerbFlag::Read | VerbFlag::Write | VerbFlag::Exec
        );
        assert_eq!(
            VerbFlag::rwxd(),
            BitEnum::new() | VerbFlag::Read | VerbFlag::Write | VerbFlag::Exec | VerbFlag::Debug
        );
    }
    #[test]
    fn test_reconstitute() {
        let vd = VerbDef::new(
            uuid::Uuid::new_v4(),
            Objid(0),
            Objid(1),
            &["foo", "bar"],
            VerbFlag::rwxd(),
            crate::model::verbs::BinaryType::LambdaMoo18X,
            VerbArgsSpec::this_none_this(),
        );

        let bytes = vd.with_byte_buffer(|bb| bb.to_vec());
        let vd2 = VerbDef::from_sliceref(SliceRef::new(Arc::new(bytes)));

        assert_eq!(vd, vd2);
        assert_eq!(vd.uuid(), vd2.uuid());
        assert_eq!(vd.location(), Objid(0));
        assert_eq!(vd.owner(), Objid(1));
        assert_eq!(vd.names(), vec!["foo".to_string(), "bar".to_string()]);
        assert_eq!(vd.flags(), VerbFlag::rwxd());
        assert_eq!(
            vd.binary_type(),
            crate::model::verbs::BinaryType::LambdaMoo18X
        );
        assert_eq!(vd.args(), VerbArgsSpec::this_none_this(),);
    }

    #[test]
    fn test_reconstitute_in_verbdefs() {
        let vd1 = VerbDef::new(
            uuid::Uuid::new_v4(),
            Objid(0),
            Objid(1),
            &["foo", "bar"],
            VerbFlag::rwxd(),
            crate::model::verbs::BinaryType::None,
            VerbArgsSpec::this_none_this(),
        );

        let vd2 = VerbDef::new(
            uuid::Uuid::new_v4(),
            Objid(10),
            Objid(20),
            &["zoinks", "zaps", "chocolates"],
            VerbFlag::rx(),
            crate::model::verbs::BinaryType::LambdaMoo18X,
            VerbArgsSpec::this_none_this(),
        );

        let vd1_id = vd1.uuid();
        let vd2_id = vd2.uuid();

        let vds = VerbDefs::from_items(&[vd1, vd2]);
        let bytes = vds.with_byte_buffer(|bb| bb.to_vec());
        let vds2 = VerbDefs::from_sliceref(SliceRef::new(Arc::new(bytes)));
        let rvd1 = vds2.find(&vd1_id).unwrap();
        let rvd2 = vds2.find(&vd2_id).unwrap();
        assert_eq!(rvd1.uuid(), vd1_id);
        assert_eq!(rvd1.location(), Objid(0));
        assert_eq!(rvd1.owner(), Objid(1));
        assert_eq!(rvd1.names(), vec!["foo".to_string(), "bar".to_string()]);
        assert_eq!(rvd1.flags(), VerbFlag::rwxd(),);
        assert_eq!(rvd1.binary_type(), crate::model::verbs::BinaryType::None);
        assert_eq!(rvd1.args(), VerbArgsSpec::this_none_this(),);

        assert_eq!(rvd2.uuid(), vd2_id);
        assert_eq!(rvd2.location(), Objid(10));
        assert_eq!(rvd2.owner(), Objid(20));
        assert_eq!(
            rvd2.names(),
            vec![
                "zoinks".to_string(),
                "zaps".to_string(),
                "chocolates".to_string()
            ]
        );
        assert_eq!(rvd2.flags(), VerbFlag::rx());
        assert_eq!(
            rvd2.binary_type(),
            crate::model::verbs::BinaryType::LambdaMoo18X
        );
        assert_eq!(rvd2.args(), VerbArgsSpec::this_none_this());
    }

    #[test]
    fn test_empty_names() {
        let vd1 = VerbDef::new(
            uuid::Uuid::new_v4(),
            Objid(0),
            Objid(1),
            &[],
            VerbFlag::rwxd(),
            crate::model::verbs::BinaryType::None,
            VerbArgsSpec::this_none_this(),
        );

        let bytes = vd1.with_byte_buffer(|bb| bb.to_vec());
        let vd2 = VerbDef::from_sliceref(SliceRef::new(Arc::new(bytes)));
        assert_eq!(vd1, vd2);
        assert_eq!(vd1.names(), Vec::<String>::new());
    }
}
