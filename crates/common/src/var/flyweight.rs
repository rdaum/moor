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

//! A "flyweight" is a lightweight object type which consists only of a delegate, a set of
//! "slots" (symbol -> var pairs), a single "contents" value (a list), and an optional sealed/signed
//! state which makes it opaque.
//!
//! It is a reference counted, immutable bucket of slots.
//! Verbs called on it dispatch to the delegate.  `this`, `caller`, perms etc all resolve to the
//!  actual flyweight.
//! Properties are resolved in the slots, then the delegate.
//! Verbs are resolved in the delegate.
//!
//! Type protocol for an unsealed flyweight is a sequence. It behaves like a list around the
//! contents portion.  The slots are accessed with a property access notation.
//!
//! The delegate is visible via the `.delegate` property access.
//! The slots can be listed with a `.slots` property access.
//! It is therefore illegal for a slot to have the name `slots` or `delegate`.
//!
//! So appending, etc can be done like:
//! `<  x.delegate, x.slots, {@x, y} >`
//!
//! Literal syntax is:
//!
//! `< delegate, [ slot -> value, ... ], contents >`
//!
//! Setting the secret is done with the `seal(priv-key, secret)` function, which signs the flyweight with a
//! private key. The flyweight then becomes opaque without calling `unseal(pub-key, secret)` with the
//! right public key and the correct secret.
//!
//! When a flyweight is sealed, `.slots`, and `.delegate` (and literal output) will not
//! be available without calling `unseal()` with the correct secret.
//!
//! The purpose is to support two kinds of scenarios:
//!    "Lightweight" non-persistent patterns where a full object would be overkill.
//!    "Capability" style security patterns where the flyweight is a capability to a full object.
//!
//!
//! The structure of the flyweight also resembles an XML/HTML node, with a delegate as the tag name,
//!  slots as the attributes, and contents as the inner text/nodes.

use crate::Error::E_TYPE;
use crate::{Error, List, Obj, Sequence, Symbol, Var, Variant};
use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use std::fmt::{Debug, Formatter};
use std::hash::Hash;

#[derive(Clone, Encode, Decode, PartialEq, Eq, PartialOrd, Ord)]
pub struct Flyweight(Box<Inner>);

#[derive(Clone, PartialOrd, Ord, Eq)]
struct Inner {
    delegate: Obj,
    slots: im::Vector<(Symbol, Var)>,
    contents: List,
    /// If `secret` is present it's a string signed with a key-pair that can be used to unseal
    /// the flyweight.
    /// The meaning of the key is up to the application.
    seal: Option<String>,
}

impl PartialEq for Inner {
    fn eq(&self, other: &Self) -> bool {
        // Two flyweights where there are 'secrets' involved are never eq.
        // To avoid leaking information about the secret.
        if self.seal.is_some() || other.seal.is_some() {
            return false;
        }
        self.delegate == other.delegate
            && self.slots == other.slots
            && self.contents == other.contents
    }
}

impl Hash for Flyweight {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.delegate.hash(state);
        self.0.slots.hash(state);
        self.0.contents.hash(state);
        self.0.seal.hash(state);
    }
}

impl Encode for Inner {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.delegate.encode(encoder)?;
        self.slots.len().encode(encoder)?;
        for (k, v) in &self.slots {
            k.encode(encoder)?;
            v.encode(encoder)?;
        }
        self.contents.encode(encoder)?;
        self.seal.encode(encoder)
    }
}

impl Decode for Inner {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let delegate = Obj::decode(decoder)?;
        let len = usize::decode(decoder)?;
        let mut slots = im::Vector::new();
        for _ in 0..len {
            let k = Symbol::decode(decoder)?;
            let v = Var::decode(decoder)?;
            slots.push_back((k, v));
        }
        let contents = List::decode(decoder)?;
        let seal = Option::<String>::decode(decoder)?;
        Ok(Self {
            delegate,
            slots,
            contents,
            seal,
        })
    }
}

impl<'a> BorrowDecode<'a> for Inner {
    fn borrow_decode<D: BorrowDecoder<'a>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let delegate = Obj::borrow_decode(decoder)?;
        let len = usize::borrow_decode(decoder)?;
        let mut slots = im::Vector::new();
        for _ in 0..len {
            let k = Symbol::borrow_decode(decoder)?;
            let v = Var::borrow_decode(decoder)?;
            slots.push_back((k, v));
        }
        let contents = List::borrow_decode(decoder)?;
        let seal = Option::<String>::borrow_decode(decoder)?;
        Ok(Self {
            delegate,
            slots,
            contents,
            seal,
        })
    }
}
impl Debug for Flyweight {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.0.seal.is_some() {
            write!(f, "<sealed flyweight>")
        } else {
            write!(
                f,
                "<{:?}, {:?}, {:?}>",
                self.0.delegate, self.0.slots, self.0.contents
            )
        }
    }
}

impl Flyweight {
    pub fn mk_flyweight(
        delegate: Obj,
        slots: &[(Symbol, Var)],
        contents: List,
        seal: Option<String>,
    ) -> Self {
        Self(Box::new(Inner {
            delegate,
            slots: slots.into(),
            contents,
            seal,
        }))
    }
}

impl Flyweight {
    /// Return the slot with the given key, if it exists.
    pub fn get_slot(&self, key: &Symbol) -> Option<&Var> {
        self.0.slots.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn slots(&self) -> &im::Vector<(Symbol, Var)> {
        &self.0.slots
    }

    pub fn delegate(&self) -> &Obj {
        &self.0.delegate
    }

    pub fn seal(&self) -> Option<&String> {
        self.0.seal.as_ref()
    }

    pub fn contents(&self) -> &List {
        &self.0.contents
    }

    pub fn is_sealed(&self) -> bool {
        self.0.seal.is_some()
    }

    pub fn with_new_contents(&self, new_contents: List) -> Var {
        let fi = Inner {
            delegate: self.0.delegate.clone(),
            slots: self.0.slots.clone(),
            contents: new_contents,
            seal: self.0.seal.clone(),
        };
        let fl = Flyweight(Box::new(fi));
        let variant = Variant::Flyweight(fl);
        Var::from_variant(variant)
    }
}

impl Sequence for Flyweight {
    fn is_empty(&self) -> bool {
        self.0.contents.is_empty()
    }

    fn len(&self) -> usize {
        self.0.contents.len()
    }

    fn index_in(&self, value: &Var, case_sensitive: bool) -> Result<Option<usize>, Error> {
        self.0.contents.index_in(value, case_sensitive)
    }

    fn contains(&self, value: &Var, case_sensitive: bool) -> Result<bool, Error> {
        self.0.contents.contains(value, case_sensitive)
    }

    fn index(&self, index: usize) -> Result<Var, Error> {
        self.0.contents.index(index)
    }

    fn index_set(&self, index: usize, value: &Var) -> Result<Var, Error> {
        let new_contents = self.0.contents.index_set(index, value)?;
        let Variant::List(new_contents_as_list) = new_contents.variant() else {
            return Err(E_TYPE);
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }

    fn push(&self, value: &Var) -> Result<Var, Error> {
        let new_contents = self.0.contents.push(value)?;
        let Variant::List(new_contents_as_list) = new_contents.variant() else {
            return Err(E_TYPE);
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }

    fn insert(&self, index: usize, value: &Var) -> Result<Var, Error> {
        let new_contents = self.0.contents.insert(index, value)?;
        let Variant::List(new_contents_as_list) = new_contents.variant() else {
            return Err(E_TYPE);
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }

    fn range(&self, from: isize, to: isize) -> Result<Var, Error> {
        self.0.contents.range(from, to)
    }

    fn range_set(&self, from: isize, to: isize, with: &Var) -> Result<Var, Error> {
        let new_contents = self.0.contents.range_set(from, to, with)?;
        let Variant::List(new_contents_as_list) = new_contents.variant() else {
            return Err(E_TYPE);
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }

    fn append(&self, other: &Var) -> Result<Var, Error> {
        let new_contents = self.0.contents.append(other)?;
        let Variant::List(new_contents_as_list) = new_contents.variant() else {
            return Err(E_TYPE);
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }

    fn remove_at(&self, index: usize) -> Result<Var, Error> {
        let new_contents = self.0.contents.remove_at(index)?;
        let Variant::List(new_contents_as_list) = new_contents.variant() else {
            return Err(E_TYPE);
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }
}
