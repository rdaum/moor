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

use crate::var::list::List;
use crate::var::storage::VarBuffer;
use crate::var::Associative;
use crate::var::{map, string, Sequence};
use crate::var::{Error, Objid, VarType};
use decorum::R64;
use flexbuffers::{Reader, VectorBuilder};
use num_traits::ToPrimitive;
use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Our series of types
#[derive(Clone)]
pub enum Variant {
    None,
    Obj(Objid),
    Int(i64),
    Float(f64),
    List(Arc<List>),
    Str(Arc<string::Str>),
    Map(Arc<map::Map>),
    Err(Error),
}

impl Hash for Variant {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Variant::None => 0.hash(state),
            Variant::Obj(o) => o.0.hash(state),
            Variant::Int(i) => i.hash(state),
            Variant::Float(f) => f.to_f64().unwrap().to_bits().hash(state),
            Variant::List(l) => l.hash(state),
            Variant::Str(s) => s.hash(state),
            Variant::Map(m) => m.hash(state),
            Variant::Err(e) => e.hash(state),
        }
    }
}

impl Ord for Variant {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Variant::None, Variant::None) => Ordering::Equal,
            (Variant::Obj(l), Variant::Obj(r)) => l.cmp(r),
            (Variant::Int(l), Variant::Int(r)) => l.cmp(r),
            (Variant::Float(l), Variant::Float(r)) => {
                // For floats, we wrap in decorum first.
                let l = R64::from(l.to_f64().unwrap());
                let r = R64::from(r.to_f64().unwrap());
                l.cmp(&r)
            }
            (Variant::List(l), Variant::List(r)) => l.cmp(r),
            (Variant::Str(l), Variant::Str(r)) => l.cmp(r),
            (Variant::Map(l), Variant::Map(r)) => l.cmp(r),
            (Variant::Err(l), Variant::Err(r)) => l.cmp(r),
            (Variant::None, _) => Ordering::Less,
            (_, Variant::None) => Ordering::Greater,
            (Variant::Obj(_), _) => Ordering::Less,
            (_, Variant::Obj(_)) => Ordering::Greater,
            (Variant::Int(_), _) => Ordering::Less,
            (_, Variant::Int(_)) => Ordering::Greater,
            (Variant::Float(_), _) => Ordering::Less,
            (_, Variant::Float(_)) => Ordering::Greater,
            (Variant::List(_), _) => Ordering::Less,
            (_, Variant::List(_)) => Ordering::Greater,
            (Variant::Str(_), _) => Ordering::Less,
            (_, Variant::Str(_)) => Ordering::Greater,
            (Variant::Map(_), _) => Ordering::Less,
            (_, Variant::Map(_)) => Ordering::Greater,
        }
    }
}

impl PartialOrd for Variant {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Debug for Variant {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Variant::None => write!(f, "None"),
            Variant::Obj(o) => write!(f, "Object({})", o.0),
            Variant::Int(i) => write!(f, "Integer({})", i),
            Variant::Float(fl) => write!(f, "Float({})", fl),
            Variant::List(l) => {
                // Items...
                let r = l.reader.iter();
                let i: Vec<_> = r.map(Variant::from_reader).collect();
                write!(f, "List([size = {}, items = {:?}])", l.len(), i)
            }
            Variant::Str(s) => write!(f, "String({:?})", s.as_string()),
            Variant::Map(m) => {
                // Items...
                let r = m.reader.iter();
                let i: Vec<_> = r.map(Variant::from_reader).collect();
                write!(f, "Map([size = {}, items = {:?}])", m.len(), i)
            }
            Variant::Err(e) => write!(f, "Error({:?})", e),
        }
    }
}

impl Variant {
    pub(crate) fn from_reader(vec: Reader<VarBuffer>) -> Self {
        // Each Var is a vector of two elements, the type and the value.
        let vec = vec.as_vector();

        let mut vec_iter = vec.iter();
        let type_reader = vec_iter.next().unwrap();
        let t = type_reader.as_u8();
        let t = VarType::from_repr(t).unwrap();

        // Now we can match on the type and pull out the value.
        match t {
            VarType::TYPE_NONE => Variant::None,
            VarType::TYPE_INT => {
                let v = vec_iter.next().unwrap();
                let i = v.as_i64();
                Self::Int(i)
            }
            VarType::TYPE_FLOAT => {
                let v = vec_iter.next().unwrap();
                let f = v.as_f64();
                Self::Float(f)
            }
            VarType::TYPE_OBJ => {
                let v = vec_iter.next().unwrap();
                let o = v.as_i64();
                Self::Obj(Objid(o))
            }
            VarType::TYPE_STR => {
                let v = vec_iter.next().unwrap();
                Self::Str(Arc::new(string::Str { reader: v }))
            }
            VarType::TYPE_ERR => {
                // Error code encoded as u8
                let v = vec_iter.next().unwrap();
                let e = v.as_u8();
                let e = Error::from_repr(e).unwrap();
                Self::Err(e)
            }
            VarType::TYPE_LIST => {
                let v = vec_iter.next().unwrap();
                let l = v.as_vector();
                Self::List(Arc::new(List { reader: l }))
            }
            VarType::TYPE_LABEL => {
                unimplemented!("Labels are not supported in actual values")
            }
            VarType::TYPE_MAP => {
                let v = vec_iter.next().unwrap();
                let m = v.as_vector();
                Self::Map(Arc::new(map::Map { reader: m }))
            }
        }
    }

    /// Push a copy of Self into a flexbuffer
    pub(crate) fn push_to(&self, item_vec: &mut VectorBuilder) {
        match self {
            Variant::None => {
                item_vec.push(VarType::TYPE_NONE as u8);
            }
            Variant::Obj(o) => {
                item_vec.push(VarType::TYPE_OBJ as u8);
                item_vec.push(o.0);
            }
            Variant::Int(i) => {
                item_vec.push(VarType::TYPE_INT as u8);
                item_vec.push(*i);
            }
            Variant::Float(f) => {
                item_vec.push(VarType::TYPE_FLOAT as u8);
                item_vec.push(f.to_f64().unwrap());
            }
            Variant::List(l) => {
                item_vec.push(VarType::TYPE_LIST as u8);
                let mut vb = item_vec.start_vector();
                // Then we iterate over the rest of the elements.
                for i in 0..l.reader.len() {
                    let item_reader = l.reader.idx(i);
                    let v = Variant::from_reader(item_reader);
                    v.push_item(&mut vb);
                }
                vb.end_vector();
            }
            Variant::Map(m) => {
                item_vec.push(VarType::TYPE_MAP as u8);
                let mut vb = item_vec.start_vector();
                let mut iter = m.reader.iter();
                // Now iterate over the pairs.
                for _ in 0..m.len() {
                    let k = iter.next().unwrap();
                    let v = iter.next().unwrap();
                    let key = Variant::from_reader(k);
                    let value = Variant::from_reader(v);
                    key.push_item(&mut vb);
                    value.push_item(&mut vb);
                }
                vb.end_vector();
            }
            Variant::Str(s) => {
                item_vec.push(VarType::TYPE_STR as u8);
                item_vec.push(s.as_string().as_str());
            }
            Variant::Err(e) => {
                item_vec.push(VarType::TYPE_ERR as u8);
                item_vec.push(*e as u8);
            }
        }
    }

    /// Push a copy along with an item vector
    pub(crate) fn push_item(&self, item_vec: &mut VectorBuilder) {
        let mut vb = item_vec.start_vector();
        self.push_to(&mut vb);
        vb.end_vector();
    }
}

impl PartialEq<Self> for Variant {
    fn eq(&self, other: &Self) -> bool {
        // If the types are different, they're not equal.
        match (self, other) {
            (Variant::Str(s), Variant::Str(o)) => s == o,
            (Variant::Int(s), Variant::Int(o)) => s == o,
            (Variant::Float(s), Variant::Float(o)) => s == o,
            (Variant::Obj(s), Variant::Obj(o)) => s == o,
            (Variant::List(s), Variant::List(o)) => s == o,
            (Variant::Map(s), Variant::Map(o)) => s == o,
            (Variant::Err(s), Variant::Err(o)) => s == o,
            (Variant::None, Variant::None) => true,
            _ => false,
        }
    }
}

impl Eq for Variant {}
