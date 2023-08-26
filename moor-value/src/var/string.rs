use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::ops::Range;
use std::str::FromStr;
use std::sync::Arc;

use bincode::{Decode, Encode};

use crate::var::error::Error;
use crate::var::{v_err, v_str, v_string, Var};

#[derive(Clone, Encode, Decode, Ord, PartialOrd)]
pub struct Str {
    inner: Arc<String>,
}

impl Str {
    pub fn from_string(s: String) -> Self {
        Self { inner: Arc::new(s) }
    }

    pub fn get(&self, offset: usize) -> Option<Var> {
        let r = self.inner.get(offset..offset + 1);
        r.map(v_str)
    }

    pub fn set(&self, offset: usize, r: &Str) -> Var {
        if r.len() != 1 {
            return v_err(Error::E_RANGE);
        }
        if offset >= self.inner.len() {
            return v_err(Error::E_RANGE);
        }
        let mut s = self.inner.as_str().to_string();
        s.replace_range(offset..offset + 1, r.as_str());
        v_string(s)
    }

    pub fn get_range(&self, range: Range<usize>) -> Option<Var> {
        let r = self.inner.get(range);
        r.map(v_str)
    }

    pub fn append(&self, other: &Str) -> Var {
        v_string(format!("{}{}", self.inner, other.inner))
    }

    pub fn append_str(&self, other: &str) -> Var {
        v_string(format!("{}{}", self.inner, other))
    }

    pub fn append_string(&self, other: String) -> Var {
        v_string(format!("{}{}", self.inner, other))
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    pub fn substring(&self, range: Range<usize>) -> Self {
        Self {
            inner: Arc::new(self.inner[range].to_string()),
        }
    }
}

// MOO's string comparisons are all case-insensitive. To get case-sensitive you have to use
// bf_is_member and bf_strcmp.
impl PartialEq for Str {
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq_ignore_ascii_case(other.inner.as_str())
    }
}
impl Eq for Str {}

impl Hash for Str {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.to_lowercase().hash(state)
    }
}

impl FromStr for Str {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            inner: Arc::new(s.to_string()),
        })
    }
}

impl Display for Str {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.inner))
    }
}
