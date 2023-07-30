use std::fmt::{Display, Formatter};

use bincode::{Decode, Encode};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct Objid(pub i64);

impl Display for Objid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#{}", self.0))
    }
}

pub const SYSTEM_OBJECT: Objid = Objid(0);
pub const NOTHING: Objid = Objid(-1);
pub const AMBIGUOUS: Objid = Objid(-2);

pub const FAILED_MATCH: Objid = Objid(-3);
