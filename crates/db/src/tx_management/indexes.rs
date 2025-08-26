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

use crate::Timestamp;
use ahash::AHasher;
use std::any::Any;
use std::hash::{BuildHasherDefault, Hash};

#[derive(Debug, Clone, PartialEq)]
pub struct Entry<T: Clone + PartialEq> {
    pub ts: Timestamp,
    pub value: T,
}

/// Trait for different indexing strategies for relation caches
pub trait RelationIndex<Domain, Codomain>: Send + Sync
where
    Domain: Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    fn insert_entry(
        &mut self,
        ts: Timestamp,
        domain: Domain,
        codomain: Codomain,
    ) -> Option<Entry<Codomain>>;
    fn insert_tombstone(&mut self, _ts: Timestamp, domain: Domain) -> Option<Entry<Codomain>>;
    fn index_lookup(&self, domain: &Domain) -> Option<&Entry<Codomain>>;
    fn remove_entry(&mut self, domain: &Domain) -> Option<Entry<Codomain>>;
    fn iter(&self) -> Box<dyn Iterator<Item = (&Domain, &Entry<Codomain>)> + '_>;
    fn len(&self) -> usize;
    fn fork(&self) -> Box<dyn RelationIndex<Domain, Codomain>>;
    fn as_any(&self) -> &dyn Any;

    /// Get all domains that map to the given codomain
    /// Returns empty Vec if no secondary index is supported
    fn get_by_codomain(&self, _codomain: &Codomain) -> Vec<Domain> {
        Vec::new()
    }

    /// Whether this index supports secondary lookups
    fn has_secondary_index(&self) -> bool {
        false
    }

    /// Whether the provider has been fully loaded into this index
    fn is_provider_fully_loaded(&self) -> bool {
        false
    }

    /// Mark the provider as fully loaded
    fn set_provider_fully_loaded(&mut self, loaded: bool);
}

/// Hash-based implementation of RelationIndex using im::HashMap
#[derive(Clone)]
pub struct HashRelationIndex<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq,
{
    /// Internal index of the cache.
    pub entries: im::HashMap<Domain, Entry<Codomain>, BuildHasherDefault<AHasher>>,

    /// Optional secondary index: Codomain -> Set<Domain>
    /// Only present if secondary indexing is enabled
    secondary_index:
        Option<im::HashMap<Codomain, im::HashSet<Domain>, BuildHasherDefault<AHasher>>>,

    /// Whether the provider has been fully loaded
    provider_fully_loaded: bool,
}

// Basic constructor for any Domain that supports Hash
impl<Domain, Codomain> HashRelationIndex<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            entries: Default::default(),
            secondary_index: None,
            provider_fully_loaded: false,
        }
    }
}

// Secondary index constructor - requires both Domain and Codomain to support Hash+Eq
impl<Domain, Codomain> HashRelationIndex<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
{
    pub fn new_with_secondary() -> Self {
        Self {
            entries: Default::default(),
            secondary_index: Some(Default::default()),
            provider_fully_loaded: false,
        }
    }
}

// Basic implementation for all types
impl<Domain, Codomain> RelationIndex<Domain, Codomain> for HashRelationIndex<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    fn insert_entry(
        &mut self,
        ts: Timestamp,
        domain: Domain,
        codomain: Codomain,
    ) -> Option<Entry<Codomain>> {
        // Update primary index
        self.entries.insert(
            domain.clone(),
            Entry {
                ts,
                value: codomain.clone(),
            },
        )
    }

    fn insert_tombstone(&mut self, _ts: Timestamp, domain: Domain) -> Option<Entry<Codomain>> {
        self.entries.remove(&domain)
    }

    fn index_lookup(&self, domain: &Domain) -> Option<&Entry<Codomain>> {
        self.entries.get(domain)
    }

    fn remove_entry(&mut self, domain: &Domain) -> Option<Entry<Codomain>> {
        self.entries.remove(domain)
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (&Domain, &Entry<Codomain>)> + '_> {
        Box::new(self.entries.iter())
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn fork(&self) -> Box<dyn RelationIndex<Domain, Codomain>> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn get_by_codomain(&self, _codomain: &Codomain) -> Vec<Domain> {
        // This will only work if created with new_with_secondary()
        // and both Domain and Codomain implement Hash+Eq
        Vec::new()
    }

    fn has_secondary_index(&self) -> bool {
        self.secondary_index.is_some()
    }

    fn is_provider_fully_loaded(&self) -> bool {
        self.provider_fully_loaded
    }

    fn set_provider_fully_loaded(&mut self, loaded: bool) {
        self.provider_fully_loaded = loaded;
    }
}

/// Wrapper that adds proper secondary index support to HashRelationIndex
pub struct SecondaryIndexRelation<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
{
    inner: HashRelationIndex<Domain, Codomain>,
}

impl<Domain, Codomain> SecondaryIndexRelation<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            inner: HashRelationIndex::new_with_secondary(),
        }
    }

    fn remove_from_secondary(&mut self, codomain: &Codomain, domain: &Domain) {
        if let Some(ref mut secondary) = self.inner.secondary_index
            && let Some(mut domain_set) = secondary.remove(codomain)
        {
            domain_set.remove(domain);
            // Only reinsert if set is not empty - this prevents memory leaks
            if !domain_set.is_empty() {
                secondary.insert(codomain.clone(), domain_set);
            }
            // If empty, we just let it get dropped - automatic cleanup
        }
    }

    fn add_to_secondary(&mut self, codomain: &Codomain, domain: &Domain) {
        if let Some(ref mut secondary) = self.inner.secondary_index {
            secondary
                .entry(codomain.clone())
                .or_insert_with(im::HashSet::new)
                .insert(domain.clone());
        }
    }
}

impl<Domain, Codomain> RelationIndex<Domain, Codomain> for SecondaryIndexRelation<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
{
    fn insert_entry(
        &mut self,
        ts: Timestamp,
        domain: Domain,
        codomain: Codomain,
    ) -> Option<Entry<Codomain>> {
        // Get old entry for secondary index cleanup
        let old_entry = self.inner.entries.get(&domain).cloned();

        // Update primary index using inner's method (handles tombstones automatically)
        let result = self
            .inner
            .insert_entry(ts, domain.clone(), codomain.clone());

        // Update secondary index
        if let Some(ref old) = old_entry {
            // Remove from old codomain's set
            self.remove_from_secondary(&old.value, &domain);
        }
        // Add to new codomain's set
        self.add_to_secondary(&codomain, &domain);

        result
    }

    fn insert_tombstone(&mut self, ts: Timestamp, domain: Domain) -> Option<Entry<Codomain>> {
        // Get old entry for secondary index cleanup
        let old_entry = self.inner.entries.get(&domain).cloned();

        // Update primary index using inner's method (handles tombstones automatically)
        let result = self.inner.insert_tombstone(ts, domain.clone());

        // Clean up secondary index
        if let Some(ref old) = old_entry {
            self.remove_from_secondary(&old.value, &domain);
        }

        result
    }

    fn index_lookup(&self, domain: &Domain) -> Option<&Entry<Codomain>> {
        self.inner.entries.get(domain)
    }

    fn remove_entry(&mut self, domain: &Domain) -> Option<Entry<Codomain>> {
        let old_entry = self.inner.entries.remove(domain);

        // Clean up secondary index
        if let Some(ref old) = old_entry {
            self.remove_from_secondary(&old.value, domain);
        }

        old_entry
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (&Domain, &Entry<Codomain>)> + '_> {
        Box::new(self.inner.entries.iter())
    }

    fn len(&self) -> usize {
        self.inner.entries.len()
    }

    fn fork(&self) -> Box<dyn RelationIndex<Domain, Codomain>> {
        Box::new(Self {
            inner: self.inner.clone(),
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn get_by_codomain(&self, codomain: &Codomain) -> Vec<Domain> {
        if let Some(ref secondary) = self.inner.secondary_index {
            secondary
                .get(codomain)
                .map(|set| set.iter().cloned().collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    fn has_secondary_index(&self) -> bool {
        true
    }

    fn is_provider_fully_loaded(&self) -> bool {
        self.inner.provider_fully_loaded
    }

    fn set_provider_fully_loaded(&mut self, loaded: bool) {
        self.inner.provider_fully_loaded = loaded;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestDomain(u64);

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestCodomain(u64);

    #[test]
    fn test_basic_hash_relation_index() {
        let mut index = HashRelationIndex::new();

        let domain = TestDomain(1);
        let codomain = TestCodomain(100);
        let ts = Timestamp(42);

        // Test insert
        let old = index.insert_entry(ts, domain.clone(), codomain.clone());
        assert!(old.is_none());

        // Test lookup
        let entry = index.index_lookup(&domain).unwrap();
        assert_eq!(entry.ts, ts);
        assert_eq!(entry.value, codomain);

        // Test update
        let new_codomain = TestCodomain(200);
        let old = index.insert_entry(Timestamp(43), domain.clone(), new_codomain.clone());
        assert!(old.is_some());
        assert_eq!(old.unwrap().value, codomain);

        // Test delete
        let old = index.insert_tombstone(Timestamp(44), domain.clone());
        assert!(old.is_some());
        assert_eq!(old.unwrap().value, new_codomain);

        // Verify deletion
        assert!(index.index_lookup(&domain).is_none());
    }

    #[test]
    fn test_secondary_index_creation() {
        let index = HashRelationIndex::<TestDomain, TestCodomain>::new_with_secondary();
        assert!(index.has_secondary_index());

        let basic_index = HashRelationIndex::<TestDomain, TestCodomain>::new();
        assert!(!basic_index.has_secondary_index());
    }

    #[test]
    fn test_get_by_codomain_basic() {
        let index = HashRelationIndex::<TestDomain, TestCodomain>::new();
        let codomain = TestCodomain(100);

        // Should return empty for basic index
        let result = index.get_by_codomain(&codomain);
        assert!(result.is_empty());
    }

    #[test]
    fn test_secondary_index_relation_get_by_codomain() {
        let mut index = SecondaryIndexRelation::new();

        let domain1 = TestDomain(1);
        let domain2 = TestDomain(2);
        let domain3 = TestDomain(3);
        let codomain_a = TestCodomain(100);
        let codomain_b = TestCodomain(200);
        let ts = Timestamp(42);

        // Insert entries
        index.insert_entry(ts, domain1.clone(), codomain_a.clone());
        index.insert_entry(ts, domain2.clone(), codomain_a.clone());
        index.insert_entry(ts, domain3.clone(), codomain_b.clone());

        // Test get_by_codomain
        let result_a = index.get_by_codomain(&codomain_a);
        assert_eq!(result_a.len(), 2);
        assert!(result_a.contains(&domain1));
        assert!(result_a.contains(&domain2));

        let result_b = index.get_by_codomain(&codomain_b);
        assert_eq!(result_b.len(), 1);
        assert!(result_b.contains(&domain3));

        // Test nonexistent codomain
        let result_empty = index.get_by_codomain(&TestCodomain(999));
        assert!(result_empty.is_empty());
    }

    #[test]
    fn test_secondary_index_update_maintains_consistency() {
        let mut index = SecondaryIndexRelation::new();

        let domain = TestDomain(1);
        let old_codomain = TestCodomain(100);
        let new_codomain = TestCodomain(200);
        let ts = Timestamp(42);

        // Insert initial entry
        index.insert_entry(ts, domain.clone(), old_codomain.clone());

        // Verify initial state
        let result = index.get_by_codomain(&old_codomain);
        assert_eq!(result.len(), 1);
        assert!(result.contains(&domain));

        // Update to new codomain
        index.insert_entry(Timestamp(43), domain.clone(), new_codomain.clone());

        // Verify old codomain no longer has this domain
        let old_result = index.get_by_codomain(&old_codomain);
        assert!(old_result.is_empty());

        // Verify new codomain has this domain
        let new_result = index.get_by_codomain(&new_codomain);
        assert_eq!(new_result.len(), 1);
        assert!(new_result.contains(&domain));
    }

    #[test]
    fn test_secondary_index_delete_cleanup() {
        let mut index = SecondaryIndexRelation::new();

        let domain1 = TestDomain(1);
        let domain2 = TestDomain(2);
        let codomain = TestCodomain(100);
        let ts = Timestamp(42);

        // Insert two entries with same codomain
        index.insert_entry(ts, domain1.clone(), codomain.clone());
        index.insert_entry(ts, domain2.clone(), codomain.clone());

        // Verify both domains are in the codomain's set
        let result = index.get_by_codomain(&codomain);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&domain1));
        assert!(result.contains(&domain2));

        // Delete one entry using insert_tombstone
        index.insert_tombstone(Timestamp(43), domain1.clone());

        // Verify only one domain remains
        let result = index.get_by_codomain(&codomain);
        assert_eq!(result.len(), 1);
        assert!(result.contains(&domain2));

        // Delete the second entry
        index.remove_entry(&domain2);

        // Verify empty set is cleaned up (no longer in secondary index)
        let result = index.get_by_codomain(&codomain);
        assert!(result.is_empty());
    }

    #[test]
    fn test_secondary_index_multiple_updates_same_domain() {
        let mut index = SecondaryIndexRelation::new();

        let domain = TestDomain(1);
        let codomain1 = TestCodomain(100);
        let codomain2 = TestCodomain(200);
        let codomain3 = TestCodomain(300);
        let ts = Timestamp(42);

        // Insert initial entry
        index.insert_entry(ts, domain.clone(), codomain1.clone());

        // Update multiple times
        index.insert_entry(Timestamp(43), domain.clone(), codomain2.clone());
        index.insert_entry(Timestamp(44), domain.clone(), codomain3.clone());

        // Verify only the latest codomain has the domain
        assert!(index.get_by_codomain(&codomain1).is_empty());
        assert!(index.get_by_codomain(&codomain2).is_empty());

        let result = index.get_by_codomain(&codomain3);
        assert_eq!(result.len(), 1);
        assert!(result.contains(&domain));
    }

    #[test]
    fn test_secondary_index_fork_consistency() {
        let mut index = SecondaryIndexRelation::new();

        let domain = TestDomain(1);
        let codomain = TestCodomain(100);
        let ts = Timestamp(42);

        // Insert entry
        index.insert_entry(ts, domain.clone(), codomain.clone());

        // Fork the index
        let forked = index.fork();

        // Verify fork has same secondary index state
        let original_result = index.get_by_codomain(&codomain);
        let forked_result = forked.get_by_codomain(&codomain);
        assert_eq!(original_result.len(), forked_result.len());
        assert_eq!(original_result, forked_result);

        // Verify fork has secondary index support
        assert!(forked.has_secondary_index());
    }

    #[test]
    fn test_secondary_index_empty_set_memory_cleanup() {
        let mut index = SecondaryIndexRelation::new();

        let domain = TestDomain(1);
        let codomain = TestCodomain(100);
        let ts = Timestamp(42);

        // Insert and then delete same entry
        index.insert_entry(ts, domain.clone(), codomain.clone());

        // Verify entry exists
        let result = index.get_by_codomain(&codomain);
        assert_eq!(result.len(), 1);

        // Delete using tombstone
        index.insert_tombstone(Timestamp(43), domain.clone());

        // Verify empty set is cleaned up
        let result = index.get_by_codomain(&codomain);
        assert!(result.is_empty());

        // Insert same codomain with different domain to verify cleanup worked
        let domain2 = TestDomain(2);
        index.insert_entry(Timestamp(44), domain2.clone(), codomain.clone());

        let result = index.get_by_codomain(&codomain);
        assert_eq!(result.len(), 1);
        assert!(result.contains(&domain2));
    }
}
