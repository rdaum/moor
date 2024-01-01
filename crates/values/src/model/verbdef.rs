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

use crate::model::defset::{Defs, HasUuid, Named};
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{BinaryType, VerbFlag};
use crate::util::bitenum::BitEnum;
use crate::util::slice_ref::SliceRef;
use crate::util::verbname_cmp;
use crate::var::objid::Objid;
use crate::{AsByteBuffer, DATA_LAYOUT_VERSION};
use binary_layout::{define_layout, Field};
use bytes::BufMut;
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbDef(SliceRef);

define_layout!(verbdef, LittleEndian, {
    data_version: u8,
    uuid: [u8; 16],
    location: Objid as i64,
    owner: Objid as i64,
    flags: BitEnum::<VerbFlag> as u16,
    binary_type: BinaryType as u8,
    args: VerbArgsSpec as u32,
    num_names: u8,
    names: [u8],
});

impl VerbDef {
    fn from_bytes(bytes: SliceRef) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn new(
        uuid: Uuid,
        location: Objid,
        owner: Objid,
        names: &[&str],
        flags: BitEnum<VerbFlag>,
        binary_type: BinaryType,
        args: VerbArgsSpec,
    ) -> Self {
        let header_size = verbdef::names::OFFSET;
        let num_names = names.len();
        let names_region_size = names.iter().map(|n| n.len()).sum::<usize>() + num_names;
        let total_size = header_size + names_region_size;

        let mut buffer = vec![0; total_size];

        let mut verbdef_layout = verbdef::View::new(&mut buffer);
        verbdef_layout.data_version_mut().write(DATA_LAYOUT_VERSION);
        verbdef_layout.uuid_mut().copy_from_slice(uuid.as_bytes());
        verbdef_layout.location_mut().write(location);
        verbdef_layout.owner_mut().write(owner);
        verbdef_layout.flags_mut().write(flags);
        verbdef_layout.binary_type_mut().write(binary_type);
        verbdef_layout.args_mut().write(args);
        verbdef_layout.num_names_mut().write(names.len() as u8);

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
            view.data_version().read(),
            DATA_LAYOUT_VERSION,
            "Unsupported data layout version: {}",
            view.data_version().read()
        );
        view
    }

    #[must_use]
    pub fn location(&self) -> Objid {
        let view = self.get_header_view();
        view.location().read()
    }
    #[must_use]
    pub fn owner(&self) -> Objid {
        let view = self.get_header_view();
        view.owner().read()
    }
    #[must_use]
    pub fn flags(&self) -> BitEnum<VerbFlag> {
        let view = self.get_header_view();
        view.flags().read()
    }
    #[must_use]
    pub fn binary_type(&self) -> BinaryType {
        let view = self.get_header_view();
        view.binary_type().read()
    }
    #[must_use]
    pub fn args(&self) -> VerbArgsSpec {
        let view = self.get_header_view();
        view.args().read()
    }

    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        let view = self.get_header_view();
        let num_names = view.num_names().read() as usize;
        let offset = verbdef::names::OFFSET;
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
        Uuid::from_bytes(*view.uuid())
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
        Self::from_bytes(bytes)
    }

    fn as_sliceref(&self) -> SliceRef {
        self.0.clone()
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

        let bytes = vd.with_byte_buffer(<[u8]>::to_vec);
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
        let bytes = vds.with_byte_buffer(<[u8]>::to_vec);
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

        let bytes = vd1.with_byte_buffer(<[u8]>::to_vec);
        let vd2 = VerbDef::from_sliceref(SliceRef::from_vec(bytes));
        assert_eq!(vd1, vd2);
        assert_eq!(vd1.names(), Vec::<String>::new());
    }
}
