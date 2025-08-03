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

use crate::model::ValSet;
use bincode::{Decode, Encode};
use itertools::Itertools;
use moor_var::AsByteBuffer;
use moor_var::{BincodeAsByteBufferExt, Symbol};
use std::fmt::{Debug, Display, Formatter};
use uuid::Uuid;

pub trait HasUuid {
    fn uuid(&self) -> Uuid;
}

pub trait Named {
    fn matches_name(&self, name: Symbol) -> bool;
    fn names(&self) -> &[Symbol];
}

/// A container for verb or property defs.
/// Immutable, and can be iterated over in sequence, or searched by name.
#[derive(Eq, PartialEq, Clone, Hash, Encode, Decode)]
pub struct Defs<T: AsByteBuffer + Clone + Sized + HasUuid + Named + 'static> {
    contents: Vec<T>,
}

impl<T: AsByteBuffer + Clone + Sized + HasUuid + Named + 'static> BincodeAsByteBufferExt
    for Defs<T>
{
}

impl<T: AsByteBuffer + Clone + Sized + HasUuid + Named + 'static> Display for Defs<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let names = self
            .iter()
            .map(|p| p.names().iter().map(|s| s.to_string()).join(":"))
            .collect::<Vec<_>>()
            .join(", ");
        f.write_fmt(format_args!("{{{names}}}"))
    }
}

impl<T: AsByteBuffer + Clone + Sized + HasUuid + Named + 'static> Debug for Defs<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Just use the display
        Display::fmt(self, f)
    }
}

pub struct DefsIter<T: AsByteBuffer, I: Iterator<Item = T>> {
    vec_iter: I,
}

impl<T: AsByteBuffer, I: Iterator<Item = T>> Iterator for DefsIter<T, I> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.vec_iter.next()
    }
}

impl<T: AsByteBuffer + Clone + HasUuid + Named> ValSet<T> for Defs<T> {
    fn empty() -> Self {
        Self {
            contents: Vec::new(),
        }
    }

    fn from_items(items: &[T]) -> Self {
        Self {
            contents: items.to_vec(),
        }
    }
    fn iter(&self) -> impl Iterator<Item = T> {
        DefsIter {
            vec_iter: self.contents.iter().cloned(),
        }
    }
    // Provides the number of items in the buffer.
    fn len(&self) -> usize {
        self.iter().count()
    }

    fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

impl<T: AsByteBuffer + Clone + HasUuid + Named> IntoIterator for Defs<T> {
    type Item = T;
    type IntoIter = ::std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.contents.into_iter()
    }
}

impl<T: AsByteBuffer + Clone + HasUuid + Named> FromIterator<T> for Defs<T> {
    fn from_iter<X: IntoIterator<Item = T>>(iter: X) -> Self {
        Self {
            contents: iter.into_iter().collect(),
        }
    }
}

impl<T: AsByteBuffer + Clone + HasUuid + Named> Defs<T> {
    #[must_use]
    pub fn contains(&self, uuid: Uuid) -> bool {
        self.iter().any(|p| p.uuid() == uuid)
    }
    #[must_use]
    pub fn find(&self, uuid: &Uuid) -> Option<T> {
        self.iter().find(|p| &p.uuid() == uuid)
    }
    #[must_use]
    pub fn find_named(&self, name: Symbol) -> Vec<T> {
        self.iter().filter(|p| p.matches_name(name)).collect()
    }
    #[must_use]
    pub fn find_first_named(&self, name: Symbol) -> Option<T> {
        self.iter().find(|p| p.matches_name(name))
    }
    #[must_use]
    pub fn with_removed(&self, uuid: Uuid) -> Option<Self> {
        let vec: Vec<_> = self.iter().filter(|p| p.uuid() != uuid).collect();
        Some(Self { contents: vec })
    }

    #[must_use]
    pub fn with_all_removed(&self, uuids: &[Uuid]) -> Self {
        let mut vec = self.contents.clone();
        for uuid in uuids {
            vec.retain(|p| p.uuid() != *uuid);
        }
        Self { contents: vec }
    }

    // TODO Add builder patterns for these that construct in-place, building the buffer right in us.
    pub fn with_added(&self, v: T) -> Self {
        let vec: Vec<_> = self.iter().chain(std::iter::once(v)).collect();
        Self { contents: vec }
    }
    pub fn with_all_added(&self, v: &[T]) -> Self {
        let mut vec = self.contents.clone();
        vec.extend(v.iter().cloned());
        Self { contents: vec }
    }
    pub fn with_updated<F: Fn(&T) -> T>(&self, uuid: Uuid, f: F) -> Option<Self> {
        let mut did_update = false;
        let vec: Vec<_> = self
            .iter()
            .map(|p| {
                if p.uuid() == uuid {
                    did_update = true;
                    f(&p)
                } else {
                    p
                }
            })
            .collect();
        did_update.then(|| Self { contents: vec })
    }
}
