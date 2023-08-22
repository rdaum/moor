use crate::model::defset::{Defs, HasUuid, Named};
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{BinaryType, VerbFlag};
use crate::util::bitenum::BitEnum;
use crate::util::slice_ref::SliceRef;
use crate::util::verbname_cmp;
use crate::var::objid::Objid;
use crate::{AsByteBuffer, DATA_LAYOUT_VERSION};
use binary_layout::define_layout;
use bytes::BufMut;
use num_traits::FromPrimitive;
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbDef(SliceRef);

define_layout!(verbdef_header, LittleEndian, {
    data_version: u8,
    uuid: [u8; 16],
    location: i64,
    owner: i64,
    flags: u8,
    binary_type: u8,
    args: [u8; 4],
    num_names: u8,
});

define_layout!(verbdef, LittleEndian, {
    header: verbdef_header::NestedView,
    names: [u8],
});

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
        let header_size = verbdef_header::SIZE.unwrap();
        let num_names = names.len();
        let names_region_size = names.iter().map(|n| n.len()).sum::<usize>() + num_names;
        let total_size = header_size + names_region_size;

        let mut buffer = vec![0; total_size];

        let mut verbdef_layout = verbdef::View::new(&mut buffer);
        let mut header_view = verbdef_layout.header_mut();
        header_view.data_version_mut().write(DATA_LAYOUT_VERSION);
        header_view.uuid_mut().copy_from_slice(uuid.as_bytes());
        header_view.location_mut().write(location.0);
        header_view.owner_mut().write(owner.0);
        header_view.flags_mut().write(flags.to_u16() as u8);
        header_view.binary_type_mut().write(binary_type as u8);
        header_view.args_mut().copy_from_slice(&args.to_bytes());
        header_view.num_names_mut().write(names.len() as u8);

        // Now write the names, into the names region.
        let mut names_buf = verbdef_layout.names_mut();
        for name in names {
            names_buf.put_u8(name.len() as u8);
            names_buf.put_slice(name.as_bytes());
        }

        Self(SliceRef::from_vec(buffer))
    }

    fn get_header_view(&self) -> verbdef::View<&[u8]> {
        let view = verbdef::View::new(self.0.as_slice());
        assert_eq!(
            view.header().data_version().read(),
            DATA_LAYOUT_VERSION,
            "Unsupported data layout version: {}",
            view.header().data_version().read()
        );
        view
    }

    pub fn location(&self) -> Objid {
        let view = self.get_header_view();
        Objid(view.header().location().read())
    }
    pub fn owner(&self) -> Objid {
        let view = self.get_header_view();
        Objid(view.header().owner().read())
    }
    pub fn flags(&self) -> BitEnum<VerbFlag> {
        let view = self.get_header_view();
        BitEnum::from_u8(view.header().flags().read())
    }
    pub fn binary_type(&self) -> BinaryType {
        let view = self.get_header_view();
        BinaryType::from_u8(view.header().binary_type().read()).unwrap()
    }
    pub fn args(&self) -> VerbArgsSpec {
        let view = self.get_header_view();
        VerbArgsSpec::from_bytes(*view.header().args())
    }

    pub fn names(&self) -> Vec<&str> {
        let view = self.get_header_view();
        let num_names = view.header().num_names().read() as usize;
        let offset = verbdef_header::SIZE.unwrap();
        let slice = &self.0.as_slice()[offset..];
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
        let view = self.get_header_view();
        Uuid::from_bytes(*view.header().uuid())
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
        let vd2 = VerbDef::from_sliceref(SliceRef::from_vec(bytes));

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
        let vds2 = VerbDefs::from_sliceref(SliceRef::from_vec(bytes));
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
        let vd2 = VerbDef::from_sliceref(SliceRef::from_vec(bytes));
        assert_eq!(vd1, vd2);
        assert_eq!(vd1.names(), Vec::<String>::new());
    }
}
