use bincode::{Decode, Encode};
use std::fmt::{Debug, Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct Objid(pub i64);

impl Display for Objid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#{}", self.0))
    }
}

impl Objid {
    pub fn to_literal(&self) -> String {
        format!("#{}", self.0)
    }
}
