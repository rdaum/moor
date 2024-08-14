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

use crate::var::{Var, Variant};
use crate::BincodeAsByteBufferExt;
use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Map(im::OrdMap<Var, Var>);
impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

impl Map {
    pub fn new() -> Self {
        Self(im::OrdMap::new())
    }

    pub fn from_pairs(pairs: &[(Var, Var)]) -> Self {
        Self(pairs.iter().cloned().collect())
    }

    fn from_map(map: im::OrdMap<Var, Var>) -> Self {
        Self(map)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, key: &Var) -> Option<&Var> {
        self.0.get(key)
    }

    /// Copy-on-write insert.
    /// Add a key-value pair to the map, returning a new map with the pair added.
    pub fn insert(&self, key: Var, value: Var) -> Var {
        let mut new_map = self.0.clone();
        new_map.insert(key, value);
        Var::new(Variant::Map(Map::from_map(new_map)))
    }

    /// Return a map which is subset of this map where the keys lay between `start` and `end`,
    /// (exclusive of `end`, inclusive of `start`.)
    pub fn range(&self, start: Var, end: Var) -> Var {
        let mut new_map = im::OrdMap::new();
        let range = self.0.range(start..end);
        for (key, value) in range {
            new_map.insert(key.clone(), value.clone());
        }

        Var::new(Variant::Map(Map::from_map(new_map)))
    }

    /// Replace the specified range in the map. `from` and `to` must be valid keys in the map, and
    /// `to` must be a Map. All tuples between from and to are removed, and the values from `to`
    /// are inserted in their place. Unlike above, the whole range is inclusive.
    pub fn range_set(&self, start: Var, end: Var, to: &Map) -> Var {
        let mut new_map = self.0.clone();
        let range = self.0.range(start..=end);
        for (key, _) in range {
            new_map.remove(key);
        }
        let new_map = new_map.union(to.0.clone());
        Var::new(Variant::Map(Map::from_map(new_map)))
    }

    /// Return a map with `key` removed, if it exists. If it does not exist, return the original map.
    /// The removed value is returned as the second element of the tuple.
    pub fn remove(&self, key: &Var) -> (Var, Option<Var>) {
        let mut removed = self.0.clone();
        let removed_value = removed.remove(key);
        let nm = Var::new(Variant::Map(Map::from_map(removed)));
        (nm, removed_value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Var, &Var)> {
        self.0.iter()
    }
}

impl BincodeAsByteBufferExt for Map {}

impl Encode for Map {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        // There's no bincode impl for im::HashMap, so we'll encode the pairs as a list of tuples.
        // Starting with the count of the number of pairs.
        self.len().encode(encoder)?;
        for (key, value) in self.iter() {
            key.encode(encoder)?;
            value.encode(encoder)?;
        }
        Ok(())
    }
}

impl Decode for Map {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let tuple_count = usize::decode(decoder)?;
        let mut pair_vec = Vec::with_capacity(tuple_count);
        for _ in 0..tuple_count {
            let key = Var::decode(decoder)?;
            let value = Var::decode(decoder)?;
            pair_vec.push((key, value));
        }
        Ok(Map::from_pairs(&pair_vec))
    }
}

impl<'de> BorrowDecode<'de> for Map {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let tuple_count = usize::decode(decoder)?;
        let mut pair_vec = Vec::with_capacity(tuple_count);
        for _ in 0..tuple_count {
            let key = Var::borrow_decode(decoder)?;
            let value = Var::borrow_decode(decoder)?;
            pair_vec.push((key, value));
        }
        Ok(Map::from_pairs(&pair_vec))
    }
}

impl Display for Map {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("[ ")?;
        for (key, value) in self.iter() {
            write!(f, "{:?} -> {:?}, ", key, value)?;
        }
        f.write_str(" ]")
    }
}
