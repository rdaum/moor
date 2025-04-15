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

use crate::Associative;
use crate::Error;
use crate::Error::{E_RANGE, E_TYPE};
use crate::var::Var;
use crate::variant::Variant;
use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use std::cmp::Ordering;
use std::hash::Hash;

#[derive(Clone)]
pub struct Map(Box<im::OrdMap<Var, Var>>);

impl Map {
    // Construct from an Iterator of paris
    pub(crate) fn build<'a, I: Iterator<Item = &'a (Var, Var)>>(pairs: I) -> Var {
        let m = im::OrdMap::from_iter(pairs.cloned());
        let m = Map(Box::new(m));
        Var::from_variant(Variant::Map(m))
    }

    pub fn iter(&self) -> impl Iterator<Item = (Var, Var)> + '_ {
        self.0.iter().map(|(k, v)| (k.clone(), v.clone()))
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
        self.0.is_empty()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn index_in(&self, key: &Var, case_sensitive: bool) -> Result<Option<usize>, Error> {
        // Check the common in the key-value pairs and return the index of the first match.
        // Linear O(N) operation.
        let pos = self.iter().position(|(_, v)| {
            if case_sensitive {
                v.cmp_case_sensitive(key) == Ordering::Equal
            } else {
                v == *key
            }
        });
        Ok(pos)
    }

    fn get(&self, key: &Var) -> Result<Var, Error> {
        self.0.get(key).cloned().ok_or(E_RANGE)
    }

    fn set(&self, key: &Var, value: &Var) -> Result<Var, Error> {
        // Stunt has a restriction that non-scalars cannot be keys (unless they're strings).
        // So we enforce that here, even though it's not strictly necessary.
        if !key.is_scalar() && !key.is_string() {
            return Err(E_TYPE);
        }

        // If the key is already in the map, we replace the value.
        // Otherwise, we add a new key-value pair, which requires re-sorting...
        // So no matter what, this is an expensive O(N) operation, requiring multiple copies.
        // We'll just build a new, vector, and then pass the iterator into the build function.
        let mut cow_value = self.0.clone();
        cow_value.insert(key.clone(), value.clone());
        Ok(Var::from_variant(Variant::Map(Map(cow_value))))
    }

    fn index(&self, index: usize) -> Result<(Var, Var), Error> {
        // Note: Our structure doesn't have sequential indexed iteration, so to get the Nth value, we
        // have to do an O(N) iteration. This is a notably expensive operation, and should be avoided.
        // We should try to come up with a way to get map iteration to... not do this.
        let iterator = self.iter();
        let mut iterator = iterator.skip(index);
        let Some((k, v)) = iterator.next() else {
            return Err(E_RANGE);
        };

        Ok((k.clone(), v.clone()))
    }

    /// Return the range of key-value pairs between the two keys.
    fn range(&self, from: &Var, to: &Var) -> Result<Var, Error> {
        // Start by seeking the start of the range
        let mut range = self.0.range(from..);

        // There has to be something at the start of the range, or we toss out an E_RANGE
        let Some(first) = range.next() else {
            return Err(E_RANGE);
        };

        let mut new_map = Box::new(im::OrdMap::new());
        new_map.insert(first.0.clone(), first.1.clone());
        for (k, v) in range {
            let ordering = k.cmp(to);
            if ordering == Ordering::Greater || ordering == Ordering::Equal {
                break;
            }
            new_map.insert(k.clone(), v.clone());
        }

        Ok(Var::from_variant(Variant::Map(Map(new_map))))
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
        self.0.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>()
    }

    fn values(&self) -> Vec<Var> {
        self.0.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>()
    }

    fn contains_key(&self, key: &Var, case_sensitive: bool) -> Result<bool, Error> {
        if self.is_empty() {
            return Ok(false);
        }
        // If case sensitive, we have to do a (choke) linear scan. Otherwise we can just do a
        // normal search using existing comparator.
        if !case_sensitive {
            return Ok(self.0.contains_key(key));
        }
        for (k, _) in self.0.iter() {
            if k.cmp_case_sensitive(key) == Ordering::Equal {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Return this map with the key/value pair removed.
    /// Return the new map and the value that was removed, if any
    fn remove(&self, key: &Var, case_sensitive: bool) -> (Var, Option<Var>) {
        if self.is_empty() {
            return (Var::from_variant(Variant::Map(self.clone())), None);
        }

        if !case_sensitive {
            let mut cow_value = self.0.clone();
            let old_value = cow_value.remove(key);
            return (Var::from_variant(Variant::Map(Map(cow_value))), old_value);
        }
        let mut new_map = Box::new(im::OrdMap::new());
        let mut iterator = self.iter();
        let mut removed = None;
        loop {
            let Some((k, v)) = iterator.next() else {
                break;
            };
            if k.cmp_case_sensitive(key) == Ordering::Equal {
                if removed.is_none() {
                    removed = Some(v.clone());
                }
                continue;
            }
            new_map.insert(k.clone(), v.clone());
        }
        (Var::from_variant(Variant::Map(Map(new_map))), removed)
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

impl Encode for Map {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        // encode the length followed by the elements in sequence
        self.len().encode(encoder)?;
        for pair in self.iter() {
            pair.encode(encoder)?;
        }
        Ok(())
    }
}

impl<Context> Decode<Context> for Map {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let len = usize::decode(decoder)?;
        let mut l = im::OrdMap::new();
        for _ in 0..len {
            let pair = (Var::decode(decoder)?, Var::decode(decoder)?);
            l.insert(pair.0.clone(), pair.1.clone());
        }
        Ok(Map(Box::new(l)))
    }
}

impl<'de, Context> BorrowDecode<'de, Context> for Map {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let len = usize::decode(decoder)?;
        let mut l = im::OrdMap::new();
        for _ in 0..len {
            let pair = (Var::decode(decoder)?, Var::decode(decoder)?);
            l.insert(pair.0.clone(), pair.1.clone());
        }
        Ok(Map(Box::new(l)))
    }
}

#[cfg(test)]
mod tests {
    use crate::var::Var;
    use crate::variant::Variant;
    use crate::{Associative, IndexMode};
    use crate::{v_bool_int, v_int, v_str};

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
        let value = m.get(&key, IndexMode::ZeroBased).unwrap();
        match value.variant() {
            Variant::Int(i) => assert_eq!(*i, 1),
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
    fn test_map_get() {
        let m = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("c")),
        ]);

        let key = Var::mk_str("b");
        let value = m.get(&key, IndexMode::OneBased).unwrap();
        match value.variant() {
            Variant::Int(i) => assert_eq!(*i, 2),
            _ => panic!("Expected integer"),
        }
    }

    #[test]
    fn test_map_set() {
        let m = Var::mk_map(&[
            (Var::mk_str("a"), Var::mk_integer(1)),
            (Var::mk_str("b"), Var::mk_integer(2)),
            (Var::mk_integer(3), Var::mk_str("c")),
        ]);

        let r = m
            .set(&Var::mk_str("b"), &Var::mk_integer(42), IndexMode::OneBased)
            .unwrap();
        let r = r.variant();
        match r {
            Variant::Map(m) => {
                let r = m.get(&Var::mk_str("b")).unwrap();
                match r.variant() {
                    Variant::Int(i) => assert_eq!(*i, 42),
                    _ => panic!("Expected integer, got {:?}", r),
                }
            }
            _ => panic!("Expected map, got {:?}", r),
        }

        // Insert new item
        let r = m
            .set(&Var::mk_str("d"), &Var::mk_integer(42), IndexMode::OneBased)
            .unwrap();
        let r = r.variant();
        match r {
            Variant::Map(m) => {
                let r = m.get(&Var::mk_str("d")).unwrap();
                match r.variant() {
                    Variant::Int(i) => assert_eq!(*i, 42),
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
        assert_eq!(m.contains(&key, false).unwrap(), v_bool_int(true));
        assert_eq!(m.contains(&not_key, true).unwrap(), v_bool_int(false));

        // Case sensitive
        assert_eq!(m.contains(&key, true).unwrap(), v_bool_int(false));
        assert_eq!(m.contains(&not_key, false).unwrap(), v_bool_int(false));
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
            .set(&Var::mk_str("a"), &Var::mk_str("a"), IndexMode::OneBased)
            .unwrap();
        let m = m
            .set(
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

    #[test]
    fn test_contains_index() {
        // ; $tmp = ["FOO" -> "BAR"];
        // ; return "bar" in $tmp;
        let m = Var::mk_map(&[(Var::mk_str("FOO"), Var::mk_str("BAR"))]);
        let key = Var::mk_str("bar");

        let result = m.index_in(&key, false, IndexMode::OneBased).unwrap();
        assert_eq!(result, v_bool_int(true));
    }
}
