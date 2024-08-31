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

use crate::var::storage::VarBuffer;
use crate::var::var::Var;
use crate::var::variant::Variant;
use crate::var::Associative;
use crate::var::Error::{E_RANGE, E_TYPE};
use crate::var::{Error, VarType};
use bytes::Bytes;
use flexbuffers::{BuilderOptions, VectorReader};
use std::cmp::Ordering;
use std::hash::Hash;

#[derive(Clone)]
pub struct Map {
    // Reader must be boxed to avoid overfilling the stack.
    pub reader: VectorReader<VarBuffer>,
}

impl Map {
    // Construct from an Iterator of paris
    pub(crate) fn build<'a, I: Iterator<Item = &'a (Var, Var)>>(pairs: I) -> Var {
        // Our maps don't use the flexbuffers map type because that only allows strings for keys.
        // Instead, we just use a vector of pairs, sorted, so binary search can be used to find
        // keys in O(log n) time.
        // Construction, however, is O(n) because we need to insert the pairs in sorted order.
        // And make a copy, to boot.
        let mut sorted: Vec<_> = pairs.collect();
        sorted.sort();

        Self::build_presorted(sorted.into_iter())
    }

    pub(crate) fn build_presorted<'a, I: Iterator<Item = &'a (Var, Var)>>(pairs: I) -> Var {
        let mut builder = flexbuffers::Builder::new(BuilderOptions::empty());
        let mut vb = builder.start_vector();
        vb.push(VarType::TYPE_MAP as u8);
        let mut mv = vb.start_vector();
        for (k, v) in pairs {
            k.variant().push_item(&mut mv);
            v.variant().push_item(&mut mv);
        }
        mv.end_vector();
        vb.end_vector();
        let buf = builder.take_buffer();
        let buf = Bytes::from(buf);
        Var::from_bytes(buf)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Var, Var)> + '_ {
        (0..self.len()).map(move |i| {
            let k = self.reader.idx(i * 2);
            let v = self.reader.idx(i * 2 + 1);

            (Var::from_reader(k), Var::from_reader(v))
        })
    }

    fn binary_search<'a, F: Fn(&'a Var, &Var) -> Ordering>(
        &self,
        f: F,
        c: &'a Var,
    ) -> Option<usize> {
        let n = self.reader.len() / 2;
        let mut low = 0;
        let mut high = (n as isize) - 1;
        while low <= high {
            let mid = (low + high) / 2;
            let k = self.reader.idx((mid * 2) as usize);
            let v = Var::from_reader(k);
            match f(c, &v) {
                Ordering::Less => high = mid - 1,
                Ordering::Greater => low = mid + 1,
                Ordering::Equal => return Some(mid as usize),
            }
        }

        None
    }
}

impl PartialEq for Map {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }

        // elements comparison using iterator
        for (a, b) in self.iter().zip(other.iter()) {
            if a != b {
                return false;
            }
        }

        true
    }
}

impl Associative for Map {
    fn is_empty(&self) -> bool {
        self.reader.len() == 0
    }

    fn len(&self) -> usize {
        self.reader.len() / 2
    }

    fn index(&self, key: &Var) -> Result<Var, Error> {
        let n = self.reader.len() / 2;

        if n == 0 {
            return Err(E_RANGE);
        }

        // Items are in sorted order, so we can binary search.
        let Some(pos) = self.binary_search(|a, b| a.cmp(b), key) else {
            return Err(E_RANGE);
        };

        let v = self.reader.idx((pos * 2) + 1);
        Ok(Var::from_reader(v))
    }

    fn index_in(&self, key: &Var, case_sensitive: bool) -> Result<Option<usize>, Error> {
        // Check the values in the key-value pairs and return the index of the first match.
        // Linear O(N) operation.
        for i in 0..self.len() {
            let v = self.reader.idx((i * 2) + 1);
            let v = Var::from_reader(v);
            let matches = if case_sensitive {
                v.eq_case_sensitive(key)
            } else {
                v.eq(key)
            };
            if matches {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    fn index_set(&self, key: &Var, value: &Var) -> Result<Var, Error> {
        // Stunt has a restriction that non-scalars cannot be keys (unless they're strings).
        // So we enforce that here, even though it's not strictly necessary.
        if !key.is_scalar() && !key.is_string() {
            return Err(E_TYPE);
        }

        // If the key is already in the map, we replace the value.
        // Otherwise, we add a new key-value pair, which requires re-sorting...
        // So no matter what, this is an expensive O(N) operation, requiring multiple copies.
        // We'll just build a new, vector, and then pass the iterator into the build function.

        // TODO: find a way to construct chained iterators for this instead...

        let mut new_vec = Vec::with_capacity(self.len() + 1);
        let mut found = false;
        for (k, v) in self.iter() {
            if k == *key {
                new_vec.push((key.clone(), value.clone()));
                found = true;
            } else {
                new_vec.push((k, v));
            }
        }
        if !found {
            new_vec.push((key.clone(), value.clone()));
        }
        Ok(Self::build(new_vec.iter()))
    }

    /// Return the range of key-value pairs between the two keys.
    fn range(&self, from: &Var, to: &Var) -> Result<Var, Error> {
        // Find start with binary search.
        let start = match self.binary_search(|a, b| a.cmp(b), from) {
            Some(pos) => pos,
            None => return Err(E_RANGE),
        };

        // Now scan forward to find the end.
        let mut new_vec = Vec::new();
        for i in start..self.len() {
            let k = self.reader.idx(i * 2);
            let k = Var::from_reader(k);
            let order = k.cmp(to);
            if order == Ordering::Greater || order == Ordering::Equal {
                break;
            }
            let v = self.reader.idx(i * 2 + 1);
            new_vec.push((k, Var::from_reader(v)));
        }

        Ok(Self::build_presorted(new_vec.iter()))
    }

    fn range_set(&self, _from: &Var, _to: &Var, _with: &Var) -> Result<Var, Error> {
        // We reject range assignment, because it's tricky to get right, and Stunt does weird
        // things. But code to do it is here, if we ever need it.
        return Err(E_TYPE);

        #[allow(unreachable_code)]
        {
            let with = match _with.variant() {
                Variant::Map(m) => m,
                _ => return Err(E_TYPE),
            };

            let mut new_pairs = Vec::with_capacity(self.len() + with.len());
            for (k, v) in self.iter() {
                if k.eq(_from)
                    || k.cmp(_from) == Ordering::Greater && k.cmp(_to) == Ordering::Less
                    || k.eq(_to)
                {
                    continue;
                }

                new_pairs.push((k, v));
            }
            for (k, v) in with.iter() {
                new_pairs.push((k, v));
            }

            Ok(Self::build(new_pairs.iter()))
        }
    }

    fn keys(&self) -> Vec<Var> {
        let mut keys = Vec::new();
        for i in 0..self.len() {
            let k = self.reader.idx(i * 2);
            keys.push(Var::from_reader(k))
        }
        keys
    }

    fn values(&self) -> Vec<Var> {
        let mut values = Vec::new();
        for i in 0..self.len() {
            let v = self.reader.idx(i * 2 + 1);
            values.push(Var::from_reader(v))
        }
        values
    }

    fn contains_key(&self, key: &Var, case_sensitive: bool) -> Result<bool, Error> {
        let n = self.reader.len() / 2;

        if n == 0 {
            return Ok(false);
        }
        let cmp = |a: &Var, b: &Var| {
            if case_sensitive {
                b.cmp_case_sensitive(a)
            } else {
                b.cmp(a)
            }
        };
        let result = self.binary_search(cmp, key).is_some();
        Ok(result)
    }

    /// Return this map with the key/value pair removed.
    /// Return the new map and the value that was removed, if any
    fn remove(&self, key: &Var, case_sensitive: bool) -> (Var, Option<Var>) {
        // Return a copy of self without the key, and the value that was removed, if any.
        let mut new_pairs = Vec::with_capacity(self.len());
        let mut removed = None;
        for (k, v) in self.iter() {
            let matches = if case_sensitive {
                k.cmp_case_sensitive(key) == Ordering::Equal
            } else {
                k == *key
            };
            if matches {
                removed = Some(v);
            } else {
                new_pairs.push((k, v));
            }
        }
        (Self::build(new_pairs.iter()), removed)
    }
}

impl Eq for Map {}

impl PartialOrd for Map {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Map {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.len() != other.len() {
            return self.len().cmp(&other.len());
        }

        // elements comparison
        for (a, b) in self.iter().zip(other.iter()) {
            match a.cmp(&b) {
                Ordering::Equal => continue,
                x => return x,
            }
        }

        Ordering::Equal
    }
}

impl Hash for Map {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for item in self.iter() {
            item.0.hash(state);
            item.1.hash(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::var::var::Var;
    use crate::var::variant::Variant;
    use crate::var::{Associative, IndexMode};
    use crate::{v_bool, v_int, v_str};

    #[test]
    fn test_map_pack_unpack_index() {
        let m = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("c")),
        ]);

        match m.variant() {
            Variant::Map(m) => {
                assert_eq!(m.len(), 3);
            }
            _ => panic!("Expected map"),
        }

        let key = Var::mk_str("a");
        let value = m.index(&key, IndexMode::ZeroBased).unwrap();
        match value.variant() {
            Variant::Int(i) => assert_eq!(i, 1),
            _ => panic!("Expected integer"),
        }
    }

    #[test]
    fn test_map_is_funcs() {
        let m = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("c")),
        ]);

        assert!(m.is_true());
        assert!(!m.is_sequence());
        assert!(m.is_associative());
        assert!(!m.is_scalar());
        assert_eq!(m.len().unwrap(), 3);
        assert!(!m.is_empty().unwrap());

        let m = Var::mk_map(&[]);
        assert!(!m.is_true());
        assert!(!m.is_sequence());
        assert!(m.is_associative());
        assert!(!m.is_scalar());
        assert_eq!(m.len().unwrap(), 0);
        assert!(m.is_empty().unwrap());
    }

    #[test]
    fn test_map_equality_inequality() {
        let m1 = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("c")),
        ]);

        let m2 = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("c")),
        ]);

        let m3 = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("d")),
        ]);

        let m4 = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
        ]);

        let m5 = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("c")),
            (Var::mk_integer(4), Var::mk_str("d")),
        ]);

        assert_eq!(m1, m2);
        assert_ne!(m1, m3);
        assert_ne!(m1, m4);
        assert_ne!(m1, m5);
    }

    #[test]
    fn test_map_index() {
        let m = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("c")),
        ]);

        let key = Var::mk_str("b");
        let value = m.index(&key, IndexMode::ZeroBased).unwrap();
        match value.variant() {
            Variant::Int(i) => assert_eq!(i, 2),
            _ => panic!("Expected integer"),
        }
    }

    #[test]
    fn test_map_index_set() {
        let m = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("c")),
        ]);

        let r = m
            .index_set(
                &Var::mk_str("b"),
                &Var::mk_integer(42),
                IndexMode::ZeroBased,
            )
            .unwrap();
        let r = r.variant();
        match r {
            Variant::Map(m) => {
                let r = m.index(&Var::mk_str("b")).unwrap();
                match r.variant() {
                    Variant::Int(i) => assert_eq!(i, 42),
                    _ => panic!("Expected integer, got {:?}", r),
                }
            }
            _ => panic!("Expected map, got {:?}", r),
        }

        // Insert new item
        let r = m
            .index_set(
                &Var::mk_str("d"),
                &Var::mk_integer(42),
                IndexMode::ZeroBased,
            )
            .unwrap();
        let r = r.variant();
        match r {
            Variant::Map(m) => {
                let r = m.index(&Var::mk_str("d")).unwrap();
                match r.variant() {
                    Variant::Int(i) => assert_eq!(i, 42),
                    _ => panic!("Expected integer, got {:?}", r),
                }
            }
            _ => panic!("Expected map, got {:?}", r),
        }
    }

    #[test]
    fn test_map_keys_values() {
        let m = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("c")),
        ]);

        let m = match m.variant() {
            Variant::Map(m) => m,
            _ => panic!("Expected map"),
        };

        // The keys come out in sorted order.
        let keys = m.keys();
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0], Var::mk_integer(3));
        assert_eq!(keys[1], Var::mk_str("a"));
        assert_eq!(keys[2], Var::mk_str("b"));

        let values = m.values();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], Var::mk_str("c"));
        assert_eq!(values[1], Var::mk_integer(1));
        assert_eq!(values[2], Var::mk_integer(2));
    }

    #[test]
    fn test_map_range() {
        let m = Var::mk_map(&[
            (Var::mk_integer(0), Var::mk_integer(1)),
            (Var::mk_integer(1), Var::mk_integer(2)),
            (Var::mk_integer(2), Var::mk_integer(3)),
            (Var::mk_integer(3), Var::mk_integer(4)),
        ]);

        let r = m
            .range(
                &Var::mk_integer(1),
                &Var::mk_integer(3),
                IndexMode::OneBased,
            )
            .unwrap();
        let r = match r.variant() {
            Variant::Map(m) => m,
            _ => panic!("Expected map"),
        };

        let key_value_results = r.iter().collect::<Vec<_>>();
        assert_eq!(
            key_value_results,
            vec![(v_int(1), v_int(2)), (v_int(2), v_int(3))]
        );
    }

    #[test]
    // Disable because range_set is stubbed out in our impl
    #[ignore]
    fn test_map_range_set() {
        let m = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_str("c"), Var::mk_integer(3)),
            (Var::mk_str("d"), Var::mk_integer(4)),
            (Var::mk_str("e"), Var::mk_integer(5)),
        ]);

        // Now replace b, c, d with x, y
        let r = m
            .range_set(
                &Var::mk_str("b"),
                &Var::mk_str("d"),
                &Var::mk_map(&[
                    (Var::mk_str("x"), Var::mk_integer(42)),
                    (Var::mk_str("y"), Var::mk_integer(43)),
                ]),
                IndexMode::ZeroBased,
            )
            .unwrap();

        let r = match r.variant() {
            Variant::Map(m) => m,
            _ => panic!("Expected map"),
        };

        assert_eq!(r.len(), 4);
        let keys = r.keys();
        assert_eq!(keys.len(), 4);
        assert_eq!(keys[0], Var::mk_str("a"));
        assert_eq!(keys[1], Var::mk_str("e"));
        assert_eq!(keys[2], Var::mk_str("x"));
        assert_eq!(keys[3], Var::mk_str("y"));
    }

    #[test]
    fn test_map_contains_key() {
        let m = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_str("c"), Var::mk_integer(3)),
        ]);

        let key = Var::mk_str("B");
        let not_key = Var::mk_str("d");

        // Case-insensitive
        assert_eq!(m.contains(&key, false).unwrap(), v_bool(true));
        assert_eq!(m.contains(&not_key, true).unwrap(), v_bool(false));

        // Case sensitive
        assert_eq!(m.contains(&key, true).unwrap(), v_bool(false));
        assert_eq!(m.contains(&not_key, false).unwrap(), v_bool(false));
    }

    #[test]
    fn test_map_remove_key() {
        let m = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_str("c"), Var::mk_integer(3)),
        ]);

        let key = Var::mk_str("b");
        let not_key = Var::mk_str("d");

        let (r, removed) = m.remove(&key, false).expect("remove failed");
        assert_eq!(r.len().unwrap(), 2);
        assert_eq!(removed.unwrap(), Var::mk_integer(2));

        let (r, removed) = m.remove(&not_key, false).expect("remove failed");
        assert_eq!(r.len().unwrap(), 3);
        assert_eq!(removed, None);

        // Case sensitive
        let not_key = Var::mk_str("B");
        let (r, removed) = m.remove(&not_key, true).expect("remove failed");
        assert_eq!(r.len().unwrap(), 3);
        assert_eq!(removed, None);

        let (r, removed) = m.remove(&key, true).expect("remove failed");
        assert_eq!(r.len().unwrap(), 2);
        assert_eq!(removed.unwrap(), Var::mk_integer(2));
    }

    #[test]
    /// Verify that sort order is preserved after insertion
    fn test_map_insertion_ordering() {
        let m = Var::mk_map(&[
            (Var::mk_integer(3), Var::mk_integer(3)),
            (Var::mk_integer(1), Var::mk_integer(1)),
            (Var::mk_integer(4), Var::mk_integer(4)),
            (Var::mk_integer(5), Var::mk_integer(5)),
            (Var::mk_integer(9), Var::mk_integer(9)),
            (Var::mk_integer(2), Var::mk_integer(2)),
        ]);

        let m = m
            .index_set(&Var::mk_str("a"), &Var::mk_str("a"), IndexMode::OneBased)
            .unwrap();
        let m = m
            .index_set(
                &Var::mk_integer(6),
                &Var::mk_integer(6),
                IndexMode::OneBased,
            )
            .unwrap();

        let m = match m.variant() {
            Variant::Map(m) => m,
            _ => panic!("Expected map"),
        };

        let pairs = m.iter().collect::<Vec<_>>();
        assert_eq!(
            pairs,
            vec![
                (v_int(1), v_int(1)),
                (v_int(2), v_int(2)),
                (v_int(3), v_int(3)),
                (v_int(4), v_int(4)),
                (v_int(5), v_int(5)),
                (v_int(6), v_int(6)),
                (v_int(9), v_int(9)),
                (v_str("a"), v_str("a")),
            ]
        );
    }

    #[test]
    fn test_index_in() {
        // ["3" -> "3", "1" -> "1", "4" -> "4", "5" -> "5", "9" -> "9", "2" -> "2"];
        let m = Var::mk_map(&[
            (Var::mk_str("3"), Var::mk_str("3")),
            (Var::mk_str("1"), Var::mk_str("1")),
            (Var::mk_str("4"), Var::mk_str("4")),
            (Var::mk_str("5"), Var::mk_str("5")),
            (Var::mk_str("9"), Var::mk_str("9")),
            (Var::mk_str("2"), Var::mk_str("2")),
        ]);
        // "2" -> 2nd position
        let key = Var::mk_str("2");
        let pos = m.index_in(&key, false, IndexMode::OneBased).unwrap();
        assert_eq!(pos, v_int(2));
    }

    #[test]
    fn test_case_sensitive_compare() {
        let m_a = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_str("a")),
            (Var::mk_str("b"), Var::mk_str("b")),
            (Var::mk_str("c"), Var::mk_str("c")),
        ]);

        let m_b = Var::mk_map(&[
            (Var::mk_str("A"), Var::mk_str("A")),
            (Var::mk_str("B"), Var::mk_str("B")),
            (Var::mk_str("C"), Var::mk_str("C")),
        ]);

        assert!(!m_a.eq_case_sensitive(&m_b));
        assert!(m_a.eq(&m_b));
    }
}
