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

// Clippy warns about Arc<im::Vector<Var>> not being Send/Sync due to circular type dependency,
// but this is a false positive - Var is Send/Sync and imbl::Vector is thread-safe.
#![allow(clippy::arc_with_non_send_sync)]

use crate::{
    Error, Sequence, Var,
    error::ErrorCode::{E_RANGE, E_TYPE},
    v_list_iter,
};
use num_traits::ToPrimitive;
use std::{
    cmp::{max, min},
    fmt::{Debug, Formatter},
    hash::Hash,
    ops::Index,
    sync::Arc,
};

#[derive(Clone)]
#[repr(transparent)]
pub struct List(Arc<imbl::Vector<Var>>);

impl List {
    pub fn build(values: &[Var]) -> Var {
        let l = imbl::Vector::from(values.to_vec());
        Var::from_list(List(Arc::new(l)))
    }

    pub fn mk_list(values: &[Var]) -> List {
        let l = imbl::Vector::from(values.to_vec());
        List(Arc::new(l))
    }

    pub fn iter(&self) -> impl Iterator<Item = Var> + '_ {
        self.0.iter().cloned()
    }

    /// Remove the first found instance of `item` from the list.
    pub fn set_remove(&self, item: &Var) -> Result<Var, Error> {
        let idx = self.0.iter().position(|v| *v == *item);
        let result = if let Some(idx) = idx {
            let mut new = (*self.0).clone();
            new.remove(idx);
            List(Arc::new(new))
        } else {
            self.clone()
        };
        Ok(Var::from_list(result))
    }

    /// Add `item` to the list but only if it's not already there.
    pub fn set_add(&self, item: &Var) -> Result<Var, Error> {
        // Is the item already in the list? If so, just clone self
        if self.iter().any(|v| v == *item) {
            return Ok(Var::from_list(self.clone()));
        }
        let mut l = (*self.0).clone();
        l.push_back(item.clone());
        Ok(Var::from_list(List(Arc::new(l))))
    }

    pub fn pop_front(&self) -> Result<(Var, Var), Error> {
        if self.is_empty() {
            return Err(E_RANGE.msg("attempt to pop from empty list"));
        }
        let mut l = (*self.0).clone();
        let first = l.pop_front().unwrap();
        Ok((first, Var::from_list(List(Arc::new(l)))))
    }
}

impl Debug for List {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
impl Sequence for List {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn len(&self) -> usize {
        self.0.len()
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

    fn index(&self, index: usize) -> Result<Var, Error> {
        // For some reason this `if` is _slightly_ faster than just using .get(index).map_err(|_| E_RANGE.into())
        // Perf counters show fewer branches.
        if index >= self.len() {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "attempt to index {} in list of length {}",
                    // We adjust this for 1-indexing because we don't actually know the indexing mode and 1 is our most common.
                    // Here and below.
                    index + 1,
                    self.len()
                )
            }));
        }
        Ok(self.0[index].clone())
    }

    fn index_set(&self, index: usize, value: &Var) -> Result<Var, Error> {
        if index >= self.len() {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "attempt to set index {} in list of length {}",
                    index + 1,
                    self.len()
                )
            }));
        }
        let new = self.0.update(index, value.clone());
        Ok(Var::from_list(List(Arc::new(new))))
    }

    fn push(&self, value: &Var) -> Result<Var, Error> {
        let mut new = (*self.0).clone();
        new.push_back(value.clone());
        Ok(Var::from_list(List(Arc::new(new))))
    }

    fn insert(&self, index: usize, value: &Var) -> Result<Var, Error> {
        let index = min(index, self.len());
        let mut result = (*self.0).clone();
        result.insert(index, value.clone());
        Ok(Var::from_list(List(Arc::new(result))))
    }

    fn range(&self, from: isize, to: isize) -> Result<Var, Error> {
        let len = self.len() as isize;
        if to < from {
            return Ok(Var::mk_list(&[]));
        }
        if from > len + 1 || to > len {
            return Err(E_RANGE.with_msg(|| {
                let (from, to) = (from + 1, to + 1);
                format!(
                    "attempt to access out of bounds range {from}..{to} in list of length {len}"
                )
            }));
        }
        let (from, to) = (max(from, 0) as usize, to as usize);
        let range_iter = self.iter().skip(from).take(to - from + 1);
        Ok(Var::mk_list_iter(range_iter))
    }

    fn range_set(&self, from: isize, to: isize, with: &Var) -> Result<Var, Error> {
        let Some(with_val) = with.as_list() else {
            return Err(E_TYPE.msg("attempt to set range with non-list"));
        };

        let base_len = self.len();

        if from < 0 {
            return Err(
                E_RANGE.with_msg(|| format!("attempt to set range with negative index {from}"))
            );
        }

        // 1..0 is a "special" MOO-ism that is short for "insert at front" and rather than trying
        // to wrangle the logic below to Do The Right Thing, we'll just handle it here.
        if from == 0 && to == -1 {
            let new_iter = with_val.iter().chain(self.iter());
            return Ok(v_list_iter(new_iter));
        }

        let from = from.to_usize().unwrap_or(0);
        let to = to.to_usize().unwrap_or(0);

        // E_RANGE if from is greater than the length of the list + 1
        if from > base_len + 1 {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "attempt to set range with index {} in list of length {base_len}",
                    from + 1
                )
            }));
        }

        // MOO does a weird thing where it allows you to set a range where the end is out of bounds,
        // and does not return E_RANGE (but does not do the same for single index set).
        // So for example:
        // foo = {}; foo[1..2] = {1, 2, 3} => {1, 2, 3}
        // but
        // foo = {}; foo[4..5] = {1, 2, 3} => E_RANGE
        //
        let to = if base_len == 0 {
            0
        } else if to + 1 > base_len {
            base_len - 1
        } else {
            to
        };

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
        let Some(other) = other.as_list() else {
            return Err(E_TYPE.msg("attempt to append non-list"));
        };

        let mut result = (*self.0).clone();
        result.append(other.0.as_ref().clone());
        Ok(Var::from_list(List(Arc::new(result))))
    }

    fn remove_at(&self, index: usize) -> Result<Var, Error> {
        if index >= self.len() {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "attempt to remove index {} in list of length {}",
                    index + 1,
                    self.len()
                )
            }));
        }

        let mut new = (*self.0).clone();
        new.remove(index);
        Ok(Var::from_list(List(Arc::new(new))))
    }
}

impl From<List> for Var {
    fn from(val: List) -> Self {
        Var::from_list(val)
    }
}

impl Index<usize> for List {
    type Output = Var;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
impl PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }

        // elements comparison
        for i in 0..self.len() {
            let a = Sequence::index(self, i).unwrap();
            let b = Sequence::index(other, i).unwrap();
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
            let a = Sequence::index(self, i).unwrap();
            let b = Sequence::index(other, i).unwrap();
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
        let l: imbl::Vector<Var> = imbl::Vector::from_iter(iter);
        Var::from_list(List(Arc::new(l)))
    }
}

impl std::iter::FromIterator<Var> for List {
    fn from_iter<T: IntoIterator<Item = Var>>(iter: T) -> Self {
        let l: imbl::Vector<Var> = imbl::Vector::from_iter(iter);
        List(Arc::new(l))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Error, IndexMode, Sequence,
        error::ErrorCode::{E_RANGE, E_TYPE},
        v_bool_int,
        variant::Variant,
        variant::{Var, v_empty_list, v_int, v_list, v_str},
    };

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
            _ => panic!("Expected integer, got {r:?}"),
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
            _ => panic!("Expected integer, got {r:?}"),
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
                    _ => panic!("Expected integer, got {r:?}"),
                }
            }
            _ => panic!("Expected list, got {r:?}"),
        }

        let fail_bad_index = l.index_set(
            &Var::mk_integer(10),
            &Var::mk_integer(42),
            IndexMode::ZeroBased,
        );
        assert!(fail_bad_index.is_err());
        assert_eq!(fail_bad_index.unwrap_err(), E_RANGE);
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
            Err(E_RANGE.into())
        );
        // test on type mismatch
        let var_int = v_int(10);
        assert_eq!(
            var_int.range(&v_int(1), &v_int(5), IndexMode::ZeroBased),
            Err(E_TYPE.into())
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
        assert_eq!(l.contains(&v_str("a"), true), Ok(v_bool_int(true)));
        assert_eq!(l.contains(&v_str("A"), false), Ok(v_bool_int(true)));
        assert_eq!(l.contains(&v_str("A"), true), Ok(v_bool_int(false)));
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

    #[test]
    fn test_range_set() {
        // foo = {}; foo[1..2] = {1, 2, 3} => {1, 2, 3}
        let l = v_list(&[]);
        let r = l
            .range_set(
                &v_int(1),
                &v_int(2),
                &v_list(&[v_int(1), v_int(2), v_int(3)]),
                IndexMode::OneBased,
            )
            .unwrap();
        assert_eq!(r, v_list(&[v_int(1), v_int(2), v_int(3)]));

        // foo = {1}; foo[1..5] = {1, 2, 3} => {1, 2, 3}
        let l = v_list(&[v_int(1)]);
        let r = l
            .range_set(
                &v_int(1),
                &v_int(5),
                &v_list(&[v_int(1), v_int(2), v_int(3)]),
                IndexMode::OneBased,
            )
            .unwrap();
        assert_eq!(r, v_list(&[v_int(1), v_int(2), v_int(3)]));

        // foo = {1}; foo[2..3] = {2, 3} => {1,2,3}}
        let l = v_list(&[v_int(1)]);
        let r = l.range_set(
            &v_int(2),
            &v_int(3),
            &v_list(&[v_int(2), v_int(3)]),
            IndexMode::OneBased,
        );
        assert_eq!(r, Ok(v_list(&[v_int(1), v_int(2), v_int(3)])));

        // ;;a = {""}; a[2..1] = {#1}; return a;
        // => {"", #1}
        let l = v_list(&[v_str("")]);
        let r = l
            .range_set(
                &v_int(2),
                &v_int(1),
                &v_list(&[v_int(1)]),
                IndexMode::OneBased,
            )
            .unwrap();
        assert_eq!(r, v_list(&[v_str(""), v_int(1)]));

        // a = {1,2,3}; a[6..7] = {6,7,8}; return a;  => E_RANGE
        let l = v_list(&[v_int(1), v_int(2), v_int(3)]);
        let r = l.range_set(
            &v_int(6),
            &v_int(7),
            &v_list(&[v_int(6), v_int(7), v_int(8)]),
            IndexMode::OneBased,
        );
        assert_eq!(r, Err(E_RANGE.into()));

        //;;a = {}; a[1..0] = {"test"}; return a;
        // => {"test"}
        let l = v_list(&[]);
        let r = l
            .range_set(
                &v_int(1),
                &v_int(0),
                &v_list(&[v_str("test")]),
                IndexMode::OneBased,
            )
            .unwrap();
        assert_eq!(r, v_list(&[v_str("test")]));

        // escapes = {".", "@abort"}; escapes[1..0] = {"?"}; return escapes;
        // => {"?", ".", "@abort"}
        let l = v_list(&[v_str("."), v_str("@abort")]);
        let r = l
            .range_set(
                &v_int(1),
                &v_int(0),
                &v_list(&[v_str("?")]),
                IndexMode::OneBased,
            )
            .unwrap();
        assert_eq!(r, v_list(&[v_str("?"), v_str("."), v_str("@abort")]));
    }
}
