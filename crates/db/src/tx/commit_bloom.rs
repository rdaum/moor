// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Bloom filter for fast commit conflict detection.
//!
//! Each commit builds a bloom filter from its modified keys. On CAS failure,
//! the loser tests its keys against the winner's filter to determine if a
//! rebase is safe (no key overlap) without touching the imbl indexes.

use std::hash::{Hash, Hasher};

/// Bloom filter sized for typical MOO transaction working sets.
/// 2048 bits (256 bytes), 2 hash probes.
/// At 64 keys: ~10% false positive rate.
/// At 16 keys: ~0.7% false positive rate.
const BLOOM_BITS: usize = 2048;
const BLOOM_BYTES: usize = BLOOM_BITS / 8;

#[derive(Clone)]
pub struct CommitBloom {
    bits: Box<[u8; BLOOM_BYTES]>,
}

impl Default for CommitBloom {
    fn default() -> Self {
        Self::new()
    }
}

impl CommitBloom {
    pub fn new() -> Self {
        Self {
            bits: Box::new([0u8; BLOOM_BYTES]),
        }
    }

    /// Insert a key into the filter.
    pub fn insert<K: Hash>(&mut self, key: &K) {
        let (h1, h2) = double_hash(key);
        self.set_bit(h1 % BLOOM_BITS);
        self.set_bit(h2 % BLOOM_BITS);
    }

    /// Test if a key might be in the filter.
    /// False means definitely not present. True means possibly present.
    pub fn might_contain<K: Hash>(&self, key: &K) -> bool {
        let (h1, h2) = double_hash(key);
        self.get_bit(h1 % BLOOM_BITS) && self.get_bit(h2 % BLOOM_BITS)
    }

    /// Returns true if the filter is empty (no keys inserted).
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|&b| b == 0)
    }

    /// Test whether two bloom filters might share any keys.
    /// Returns false if the filters are definitely disjoint (no common bits set).
    pub fn might_intersect(&self, other: &CommitBloom) -> bool {
        for i in 0..BLOOM_BYTES {
            if self.bits[i] & other.bits[i] != 0 {
                return true;
            }
        }
        false
    }

    /// OR another bloom filter into this one (cumulative union).
    pub fn merge(&mut self, other: &CommitBloom) {
        for i in 0..BLOOM_BYTES {
            self.bits[i] |= other.bits[i];
        }
    }

    fn set_bit(&mut self, bit: usize) {
        self.bits[bit / 8] |= 1 << (bit % 8);
    }

    fn get_bit(&self, bit: usize) -> bool {
        self.bits[bit / 8] & (1 << (bit % 8)) != 0
    }
}

/// Produce two independent hash values from a single key using ahash.
/// Uses the standard double-hashing technique: hash with two different seeds.
fn double_hash<K: Hash>(key: &K) -> (usize, usize) {
    let mut h1 = ahash::AHasher::default();
    key.hash(&mut h1);
    let v1 = h1.finish() as usize;

    // Second hash: feed the first hash value as additional entropy
    let mut h2 = ahash::AHasher::default();
    key.hash(&mut h2);
    v1.hash(&mut h2);
    let v2 = h2.finish() as usize;

    (v1, v2)
}

/// Concurrent bloom filter using atomic operations for lock-free insert and query.
/// Suitable for long-lived shared state like the provider tombstone cache.
///
/// Larger than `CommitBloom` (64KB / 524288 bits) since it accumulates over the
/// provider's lifetime. At 10K keys with 2 probes: ~0.02% false positive rate.
/// At 100K keys: ~1.8% false positive rate.
const ATOMIC_BLOOM_BITS: usize = 524288;
const ATOMIC_BLOOM_BYTES: usize = ATOMIC_BLOOM_BITS / 8;

pub struct AtomicBloom {
    bits: Box<[std::sync::atomic::AtomicU8; ATOMIC_BLOOM_BYTES]>,
}

impl AtomicBloom {
    pub fn new() -> Self {
        Self {
            bits: (0..ATOMIC_BLOOM_BYTES)
                .map(|_| std::sync::atomic::AtomicU8::new(0))
                .collect::<Vec<_>>()
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!()),
        }
    }

    /// Insert a key into the filter. Lock-free, safe to call concurrently.
    pub fn insert<K: Hash>(&self, key: &K) {
        let (h1, h2) = double_hash(key);
        let b1 = h1 % ATOMIC_BLOOM_BITS;
        let b2 = h2 % ATOMIC_BLOOM_BITS;
        self.bits[b1 / 8].fetch_or(1 << (b1 % 8), std::sync::atomic::Ordering::Relaxed);
        self.bits[b2 / 8].fetch_or(1 << (b2 % 8), std::sync::atomic::Ordering::Relaxed);
    }

    /// Test if a key might be in the filter. Lock-free.
    /// False means definitely not present.
    pub fn might_contain<K: Hash>(&self, key: &K) -> bool {
        let (h1, h2) = double_hash(key);
        let b1 = h1 % ATOMIC_BLOOM_BITS;
        let b2 = h2 % ATOMIC_BLOOM_BITS;
        (self.bits[b1 / 8].load(std::sync::atomic::Ordering::Relaxed) & (1 << (b1 % 8)) != 0)
            && (self.bits[b2 / 8].load(std::sync::atomic::Ordering::Relaxed) & (1 << (b2 % 8)) != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_query() {
        let mut bloom = CommitBloom::new();
        bloom.insert(&42u64);
        bloom.insert(&99u64);

        assert!(bloom.might_contain(&42u64));
        assert!(bloom.might_contain(&99u64));
    }

    #[test]
    fn test_empty() {
        let bloom = CommitBloom::new();
        assert!(bloom.is_empty());
        // Empty filter should not match anything (both bits must be set)
        // With high probability, a random key won't match
        let mut false_positives = 0;
        for i in 0u64..1000 {
            if bloom.might_contain(&i) {
                false_positives += 1;
            }
        }
        assert_eq!(false_positives, 0);
    }

    #[test]
    fn test_atomic_bloom_insert_and_query() {
        let bloom = AtomicBloom::new();
        bloom.insert(&42u64);
        bloom.insert(&99u64);

        assert!(bloom.might_contain(&42u64));
        assert!(bloom.might_contain(&99u64));

        // Test something not inserted — should very likely be false
        let mut false_positives = 0;
        for i in 1000u64..2000 {
            if bloom.might_contain(&i) {
                false_positives += 1;
            }
        }
        assert!(
            false_positives < 50,
            "Too many false positives: {false_positives}/1000"
        );
    }

    #[test]
    fn test_false_positive_rate() {
        let mut bloom = CommitBloom::new();
        // Insert 64 keys
        for i in 0u64..64 {
            bloom.insert(&i);
        }

        // Test 10000 keys that were NOT inserted
        let mut false_positives = 0;
        for i in 1000u64..11000 {
            if bloom.might_contain(&i) {
                false_positives += 1;
            }
        }

        // At 64 keys in 2048 bits with 2 probes, expected FP rate ~10%
        // Allow up to 20% for test stability
        assert!(
            false_positives < 2000,
            "False positive rate too high: {false_positives}/10000"
        );
    }
}
