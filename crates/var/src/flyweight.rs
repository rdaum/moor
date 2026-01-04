// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
//! Literal syntax is:
//!
//! `< delegate, .slot = value, ..., { contents, ... } >`
//!
//! Flyweights are immutable. Use builtins such as `toflyweight`, `flyslots`, `flycontents`,
//! `flyslotset`, and `flyslotremove` to construct or modify them.

use crate::{List, Obj, Sequence, Symbol, Var};
use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Flyweight(Box<Inner>);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Inner {
    delegate: Obj,
    slots: Arc<Vec<(Symbol, Var)>>,
    contents: List,
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
    fn canonicalize_slots(mut slots: Vec<(Symbol, Var)>) -> Vec<(Symbol, Var)> {
        slots.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        let mut deduped = Vec::with_capacity(slots.len());
        for (key, value) in slots {
            match deduped.last_mut() {
                Some((last_key, last_value)) if *last_key == key => {
                    *last_value = value;
                }
                _ => deduped.push((key, value)),
            }
        }
        deduped
    }

    pub fn mk_flyweight(delegate: Obj, slots: &[(Symbol, Var)], contents: List) -> Self {
        let mut final_slots = Vec::with_capacity(slots.len());
        final_slots.extend(slots.iter().map(|(k, v)| (*k, v.clone())));
        let final_slots = Self::canonicalize_slots(final_slots);
        Self::from_parts_unchecked(delegate, Arc::new(final_slots), contents)
    }

    pub fn from_parts(delegate: Obj, slots: Arc<Vec<(Symbol, Var)>>, contents: List) -> Self {
        let slots = Self::canonicalize_slots((*slots).clone());
        Self::from_parts_unchecked(delegate, Arc::new(slots), contents)
    }

    fn from_parts_unchecked(delegate: Obj, slots: Arc<Vec<(Symbol, Var)>>, contents: List) -> Self {
        Self(Box::new(Inner {
            delegate,
            slots,
            contents,
        }))
    }
}

impl Flyweight {
    /// Return the slot with the given key, if it exists.
    pub fn get_slot(&self, key: &Symbol) -> Option<&Var> {
        let slots = self.0.slots.as_slice();
        match slots.binary_search_by(|(k, _)| k.cmp(key)) {
            Ok(pos) => Some(&slots[pos].1),
            Err(_) => None,
        }
    }

    pub fn slots(&self) -> Vec<(Symbol, Var)> {
        self.0.slots.to_vec()
    }

    pub fn slots_storage(&self) -> &Vec<(Symbol, Var)> {
        &self.0.slots
    }

    pub fn delegate(&self) -> &Obj {
        &self.0.delegate
    }

    pub fn contents(&self) -> &List {
        &self.0.contents
    }

    pub fn is_contents_empty(&self) -> bool {
        self.0.contents.is_empty()
    }

    pub fn with_slots_vec(&self, slots: Arc<Vec<(Symbol, Var)>>) -> Self {
        Self::from_parts(self.0.delegate, slots, self.0.contents.clone())
    }

    pub fn with_contents(&self, contents: List) -> Self {
        Self::from_parts_unchecked(self.0.delegate, self.0.slots.clone(), contents)
    }

    /// Add or update a slot, returning a new Flyweight with the change.
    pub fn add_slot(&self, key: Symbol, value: Var) -> Self {
        let mut new_slots = (*self.0.slots).clone();
        match new_slots.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(pos) => new_slots[pos] = (key, value),
            Err(pos) => new_slots.insert(pos, (key, value)),
        }
        Self::from_parts_unchecked(
            self.0.delegate,
            Arc::new(new_slots),
            self.0.contents.clone(),
        )
    }

    /// Remove a slot, returning a new Flyweight without that slot.
    pub fn remove_slot(&self, key: Symbol) -> Self {
        let mut new_slots = (*self.0.slots).clone();
        if let Ok(pos) = new_slots.binary_search_by(|(k, _)| k.cmp(&key)) {
            new_slots.remove(pos);
        }
        Self::from_parts_unchecked(
            self.0.delegate,
            Arc::new(new_slots),
            self.0.contents.clone(),
        )
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
        assert_eq!(fw2.slots_storage().len(), 3);
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
        assert_eq!(fw2.slots_storage().len(), 2);
    }
}
