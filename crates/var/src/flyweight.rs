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

//! A "flyweight" is a lightweight object type which consists only of a delegate, a set of
//! "slots" (symbol -> var pairs), a single "contents" value (a list)
//!
//! It is a reference counted, immutable bucket of slots.
//! Verbs called on it dispatch to the delegate.  `this`, `caller`, perms etc all resolve to the
//!  actual flyweight.
//! Properties are resolved in the slots, then the delegate.
//! Verbs are resolved in the delegate.
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

use crate::{Error, List, Obj, Sequence, Symbol, Var, Variant, error::ErrorCode::E_TYPE};
use std::{
    fmt::{Debug, Formatter},
    hash::Hash,
};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Flyweight(Box<Inner>);

#[derive(Clone, PartialOrd, Ord, Eq)]
struct Inner {
    delegate: Obj,
    slots: imbl::OrdMap<Symbol, Var>,
    contents: List,
}

impl PartialEq for Inner {
    fn eq(&self, other: &Self) -> bool {
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
    }
}

impl Debug for Flyweight {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<{:?}, {:?}, {:?}>",
            self.0.delegate, self.0.slots, self.0.contents
        )
    }
}

impl Flyweight {
    pub fn mk_flyweight(delegate: Obj, slots: &[(Symbol, Var)], contents: List) -> Self {
        Self(Box::new(Inner {
            delegate,
            slots: slots.iter().cloned().collect(),
            contents,
        }))
    }
}

impl Flyweight {
    /// Return the slot with the given key, if it exists.
    pub fn get_slot(&self, key: &Symbol) -> Option<&Var> {
        self.0.slots.get(key)
    }

    pub fn slots(&self) -> Vec<(Symbol, Var)> {
        self.0.slots.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    pub fn delegate(&self) -> &Obj {
        &self.0.delegate
    }

    pub fn contents(&self) -> &List {
        &self.0.contents
    }

    /// Add or update a slot, returning a new Flyweight with the change.
    pub fn add_slot(&self, key: Symbol, value: Var) -> Self {
        let mut new_slots = self.0.slots.clone();
        new_slots.insert(key, value);
        Self(Box::new(Inner {
            delegate: self.0.delegate,
            slots: new_slots,
            contents: self.0.contents.clone(),
        }))
    }

    /// Remove a slot, returning a new Flyweight without that slot.
    pub fn remove_slot(&self, key: Symbol) -> Self {
        let mut new_slots = self.0.slots.clone();
        new_slots.remove(&key);
        Self(Box::new(Inner {
            delegate: self.0.delegate,
            slots: new_slots,
            contents: self.0.contents.clone(),
        }))
    }

    /// Get slots as a map (for the slots() builtin).
    pub fn slots_as_map(&self) -> imbl::OrdMap<Symbol, Var> {
        self.0.slots.clone()
    }

    pub fn with_new_contents(&self, new_contents: List) -> Var {
        let fi = Inner {
            delegate: self.0.delegate,
            slots: self.0.slots.clone(),
            contents: new_contents,
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
        let Some(new_contents_as_list) = new_contents.as_list() else {
            return Err(E_TYPE.msg("invalid contents type in flyweight"));
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }

    fn push(&self, value: &Var) -> Result<Var, Error> {
        let new_contents = self.0.contents.push(value)?;
        let Some(new_contents_as_list) = new_contents.as_list() else {
            return Err(E_TYPE.msg("invalid contents type in flyweight"));
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }

    fn insert(&self, index: usize, value: &Var) -> Result<Var, Error> {
        let new_contents = self.0.contents.insert(index, value)?;
        let Some(new_contents_as_list) = new_contents.as_list() else {
            return Err(E_TYPE.msg("invalid contents type in flyweight"));
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }

    fn range(&self, from: isize, to: isize) -> Result<Var, Error> {
        self.0.contents.range(from, to)
    }

    fn range_set(&self, from: isize, to: isize, with: &Var) -> Result<Var, Error> {
        let new_contents = self.0.contents.range_set(from, to, with)?;
        let Some(new_contents_as_list) = new_contents.as_list() else {
            return Err(E_TYPE.msg("invalid contents type in flyweight"));
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }

    fn append(&self, other: &Var) -> Result<Var, Error> {
        let new_contents = self.0.contents.append(other)?;
        let Some(new_contents_as_list) = new_contents.as_list() else {
            return Err(E_TYPE.msg("invalid contents type in flyweight"));
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }

    fn remove_at(&self, index: usize) -> Result<Var, Error> {
        let new_contents = self.0.contents.remove_at(index)?;
        let Some(new_contents_as_list) = new_contents.as_list() else {
            return Err(E_TYPE.msg("invalid contents type in flyweight"));
        };
        Ok(self.with_new_contents(new_contents_as_list.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Obj, v_int};

    #[test]
    fn test_add_slot_doesnt_clobber_existing_slots() {
        // This is the critical test - adding a new slot should not clobber existing slots
        let sym_dobj = Symbol::mk("dobj");
        let sym_dobj_name = Symbol::mk("dobj_name");
        let sym_iobj = Symbol::mk("iobj");

        // Create initial flyweight with multiple slots
        let initial_slots = vec![(sym_dobj, v_int(12)), (sym_iobj, v_int(13))];
        let fw = Flyweight::mk_flyweight(Obj::mk_id(0), &initial_slots, List::mk_list(&[]));

        // Add a new slot with a similar name
        let fw2 = fw.add_slot(sym_dobj_name, v_int(100));

        // Original slots should still have their values
        assert_eq!(fw2.get_slot(&sym_dobj), Some(&v_int(12)));
        assert_eq!(fw2.get_slot(&sym_iobj), Some(&v_int(13)));
        assert_eq!(fw2.get_slot(&sym_dobj_name), Some(&v_int(100)));
        assert_eq!(fw2.slots().len(), 3);
    }

    #[test]
    fn test_add_slot_updates_existing_slot() {
        // Adding a slot that already exists should update it
        let sym_a = Symbol::mk("a");
        let sym_b = Symbol::mk("b");

        let initial_slots = vec![(sym_a, v_int(1)), (sym_b, v_int(2))];
        let fw = Flyweight::mk_flyweight(Obj::mk_id(0), &initial_slots, List::mk_list(&[]));

        // Update existing slot
        let fw2 = fw.add_slot(sym_a, v_int(999));

        assert_eq!(fw2.get_slot(&sym_a), Some(&v_int(999)));
        assert_eq!(fw2.get_slot(&sym_b), Some(&v_int(2)));
        assert_eq!(fw2.slots().len(), 2);
    }
}
