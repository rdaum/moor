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

use crate::v_list_iter;
use crate::var::storage::VarBuffer;
use crate::var::var::Var;
use crate::var::variant::Variant;
use crate::var::Error::E_RANGE;
use crate::var::Sequence;
use crate::var::{Error, VarType};
use bytes::Bytes;
use flexbuffers::{BuilderOptions, VectorReader};
use num_traits::ToPrimitive;
use std::cmp::max;
use std::hash::Hash;

#[derive(Clone)]
pub struct List {
    // Reader must be boxed to avoid overfilling the stack.
    pub reader: VectorReader<VarBuffer>,
}

impl List {
    pub fn build(values: &[Var]) -> Var {
        let mut builder = flexbuffers::Builder::new(BuilderOptions::empty());
        let mut vb = builder.start_vector();
        vb.push(VarType::TYPE_LIST as u8);
        let mut lv = vb.start_vector();
        for v in values {
            let v = v.variant();
            v.push_item(&mut lv);
        }
        lv.end_vector();
        vb.end_vector();
        let buf = builder.take_buffer();
        let buf = Bytes::from(buf);
        let buf = VarBuffer(buf);
        Var(buf)
    }

    pub fn iter(&self) -> impl Iterator<Item = Var> + '_ {
        (0..self.len()).map(move |i| self.index(i).unwrap())
    }

    /// Remove the first found instance of `item` from the list.
    pub fn set_remove(&self, item: &Var) -> Result<Var, Error> {
        let mut found = false;
        let set_removed_iter = self.iter().filter_map(|v| {
            if v == *item && !found {
                found = true;
                None
            } else {
                Some(v)
            }
        });
        Ok(v_list_iter(set_removed_iter))
    }

    /// Add `item` to the list but only if it's not already there.
    pub fn set_add(&self, item: &Var) -> Result<Var, Error> {
        // Is the item already in the list? If so, just clone self
        if self.iter().any(|v| v == *item) {
            return Ok(Var::from_variant(Variant::List(self.clone().into())));
        }
        let set_added_iter = self.iter().chain(std::iter::once(item.clone()));
        Ok(v_list_iter(set_added_iter))
    }

    pub fn pop_front(&self) -> Result<(Var, Var), Error> {
        if self.is_empty() {
            return Err(E_RANGE);
        }
        let mut iter = self.iter();
        let first = iter.next().unwrap();
        let rest = v_list_iter(iter);
        Ok((first, rest))
    }
}

impl Sequence for List {
    fn is_empty(&self) -> bool {
        self.reader.len() == 0
    }

    fn len(&self) -> usize {
        self.reader.len()
    }

    fn contains(&self, value: &Var, case_sensitive: bool) -> Result<bool, Error> {
        for v in self.iter() {
            if case_sensitive {
                if v.eq_case_sensitive(value) {
                    return Ok(true);
                }
            } else if v == *value {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn index_in(&self, value: &Var, case_sensitive: bool) -> Result<Option<usize>, Error> {
        for (i, v) in self.iter().enumerate() {
            if case_sensitive {
                if v.eq_case_sensitive(value) {
                    return Ok(Some(i));
                }
            } else if v == *value {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    fn index(&self, index: usize) -> Result<Var, Error> {
        if index >= self.reader.len() {
            return Err(E_RANGE);
        }
        Ok(Var::from_reader(self.reader.index(index).unwrap()))
    }

    fn index_set(&self, index: usize, value: &Var) -> Result<Var, Error> {
        if index >= self.reader.len() {
            return Err(E_RANGE);
        }
        let replaced_iter = self
            .iter()
            .enumerate()
            .map(|(i, v)| if i == index { value.clone() } else { v });
        Ok(v_list_iter(replaced_iter))
    }

    fn push(&self, value: &Var) -> Result<Var, Error> {
        let with_added = self.iter().chain(std::iter::once(value.clone()));
        Ok(Var::mk_list_iter(with_added))
    }

    fn insert(&self, index: usize, value: &Var) -> Result<Var, Error> {
        let inserted_iter = self
            .iter()
            .take(index)
            .chain(std::iter::once(value.clone()))
            .chain(self.iter().skip(index));
        Ok(v_list_iter(inserted_iter))
    }

    fn range(&self, from: isize, to: isize) -> Result<Var, Error> {
        let len = self.len() as isize;
        if to < from {
            return Ok(Var::mk_list(&[]));
        }
        if from > len + 1 || to > len {
            return Err(E_RANGE);
        }
        let (from, to) = (max(from, 0) as usize, to as usize);
        let range_iter = self.iter().skip(from).take(to - from + 1);
        Ok(Var::mk_list_iter(range_iter))
    }

    fn range_set(&self, from: isize, to: isize, with: &Var) -> Result<Var, Error> {
        let with_val = match with.variant() {
            Variant::List(s) => s,
            _ => return Err(Error::E_TYPE),
        };

        let base_len = self.len();
        let from = from.to_usize().unwrap_or(0);
        let to = to.to_usize().unwrap_or(0);
        if to + 1 > base_len {
            return Err(E_RANGE);
        }
        // Iterator taking up to `from`
        let base_iter = self.iter().take(from);
        // Iterator for with_val...
        let with_iter = with_val.iter();
        // Iterator from after to, up to the end
        let end_iter = self.iter().skip(to + 1);
        let new_iter = base_iter.chain(with_iter).chain(end_iter);
        Ok(v_list_iter(new_iter))
    }

    fn append(&self, other: &Var) -> Result<Var, Error> {
        let other = match other.variant() {
            Variant::List(l) => l,
            _ => return Err(Error::E_TYPE),
        };

        let combined_iter = self.iter().chain(other.iter());
        Ok(Var::mk_list_iter(combined_iter))
    }

    fn remove_at(&self, index: usize) -> Result<Var, Error> {
        if index >= self.len() {
            return Err(E_RANGE);
        }

        let new = self
            .iter()
            .enumerate()
            .filter_map(|(i, v)| if i == index { None } else { Some(v) });
        Ok(Var::mk_list_iter(new))
    }
}

impl PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }

        // elements comparison
        for i in 0..self.len() {
            let a = self.index(i).unwrap();
            let b = other.index(i).unwrap();
            if a != b {
                return false;
            }
        }

        true
    }
}

impl Eq for List {}

impl PartialOrd for List {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for List {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.len() != other.len() {
            return self.len().cmp(&other.len());
        }

        // elements comparison
        for i in 0..self.len() {
            let a = self.index(i).unwrap();
            let b = other.index(i).unwrap();
            match a.cmp(&b) {
                std::cmp::Ordering::Equal => continue,
                x => return x,
            }
        }

        std::cmp::Ordering::Equal
    }
}

impl Hash for List {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for item in self.iter() {
            item.hash(state);
        }
    }
}

impl FromIterator<Var> for Var {
    fn from_iter<T: IntoIterator<Item = Var>>(iter: T) -> Self {
        let mut builder = flexbuffers::Builder::new(BuilderOptions::empty());
        let mut vb = builder.start_vector();
        vb.push(VarType::TYPE_LIST as u8);
        let mut lv = vb.start_vector();
        for v in iter {
            let v = v.variant();
            v.push_item(&mut lv);
        }
        lv.end_vector();
        vb.end_vector();
        let buf = builder.take_buffer();
        let buf = Bytes::from(buf);
        Var(VarBuffer(buf))
    }
}
#[cfg(test)]
mod tests {
    use crate::v_bool;
    use crate::var::var::{v_empty_list, v_int, v_list, v_str, Var};
    use crate::var::variant::Variant;
    use crate::var::Error;
    use crate::var::Error::{E_RANGE, E_TYPE};
    use crate::var::{IndexMode, Sequence};

    #[test]
    fn test_list_pack_unpack_index() {
        let l = Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(2), Var::mk_integer(3)]);

        match l.variant() {
            Variant::List(l) => {
                assert_eq!(l.len(), 3);
            }
            _ => panic!("Expected list, got {:?}", l.variant()),
        }
        eprintln!("List: {:?}", l.variant());
        let r = l.index(&Var::mk_integer(1), IndexMode::ZeroBased).unwrap();
        let r = r.variant();
        match r {
            Variant::Int(i) => assert_eq!(i, 2),
            _ => panic!("Expected integer, got {:?}", r),
        }
    }

    #[test]
    fn test_list_equality_inequality() {
        let l1 = Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(2), Var::mk_integer(3)]);
        let l2 = Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(2), Var::mk_integer(3)]);
        let l3 = Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(2), Var::mk_integer(4)]);
        let l4 = Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(2)]);
        let l5 = Var::mk_list(&[
            Var::mk_integer(1),
            Var::mk_integer(2),
            Var::mk_integer(3),
            Var::mk_integer(4),
        ]);

        assert_eq!(l1, l2);
        assert_ne!(l1, l3);
        assert_ne!(l1, l4);
        assert_ne!(l1, l5);
    }

    #[test]
    fn test_list_is_funcs() {
        let l = Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(2), Var::mk_integer(3)]);
        assert!(l.is_true());
        assert!(l.is_sequence());
        assert!(!l.is_associative());
        assert!(!l.is_scalar());
        assert_eq!(l.len().unwrap(), 3);
        assert!(!l.is_empty().unwrap());

        let l = Var::mk_list(&[]);
        assert!(!l.is_true());
        assert!(l.is_sequence());
        assert!(!l.is_associative());
        assert!(!l.is_scalar());
        assert_eq!(l.len().unwrap(), 0);
        assert!(l.is_empty().unwrap());
    }

    #[test]
    fn test_list_index() {
        let l = Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(2), Var::mk_integer(3)]);
        let r = l.index(&Var::mk_integer(1), IndexMode::ZeroBased).unwrap();
        let r = r.variant();
        match r {
            Variant::Int(i) => assert_eq!(i, 2),
            _ => panic!("Expected integer, got {:?}", r),
        }
    }

    #[test]
    fn test_list_index_set() {
        let l = Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(2), Var::mk_integer(3)]);
        let r = l
            .index_set(
                &Var::mk_integer(1),
                &Var::mk_integer(42),
                IndexMode::ZeroBased,
            )
            .unwrap();
        let r = r.variant();
        match r {
            Variant::List(l) => {
                let r = l.index(1).unwrap();
                let r = r.variant();
                match r {
                    Variant::Int(i) => assert_eq!(i, 42),
                    _ => panic!("Expected integer, got {:?}", r),
                }
            }
            _ => panic!("Expected list, got {:?}", r),
        }

        let fail_bad_index = l.index_set(
            &Var::mk_integer(10),
            &Var::mk_integer(42),
            IndexMode::ZeroBased,
        );
        assert!(fail_bad_index.is_err());
        assert_eq!(fail_bad_index.unwrap_err(), crate::var::Error::E_RANGE);
    }

    #[test]
    fn test_list_set_remove() {
        let l = Var::mk_list(&[
            Var::mk_integer(1),
            Var::mk_integer(2),
            Var::mk_integer(3),
            Var::mk_integer(2),
        ]);
        // Only works on list variants.
        let l = match l.variant() {
            Variant::List(l) => l,
            _ => panic!("Expected list"),
        };
        // This will only remove the first instance of 2...
        let removed = l.set_remove(&Var::mk_integer(2)).unwrap();
        let removed_v = match removed.variant() {
            Variant::List(l) => l,
            _ => panic!("Expected list"),
        };
        // should now b e [1, 3, 2]
        assert_eq!(removed_v.len(), 3);
        assert_eq!(
            removed,
            Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(3), Var::mk_integer(2)])
        );
    }

    #[test]
    fn test_list_set_add() {
        let l = Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(2), Var::mk_integer(3)]);
        // Only works on list variants.
        let l = match l.variant() {
            Variant::List(l) => l.clone(),
            _ => panic!("Expected list"),
        };
        // This will only add the first instance of 2...
        let added = l.set_add(&Var::mk_integer(2)).unwrap();
        let added_v = match added.variant() {
            Variant::List(l) => l.clone(),
            _ => panic!("Expected list"),
        };
        // should still be [1, 2, 3]
        assert_eq!(added_v.len(), 3);
        assert_eq!(
            added,
            Var::mk_list(&[Var::mk_integer(1), Var::mk_integer(2), Var::mk_integer(3)])
        );

        // now add 4
        let added = l.clone().set_add(&Var::mk_integer(4)).unwrap();
        let added_v = match added.variant() {
            Variant::List(l) => l,
            _ => panic!("Expected list"),
        };
        // should now be [1, 2, 3, 4]
        assert_eq!(added_v.len(), 4);
        assert_eq!(
            added,
            Var::mk_list(&[
                Var::mk_integer(1),
                Var::mk_integer(2),
                Var::mk_integer(3),
                Var::mk_integer(4)
            ])
        );
    }

    #[test]
    fn test_list_range() -> Result<(), Error> {
        // test on integer list
        let int_list = v_list(&[1.into(), 2.into(), 3.into(), 4.into(), 5.into()]);
        assert_eq!(
            int_list.range(&v_int(2), &v_int(4), IndexMode::ZeroBased)?,
            v_list(&[3.into(), 4.into(), 5.into()])
        );

        let int_list = v_list(&[1.into(), 2.into(), 3.into(), 4.into(), 5.into()]);
        assert_eq!(
            int_list.range(&v_int(3), &v_int(5), IndexMode::OneBased)?,
            v_list(&[3.into(), 4.into(), 5.into()])
        );

        // range with upper higher than lower, moo returns empty list for this (!)
        let empty_list = v_empty_list();
        assert_eq!(
            empty_list.range(&v_int(1), &v_int(0), IndexMode::ZeroBased),
            Ok(v_empty_list())
        );
        // test on out of range
        let int_list = v_list(&[1.into(), 2.into(), 3.into()]);
        assert_eq!(
            int_list.range(&v_int(2), &v_int(4), IndexMode::ZeroBased),
            Err(E_RANGE)
        );
        // test on type mismatch
        let var_int = v_int(10);
        assert_eq!(
            var_int.range(&v_int(1), &v_int(5), IndexMode::ZeroBased),
            Err(E_TYPE)
        );

        let list = v_list(&[v_int(0), v_int(0)]);
        assert_eq!(
            list.range(&v_int(1), &v_int(2), IndexMode::OneBased)?,
            v_list(&[v_int(0), v_int(0)])
        );
        Ok(())
    }

    #[test]
    fn test_list_range_set() {
        let base = v_list(&[v_int(1), v_int(2), v_int(3), v_int(4)]);

        // {1,2,3,4}[1..2] = {"a", "b", "c"} => {1, "a", "b", "c", 4}
        let value = v_list(&[v_str("a"), v_str("b"), v_str("c")]);
        let expected = v_list(&[v_int(1), v_str("a"), v_str("b"), v_str("c"), v_int(4)]);
        let result = base.range_set(&v_int(2), &v_int(3), &value, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));

        // {1,2,3,4}[1..2] = {"a"} => {1, "a", 4}
        let value = v_list(&[v_str("a")]);
        let expected = v_list(&[v_int(1), v_str("a"), v_int(4)]);
        let result = base.range_set(&v_int(2), &v_int(3), &value, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));

        // {1,2,3,4}[1..2] = {} => {1,4}
        let value = v_empty_list();
        let expected = v_list(&[v_int(1), v_int(4)]);
        let result = base.range_set(&v_int(2), &v_int(3), &value, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));

        // {1,2,3,4}[1..2] = {"a", "b"} => {1, "a", "b", 4}
        let value = v_list(&[v_str("a"), v_str("b")]);
        let expected = v_list(&[v_int(1), v_str("a"), v_str("b"), v_int(4)]);
        let result = base.range_set(&v_int(2), &v_int(3), &value, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_list_range_set2() {
        let base = v_list(&[v_int(1), v_int(2), v_int(3), v_int(4)]);
        let with_val = v_list(&[v_int(3), v_int(4)]);
        let expected = v_list(&[v_int(3), v_int(4), v_int(3), v_int(4)]);
        let result = base.range_set(&v_int(1), &v_int(2), &with_val, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_list_push() {
        let l = v_list(&[v_int(1), v_int(2), v_int(3)]);
        let r = l.push(&v_int(4)).unwrap();
        assert_eq!(r, v_list(&[v_int(1), v_int(2), v_int(3), v_int(4)]));
    }

    #[test]
    fn test_list_append() {
        let l1 = v_list(&[v_int(1), v_int(2), v_int(3)]);
        let l2 = v_list(&[v_int(4), v_int(5), v_int(6)]);
        let l3 = v_list(&[v_int(1), v_int(2), v_int(3), v_int(4), v_int(5), v_int(6)]);
        assert_eq!(l1.append(&l2), Ok(l3));
    }

    #[test]
    fn test_list_remove_at() {
        let l = v_list(&[v_int(1), v_int(2), v_int(3), v_int(4)]);
        let r = l.remove_at(&v_int(1), IndexMode::ZeroBased).unwrap();
        assert_eq!(r, v_list(&[v_int(1), v_int(3), v_int(4)]));
    }

    #[test]
    fn test_list_contains() {
        // Case sensitive and case-insensitive tests
        let l = v_list(&[v_str("a"), v_str("b"), v_str("c")]);
        assert_eq!(l.contains(&v_str("a"), true), Ok(v_bool(true)));
        assert_eq!(l.contains(&v_str("A"), false), Ok(v_bool(true)));
        assert_eq!(l.contains(&v_str("A"), true), Ok(v_bool(false)));
    }

    #[test]
    fn test_index_in() {
        let l = v_list(&[v_str("a"), v_str("b"), v_str("c")]);
        assert_eq!(
            l.index_in(&v_str("a"), false, IndexMode::OneBased).unwrap(),
            v_int(1)
        );
        assert_eq!(
            l.index_in(&v_str("A"), false, IndexMode::OneBased).unwrap(),
            v_int(1)
        );
        assert_eq!(
            l.index_in(&v_str("A"), true, IndexMode::OneBased).unwrap(),
            v_int(0)
        );

        assert_eq!(
            l.index_in(&v_str("A"), true, IndexMode::ZeroBased).unwrap(),
            v_int(-1)
        );
    }

    #[test]
    fn test_list_case_sensitive_compare() {
        let a = v_list(&[v_str("a"), v_str("b"), v_str("c")]);
        let b = v_list(&[v_str("A"), v_str("B"), v_str("C")]);

        assert!(!a.eq_case_sensitive(&b));
        assert!(a == b);
    }

    #[test]
    fn test_list_insert() {
        let l = v_list(&[v_int(1), v_int(2), v_int(3)]);
        let r = l.insert(&v_int(0), &v_int(0), IndexMode::OneBased).unwrap();
        assert_eq!(r, v_list(&[v_int(0), v_int(1), v_int(2), v_int(3)]));

        // Insert to the end
        let l = v_list(&[v_int(1), v_int(2), v_int(3)]);
        let r = l.insert(&v_int(1), &v_int(1), IndexMode::OneBased).unwrap();
        assert_eq!(r, v_list(&[v_int(1), v_int(1), v_int(2), v_int(3)]));

        // Out of range just goes to the end
        let l = v_list(&[v_int(1), v_int(2), v_int(3)]);
        let r = l
            .insert(&v_int(10), &v_int(10), IndexMode::OneBased)
            .unwrap();
        assert_eq!(r, v_list(&[v_int(1), v_int(2), v_int(3), v_int(10)]));
    }
}
