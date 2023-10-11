use crate::tuplebox::RelationId;
use moor_values::util::slice_ref::SliceRef;
use std::collections::HashSet;

/// Represents a 'canonical' base binary relation, which is a set of tuples of domain -> codomain.
/// In this representation we do not differentiate the Domain & Codomain type; they are
/// stored and managed as raw byte-arrays and it is up to layers above to interpret the the values
/// correctly.
// TODO: Add some kind of 'type' flag to the relation & tuple values, so that we can do
//   type-checking on the values, though for our purposes this may be overkill at this time.
#[derive(Clone)]
pub struct BaseRelation {
    pub(crate) id: RelationId,

    /// The last successful committer's tx timestamp
    pub(crate) ts: u64,

    /// The domain-indexed tuples in this relation, which are in this case expressed purely as bytes.
    /// It is up to the caller to interpret them.
    domain_tuples: im::HashMap<Vec<u8>, TupleValue>,

    /// Optional reverse index from codomain -> domains, which is used to support (more) efficient
    /// reverse lookups.
    codomain_domain: Option<im::HashMap<Vec<u8>, HashSet<Vec<u8>>>>,
}

impl BaseRelation {
    pub fn new(id: RelationId, timestamp: u64) -> Self {
        Self {
            id,
            ts: timestamp,
            domain_tuples: im::HashMap::new(),
            codomain_domain: None,
        }
    }
    /// Add a secondary index onto the given relation to map its codomain back to its domain.
    /// If the relation already has a secondary index, this will panic.
    /// If there is no relation with the given ID, this will panic.
    /// If the relation already has tuples, they will be indexed.
    pub fn add_secondary_index(&mut self) {
        if self.codomain_domain.is_some() {
            panic!("Relation already has a secondary index");
        }
        self.codomain_domain = Some(im::HashMap::new());
        for (domain, tuple) in self.domain_tuples.iter() {
            self.codomain_domain
                .as_mut()
                .unwrap()
                .entry(tuple.v.as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .insert(domain.clone());
        }
    }
    pub fn seek_by_domain(&self, domain: &[u8]) -> Option<&TupleValue> {
        self.domain_tuples.get(domain)
    }
    pub fn seek_by_codomain(&self, codomain: &[u8]) -> HashSet<Vec<u8>> {
        // Attempt to seek on codomain without an index is a panic.
        // We could do full-scan, but in this case we're going to assume that the caller knows
        // what they're doing.
        let codomain_domain = self.codomain_domain.as_ref().expect("No codomain index");
        codomain_domain
            .get(codomain)
            .map(|v| v.clone())
            .unwrap_or_else(HashSet::new)
    }
    pub fn remove_by_domain(&mut self, domain: &[u8]) {
        self.domain_tuples.remove(domain);
    }

    /// Update or insert a tuple into the relation, either.
    pub fn upsert_tuple(&mut self, domain: Vec<u8>, ts: u64, codomain: SliceRef) {
        // We first need to know if we're updating an existing mapping, because that will mostly
        // effect how we handle the reverse index.
        let existing = self.domain_tuples.get(&domain);

        // Since ownership will be taken over 'codomain' & its bytes, if we need to use it for
        // reverse indexing, we will need to stash a copy now. So we start with the reverse index
        self.update_secondary_index(
            domain.as_slice(),
            existing.map(|v| v.v.clone()),
            Some(codomain.clone()),
        );

        // Domain is easy, we just update the domain_tuples map.
        self.domain_tuples
            .insert(domain, TupleValue { ts, v: codomain });
    }

    fn update_secondary_index(
        &mut self,
        domain: &[u8],
        old_codomain: Option<SliceRef>,
        new_codomain: Option<SliceRef>,
    ) {
        // If there's no secondary index, we don't need to do anything.
        let Some(index) = &mut self.codomain_domain else {
            return;
        };

        // Clear out the old entry, if there was one.
        if let Some(old_codomain) = old_codomain {
            index
                .entry(old_codomain.as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .remove(domain);
        }
        if let Some(new_codomain) = new_codomain {
            index
                .entry(new_codomain.as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .insert(domain.to_vec());
        }
    }
}

/// Storage for individual tuple codomain values in a relation, which includes the timestamp of
/// the last successful committer and the raw byte arrays for the value.
#[derive(Clone)]
pub struct TupleValue {
    pub(crate) ts: u64,
    pub(crate) v: SliceRef,
}
