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
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{BinaryType, VerbFlag};
use crate::model::{uuid_fb, values_flatbuffers, ArgSpec, PrepSpec, Preposition};
use crate::util::verbname_cmp;
use crate::util::BitEnum;
use crate::Objid;
use crate::Symbol;
use crate::{AsByteBuffer, DATA_LAYOUT_VERSION};
use bytes::Bytes;
use num_traits::FromPrimitive;
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbDef(Bytes);

fn pack_arg_spec(s: &ArgSpec) -> values_flatbuffers::moor::values::ArgSpec {
    match s {
        ArgSpec::None => values_flatbuffers::moor::values::ArgSpec::None,
        ArgSpec::Any => values_flatbuffers::moor::values::ArgSpec::Any,
        ArgSpec::This => values_flatbuffers::moor::values::ArgSpec::This,
    }
}

fn unpack_arg_spec(s: values_flatbuffers::moor::values::ArgSpec) -> ArgSpec {
    match s {
        values_flatbuffers::moor::values::ArgSpec::None => ArgSpec::None,
        values_flatbuffers::moor::values::ArgSpec::Any => ArgSpec::Any,
        values_flatbuffers::moor::values::ArgSpec::This => ArgSpec::This,
        _ => panic!("Invalid ArgSpec"),
    }
}
fn pack_prep(p: &PrepSpec) -> i8 {
    match p {
        PrepSpec::Any => -2,
        PrepSpec::None => -1,
        PrepSpec::Other(p) => (*p as u16) as i8,
    }
}

fn unpack_prep(p: i8) -> PrepSpec {
    match p {
        -2 => PrepSpec::Any,
        -1 => PrepSpec::None,
        p => PrepSpec::Other(Preposition::from_repr(p as u16).unwrap()),
    }
}

impl VerbDef {
    fn from_bytes(bytes: Bytes) -> Self {
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
        let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(256);
        let name_strings = names
            .iter()
            .map(|name| builder.create_string(name))
            .collect::<Vec<_>>();
        let names = builder.create_vector(&name_strings);
        let mut vbuilder = values_flatbuffers::moor::values::VerbDefBuilder::new(&mut builder);
        vbuilder.add_data_version(DATA_LAYOUT_VERSION);
        vbuilder.add_uuid(&uuid_fb(&uuid));
        vbuilder.add_location(location.0);
        vbuilder.add_owner(owner.0);
        vbuilder.add_flags(flags.to_u16() as u8);
        vbuilder.add_binary_type(binary_type as u8);
        vbuilder.add_names(names);

        let args_builder = values_flatbuffers::moor::values::VerbArgsSpec::new(
            pack_arg_spec(&args.dobj),
            pack_prep(&args.prep),
            pack_arg_spec(&args.iobj),
        );
        vbuilder.add_args(&args_builder);
        let root = vbuilder.finish();
        builder.finish_minimal(root);
        let (vec, start) = builder.collapse();
        let b = Bytes::from(vec);
        let b = b.slice(start..);
        Self(b)
    }

    pub(crate) fn get_flatbuffer(&self) -> values_flatbuffers::moor::values::VerbDef {
        let vd = flatbuffers::root::<values_flatbuffers::moor::values::VerbDef>(self.0.as_ref())
            .unwrap();
        assert_eq!(vd.data_version(), DATA_LAYOUT_VERSION);
        vd
    }

    #[must_use]
    pub fn location(&self) -> Objid {
        Objid(self.get_flatbuffer().location())
    }

    #[must_use]
    pub fn owner(&self) -> Objid {
        Objid(self.get_flatbuffer().owner())
    }
    #[must_use]
    pub fn flags(&self) -> BitEnum<VerbFlag> {
        let flags = self.get_flatbuffer().flags();
        BitEnum::from_u8(flags)
    }
    #[must_use]
    pub fn binary_type(&self) -> BinaryType {
        let binary_type = self.get_flatbuffer().binary_type();
        BinaryType::from_u8(binary_type).expect("Invalid binary type")
    }

    #[must_use]
    pub fn args(&self) -> VerbArgsSpec {
        let args = self.get_flatbuffer().args().unwrap();
        VerbArgsSpec {
            dobj: unpack_arg_spec(args.dobj()),
            prep: unpack_prep(args.prep()),
            iobj: unpack_arg_spec(args.iobj()),
        }
    }
}

impl Named for VerbDef {
    fn matches_name(&self, name: Symbol) -> bool {
        self.names()
            .iter()
            .any(|verb| verbname_cmp(verb.to_lowercase().as_str(), name.as_str()))
    }

    #[must_use]
    fn names(&self) -> Vec<&str> {
        self.get_flatbuffer()
            .names()
            .map(|names| names.iter().collect())
            .unwrap_or_else(Vec::new)
    }
}

impl HasUuid for VerbDef {
    fn uuid(&self) -> Uuid {
        let fb = self.get_flatbuffer();
        let uuid = fb.uuid().unwrap();
        Uuid::from_bytes(uuid.0)
    }
}

impl AsByteBuffer for VerbDef {
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
        // TODO: Validate VerbDef on decode
        Ok(Self::from_bytes(bytes))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(self.0.clone())
    }
}

pub type VerbDefs = Defs<VerbDef>;

#[cfg(test)]
mod tests {
    use crate::model::defset::{HasUuid, Named};
    use crate::model::r#match::VerbArgsSpec;
    use crate::model::verbdef::{VerbDef, VerbDefs};
    use crate::model::verbs::VerbFlag;
    use crate::model::ValSet;
    use crate::util::BitEnum;
    use crate::AsByteBuffer;
    use crate::Objid;
    use bytes::Bytes;

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

        let bytes = vd.0.clone();
        let vd2 = VerbDef::from_bytes(bytes);

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
        let bytes = vds.with_byte_buffer(<[u8]>::to_vec).unwrap();
        let vds2 = VerbDefs::from_bytes(Bytes::from(bytes)).unwrap();
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

        let bytes = vd1.with_byte_buffer(<[u8]>::to_vec).unwrap();
        let vd2 = VerbDef::from_bytes(Bytes::from(bytes));
        assert_eq!(vd1, vd2);
        assert_eq!(vd1.names(), Vec::<String>::new());
    }
}
