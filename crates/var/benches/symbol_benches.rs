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

use moor_bench_utils::{BenchContext, black_box};
use moor_var::Symbol;
use std::collections::HashMap;

// ============================================================================
// SYMBOL CREATION BENCHMARKS
// ============================================================================

// Pre-generate unique strings to avoid measuring string formatting overhead
struct UniqueStringsContext {
    strings: Vec<String>,
    index: usize,
}

impl BenchContext for UniqueStringsContext {
    fn prepare(num_chunks: usize) -> Self {
        // Pre-generate enough unique strings for all chunks
        // Each chunk will use 10k operations, we need num_chunks * chunk_size strings
        let total = num_chunks * 10_000;
        let strings: Vec<String> = (0..total).map(|i| format!("unique_symbol_{i}")).collect();
        UniqueStringsContext { strings, index: 0 }
    }

    fn chunk_size() -> Option<usize> {
        Some(10_000) // Smaller chunk for creation since it's slower
    }
}

fn symbol_create_unique(ctx: &mut UniqueStringsContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let s = &ctx.strings[ctx.index];
        ctx.index += 1;
        let sym = Symbol::mk(s);
        black_box(sym);
    }
}

// Context for repeated symbol lookups (cache hit path)
struct RepeatedSymbolContext {
    test_string: String,
}

impl BenchContext for RepeatedSymbolContext {
    fn prepare(_num_chunks: usize) -> Self {
        // Create the symbol once so it's in the interner AND in thread-local cache
        let test_string = "repeated_lookup_test".to_string();
        let _ = Symbol::mk(&test_string);
        RepeatedSymbolContext { test_string }
    }
}

fn symbol_lookup_cached(ctx: &mut RepeatedSymbolContext, chunk_size: usize, _chunk_num: usize) {
    let s = &ctx.test_string;
    for _ in 0..chunk_size {
        // This will hit the thread-local cache
        let sym = Symbol::mk(s);
        black_box(sym);
    }
}

// Context for case variant lookups
struct CaseVariantContext {
    variants: Vec<String>,
}

impl BenchContext for CaseVariantContext {
    fn prepare(_num_chunks: usize) -> Self {
        // Create initial symbol
        let _ = Symbol::mk("CaseVariantTest");
        CaseVariantContext {
            variants: vec![
                "CaseVariantTest".to_string(),
                "casevarianttest".to_string(),
                "CASEVARIANTTEST".to_string(),
                "caseVariantTest".to_string(),
            ],
        }
    }
}

fn symbol_lookup_case_variants(ctx: &mut CaseVariantContext, chunk_size: usize, _chunk_num: usize) {
    let variants = &ctx.variants;
    for i in 0..chunk_size {
        let s = &variants[i % variants.len()];
        let sym = Symbol::mk(s);
        black_box(sym);
    }
}

// ============================================================================
// SYMBOL STRING RETRIEVAL BENCHMARKS
// ============================================================================

struct SymbolRetrievalContext {
    symbols: Vec<Symbol>,
}

impl BenchContext for SymbolRetrievalContext {
    fn prepare(_num_chunks: usize) -> Self {
        // Create a variety of symbols
        let symbols: Vec<Symbol> = (0..100)
            .map(|i| Symbol::mk(&format!("retrieval_test_{i}")))
            .collect();
        SymbolRetrievalContext { symbols }
    }
}

fn symbol_as_string(ctx: &mut SymbolRetrievalContext, chunk_size: usize, _chunk_num: usize) {
    let symbols = &ctx.symbols;
    for i in 0..chunk_size {
        let sym = &symbols[i % symbols.len()];
        let s = sym.as_string();
        black_box(s);
    }
}

fn symbol_as_arc_str(ctx: &mut SymbolRetrievalContext, chunk_size: usize, _chunk_num: usize) {
    let symbols = &ctx.symbols;
    for i in 0..chunk_size {
        let sym = &symbols[i % symbols.len()];
        let s = sym.as_arc_str();
        black_box(s);
    }
}

// ============================================================================
// SYMBOL COMPARISON BENCHMARKS
// ============================================================================

struct SymbolCompareContext {
    sym1: Symbol,
    sym2_same: Symbol,
    sym2_case_variant: Symbol,
    sym3_different: Symbol,
}

impl BenchContext for SymbolCompareContext {
    fn prepare(_num_chunks: usize) -> Self {
        let sym1 = Symbol::mk("compare_test");
        let sym2_same = Symbol::mk("compare_test");
        let sym2_case_variant = Symbol::mk("COMPARE_TEST");
        let sym3_different = Symbol::mk("different_symbol");
        SymbolCompareContext {
            sym1,
            sym2_same,
            sym2_case_variant,
            sym3_different,
        }
    }
}

fn symbol_eq_same(ctx: &mut SymbolCompareContext, chunk_size: usize, _chunk_num: usize) {
    let sym1 = &ctx.sym1;
    let sym2 = &ctx.sym2_same;
    for _ in 0..chunk_size {
        let eq = sym1 == sym2;
        black_box(eq);
    }
}

fn symbol_eq_case_variant(ctx: &mut SymbolCompareContext, chunk_size: usize, _chunk_num: usize) {
    let sym1 = &ctx.sym1;
    let sym2 = &ctx.sym2_case_variant;
    for _ in 0..chunk_size {
        let eq = sym1 == sym2;
        black_box(eq);
    }
}

fn symbol_eq_different(ctx: &mut SymbolCompareContext, chunk_size: usize, _chunk_num: usize) {
    let sym1 = &ctx.sym1;
    let sym3 = &ctx.sym3_different;
    for _ in 0..chunk_size {
        let eq = sym1 == sym3;
        black_box(eq);
    }
}

// ============================================================================
// SYMBOL HASHING BENCHMARKS
// ============================================================================

struct SymbolHashContext {
    symbols: Vec<Symbol>,
    map: HashMap<Symbol, i32>,
}

impl BenchContext for SymbolHashContext {
    fn prepare(_num_chunks: usize) -> Self {
        let symbols: Vec<Symbol> = (0..1000)
            .map(|i| Symbol::mk(&format!("hash_test_{i}")))
            .collect();
        let mut map = HashMap::new();
        for (i, sym) in symbols.iter().enumerate() {
            map.insert(*sym, i as i32);
        }
        SymbolHashContext { symbols, map }
    }
}

fn symbol_hash_lookup(ctx: &mut SymbolHashContext, chunk_size: usize, _chunk_num: usize) {
    let symbols = &ctx.symbols;
    let map = &ctx.map;
    for i in 0..chunk_size {
        let sym = &symbols[i % symbols.len()];
        let val = map.get(sym);
        black_box(val);
    }
}

fn symbol_hash_insert(ctx: &mut SymbolHashContext, chunk_size: usize, _chunk_num: usize) {
    let symbols = &ctx.symbols;
    let mut map = HashMap::new();
    for i in 0..chunk_size {
        let sym = symbols[i % symbols.len()];
        map.insert(sym, i as i32);
    }
    black_box(map);
}

// ============================================================================
// SYMBOL CLONE BENCHMARKS
// ============================================================================

struct SymbolCloneContext {
    sym: Symbol,
}

impl BenchContext for SymbolCloneContext {
    fn prepare(_num_chunks: usize) -> Self {
        SymbolCloneContext {
            sym: Symbol::mk("clone_test_symbol"),
        }
    }
}

fn symbol_clone(ctx: &mut SymbolCloneContext, chunk_size: usize, _chunk_num: usize) {
    let sym = ctx.sym;
    for _ in 0..chunk_size {
        let cloned = sym;
        black_box(cloned);
    }
}

// ============================================================================
// SYMBOL DISPLAY/DEBUG BENCHMARKS
// ============================================================================

fn symbol_display(ctx: &mut SymbolRetrievalContext, chunk_size: usize, _chunk_num: usize) {
    let symbols = &ctx.symbols;
    for i in 0..chunk_size {
        let sym = &symbols[i % symbols.len()];
        let s = format!("{sym}");
        black_box(s);
    }
}

fn symbol_debug(ctx: &mut SymbolRetrievalContext, chunk_size: usize, _chunk_num: usize) {
    let symbols = &ctx.symbols;
    for i in 0..chunk_size {
        let sym = &symbols[i % symbols.len()];
        let s = format!("{sym:?}");
        black_box(s);
    }
}

// ============================================================================
// SYMBOL SERIALIZATION BENCHMARKS
// ============================================================================

struct SymbolSerializeContext {
    symbols: Vec<Symbol>,
    serialized: Vec<String>,
}

impl BenchContext for SymbolSerializeContext {
    fn prepare(_num_chunks: usize) -> Self {
        let symbols: Vec<Symbol> = (0..100)
            .map(|i| Symbol::mk(&format!("serialize_test_{i}")))
            .collect();
        let serialized: Vec<String> = symbols
            .iter()
            .map(|s| serde_json::to_string(s).unwrap())
            .collect();
        SymbolSerializeContext {
            symbols,
            serialized,
        }
    }
}

fn symbol_serialize(ctx: &mut SymbolSerializeContext, chunk_size: usize, _chunk_num: usize) {
    let symbols = &ctx.symbols;
    for i in 0..chunk_size {
        let sym = &symbols[i % symbols.len()];
        let s = serde_json::to_string(sym).unwrap();
        black_box(s);
    }
}

fn symbol_deserialize(ctx: &mut SymbolSerializeContext, chunk_size: usize, _chunk_num: usize) {
    let serialized = &ctx.serialized;
    for i in 0..chunk_size {
        let s = &serialized[i % serialized.len()];
        let sym: Symbol = serde_json::from_str(s).unwrap();
        black_box(sym);
    }
}

// ============================================================================
// STRING LENGTH VARIATION BENCHMARKS
// ============================================================================

struct ShortStringContext {
    strings: Vec<String>,
}

impl BenchContext for ShortStringContext {
    fn prepare(_num_chunks: usize) -> Self {
        // 4-char strings
        let strings: Vec<String> = (0..1000).map(|i| format!("s{i:03}")).collect();
        // Intern them first
        for s in &strings {
            let _ = Symbol::mk(s);
        }
        ShortStringContext { strings }
    }
}

struct MediumStringContext {
    strings: Vec<String>,
}

impl BenchContext for MediumStringContext {
    fn prepare(_num_chunks: usize) -> Self {
        // ~20 char strings
        let strings: Vec<String> = (0..1000)
            .map(|i| format!("medium_length_str_{i:04}"))
            .collect();
        for s in &strings {
            let _ = Symbol::mk(s);
        }
        MediumStringContext { strings }
    }
}

struct LongStringContext {
    strings: Vec<String>,
}

impl BenchContext for LongStringContext {
    fn prepare(_num_chunks: usize) -> Self {
        // ~100 char strings
        let strings: Vec<String> = (0..1000)
            .map(|i| format!("this_is_a_very_long_symbol_name_that_might_be_used_for_method_names_or_properties_{i:04}"))
            .collect();
        for s in &strings {
            let _ = Symbol::mk(s);
        }
        LongStringContext { strings }
    }
}

fn symbol_lookup_short(ctx: &mut ShortStringContext, chunk_size: usize, _chunk_num: usize) {
    let strings = &ctx.strings;
    for i in 0..chunk_size {
        let s = &strings[i % strings.len()];
        let sym = Symbol::mk(s);
        black_box(sym);
    }
}

fn symbol_lookup_medium(ctx: &mut MediumStringContext, chunk_size: usize, _chunk_num: usize) {
    let strings = &ctx.strings;
    for i in 0..chunk_size {
        let s = &strings[i % strings.len()];
        let sym = Symbol::mk(s);
        black_box(sym);
    }
}

fn symbol_lookup_long(ctx: &mut LongStringContext, chunk_size: usize, _chunk_num: usize) {
    let strings = &ctx.strings;
    for i in 0..chunk_size {
        let s = &strings[i % strings.len()];
        let sym = Symbol::mk(s);
        black_box(sym);
    }
}

// ============================================================================
// CONCURRENT ACCESS SIMULATION
// ============================================================================

// Simulates the access pattern of looking up the same symbol repeatedly
// (common in verb dispatch where the same verb name is looked up many times)
struct HotSymbolContext {
    hot_symbol_str: String,
    hot_symbol: Symbol,
}

impl BenchContext for HotSymbolContext {
    fn prepare(_num_chunks: usize) -> Self {
        let hot_symbol_str = "tell".to_string(); // Common verb name
        let hot_symbol = Symbol::mk(&hot_symbol_str);
        HotSymbolContext {
            hot_symbol_str,
            hot_symbol,
        }
    }
}

fn symbol_hot_path_lookup(ctx: &mut HotSymbolContext, chunk_size: usize, _chunk_num: usize) {
    let s = &ctx.hot_symbol_str;
    for _ in 0..chunk_size {
        let sym = Symbol::mk(s);
        black_box(sym);
    }
}

fn symbol_hot_path_compare_id(ctx: &mut HotSymbolContext, chunk_size: usize, _chunk_num: usize) {
    let sym = ctx.hot_symbol;
    for _ in 0..chunk_size {
        let id = sym.compare_id();
        black_box(id);
    }
}

// ============================================================================
// MAIN
// ============================================================================

pub fn main() {
    use moor_bench_utils::{BenchmarkDef, generate_session_summary, run_benchmark_group};
    use std::env;

    #[cfg(target_os = "linux")]
    {
        use moor_bench_utils::perf_event::{Builder, events::Hardware};
        if Builder::new(Hardware::INSTRUCTIONS).build().is_err() {
            eprintln!(
                "⚠️  Perf events are not available on this system (insufficient permissions or kernel support)."
            );
            eprintln!("   Continuing with timing-only benchmarks (performance counters disabled).");
            eprintln!();
        }
    }

    let args: Vec<String> = env::args().collect();
    let filter = if let Some(separator_pos) = args.iter().position(|arg| arg == "--") {
        args.get(separator_pos + 1).map(|s| s.as_str())
    } else {
        args.iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--") && !args[0].contains(arg.as_str()))
            .map(|s| s.as_str())
    };

    if let Some(f) = filter {
        eprintln!("Running benchmarks matching filter: '{f}'");
    }

    // Symbol creation benchmarks
    let creation_unique_benchmarks = [BenchmarkDef {
        name: "symbol_create_unique",
        group: "symbol_create",
        func: symbol_create_unique,
    }];

    let creation_cached_benchmarks = [BenchmarkDef {
        name: "symbol_lookup_cached",
        group: "symbol_create",
        func: symbol_lookup_cached,
    }];

    let creation_case_benchmarks = [BenchmarkDef {
        name: "symbol_lookup_case_variants",
        group: "symbol_create",
        func: symbol_lookup_case_variants,
    }];

    // Symbol retrieval benchmarks
    let retrieval_benchmarks = [
        BenchmarkDef {
            name: "symbol_as_string",
            group: "symbol_retrieval",
            func: symbol_as_string,
        },
        BenchmarkDef {
            name: "symbol_as_arc_str",
            group: "symbol_retrieval",
            func: symbol_as_arc_str,
        },
        BenchmarkDef {
            name: "symbol_display",
            group: "symbol_retrieval",
            func: symbol_display,
        },
        BenchmarkDef {
            name: "symbol_debug",
            group: "symbol_retrieval",
            func: symbol_debug,
        },
    ];

    // Symbol comparison benchmarks
    let compare_benchmarks = [
        BenchmarkDef {
            name: "symbol_eq_same",
            group: "symbol_compare",
            func: symbol_eq_same,
        },
        BenchmarkDef {
            name: "symbol_eq_case_variant",
            group: "symbol_compare",
            func: symbol_eq_case_variant,
        },
        BenchmarkDef {
            name: "symbol_eq_different",
            group: "symbol_compare",
            func: symbol_eq_different,
        },
    ];

    // Symbol hash benchmarks
    let hash_benchmarks = [
        BenchmarkDef {
            name: "symbol_hash_lookup",
            group: "symbol_hash",
            func: symbol_hash_lookup,
        },
        BenchmarkDef {
            name: "symbol_hash_insert",
            group: "symbol_hash",
            func: symbol_hash_insert,
        },
    ];

    // Symbol clone benchmarks
    let clone_benchmarks = [BenchmarkDef {
        name: "symbol_clone",
        group: "symbol_clone",
        func: symbol_clone,
    }];

    // Serialization benchmarks
    let serialize_benchmarks = [
        BenchmarkDef {
            name: "symbol_serialize",
            group: "symbol_serde",
            func: symbol_serialize,
        },
        BenchmarkDef {
            name: "symbol_deserialize",
            group: "symbol_serde",
            func: symbol_deserialize,
        },
    ];

    // String length benchmarks
    let short_string_benchmarks = [BenchmarkDef {
        name: "symbol_lookup_short",
        group: "symbol_length",
        func: symbol_lookup_short,
    }];

    let medium_string_benchmarks = [BenchmarkDef {
        name: "symbol_lookup_medium",
        group: "symbol_length",
        func: symbol_lookup_medium,
    }];

    let long_string_benchmarks = [BenchmarkDef {
        name: "symbol_lookup_long",
        group: "symbol_length",
        func: symbol_lookup_long,
    }];

    // Hot path benchmarks
    let hot_path_benchmarks = [
        BenchmarkDef {
            name: "symbol_hot_path_lookup",
            group: "symbol_hot",
            func: symbol_hot_path_lookup,
        },
        BenchmarkDef {
            name: "symbol_hot_path_compare_id",
            group: "symbol_hot",
            func: symbol_hot_path_compare_id,
        },
    ];

    // Run all benchmark groups
    run_benchmark_group::<UniqueStringsContext>(
        &creation_unique_benchmarks,
        "Symbol Creation (Unique)",
        filter,
    );
    run_benchmark_group::<RepeatedSymbolContext>(
        &creation_cached_benchmarks,
        "Symbol Creation (Cached)",
        filter,
    );
    run_benchmark_group::<CaseVariantContext>(
        &creation_case_benchmarks,
        "Symbol Creation (Case Variants)",
        filter,
    );
    run_benchmark_group::<SymbolRetrievalContext>(
        &retrieval_benchmarks,
        "Symbol Retrieval",
        filter,
    );
    run_benchmark_group::<SymbolCompareContext>(&compare_benchmarks, "Symbol Comparison", filter);
    run_benchmark_group::<SymbolHashContext>(&hash_benchmarks, "Symbol Hashing", filter);
    run_benchmark_group::<SymbolCloneContext>(&clone_benchmarks, "Symbol Clone", filter);
    run_benchmark_group::<SymbolSerializeContext>(
        &serialize_benchmarks,
        "Symbol Serialization",
        filter,
    );
    run_benchmark_group::<ShortStringContext>(
        &short_string_benchmarks,
        "Symbol Lookup (Short Strings)",
        filter,
    );
    run_benchmark_group::<MediumStringContext>(
        &medium_string_benchmarks,
        "Symbol Lookup (Medium Strings)",
        filter,
    );
    run_benchmark_group::<LongStringContext>(
        &long_string_benchmarks,
        "Symbol Lookup (Long Strings)",
        filter,
    );
    run_benchmark_group::<HotSymbolContext>(&hot_path_benchmarks, "Symbol Hot Path", filter);

    if filter.is_some() {
        eprintln!("\nBenchmark filtering complete.");
    }

    generate_session_summary();
}
