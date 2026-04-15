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

use micromeasure::{BenchContext, black_box};
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
    use micromeasure::BenchmarkRunner;
    use std::env;

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

    let runner = BenchmarkRunner::new().with_filter(filter);

    runner.group::<UniqueStringsContext>("Symbol Creation (Unique)", |g| {
        g.bench("symbol_create_unique", symbol_create_unique);
    });

    runner.group::<RepeatedSymbolContext>("Symbol Creation (Cached)", |g| {
        g.bench("symbol_lookup_cached", symbol_lookup_cached);
    });

    runner.group::<CaseVariantContext>("Symbol Creation (Case Variants)", |g| {
        g.bench("symbol_lookup_case_variants", symbol_lookup_case_variants);
    });

    runner.group::<SymbolRetrievalContext>("Symbol Retrieval", |g| {
        g.bench("symbol_as_string", symbol_as_string);
        g.bench("symbol_as_arc_str", symbol_as_arc_str);
        g.bench("symbol_display", symbol_display);
        g.bench("symbol_debug", symbol_debug);
    });

    runner.group::<SymbolCompareContext>("Symbol Comparison", |g| {
        g.bench("symbol_eq_same", symbol_eq_same);
        g.bench("symbol_eq_case_variant", symbol_eq_case_variant);
        g.bench("symbol_eq_different", symbol_eq_different);
    });

    runner.group::<SymbolHashContext>("Symbol Hashing", |g| {
        g.bench("symbol_hash_lookup", symbol_hash_lookup);
        g.bench("symbol_hash_insert", symbol_hash_insert);
    });

    runner.group::<SymbolCloneContext>("Symbol Clone", |g| {
        g.bench("symbol_clone", symbol_clone);
    });

    runner.group::<SymbolSerializeContext>("Symbol Serialization", |g| {
        g.bench("symbol_serialize", symbol_serialize);
        g.bench("symbol_deserialize", symbol_deserialize);
    });

    runner.group::<ShortStringContext>("Symbol Lookup (Short Strings)", |g| {
        g.bench("symbol_lookup_short", symbol_lookup_short);
    });

    runner.group::<MediumStringContext>("Symbol Lookup (Medium Strings)", |g| {
        g.bench("symbol_lookup_medium", symbol_lookup_medium);
    });

    runner.group::<LongStringContext>("Symbol Lookup (Long Strings)", |g| {
        g.bench("symbol_lookup_long", symbol_lookup_long);
    });

    runner.group::<HotSymbolContext>("Symbol Hot Path", |g| {
        g.bench("symbol_hot_path_lookup", symbol_hot_path_lookup);
        g.bench("symbol_hot_path_compare_id", symbol_hot_path_compare_id);
    });

    if filter.is_some() {
        eprintln!("\nBenchmark filtering complete.");
    }

    let report = runner.report();
    report.print_summary_with(micromeasure::ComparisonPolicy::LatestCompatible);
    match report.save_to_default_location() {
        Ok(path) => println!("\n💾 Results saved to: {}", path.display()),
        Err(error) => println!("\n⚠️  Failed to save results: {error}"),
    }
}
