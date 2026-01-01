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

//! Symbol interning implementation for moor.
//!
//! This module provides efficient string interning with case-insensitive comparison
//! while preserving the original case of strings. Symbols are represented by two IDs:
//! - `compare_id`: Used for equality comparisons (case-insensitive)
//! - `repr_id`: Used for retrieving the original string representation
//!
//! The implementation is thread-safe and uses lock-free data structures where possible.

use ahash::AHasher;
use arcstr::ArcStr;
use boxcar::Vec as BoxcarVec;
use once_cell::sync::Lazy;
use papaya::HashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::{Debug, Display},
    hash::{BuildHasherDefault, Hash, Hasher},
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
};
use unicase::UniCase;
use zerocopy::{FromBytes, Immutable, IntoBytes};

// ============================================================================
// Global Interner State
// ============================================================================

/// Store all symbol data in a single entry to ensure atomicity.
#[derive(Clone)]
struct SymbolData {
    original_string: ArcStr,
    repr_id: u32,
    compare_id: u32,
}

/// Container for all case variants of a symbol.
struct SymbolGroup {
    compare_id: u32,
    variants: BoxcarVec<SymbolData>,
}

impl SymbolGroup {
    fn new(compare_id: u32) -> Self {
        Self {
            compare_id,
            variants: BoxcarVec::new(),
        }
    }

    fn get_or_insert_variant(
        &self,
        original: &str,
        global_state: &GlobalInternerState,
    ) -> &SymbolData {
        // Linear search through existing variants to find exact match
        for (_index, variant) in self.variants.iter() {
            if &*variant.original_string == original {
                return variant;
            }
        }

        // Not found, create new variant - need atomic reservation
        let _lock = global_state.allocation_lock.lock();

        // Double-check after acquiring lock (another thread might have added it)
        for (_index, variant) in self.variants.iter() {
            if &*variant.original_string == original {
                return variant;
            }
        }

        // Push to global boxcar and use returned index as repr_id
        let arc_str = ArcStr::from(original);
        let repr_id = global_state.repr_id_to_symbol.push(arc_str.clone()) as u32;

        let symbol_data = SymbolData {
            original_string: arc_str,
            repr_id,
            compare_id: self.compare_id,
        };

        // Push to group's variants boxcar
        let offset = self.variants.push(symbol_data);

        &self.variants[offset]
    }
}

#[cfg_attr(
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
    ),
    repr(align(128))
)]
#[cfg_attr(
    any(
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "riscv64",
    ),
    repr(align(32))
)]
#[cfg_attr(target_arch = "s390x", repr(align(256)))]
#[cfg_attr(
    not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "riscv64",
        target_arch = "s390x",
    )),
    repr(align(64))
)]
pub struct CachePadded<T> {
    pub value: T,
}

impl<T> CachePadded<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> std::ops::Deref for CachePadded<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

struct GlobalInternerState {
    /// Single map: case-insensitive key -> symbol group containing all case variants
    groups: HashMap<UniCase<String>, std::sync::Arc<SymbolGroup>, BuildHasherDefault<AHasher>>,
    /// Fast reverse lookup: repr_id as index -> symbol data
    repr_id_to_symbol: BoxcarVec<ArcStr>,
    /// Atomic counter for compare_id generation
    next_compare_id: CachePadded<AtomicU32>,
    /// Lock for atomic reservation of repr_id + boxcar slot (only used for NEW symbols)
    allocation_lock: Mutex<()>,
}

impl GlobalInternerState {
    fn new() -> Self {
        Self {
            groups: Default::default(),
            repr_id_to_symbol: BoxcarVec::new(),
            next_compare_id: CachePadded::new(AtomicU32::new(0)),
            allocation_lock: Mutex::new(()),
        }
    }

    /// Interns a string, returning (compare_id, repr_id).
    fn intern(&self, s: &str) -> (u32, u32) {
        let case_insensitive_key = UniCase::new(s.to_string());

        // Pin the map to get a guard
        let guard = self.groups.pin();

        // Get or create the symbol group for this case-insensitive key using get_or_insert
        let group = guard.get_or_insert(case_insensitive_key, {
            // If group doesn't exist, create new one
            let compare_id = self.next_compare_id.fetch_add(1, Ordering::Relaxed);
            std::sync::Arc::new(SymbolGroup::new(compare_id))
        });

        // Get or create the specific case variant within the group
        let symbol_data = group.get_or_insert_variant(s, self);

        (symbol_data.compare_id, symbol_data.repr_id)
    }

    fn get_string_by_repr_id(&self, repr_id: u32) -> Option<&ArcStr> {
        // Fast O(1) lookup using direct boxcar indexing
        // repr_id == boxcar_index invariant is maintained by using push() return value as repr_id
        self.repr_id_to_symbol.get(repr_id as usize)
    }
}

static GLOBAL_INTERNER: Lazy<GlobalInternerState> = Lazy::new(GlobalInternerState::new);

// ============================================================================
// Symbol Type
// ============================================================================

/// An interned string used for things like verb names and property names.
///
/// Symbols provide case-insensitive equality while preserving the original case
/// of the string. Two symbols with the same case-insensitive content will be
/// equal, but may have different string representations.
///
/// # Examples
///
/// ```
/// use moor_var::Symbol;
///
/// let sym1 = Symbol::mk("Hello");
/// let sym2 = Symbol::mk("hello");
/// let sym3 = Symbol::mk("HELLO");
///
/// // All are equal (case-insensitive)
/// assert_eq!(sym1, sym2);
/// assert_eq!(sym2, sym3);
///
/// // But preserve original case
/// assert_eq!(sym1.as_string(), "Hello");
/// assert_eq!(sym2.as_string(), "hello");
/// assert_eq!(sym3.as_string(), "HELLO");
/// ```
#[derive(Copy, Clone, IntoBytes, FromBytes, Immutable)]
#[repr(C)]
pub struct Symbol {
    compare_id: u32,
    repr_id: u32,
}

// ============================================================================
// Core Symbol Implementation
// ============================================================================

impl Symbol {
    /// Create a new symbol from a string slice.
    ///
    /// This method interns the string, making subsequent creations of symbols
    /// with the same case-insensitive content very fast.
    pub fn mk(s: &str) -> Self {
        let (compare_id, repr_id) = GLOBAL_INTERNER.intern(s);
        Symbol {
            compare_id,
            repr_id,
        }
    }

    /// Get the original string as an owned `String`.
    pub fn as_string(&self) -> String {
        GLOBAL_INTERNER
            .get_string_by_repr_id(self.repr_id)
            .unwrap_or_else(|| {
                panic!(
                    "Symbol: Invalid repr_id {}. String not found in interner.",
                    self.repr_id
                )
            })
            .to_string()
    }

    /// Get the original string as an `ArcStr`.
    ///
    /// This is more efficient than `as_string()` when you need to share
    /// the string data or when the string will be cloned multiple times.
    pub fn as_arc_str(&self) -> ArcStr {
        GLOBAL_INTERNER
            .get_string_by_repr_id(self.repr_id)
            .unwrap_or_else(|| {
                panic!(
                    "Symbol: Invalid repr_id {}. String not found in interner.",
                    self.repr_id
                )
            })
            .clone()
    }

    /// Get the compare_id for this symbol.
    ///
    /// This is used for case-insensitive equality comparisons and can be useful
    /// for optimized cache keys.
    pub fn compare_id(&self) -> u32 {
        self.compare_id
    }
}

// ============================================================================
// Trait Implementations
// ============================================================================

impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        self.compare_id == other.compare_id
    }
}

impl Eq for Symbol {}

impl PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Symbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Only compare by compare_id for case-insensitive ordering
        // repr_id should not affect ordering since symbols with same compare_id
        // are considered equal for comparison purposes
        self.compare_id.cmp(&other.compare_id)
    }
}

impl Hash for Symbol {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.compare_id.hash(state);
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &*self.as_arc_str())
    }
}

impl Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match GLOBAL_INTERNER.get_string_by_repr_id(self.repr_id) {
            Some(s) => f
                .debug_struct("Symbol")
                .field("value", &&**s)
                .field("cmp_id", &self.compare_id)
                .field("repr_id", &self.repr_id)
                .finish(),
            None => f
                .debug_struct("Symbol")
                .field("value", &"<invalid_repr_id>")
                .field("cmp_id", &self.compare_id)
                .field("repr_id", &self.repr_id)
                .finish(),
        }
    }
}

// ============================================================================
// Conversion Traits
// ============================================================================

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Symbol::mk(s)
    }
}

impl From<String> for Symbol {
    fn from(s: String) -> Self {
        Symbol::mk(&s)
    }
}

impl From<&String> for Symbol {
    fn from(s: &String) -> Self {
        Symbol::mk(s.as_str())
    }
}

// ============================================================================
// Serialization Support
// ============================================================================

impl Serialize for Symbol {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.as_arc_str())
    }
}

impl<'de> Deserialize<'de> for Symbol {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s: String = Deserialize::deserialize(deserializer)?;
        let (compare_id, repr_id) = GLOBAL_INTERNER.intern(&s);
        Ok(Symbol {
            compare_id,
            repr_id,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::{HashMap, HashSet},
        thread,
    };

    // Create a fresh Symbol from an isolated interner for testing
    fn make_test_symbol(s: &str, interner: &GlobalInternerState) -> Symbol {
        let (compare_id, repr_id) = interner.intern(s);
        Symbol {
            compare_id,
            repr_id,
        }
    }

    #[test]
    fn test_basic_symbol_creation() {
        let interner = GlobalInternerState::new();

        let sym1 = make_test_symbol("test", &interner);
        let sym2 = make_test_symbol("test", &interner);
        let sym3 = make_test_symbol("TEST", &interner);

        // Same string should produce identical symbols
        assert_eq!(sym1, sym2);
        assert_eq!(sym1.repr_id, sym2.repr_id);
        assert_eq!(sym1.compare_id, sym2.compare_id);

        // Case-insensitive comparison
        assert_eq!(sym1, sym3);
        assert_eq!(sym1.compare_id, sym3.compare_id);

        // But different repr_id for different case
        assert_ne!(sym1.repr_id, sym3.repr_id);
    }

    #[test]
    fn test_symbol_string_conversion() {
        let interner = GlobalInternerState::new();
        let original = "TestString";
        let sym = make_test_symbol(original, &interner);

        // Test retrieval from the interner we used to create it
        assert_eq!(
            interner
                .get_string_by_repr_id(sym.repr_id)
                .unwrap()
                .as_str(),
            original
        );
    }

    #[test]
    fn test_symbol_arc_methods() {
        // Test with global interner since that's what the Symbol methods use
        let sym1 = Symbol::mk("TestArcMethods");
        let sym2 = Symbol::mk("testarcmethods");

        // Test Arc<str> method
        let arc_str = sym1.as_arc_str();
        assert_eq!(&*arc_str, "TestArcMethods");

        // Test that case variants have same compare_id but different strings
        assert_eq!(sym1.compare_id, sym2.compare_id);
        assert_eq!(&*sym1.as_arc_str(), "TestArcMethods");
        assert_eq!(&*sym2.as_arc_str(), "testarcmethods");
    }

    #[test]
    fn test_symbol_from_implementations() {
        let sym1 = Symbol::from("test");
        let sym2 = Symbol::from(String::from("test"));
        let s = String::from("test");
        let sym3 = Symbol::from(&s);

        assert_eq!(sym1, sym2);
        assert_eq!(sym2, sym3);
    }

    #[test]
    fn test_symbol_ordering() {
        let interner = GlobalInternerState::new();

        let sym_a = make_test_symbol("a", &interner);
        let sym_b = make_test_symbol("b", &interner);
        let sym_a_upper = make_test_symbol("A", &interner);

        // Case-insensitive ordering - same compare_id should be equal
        assert_eq!(sym_a.compare_id, sym_a_upper.compare_id);
        assert_eq!(sym_a.cmp(&sym_a_upper), std::cmp::Ordering::Equal);

        // Different strings should have different compare_ids
        assert_ne!(sym_a.compare_id, sym_b.compare_id);
    }

    #[test]
    fn test_symbol_hashing() {
        let sym1 = Symbol::mk("test");
        let sym2 = Symbol::mk("TEST");
        let sym3 = Symbol::mk("different");

        let mut map = HashMap::new();
        map.insert(sym1, "value1");

        // Case-insensitive lookup should work
        assert_eq!(map.get(&sym2), Some(&"value1"));
        assert_eq!(map.get(&sym3), None);
    }

    #[test]
    fn test_symbol_serialization() {
        let sym = Symbol::mk("test_symbol");

        // Test serde serialization
        let serialized = serde_json::to_string(&sym).unwrap();
        let deserialized: Symbol = serde_json::from_str(&serialized).unwrap();

        assert_eq!(sym, deserialized);
        assert_eq!(sym.as_arc_str(), deserialized.as_arc_str());
    }

    #[test]
    fn test_many_symbols() {
        let mut symbols = Vec::new();
        let mut strings = HashSet::new();

        // Create many unique symbols
        for i in 0..1000 {
            let s = format!("symbol_{i}");
            strings.insert(s.clone());
            symbols.push(Symbol::mk(&s));
        }

        // All should be unique by repr_id
        let mut repr_ids = HashSet::new();
        for sym in &symbols {
            assert!(repr_ids.insert(sym.repr_id), "Duplicate repr_id found");
        }

        // All should convert back to original strings
        for (i, sym) in symbols.iter().enumerate() {
            let expected = format!("symbol_{i}");
            assert_eq!(sym.as_string(), expected);
        }
    }

    #[test]
    fn test_concurrent_symbol_creation() {
        let num_threads = 10;
        let symbols_per_thread = 100;

        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                thread::spawn(move || {
                    let mut symbols = Vec::new();
                    for i in 0..symbols_per_thread {
                        let s = format!("thread_{thread_id}_{i}");
                        symbols.push(Symbol::mk(&s));
                    }
                    symbols
                })
            })
            .collect();

        let mut all_symbols = Vec::new();
        for handle in handles {
            all_symbols.extend(handle.join().unwrap());
        }

        // Check that all symbols are valid and unique
        let mut repr_ids = HashSet::new();
        for sym in &all_symbols {
            assert!(repr_ids.insert(sym.repr_id), "Duplicate repr_id found");
            // Ensure we can still retrieve the string
            assert!(!sym.as_string().is_empty());
        }

        assert_eq!(all_symbols.len(), num_threads * symbols_per_thread);
    }

    #[test]
    fn test_concurrent_same_string() {
        let num_threads = 20;
        let test_string = "concurrent_test";

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                thread::spawn(move || {
                    let mut symbols = Vec::new();
                    for _ in 0..50 {
                        symbols.push(Symbol::mk(test_string));
                    }
                    symbols
                })
            })
            .collect();

        let mut all_symbols = Vec::new();
        for handle in handles {
            all_symbols.extend(handle.join().unwrap());
        }

        // All symbols should be identical
        let first_symbol = all_symbols[0];
        for sym in &all_symbols {
            assert_eq!(*sym, first_symbol);
            assert_eq!(sym.repr_id, first_symbol.repr_id);
            assert_eq!(sym.compare_id, first_symbol.compare_id);
        }
    }

    #[test]
    fn test_concurrent_case_variants() {
        let base_strings = vec!["test", "example", "symbol"];
        let num_threads = 10;

        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                let strings = base_strings.clone();
                thread::spawn(move || {
                    let mut symbols = Vec::new();
                    for base in strings {
                        // Create various case combinations
                        symbols.push(Symbol::mk(base));
                        symbols.push(Symbol::mk(&base.to_uppercase()));
                        symbols.push(Symbol::mk(&format!("{base}_{thread_id}")));
                    }
                    symbols
                })
            })
            .collect();

        let mut all_symbols = Vec::new();
        for handle in handles {
            all_symbols.extend(handle.join().unwrap());
        }

        // Group symbols by their string representation
        let mut by_string = HashMap::new();
        for sym in all_symbols {
            by_string
                .entry(sym.as_string())
                .or_insert_with(Vec::new)
                .push(sym);
        }

        // Verify that identical strings have identical symbols
        for symbols in by_string.values() {
            if symbols.len() > 1 {
                let first = symbols[0];
                for sym in symbols {
                    assert_eq!(*sym, first);
                    assert_eq!(sym.repr_id, first.repr_id);
                }
            }
        }
    }
}

#[cfg(test)]
mod shuttle_tests {
    use super::*;
    use shuttle::{check, sync::Arc, thread};
    use std::collections::HashMap;

    #[test]
    fn shuttle_concurrent_intern_same_string() {
        check(|| {
            let test_string = "shuttle_test";

            let handle1 = thread::spawn(move || Symbol::mk(test_string));
            let handle2 = thread::spawn(move || Symbol::mk(test_string));
            let handle3 = thread::spawn(move || Symbol::mk(test_string));

            let sym1 = handle1.join().unwrap();
            let sym2 = handle2.join().unwrap();
            let sym3 = handle3.join().unwrap();

            // All should be identical
            assert_eq!(sym1, sym2);
            assert_eq!(sym2, sym3);
            assert_eq!(sym1.repr_id, sym2.repr_id);
            assert_eq!(sym1.compare_id, sym2.compare_id);
        });
    }

    #[test]
    fn shuttle_concurrent_intern_different_strings() {
        check(|| {
            let handle1 = thread::spawn(|| Symbol::mk("string1"));
            let handle2 = thread::spawn(|| Symbol::mk("string2"));
            let handle3 = thread::spawn(|| Symbol::mk("string3"));

            let sym1 = handle1.join().unwrap();
            let sym2 = handle2.join().unwrap();
            let sym3 = handle3.join().unwrap();

            // All should be different
            assert_ne!(sym1.repr_id, sym2.repr_id);
            assert_ne!(sym2.repr_id, sym3.repr_id);
            assert_ne!(sym1.repr_id, sym3.repr_id);

            // Should be able to retrieve original strings
            assert_eq!(sym1.as_string(), "string1");
            assert_eq!(sym2.as_string(), "string2");
            assert_eq!(sym3.as_string(), "string3");
        });
    }

    #[test]
    fn shuttle_concurrent_case_insensitive() {
        check(|| {
            let handle1 = thread::spawn(|| Symbol::mk("Test"));
            let handle2 = thread::spawn(|| Symbol::mk("test"));
            let handle3 = thread::spawn(|| Symbol::mk("TEST"));

            let sym1 = handle1.join().unwrap();
            let sym2 = handle2.join().unwrap();
            let sym3 = handle3.join().unwrap();

            // All should compare equal (case-insensitive)
            assert_eq!(sym1, sym2);
            assert_eq!(sym2, sym3);
            assert_eq!(sym1.compare_id, sym2.compare_id);
            assert_eq!(sym1.compare_id, sym3.compare_id);

            // But should preserve original case
            assert_eq!(sym1.as_string(), "Test");
            assert_eq!(sym2.as_string(), "test");
            assert_eq!(sym3.as_string(), "TEST");
        });
    }

    #[test]
    fn shuttle_concurrent_many_symbols() {
        check(|| {
            let shared_counter = Arc::new(shuttle::sync::atomic::AtomicUsize::new(0));
            let symbols = Arc::new(shuttle::sync::Mutex::new(Vec::new()));

            let handles: Vec<_> = (0..3)
                .map(|_| {
                    let counter = shared_counter.clone();
                    let symbols = symbols.clone();
                    thread::spawn(move || {
                        for _ in 0..5 {
                            let id = counter.fetch_add(1, shuttle::sync::atomic::Ordering::Relaxed);
                            let sym = Symbol::mk(&format!("symbol_{id}"));
                            symbols.lock().unwrap().push(sym);
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }

            let symbols = symbols.lock().unwrap();

            // Check that all symbols are unique and valid
            let mut repr_ids = std::collections::HashSet::new();
            for sym in symbols.iter() {
                assert!(repr_ids.insert(sym.repr_id), "Duplicate repr_id");
                assert!(!sym.as_string().is_empty());
            }

            assert_eq!(symbols.len(), 15);
        });
    }

    #[test]
    fn shuttle_concurrent_lookup_vs_insert() {
        check(|| {
            // One thread tries to create a symbol
            let handle1 = thread::spawn(|| Symbol::mk("lookup_test"));

            // Another thread tries to create the same symbol
            let handle2 = thread::spawn(|| {
                let sym = Symbol::mk("lookup_test");
                (sym, sym.as_string())
            });

            let sym1 = handle1.join().unwrap();
            let (sym2, string2) = handle2.join().unwrap();

            // Both should be identical
            assert_eq!(sym1, sym2);
            assert_eq!(sym1.repr_id, sym2.repr_id);
            assert_eq!(string2, "lookup_test");
        });
    }

    #[test]
    fn shuttle_stress_test_interning() {
        check(|| {
            let base_strings = ["foo", "bar"];
            let handles: Vec<_> = (0..3)
                .map(|thread_id| {
                    thread::spawn(move || {
                        let mut local_symbols = HashMap::new();

                        for i in 0..3 {
                            for base in &base_strings {
                                // Create variations
                                let variants = [
                                    base.to_string(),
                                    format!("{base}{i}"),
                                    format!("{base}_{thread_id}"),
                                ];

                                for variant in &variants {
                                    let sym = Symbol::mk(variant);
                                    local_symbols.insert(variant.clone(), sym);

                                    // Verify we can retrieve the string
                                    assert_eq!(sym.as_string(), *variant);
                                }
                            }
                        }

                        local_symbols
                    })
                })
                .collect();

            let mut all_symbols = HashMap::new();
            for handle in handles {
                let thread_symbols = handle.join().unwrap();
                for (string, symbol) in thread_symbols {
                    if let Some(existing) = all_symbols.get(&string) {
                        // Same string should produce same symbol
                        assert_eq!(*existing, symbol);
                    } else {
                        all_symbols.insert(string, symbol);
                    }
                }
            }

            // Verify all symbols are still valid
            for (string, symbol) in &all_symbols {
                assert_eq!(symbol.as_string(), *string);
            }
        });
    }
}
