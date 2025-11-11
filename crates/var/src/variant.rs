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

use crate::{
    Associative, ByteSized, Error, ErrorCode, Obj, Sequence, Symbol, binary::Binary,
    flyweight::Flyweight, lambda::Lambda, list::List, map, string,
};
use std::{
    cmp::Ordering,
    fmt::{Debug, Formatter},
    hash::{Hash, Hasher},
    sync::Arc,
};

/// Our series of types
#[derive(Clone)]
pub enum Variant {
    None,
    Bool(bool),
    Obj(Obj),
    Int(i64),
    Float(f64),
    List(List),
    Str(string::Str),
    Map(map::Map),
    Err(Arc<Error>),
    Flyweight(Flyweight),
    Sym(Symbol),
    Binary(Box<Binary>),
    Lambda(Box<Lambda>),
}

impl Drop for Variant {
    #[inline(always)]
    fn drop(&mut self) {
        // Force inlining of the discriminant check and field drops
        // This should eliminate the 61% function call overhead we see in perf
        match self {
            Variant::None
            | Variant::Bool(_)
            | Variant::Obj(_)
            | Variant::Int(_)
            | Variant::Float(_)
            | Variant::Sym(_) => {
                // These types have trivial drops - no work needed
            }
            Variant::List(_)
            | Variant::Str(_)
            | Variant::Map(_)
            | Variant::Err(_)
            | Variant::Flyweight(_)
            | Variant::Binary(_)
            | Variant::Lambda(_) => {
                // Let the compiler generate the appropriate drop code for these
                // The fields will be dropped automatically when this function returns
            }
        }
    }
}

impl Hash for Variant {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Variant::None => 0.hash(state),
            Variant::Bool(b) => b.hash(state),
            Variant::Obj(o) => o.hash(state),
            Variant::Int(i) => i.hash(state),
            Variant::Float(f) => f.to_bits().hash(state),
            Variant::List(l) => l.hash(state),
            Variant::Str(s) => s.hash(state),
            Variant::Map(m) => m.hash(state),
            Variant::Err(e) => e.hash(state),
            Variant::Flyweight(f) => f.hash(state),
            Variant::Sym(s) => s.hash(state),
            Variant::Binary(b) => b.hash(state),
            Variant::Lambda(l) => std::ptr::hash(&*l.0.body.0, state),
        }
    }
}

impl Ord for Variant {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Variant::None, Variant::None) => Ordering::Equal,
            (Variant::Bool(l), Variant::Bool(r)) => l.cmp(r),
            (Variant::Obj(l), Variant::Obj(r)) => l.cmp(r),
            (Variant::Int(l), Variant::Int(r)) => l.cmp(r),
            (Variant::Float(l), Variant::Float(r)) => l.total_cmp(r),
            (Variant::List(l), Variant::List(r)) => l.cmp(r),
            (Variant::Str(l), Variant::Str(r)) => l.cmp(r),
            (Variant::Map(l), Variant::Map(r)) => l.cmp(r),
            (Variant::Err(l), Variant::Err(r)) => l.cmp(r),
            (Variant::Flyweight(l), Variant::Flyweight(r)) => l.cmp(r),
            (Variant::Sym(l), Variant::Sym(r)) => l.cmp(r),
            (Variant::Binary(l), Variant::Binary(r)) => l.cmp(r),
            (Variant::Lambda(l), Variant::Lambda(r)) => {
                use crate::program::program::PrgInner;
                let l_ptr = &*l.0.body.0 as *const PrgInner;
                let r_ptr = &*r.0.body.0 as *const PrgInner;
                l_ptr.cmp(&r_ptr)
            }

            (Variant::Int(l), Variant::Float(r)) => (*l as f64).total_cmp(r),
            (Variant::Float(l), Variant::Int(r)) => l.total_cmp(&(*r as f64)),

            (Variant::None, _) => Ordering::Less,
            (_, Variant::None) => Ordering::Greater,
            (Variant::Bool(_), _) => Ordering::Less,
            (_, Variant::Bool(_)) => Ordering::Greater,
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
            (Variant::Flyweight(_), _) => Ordering::Less,
            (_, Variant::Flyweight(_)) => Ordering::Greater,
            (Variant::Sym(_), _) => Ordering::Less,
            (_, Variant::Sym(_)) => Ordering::Greater,
            (Variant::Binary(_), _) => Ordering::Less,
            (_, Variant::Binary(_)) => Ordering::Greater,
            (Variant::Lambda(_), _) => Ordering::Greater,
            (_, Variant::Lambda(_)) => Ordering::Less,
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
            Variant::Bool(b) => write!(f, "{}", *b),
            Variant::Obj(o) => write!(f, "Object({o})"),
            Variant::Int(i) => write!(f, "Integer({i})"),
            Variant::Float(fl) => write!(f, "Float({fl})"),
            Variant::List(l) => {
                // Items...
                let r = l.iter();
                let i: Vec<_> = r.collect();
                write!(f, "List([size = {}, items = {:?}])", l.len(), i)
            }
            Variant::Str(s) => write!(f, "String({:?})", s.as_str()),
            Variant::Map(m) => {
                // Items...
                let r = m.iter();
                let i: Vec<_> = r.collect();
                write!(f, "Map([size = {}, items = {:?}])", m.len(), i)
            }
            Variant::Err(e) => write!(f, "Error({e:?})"),
            Variant::Flyweight(fl) => write!(f, "Flyweight({fl:?})"),
            Variant::Sym(s) => write!(f, "Symbol({s})"),
            Variant::Binary(b) => write!(f, "Binary({} bytes)", b.len()),
            Variant::Lambda(l) => {
                use crate::program::opcode::ScatterLabel;
                let param_str =
                    l.0.params
                        .labels
                        .iter()
                        .map(|label| match label {
                            ScatterLabel::Required(_) => "x",
                            ScatterLabel::Optional(_, _) => "?x",
                            ScatterLabel::Rest(_) => "@x",
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                write!(f, "Lambda(({param_str}))")
            }
        }
    }
}

impl PartialEq<Self> for Variant {
    fn eq(&self, other: &Self) -> bool {
        // If the types are different, they're not equal.
        match (self, other) {
            (Variant::Bool(s), Variant::Bool(o)) => s == o,
            (Variant::Str(s), Variant::Str(o)) => s == o,
            (Variant::Sym(s), Variant::Sym(o)) => s == o,
            (Variant::Int(s), Variant::Int(o)) => s == o,
            (Variant::Float(s), Variant::Float(o)) => s == o,
            (Variant::Obj(s), Variant::Obj(o)) => s == o,
            (Variant::List(s), Variant::List(o)) => s == o,
            (Variant::Map(s), Variant::Map(o)) => s == o,
            (Variant::Err(s), Variant::Err(o)) => s == o,
            (Variant::Flyweight(s), Variant::Flyweight(o)) => s == o,
            (Variant::Binary(s), Variant::Binary(o)) => s == o,
            (Variant::Lambda(s), Variant::Lambda(o)) => s == o,
            (Variant::None, Variant::None) => true,
            _ => false,
        }
    }
}

impl ByteSized for Variant {
    fn size_bytes(&self) -> usize {
        match self {
            Variant::List(l) => l.iter().map(|e| e.size_bytes()).sum::<usize>(),
            Variant::Str(s) => s.as_arc_string().len(),
            Variant::Map(m) => m
                .iter()
                .map(|(k, v)| k.size_bytes() + v.size_bytes())
                .sum::<usize>(),
            Variant::Err(e) => {
                e.msg.as_ref().map(|s| s.len()).unwrap_or(0)
                    + e.value.as_ref().map(|s| s.size_bytes()).unwrap_or(0)
                    + size_of::<ErrorCode>()
            }
            Variant::Flyweight(f) => {
                size_of::<Obj>()
                    + f.contents().iter().map(|e| e.size_bytes()).sum::<usize>()
                    + f.slots()
                        .iter()
                        .map(|(_, v)| size_of::<Symbol>() + v.size_bytes())
                        .sum::<usize>()
            }
            Variant::Binary(b) => b.as_byte_view().len(),
            Variant::Lambda(l) => size_of_val(l),
            _ => size_of::<Variant>(),
        }
    }
}
impl Eq for Variant {}
