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

use ahash::AHasher;
use moor_common::model::{ObjSet, VerbDef};
use moor_var::{Obj, Symbol};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// Very naive per-tx_management verb resolution cache.
/// Not very aggressive here, it flushes on every verbdef mutation on any object, regardless of
/// inheritance chain.
/// It's net-new empty for every transaction every time.
/// The goal is really just to optimize tight-loop verb lookups
/// Lots of room for improvement here:
///     Keep a separate global cache which can be shared between transactions
///     Flush entries for an object only if inheritance chain touched
///     Speed up named lookups more for when verbs have many names
#[derive(Default)]
pub(crate) struct VerbResolutionCache {
    #[allow(clippy::type_complexity)]
    entries: RefCell<HashMap<(Obj, Symbol), Option<Vec<VerbDef>>, BuildHasherDefault<AHasher>>>,

    first_parent_with_verbs_cache: RefCell<HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>>>,
}

impl VerbResolutionCache {
    pub(crate) fn lookup_first_parent_with_verbs(&self, obj: &Obj) -> Option<Option<Obj>> {
        self.first_parent_with_verbs_cache
            .borrow()
            .get(obj)
            .cloned()
    }

    pub(crate) fn fill_first_parent_with_verbs(&self, obj: &Obj, parent: Option<Obj>) {
        self.first_parent_with_verbs_cache
            .borrow_mut()
            .insert(obj.clone(), parent);
    }

    pub(crate) fn lookup(&self, obj: &Obj, verb: &Symbol) -> Option<Option<Vec<VerbDef>>> {
        self.entries.borrow().get(&(obj.clone(), *verb)).cloned()
    }

    pub(crate) fn flush(&self) {
        self.entries.borrow_mut().clear();
        self.first_parent_with_verbs_cache.borrow_mut().clear();
    }

    pub(crate) fn fill_hit(&self, obj: &Obj, verb: &Symbol, verbs: &[VerbDef]) {
        self.entries
            .borrow_mut()
            .insert((obj.clone(), *verb), Some(verbs.to_vec()));
    }

    pub(crate) fn fill_miss(&self, obj: &Obj, verb: &Symbol) {
        self.entries.borrow_mut().insert((obj.clone(), *verb), None);
    }
}

#[derive(Default)]
pub struct AncestryCache {
    #[allow(clippy::type_complexity)]
    entries: RefCell<HashMap<Obj, ObjSet, BuildHasherDefault<AHasher>>>,
}

impl AncestryCache {
    pub(crate) fn lookup(&self, obj: &Obj) -> Option<ObjSet> {
        self.entries.borrow().get(obj).cloned()
    }

    pub(crate) fn flush(&self) {
        self.entries.borrow_mut().clear();
    }

    pub(crate) fn fill(&self, obj: &Obj, ancestors: &ObjSet) {
        self.entries
            .borrow_mut()
            .insert(obj.clone(), ancestors.clone());
    }
}
