// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::var::var::{v_str, Var};
use crate::var::variant::Variant;
use crate::var::Error;
use crate::var::Error::{E_INVARG, E_RANGE, E_TYPE};
use crate::var::Sequence;
use bincode::{Decode, Encode};
use num_traits::ToPrimitive;
use std::cmp::max;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::sync::Arc;

#[derive(Clone, Encode, Decode)]
pub struct Str(Arc<String>);

impl Str {
    pub fn mk_str(s: &str) -> Self {
        Str(Arc::new(s.to_string()))
    }

    pub fn as_string(&self) -> &String {
        self.0.as_ref()
    }

    pub fn index_set(&self, index: usize, value: &Self) -> Result<Var, Error> {
        if value.len() != 1 {
            return Err(E_INVARG);
        }
        let string = self.0.as_ref();
        if index >= string.len() {
            return Err(E_RANGE);
        }
        let mut s = string.clone();
        s.replace_range(index..=index, value.as_string());
        let s = Str(Arc::new(s));
        let v = Variant::Str(s);
        Ok(Var::from_variant(v))
    }

    pub fn append(&self, other: &Self) -> Var {
        let mut s = self.0.as_ref().clone();
        s.push_str(other.as_string());
        let s = Str(Arc::new(s));
        let v = Variant::Str(s);
        Var::from_variant(v)
    }
}

impl Sequence for Str {
    fn is_empty(&self) -> bool {
        self.as_string().is_empty()
    }

    fn len(&self) -> usize {
        self.as_string().len()
    }

    fn index(&self, index: usize) -> Result<Var, Error> {
        if index >= self.as_string().len() {
            return Err(E_RANGE);
        }
        let c = self.as_string().chars().nth(index).unwrap();
        let c_str = c.to_string();
        let v = Var::mk_str(&c_str);
        Ok(v)
    }

    fn index_set(&self, index: usize, value: &Var) -> Result<Var, Error> {
        if index >= self.as_string().len() {
            return Err(E_RANGE);
        }

        // Index set for strings requires that the `value` being set is a string, otherwise it's.
        // E_TYPE.
        // And it must be a single character character, otherwise, E_INVARG is returned.
        let value = match value.variant() {
            Variant::Str(s) => s,
            _ => return Err(E_TYPE),
        };

        if value.len() != 1 {
            return Err(E_INVARG);
        }

        let mut s = self.as_string().clone();
        s.replace_range(index..=index, value.as_string().as_str());
        Ok(Var::mk_str(&s))
    }

    fn insert(&self, index: usize, value: &Var) -> Result<Var, Error> {
        // If value is not a string, return E_TYPE.
        let value = match value.variant() {
            Variant::Str(s) => s,
            _ => return Err(E_TYPE),
        };

        let mut new_copy = self.as_string().clone();
        new_copy.insert_str(index, value.as_string().as_str());
        Ok(Var::mk_str(&new_copy))
    }

    fn range(&self, from: isize, to: isize) -> Result<Var, Error> {
        if to < from {
            return Ok(Var::mk_str(""));
        }
        let s = self.as_string();
        let start = max(from, 0) as usize;
        let to = to as usize;
        if start >= s.len() || to >= s.len() {
            return Err(E_RANGE);
        }
        let s = s.get(start..=to).unwrap();
        Ok(Var::mk_str(s))
    }

    fn range_set(&self, from: isize, to: isize, with: &Var) -> Result<Var, Error> {
        let with_val = match with.variant() {
            Variant::Str(s) => s,
            _ => return Err(Error::E_TYPE),
        };

        let base_len = self.len();
        let from = from.to_usize().unwrap_or(0);
        let to = to.to_usize().unwrap_or(0);
        if to + 1 > base_len {
            return Err(E_RANGE);
        }
        let base_str = self.as_string();
        let mut ans = base_str.get(..from).unwrap_or("").to_string();
        ans.push_str(with_val.as_string());
        if to == 0 {
            ans.push_str(base_str);
        } else {
            ans.push_str(base_str.get(to + 1..).unwrap_or(""));
        }
        Ok(v_str(&ans))
    }

    fn push(&self, value: &Var) -> Result<Var, Error> {
        let value = match value.variant() {
            Variant::Str(s) => s,
            _ => return Err(E_TYPE),
        };

        let mut new_copy = self.as_string().clone();
        new_copy.push_str(value.as_string().as_str());
        Ok(Var::mk_str(&new_copy))
    }

    fn append(&self, other: &Var) -> Result<Var, Error> {
        let other = match other.variant() {
            Variant::Str(s) => s,
            _ => return Err(E_TYPE),
        };

        let mut new_copy = self.as_string().clone();
        new_copy.push_str(other.as_string().as_str());
        Ok(Var::mk_str(&new_copy))
    }

    fn remove_at(&self, index: usize) -> Result<Var, Error> {
        if index >= self.as_string().len() {
            return Err(E_RANGE);
        }

        let mut new_copy = self.as_string().clone();
        new_copy.remove(index);
        Ok(Var::mk_str(&new_copy))
    }

    fn contains(&self, value: &Var, case_sensitive: bool) -> Result<bool, Error> {
        let value = match value.variant() {
            Variant::Str(s) => s,
            _ => return Err(E_TYPE),
        };

        let s = self.as_string();
        let value = value.as_string();
        let contains = if case_sensitive {
            s.contains(value)
        } else {
            s.to_lowercase().contains(&value.to_lowercase())
        };

        Ok(contains)
    }
    fn index_in(&self, value: &Var, case_sensitive: bool) -> Result<Option<usize>, Error> {
        let value = match value.variant() {
            Variant::Str(s) => s,
            _ => return Err(E_TYPE),
        };

        let s = self.as_string();
        let value = value.as_string();
        let contains = if case_sensitive {
            // Get the index of the substring in the string.
            s.find(value)
        } else {
            s.to_lowercase().find(&value.to_lowercase())
        };

        Ok(contains)
    }
}

impl Display for Str {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_string())
    }
}
impl PartialEq for Str {
    // MOO strings are case-insensitive on comparison unless an explicit case sensitive comparison
    // is needed.
    fn eq(&self, other: &Self) -> bool {
        self.as_string().to_lowercase() == other.as_string().to_lowercase()
    }
}

impl Eq for Str {}

impl PartialOrd for Str {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Str {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_string()
            .to_lowercase()
            .cmp(&other.as_string().to_lowercase())
    }
}

impl Hash for Str {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_string().to_lowercase().hash(state)
    }
}

#[cfg(test)]
mod tests {
    use crate::v_bool;
    use crate::var::var::{v_int, v_str, Var};
    use crate::var::variant::Variant;
    use crate::var::IndexMode;

    #[test]
    fn test_str_pack_unpack() {
        let s = Var::mk_str("hello");
        match s.variant() {
            Variant::Str(s) => assert_eq!(s.as_string().as_str(), "hello"),
            _ => panic!("Expected string"),
        }
    }

    #[test]
    fn test_string_is_funcs() {
        let s = Var::mk_str("hello");
        assert!(s.is_true());
        assert!(s.is_sequence());
        assert!(!s.is_associative());
        assert!(!s.is_scalar());
        assert_eq!(s.len().unwrap(), 5);
        assert!(!s.is_empty().unwrap());

        let s = Var::mk_str("");
        assert!(!s.is_true());
        assert!(s.is_sequence());
        assert!(!s.is_associative());
        assert!(!s.is_scalar());
        assert_eq!(s.len().unwrap(), 0);
        assert!(s.is_empty().unwrap());
    }

    #[test]
    fn test_string_equality_inquality() {
        let s1 = Var::mk_str("hello");
        let s2 = Var::mk_str("hello");
        let s3 = Var::mk_str("world");
        let s4 = Var::mk_str("hello world");

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
        assert_ne!(s1, s4);
    }

    #[test]
    fn test_string_index() {
        let s = Var::mk_str("hello");
        let r = s.index(&Var::mk_integer(1), IndexMode::ZeroBased).unwrap();
        let r = r.variant();
        match r {
            Variant::Str(s) => assert_eq!(s.as_string().as_str(), "e"),
            _ => panic!("Expected string, got {:?}", r),
        }
    }

    #[test]
    fn test_string_index_set() {
        let s = Var::mk_str("hello");
        let r = s
            .index_set(&Var::mk_integer(1), &Var::mk_str("a"), IndexMode::ZeroBased)
            .unwrap();
        let r = r.variant();
        match r {
            Variant::Str(s) => assert_eq!(s.as_string().as_str(), "hallo"),
            _ => panic!("Expected string, got {:?}", r),
        }

        let fail_bad_index = s.index_set(
            &Var::mk_integer(10),
            &Var::mk_str("a"),
            IndexMode::ZeroBased,
        );
        assert!(fail_bad_index.is_err());
        assert_eq!(fail_bad_index.unwrap_err(), crate::var::Error::E_RANGE);
    }

    #[test]
    fn test_one_index_slice() {
        let s = v_str("hello world");
        let result = s.range(&v_int(2), &v_int(7), IndexMode::OneBased).unwrap();
        assert_eq!(result, v_str("ello w"));
    }

    #[test]
    fn test_zero_index_slice() {
        let s = v_str("hello world");
        let result = s.range(&v_int(1), &v_int(6), IndexMode::ZeroBased).unwrap();
        assert_eq!(result, v_str("ello w"));
    }

    #[test]
    fn test_string_range_set() {
        // Test a one-indexed assignment, comparing against a known MOO behaviour.
        let base = v_str("mandalorian");
        let (start, end) = (v_int(4), v_int(7));
        let replace = v_str("bozo");
        let expected = v_str("manbozorian");
        let result = base.range_set(&start, &end, &replace, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));

        // Test interior insertion
        let base = v_str("12345");
        let value = v_str("abc");
        let expected = v_str("1abc45");
        let result = base.range_set(&v_int(2), &v_int(3), &value, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));

        // Test interior replacement
        let base = v_str("12345");
        let value = v_str("ab");
        let expected = v_str("1ab45");
        let result = base.range_set(&v_int(1), &v_int(2), &value, IndexMode::ZeroBased);
        assert_eq!(result, Ok(expected));

        // Test interior deletion
        let base = v_str("12345");
        let value = v_str("");
        let expected = v_str("145");
        let result = base.range_set(&v_int(1), &v_int(2), &value, IndexMode::ZeroBased);
        assert_eq!(result, Ok(expected));

        // Test interior subtraction
        let base = v_str("12345");
        let value = v_str("z");
        let expected = v_str("1z45");
        let result = base.range_set(&v_int(1), &v_int(2), &value, IndexMode::ZeroBased);
        assert_eq!(result, Ok(expected));
    }

    /// Moo supports this weird behavior
    #[test]
    fn test_string_range_set_odd_range_end() {
        let base = v_str("me:words");
        let value = v_str("");
        let expected = v_str("me:words");
        let result = base.range_set(&v_int(1), &v_int(0), &value, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_string_push() {
        let s = Var::mk_str("hello");
        let r = s.push(&Var::mk_str(" world")).unwrap();
        let r = r.variant();
        match r {
            Variant::Str(s) => assert_eq!(s.as_string().as_str(), "hello world"),
            _ => panic!("Expected string, got {:?}", r),
        }
    }

    #[test]
    fn test_string_append() {
        let s1 = Var::mk_str("hello");
        let s2 = Var::mk_str(" world");
        let r = s1.append(&s2).unwrap();
        let r = r.variant();
        match r {
            Variant::Str(s) => assert_eq!(s.as_string().as_str(), "hello world"),
            _ => panic!("Expected string, got {:?}", r),
        }
    }

    #[test]
    fn test_string_remove_at() {
        let s = Var::mk_str("hello");
        let r = s
            .remove_at(&Var::mk_integer(1), IndexMode::ZeroBased)
            .unwrap();
        let r = r.variant();
        match r {
            Variant::Str(s) => assert_eq!(s.as_string().as_str(), "hllo"),
            _ => panic!("Expected string, got {:?}", r),
        }

        let fail_bad_index = s.remove_at(&Var::mk_integer(10), IndexMode::ZeroBased);
        assert!(fail_bad_index.is_err());
        assert_eq!(fail_bad_index.unwrap_err(), crate::var::Error::E_RANGE);
    }

    #[test]
    fn test_string_contains() {
        // Check both case-sensitive and case-insensitive
        let s = Var::mk_str("hello");
        assert_eq!(s.contains(&Var::mk_str("ell"), true).unwrap(), v_bool(true));
        assert_eq!(
            s.contains(&Var::mk_str("Ell"), false).unwrap(),
            v_bool(true)
        );
        assert_eq!(
            s.contains(&Var::mk_str("world"), true).unwrap(),
            v_bool(false)
        );
    }

    #[test]
    fn test_string_case_sensitive() {
        let s = Var::mk_str("hello");
        let s2 = Var::mk_str("Hello");
        assert_eq!(s, s2);
        assert!(!s.eq_case_sensitive(&s2));
    }
}
