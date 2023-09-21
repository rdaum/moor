use binary_layout::LayoutAs;
use bincode::{Decode, Encode};
use std::fmt::{Debug, Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct Objid(pub i64);

impl LayoutAs<i64> for Objid {
    fn read(v: i64) -> Self {
        Self(v)
    }

    fn write(v: Self) -> i64 {
        v.0
    }
}

impl Display for Objid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#{}", self.0))
    }
}

impl Objid {
    #[must_use]
    pub fn to_literal(&self) -> String {
        format!("#{}", self.0)
    }

    #[must_use]
    pub fn is_sysobj(&self) -> bool {
        self.0 == 0
    }
}
