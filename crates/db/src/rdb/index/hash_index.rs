use crate::rdb::index::Index;
use crate::rdb::tuples::{TupleId, TupleRef};
use crate::rdb::{RelationError, RelationInfo};
use moor_values::util::SliceRef;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct HashIndex {
    /// The information about the relation that this index is for.
    relation_info: RelationInfo,

    /// The domain-indexed tuples in this relation, which are in this case expressed purely as bytes.
    /// It is up to the caller to interpret them.
    index_domain: HashMap<SliceRef, HashSet<TupleId>>,

    /// Optional reverse index from codomain -> tuples, which is used to support (more) efficient
    /// reverse lookups.
    index_codomain: Option<HashMap<SliceRef, HashSet<TupleId>>>,
}

impl HashIndex {
    pub fn new(relation_info: RelationInfo) -> Self {
        let index_codomain = if relation_info.secondary_indexed {
            Some(HashMap::new())
        } else {
            None
        };
        Self {
            relation_info,
            index_domain: HashMap::new(),
            index_codomain,
        }
    }
}

pub struct Iter<'a> {
    iter: Box<dyn Iterator<Item = TupleId> + 'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = TupleId;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl Index for HashIndex {
    fn check_for_update(&self, domain: &SliceRef) -> Result<(), RelationError> {
        if let Some(domain_entry) = self.index_domain.get(domain) {
            if domain_entry.len() > 1 {
                return Err(RelationError::AmbiguousTuple);
            }
        }
        Ok(())
    }

    fn check_domain_constraints(&self, domain: &SliceRef) -> Result<(), RelationError> {
        if let Some(domain_entry) = self.index_domain.get(domain) {
            if self.relation_info.unique_domain && !domain_entry.is_empty() {
                return Err(RelationError::UniqueConstraintViolation);
            }
        }
        Ok(())
    }

    fn seek_domain(&self, domain: &SliceRef) -> Box<dyn Iterator<Item = TupleId> + '_> {
        let Some(set) = self.index_domain.get(domain) else {
            return Box::new(Iter {
                iter: Box::new(std::iter::empty()),
            });
        };

        Box::new(Iter {
            iter: Box::new(set.iter().cloned()),
        })
    }

    fn seek_codomain(&self, codomain: &SliceRef) -> Box<dyn Iterator<Item = TupleId> + '_> {
        let index_codomain = self.index_codomain.as_ref().expect("No codomain index");
        let Some(set) = index_codomain.get(codomain) else {
            return Box::new(Iter {
                iter: Box::new(std::iter::empty()),
            });
        };
        Box::new(Iter {
            iter: Box::new(set.iter().cloned()),
        })
    }

    fn index_tuple(&mut self, tuple_ref: &TupleRef) -> Result<(), RelationError> {
        let domain_entry = self.index_domain.entry(tuple_ref.domain()).or_default();
        if self.relation_info.unique_domain && !domain_entry.is_empty() {
            return Err(RelationError::UniqueConstraintViolation);
        }
        domain_entry.insert(tuple_ref.id());

        if let Some(index_codomain) = &mut self.index_codomain {
            index_codomain
                .entry(tuple_ref.codomain())
                .or_default()
                .insert(tuple_ref.id());
        }
        Ok(())
    }

    fn unindex_tuple(&mut self, tuple_ref: &TupleRef) {
        self.index_domain
            .entry(tuple_ref.domain())
            .or_default()
            .remove(&tuple_ref.id());

        if let Some(index_codomain) = &mut self.index_codomain {
            index_codomain
                .entry(tuple_ref.codomain())
                .or_default()
                .remove(&tuple_ref.id());
        }
    }

    fn clone_index(&self) -> Box<dyn Index + Send + Sync> {
        Box::new(self.clone())
    }

    fn clear(&mut self) {
        self.index_domain.clear();
        if let Some(index_codomain) = &mut self.index_codomain {
            index_codomain.clear();
        }
    }
}
