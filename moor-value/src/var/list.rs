use std::cmp::min;
use std::ops::{Index, Range, RangeFrom, RangeFull, RangeTo};
use std::sync::Arc;

use bincode::{Decode, Encode};

use crate::var::variant::Variant;
use crate::var::{v_empty_list, Var};

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
        if self.inner.is_empty() {
            return v_empty_list();
        }
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

    pub fn insert(&self, index: isize, v: &Var) -> Var {
        let mut new_list = Vec::with_capacity(self.inner.len() + 1);
        let index = if index < 0 {
            0
        } else {
            min(index as usize, self.inner.len())
        };
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

    // "in" operator is case insensitive...
    pub fn contains(&self, v: &Var) -> bool {
        self.inner.contains(v)
    }

    // but bf_is_member is not... sigh.
    pub fn contains_case_sensitive(&self, v: &Var) -> bool {
        if let Variant::Str(s) = v.variant() {
            for item in self.inner.iter() {
                if let Variant::Str(s2) = item.variant() {
                    if s.as_str() == s2.as_str() {
                        return true;
                    }
                }
            }
            return false;
        }
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

impl From<List> for Vec<Var> {
    fn from(val: List) -> Self {
        val.inner[..].to_vec()
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

#[cfg(test)]
mod tests {
    use crate::var::list::List;
    use crate::var::{v_int, v_list};

    #[test]
    pub fn weird_moo_insert_scenarios() {
        // MOO supports negative indexes, which just floor to 0...
        let list = List::from_vec(vec![v_int(1), v_int(2), v_int(3)]);
        assert_eq!(
            list.insert(-1, &v_int(0)),
            v_list(vec![v_int(0), v_int(1), v_int(2), v_int(3)])
        );

        // MOO supports indexes beyond length of the list, which just append to the end...
        let list = List::from_vec(vec![v_int(1), v_int(2), v_int(3)]);
        assert_eq!(
            list.insert(100, &v_int(0)),
            v_list(vec![v_int(1), v_int(2), v_int(3), v_int(0)])
        );
    }
}
