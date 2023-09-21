use crate::model::r#match::VerbArgsSpec;
use crate::util::bitenum::BitEnum;
use crate::var::objid::Objid;
use binary_layout::LayoutAs;
use bincode::{Decode, Encode};
use enum_primitive_derive::Primitive;
use num_traits::FromPrimitive;

#[derive(Debug, Ord, PartialOrd, Copy, Clone, Eq, PartialEq, Hash, Primitive, Encode, Decode)]
pub enum VerbFlag {
    Read = 0,
    Write = 1,
    Exec = 2,
    Debug = 3,
}

impl LayoutAs<u8> for VerbFlag {
    fn read(v: u8) -> Self {
        Self::from_u8(v).unwrap()
    }

    fn write(v: Self) -> u8 {
        v as u8
    }
}

impl VerbFlag {
    #[must_use]
    pub fn rwxd() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read) | Self::Write | Self::Exec | Self::Debug
    }
    #[must_use]
    pub fn rwx() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read) | Self::Write | Self::Exec
    }
    #[must_use]
    pub fn rw() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read) | Self::Write
    }
    #[must_use]
    pub fn rx() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read) | Self::Exec
    }
    #[must_use]
    pub fn rxd() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read) | Self::Exec | Self::Debug
    }
    #[must_use]
    pub fn r() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read)
    }

    #[must_use]
    pub fn w() -> BitEnum<Self> {
        BitEnum::new_with(Self::Write)
    }
    #[must_use]
    pub fn x() -> BitEnum<Self> {
        BitEnum::new_with(Self::Exec)
    }
    #[must_use]
    pub fn d() -> BitEnum<Self> {
        BitEnum::new_with(Self::Debug)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct Vid(pub i64);

#[derive(Clone, Copy, Debug, Primitive)]
pub enum VerbAttr {
    Definer = 0,
    Owner = 1,
    Flags = 2,
    ArgsSpec = 3,
    Binary = 4,
}

/// The program type encoded for a verb.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Encode, Decode, Primitive)]
#[repr(u8)]
pub enum BinaryType {
    /// For builtin functions in stack frames -- or empty code blobs.
    None = 0,
    /// Opcodes match almost 1:1 with LambdaMOO 1.8.x, but is not "binary" compatible.
    LambdaMoo18X = 1,
}

impl LayoutAs<u8> for BinaryType {
    fn read(v: u8) -> Self {
        Self::from_u8(v).unwrap()
    }

    fn write(v: Self) -> u8 {
        v as u8
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, Decode)]
pub struct VerbAttrs {
    pub definer: Option<Objid>,
    pub owner: Option<Objid>,
    pub names: Option<Vec<String>>,
    pub flags: Option<BitEnum<VerbFlag>>,
    pub args_spec: Option<VerbArgsSpec>,
    pub binary_type: Option<BinaryType>,
    pub binary: Option<Vec<u8>>,
}
