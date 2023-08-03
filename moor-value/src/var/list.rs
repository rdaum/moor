use crate::var::variant::Variant;
use crate::var::Var;
use bincode::{Decode, Encode};
use std::ops::{Index, Range, RangeFrom, RangeFull, RangeTo};
use std::sync::Arc;

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct List {
    inner: Arc<Vec<Var>>,
}

impl List {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Vec::new()),
        }
    }

    pub fn from_vec(vec: Vec<Var>) -> Self {
        Self {
            inner: Arc::new(vec),
        }
    }

    pub fn push(&self, v: &Var) -> Var {
        let mut new_list = Vec::with_capacity(self.inner.len() + 1);
        new_list.extend_from_slice(&self.inner);
        new_list.push(v.clone());
        Var::new(Variant::List(Self::from_vec(new_list)))
    }

    pub fn pop(&self) -> Var {
        let mut new_list = Vec::with_capacity(self.inner.len() - 1);
        new_list.extend_from_slice(&self.inner[..self.inner.len() - 1]);
        Var::new(Variant::List(Self::from_vec(new_list)))
    }

    pub fn append(&self, other: &List) -> Var {
        let mut new_list = Vec::with_capacity(self.inner.len() + other.inner.len());
        new_list.extend_from_slice(&self.inner);
        new_list.extend_from_slice(&other.inner);
        Var::new(Variant::List(Self::from_vec(new_list)))
    }

    pub fn remove_at(&self, index: usize) -> Var {
        let mut new_list = Vec::with_capacity(self.inner.len() - 1);
        new_list.extend_from_slice(&self.inner[..index]);
        new_list.extend_from_slice(&self.inner[index + 1..]);
        Var::new(Variant::List(Self::from_vec(new_list)))
    }

    /// Remove the first found instance of the given value from the list.
    pub fn setremove(&self, value: &Var) -> Var {
        let mut new_list = Vec::with_capacity(self.inner.len() - 1);
        let mut found = false;
        for v in self.inner.iter() {
            if !found && v == value {
                found = true;
                continue;
            }
            new_list.push(v.clone());
        }
        Var::new(Variant::List(Self::from_vec(new_list)))
    }

    pub fn insert(&self, index: usize, v: &Var) -> Var {
        let mut new_list = Vec::with_capacity(self.inner.len() + 1);
        new_list.extend_from_slice(&self.inner[..index]);
        new_list.push(v.clone());
        new_list.extend_from_slice(&self.inner[index..]);
        Var::new(Variant::List(Self::from_vec(new_list)))
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn contains(&self, v: &Var) -> bool {
        self.inner.contains(v)
    }

    pub fn get(&self, index: usize) -> Option<&Var> {
        self.inner.get(index)
    }

    pub fn set(&self, index: usize, value: &Var) -> Var {
        let mut new_vec = self.inner.as_slice().to_vec();
        new_vec[index] = value.clone();
        Var::new(Variant::List(Self::from_vec(new_vec)))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Var> {
        self.inner.iter()
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<usize> for List {
    type Output = Var;

    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}

impl Index<Range<usize>> for List {
    type Output = [Var];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        &self.inner[index]
    }
}

impl Index<RangeFrom<usize>> for List {
    type Output = [Var];

    fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
        &self.inner[index]
    }
}

impl Index<RangeTo<usize>> for List {
    type Output = [Var];

    fn index(&self, index: RangeTo<usize>) -> &Self::Output {
        &self.inner[index]
    }
}

impl Index<RangeFull> for List {
    type Output = [Var];

    fn index(&self, index: RangeFull) -> &Self::Output {
        &self.inner[index]
    }
}
