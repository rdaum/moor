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

use micromeasure::{BenchContext, BenchmarkMainOptions, NoContext, benchmark_main, black_box};
use moor_var::{
    IndexMode, Obj, Symbol, Var, v_arc_str, v_bool, v_float, v_int, v_list, v_none, v_obj, v_str,
    v_string, v_sym, v_symbol_str,
};

// Context for integer benchmarks
struct IntContext(Var);
impl BenchContext for IntContext {
    fn prepare(_num_chunks: usize) -> Self {
        IntContext(v_int(0))
    }
}

// Context for small list benchmarks
struct SmallListContext(Var);
impl BenchContext for SmallListContext {
    fn prepare(_num_chunks: usize) -> Self {
        SmallListContext(v_list(&(0..8).map(v_int).collect::<Vec<_>>()))
    }
}

// Context for large list benchmarks
struct LargeListContext(Var);
impl BenchContext for LargeListContext {
    fn prepare(_num_chunks: usize) -> Self {
        LargeListContext(v_list(&(0..100_000).map(v_int).collect::<Vec<_>>()))
    }
}

fn int_add(ctx: &mut IntContext, chunk_size: usize, _chunk_num: usize) {
    let mut v = ctx.0.clone();
    for _ in 0..chunk_size {
        v = v.add(&v_int(1)).unwrap();
    }
}

// Context for float benchmarks
struct FloatContext(Var);
impl BenchContext for FloatContext {
    fn prepare(_num_chunks: usize) -> Self {
        FloatContext(v_float(0.0))
    }
}

fn float_add(ctx: &mut FloatContext, chunk_size: usize, _chunk_num: usize) {
    let mut v = ctx.0.clone();
    for _ in 0..chunk_size {
        v = v.add(&v_float(1.0)).unwrap();
    }
}

// Mixed type: int + float (tests the slow path)
fn mixed_add(ctx: &mut IntContext, chunk_size: usize, _chunk_num: usize) {
    let mut v = ctx.0.clone();
    for _ in 0..chunk_size {
        v = v.add(&v_float(1.0)).unwrap();
    }
}

fn int_eq(ctx: &mut IntContext, chunk_size: usize, _chunk_num: usize) {
    let v = ctx.0.clone();
    for _ in 0..chunk_size {
        let _ = v.eq(&v);
    }
}

fn int_cmp(ctx: &mut IntContext, chunk_size: usize, _chunk_num: usize) {
    let v = ctx.0.clone();
    for _ in 0..chunk_size {
        let _ = v.cmp(&v);
    }
}

fn list_push(ctx: &mut SmallListContext, chunk_size: usize, _chunk_num: usize) {
    let mut v = ctx.0.clone();
    for _ in 0..chunk_size {
        v = v.push(&v_int(1)).unwrap();
    }
}

fn list_index_pos(ctx: &mut LargeListContext, chunk_size: usize, _chunk_num: usize) {
    let v = ctx.0.clone();
    let list_len = 100_000; // LargeListContext has 100k items
    for c in 0..chunk_size {
        let index = c % list_len; // Cycle through available indices
        let _ = v.index(&v_int(index as i64), IndexMode::ZeroBased).unwrap();
    }
}

fn list_index_assign(ctx: &mut LargeListContext, chunk_size: usize, _chunk_num: usize) {
    let mut v = ctx.0.clone();
    let list_len = 100_000; // LargeListContext has 100k items
    for c in 0..chunk_size {
        let index = c % list_len; // Cycle through available indices
        v = v
            .index_set(
                &v_int(index as i64),
                &v_int(index as i64),
                IndexMode::ZeroBased,
            )
            .unwrap();
    }
}

// === VAR CONSTRUCTION BENCHMARKS ===
// These measure the cost of creating different types of Vars

fn var_construct_ints(_ctx: &mut NoContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let var = v_int(i as i64);
        black_box(var);
    }
}

fn var_construct_strings(_ctx: &mut NoContext, chunk_size: usize, chunk_num: usize) {
    for i in 0..chunk_size {
        let s = format!("string_{chunk_num}_{i})");
        let var = v_str(&s);
        black_box(var);
    }
}

fn var_construct_small_lists(_ctx: &mut NoContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let var = v_list(&[v_int(i as i64), v_int((i + 1) as i64)]);
        black_box(var);
    }
}

fn var_construct_nested_lists(_ctx: &mut NoContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let inner = v_list(&[v_int(i as i64), v_str("nested")]);
        let var = v_list(&[inner, v_int((i + 1) as i64)]);
        black_box(var);
    }
}

const VAR_CONSTRUCT_INPUT_POOL_SIZE: usize = 4096;
const VAR_CONSTRUCT_SYMBOL_HOT_SET_SIZE: usize = 8;
const VAR_CONSTRUCT_HOT_SET_SIZE: usize = 8;

struct VarConstructInputsContext {
    ints: Vec<i64>,
    objs: Vec<Obj>,
    ascii_strings: Vec<String>,
    unicode_strings: Vec<String>,
    ascii_arc_strings: Vec<arcstr::ArcStr>,
    hot_symbol_names: Vec<String>,
    hot_symbols: Vec<Symbol>,
}

impl BenchContext for VarConstructInputsContext {
    fn prepare(_num_chunks: usize) -> Self {
        let mut ints = Vec::with_capacity(VAR_CONSTRUCT_INPUT_POOL_SIZE);
        let mut objs = Vec::with_capacity(VAR_CONSTRUCT_INPUT_POOL_SIZE);
        let mut ascii_strings = Vec::with_capacity(VAR_CONSTRUCT_INPUT_POOL_SIZE);
        let mut unicode_strings = Vec::with_capacity(VAR_CONSTRUCT_INPUT_POOL_SIZE);
        let mut ascii_arc_strings = Vec::with_capacity(VAR_CONSTRUCT_INPUT_POOL_SIZE);

        for i in 0..VAR_CONSTRUCT_INPUT_POOL_SIZE {
            ints.push(i as i64);
            objs.push(Obj::mk_id((i as i32) + 1));

            let ascii = format!("bench_ascii_{i}");
            let unicode = format!("bench_{i}_héllø");

            ascii_arc_strings.push(arcstr::ArcStr::from(ascii.clone()));
            ascii_strings.push(ascii);
            unicode_strings.push(unicode);
        }

        let mut hot_symbol_names = Vec::with_capacity(VAR_CONSTRUCT_SYMBOL_HOT_SET_SIZE);
        let mut hot_symbols = Vec::with_capacity(VAR_CONSTRUCT_SYMBOL_HOT_SET_SIZE);
        for i in 0..VAR_CONSTRUCT_SYMBOL_HOT_SET_SIZE {
            let name = format!("bench_hot_symbol_{i}");
            hot_symbols.push(Symbol::mk(&name));
            hot_symbol_names.push(name);
        }

        Self {
            ints,
            objs,
            ascii_strings,
            unicode_strings,
            ascii_arc_strings,
            hot_symbol_names,
            hot_symbols,
        }
    }
}

fn var_construct_variant_int(
    ctx: &mut VarConstructInputsContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_INPUT_POOL_SIZE - 1);
        let var = v_int(ctx.ints[idx]);
        black_box(var);
    }
}

fn var_construct_variant_obj(
    ctx: &mut VarConstructInputsContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_INPUT_POOL_SIZE - 1);
        let var = v_obj(ctx.objs[idx]);
        black_box(var);
    }
}

fn var_construct_variant_str_ascii(
    ctx: &mut VarConstructInputsContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_INPUT_POOL_SIZE - 1);
        let var = v_str(ctx.ascii_strings[idx].as_str());
        black_box(var);
    }
}

fn var_construct_variant_str_unicode(
    ctx: &mut VarConstructInputsContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_INPUT_POOL_SIZE - 1);
        let var = v_str(ctx.unicode_strings[idx].as_str());
        black_box(var);
    }
}

fn var_construct_variant_string_owned_ascii(
    ctx: &mut VarConstructInputsContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_INPUT_POOL_SIZE - 1);
        let var = v_string(ctx.ascii_strings[idx].clone());
        black_box(var);
    }
}

fn var_construct_variant_arc_str_ascii(
    ctx: &mut VarConstructInputsContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_INPUT_POOL_SIZE - 1);
        let var = v_arc_str(ctx.ascii_arc_strings[idx].clone());
        black_box(var);
    }
}

fn var_construct_variant_symbol_str_cached(
    ctx: &mut VarConstructInputsContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_SYMBOL_HOT_SET_SIZE - 1);
        let var = v_symbol_str(ctx.hot_symbols[idx]);
        black_box(var);
    }
}

fn var_construct_variant_sym_from_hot_str(
    ctx: &mut VarConstructInputsContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_SYMBOL_HOT_SET_SIZE - 1);
        let var = v_sym(ctx.hot_symbol_names[idx].as_str());
        black_box(var);
    }
}

struct VarConstructHotSetContext {
    ascii_strings: Vec<String>,
    unicode_strings: Vec<String>,
    ascii_arc_strings: Vec<arcstr::ArcStr>,
    symbols: Vec<Symbol>,
}

impl BenchContext for VarConstructHotSetContext {
    fn prepare(_num_chunks: usize) -> Self {
        let ascii_literals = [
            "hot_ascii_0",
            "hot_ascii_1",
            "hot_ascii_2",
            "hot_ascii_3",
            "hot_ascii_4",
            "hot_ascii_5",
            "hot_ascii_6",
            "hot_ascii_7",
        ];
        let unicode_literals = [
            "hot_0_héllø",
            "hot_1_héllø",
            "hot_2_héllø",
            "hot_3_héllø",
            "hot_4_héllø",
            "hot_5_héllø",
            "hot_6_héllø",
            "hot_7_héllø",
        ];

        let mut ascii_strings = Vec::with_capacity(VAR_CONSTRUCT_HOT_SET_SIZE);
        let mut unicode_strings = Vec::with_capacity(VAR_CONSTRUCT_HOT_SET_SIZE);
        let mut ascii_arc_strings = Vec::with_capacity(VAR_CONSTRUCT_HOT_SET_SIZE);
        let mut symbols = Vec::with_capacity(VAR_CONSTRUCT_HOT_SET_SIZE);

        for i in 0..VAR_CONSTRUCT_HOT_SET_SIZE {
            let ascii = ascii_literals[i].to_string();
            ascii_arc_strings.push(arcstr::ArcStr::from(ascii.clone()));
            symbols.push(Symbol::mk(ascii_literals[i]));
            ascii_strings.push(ascii);
            unicode_strings.push(unicode_literals[i].to_string());
        }

        Self {
            ascii_strings,
            unicode_strings,
            ascii_arc_strings,
            symbols,
        }
    }
}

fn var_construct_variant_str_ascii_hot(
    ctx: &mut VarConstructHotSetContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_HOT_SET_SIZE - 1);
        let var = v_str(ctx.ascii_strings[idx].as_str());
        black_box(var);
    }
}

fn var_construct_variant_str_unicode_hot(
    ctx: &mut VarConstructHotSetContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_HOT_SET_SIZE - 1);
        let var = v_str(ctx.unicode_strings[idx].as_str());
        black_box(var);
    }
}

fn var_construct_variant_string_owned_ascii_hot(
    ctx: &mut VarConstructHotSetContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_HOT_SET_SIZE - 1);
        let var = v_string(ctx.ascii_strings[idx].clone());
        black_box(var);
    }
}

fn var_construct_variant_arc_str_ascii_hot(
    ctx: &mut VarConstructHotSetContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_HOT_SET_SIZE - 1);
        let var = v_arc_str(ctx.ascii_arc_strings[idx].clone());
        black_box(var);
    }
}

fn var_construct_variant_symbol_str_cached_hot(
    ctx: &mut VarConstructHotSetContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let idx = i & (VAR_CONSTRUCT_HOT_SET_SIZE - 1);
        let var = v_symbol_str(ctx.symbols[idx]);
        black_box(var);
    }
}

// CLONE BENCHMARKS ===
// These measure cloning costs which are relevant for scope operations

// Context for int clone benchmarks (simple Copy-like types)
struct IntCloneContext(Var, Var);
impl BenchContext for IntCloneContext {
    fn prepare(_num_chunks: usize) -> Self {
        IntCloneContext(v_int(42), v_none())
    }
}

// Context for string clone benchmarks
struct StringCloneContext(Var, Var);
impl BenchContext for StringCloneContext {
    fn prepare(_num_chunks: usize) -> Self {
        StringCloneContext(v_str("test_string_for_cloning"), v_none())
    }
}

// Context for list clone benchmarks
struct ListCloneContext(Var, Var);
impl BenchContext for ListCloneContext {
    fn prepare(_num_chunks: usize) -> Self {
        ListCloneContext(
            v_list(&[v_int(1), v_str("test"), v_int(2), v_str("clone")]),
            v_none(),
        )
    }
}

fn var_clone_ints(ctx: &mut IntCloneContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        ctx.1 = ctx.0.clone();
    }
}

fn var_clone_strings(ctx: &mut StringCloneContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        ctx.1 = ctx.0.clone();
    }
}

fn var_clone_lists(ctx: &mut ListCloneContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        ctx.1 = ctx.0.clone();
    }
}

// === AS_INTEGER BENCHMARKS ===

fn var_as_integer(ctx: &mut IntCloneContext, chunk_size: usize, _chunk_num: usize) {
    let v = &ctx.0;
    for _ in 0..chunk_size {
        let _ = black_box(v.as_integer());
    }
}

// === STRING SEARCH BENCHMARKS ===
// These measure the performance of str_find, str_rfind, and str_replace
// with both ASCII fast path and Unicode fallback

// Context for ASCII string search benchmarks
struct AsciiStringSearchContext {
    subject: Var,
    needle: Var,
    replacement: Var,
}
impl BenchContext for AsciiStringSearchContext {
    fn prepare(_num_chunks: usize) -> Self {
        // A longer string to search in - all ASCII
        let subject = "The quick brown fox jumps over the lazy dog. ".repeat(100);
        AsciiStringSearchContext {
            subject: Var::mk_str(&subject),
            needle: Var::mk_str("lazy"),
            replacement: Var::mk_str("sleepy"),
        }
    }
}

// Context for Unicode string search benchmarks
struct UnicodeStringSearchContext {
    subject: Var,
    needle: Var,
    replacement: Var,
}
impl BenchContext for UnicodeStringSearchContext {
    fn prepare(_num_chunks: usize) -> Self {
        // String with İ (U+0130) which lowercases to 'i' + combining char
        let subject = "Türkİye İstanbul İzmir ".repeat(100);
        UnicodeStringSearchContext {
            subject: Var::mk_str(&subject),
            needle: Var::mk_str("i"), // Should match İ case-insensitively
            replacement: Var::mk_str("X"),
        }
    }
}

fn str_find_ascii_cs(ctx: &mut AsciiStringSearchContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let _ = black_box(ctx.subject.str_find(&ctx.needle, true, 0));
    }
}

fn str_find_ascii_ci(ctx: &mut AsciiStringSearchContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let _ = black_box(ctx.subject.str_find(&ctx.needle, false, 0));
    }
}

fn str_find_unicode_ci(ctx: &mut UnicodeStringSearchContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let _ = black_box(ctx.subject.str_find(&ctx.needle, false, 0));
    }
}

fn str_rfind_ascii_cs(ctx: &mut AsciiStringSearchContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let _ = black_box(ctx.subject.str_rfind(&ctx.needle, true, 0));
    }
}

fn str_rfind_ascii_ci(ctx: &mut AsciiStringSearchContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let _ = black_box(ctx.subject.str_rfind(&ctx.needle, false, 0));
    }
}

fn str_rfind_unicode_ci(
    ctx: &mut UnicodeStringSearchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for _ in 0..chunk_size {
        let _ = black_box(ctx.subject.str_rfind(&ctx.needle, false, 0));
    }
}

fn str_replace_ascii_cs(ctx: &mut AsciiStringSearchContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let _ = black_box(ctx.subject.str_replace(&ctx.needle, &ctx.replacement, true));
    }
}

fn str_replace_ascii_ci(ctx: &mut AsciiStringSearchContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let _ = black_box(
            ctx.subject
                .str_replace(&ctx.needle, &ctx.replacement, false),
        );
    }
}

fn str_replace_unicode_ci(
    ctx: &mut UnicodeStringSearchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for _ in 0..chunk_size {
        let _ = black_box(
            ctx.subject
                .str_replace(&ctx.needle, &ctx.replacement, false),
        );
    }
}

// Context that pre-creates pools of Vars to drop
struct DropContext {
    int_vars: Vec<Var>,
    string_vars: Vec<Var>,
    list_vars: Vec<Var>,
    mixed_vars: Vec<Var>,
}

impl BenchContext for DropContext {
    fn prepare(_num_chunks: usize) -> Self {
        let pool_size = 100_000; // Pool size matches our preferred chunk size
        DropContext {
            int_vars: (0..pool_size).map(|i| v_int(i as i64)).collect(),
            string_vars: (0..pool_size).map(|i| v_str(&format!("str_{i}"))).collect(),
            list_vars: (0..pool_size)
                .map(|i| v_list(&[v_int(i as i64), v_str("item")]))
                .collect(),
            mixed_vars: (0..pool_size)
                .map(|i| match i % 5 {
                    0 => v_int(i as i64),
                    1 => v_str(&format!("str_{i}")),
                    2 => v_list(&[v_int(i as i64)]),
                    3 => v_float(i as f64),
                    _ => v_bool(i % 2 == 0),
                })
                .collect(),
        }
    }

    fn chunk_size() -> Option<usize> {
        Some(50_000) // Use half the pool size so we can do multiple samples
    }
}

fn var_drop_ints(ctx: &mut DropContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size items from the pool
    ctx.int_vars.truncate(ctx.int_vars.len() - chunk_size);
}

fn var_drop_strings(ctx: &mut DropContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size items from the pool
    ctx.string_vars.truncate(ctx.string_vars.len() - chunk_size);
}

fn var_drop_lists(ctx: &mut DropContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size items from the pool
    ctx.list_vars.truncate(ctx.list_vars.len() - chunk_size);
}

fn var_drop_mixed(ctx: &mut DropContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size items from the pool
    ctx.mixed_vars.truncate(ctx.mixed_vars.len() - chunk_size);
}

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some(
            "all, int, list, scope, construct, drop, clone, or any benchmark name substring".to_string()
        ),
        ..BenchmarkMainOptions::default()
    },
    |runner| {
    runner.group::<IntContext>("Integer Operations", |g| {
        g.bench("int_add", int_add);
        g.bench("mixed_add", mixed_add);
        g.bench("int_eq", int_eq);
        g.bench("int_cmp", int_cmp);
    });

    runner.group::<FloatContext>("Float Operations", |g| {
        g.bench("float_add", float_add);
    });

    runner.group::<SmallListContext>("Small List Operations", |g| {
        g.bench("list_push", list_push);
    });

    runner.group::<LargeListContext>("Large List Operations", |g| {
        g.bench("list_index_pos", list_index_pos);
        g.bench("list_index_assign", list_index_assign);
    });

    runner.group::<NoContext>("Var Construction Benchmarks", |g| {
        g.bench("var_construct_ints", var_construct_ints);
        g.bench("var_construct_strings", var_construct_strings);
        g.bench("var_construct_small_lists", var_construct_small_lists);
        g.bench("var_construct_nested_lists", var_construct_nested_lists);
    });

    runner.group::<VarConstructInputsContext>("Var Constructor Variant Benchmarks", |g| {
        g.bench("var_construct_variant_int", var_construct_variant_int);
        g.bench("var_construct_variant_obj", var_construct_variant_obj);
        g.bench("var_construct_variant_str_ascii", var_construct_variant_str_ascii);
        g.bench("var_construct_variant_str_unicode", var_construct_variant_str_unicode);
        g.bench(
            "var_construct_variant_string_owned_ascii",
            var_construct_variant_string_owned_ascii,
        );
        g.bench("var_construct_variant_arc_str_ascii", var_construct_variant_arc_str_ascii);
        g.bench(
            "var_construct_variant_symbol_str_cached",
            var_construct_variant_symbol_str_cached,
        );
        g.bench("var_construct_variant_sym_from_hot_str", var_construct_variant_sym_from_hot_str);
    });

    runner.group::<VarConstructHotSetContext>("Var Constructor Hot-Set Benchmarks", |g| {
        g.bench("var_construct_variant_str_ascii_hot", var_construct_variant_str_ascii_hot);
        g.bench("var_construct_variant_str_unicode_hot", var_construct_variant_str_unicode_hot);
        g.bench(
            "var_construct_variant_string_owned_ascii_hot",
            var_construct_variant_string_owned_ascii_hot,
        );
        g.bench("var_construct_variant_arc_str_ascii_hot", var_construct_variant_arc_str_ascii_hot);
        g.bench(
            "var_construct_variant_symbol_str_cached_hot",
            var_construct_variant_symbol_str_cached_hot,
        );
    });

    runner.group::<DropContext>("Var Drop Benchmarks", |g| {
        g.bench("var_drop_ints", var_drop_ints);
        g.bench("var_drop_strings", var_drop_strings);
        g.bench("var_drop_lists", var_drop_lists);
        g.bench("var_drop_mixed", var_drop_mixed);
    });

    runner.group::<IntCloneContext>("Var Clone (Int) Benchmarks", |g| {
        g.bench("var_clone_ints", var_clone_ints);
    });

    runner.group::<StringCloneContext>("Var Clone (String) Benchmarks", |g| {
        g.bench("var_clone_strings", var_clone_strings);
    });

    runner.group::<ListCloneContext>("Var Clone (List) Benchmarks", |g| {
        g.bench("var_clone_lists", var_clone_lists);
    });

    runner.group::<IntCloneContext>("Var.as_integer() Benchmarks", |g| {
        g.bench("var_as_integer", var_as_integer);
    });

    runner.group::<AsciiStringSearchContext>("String Search (ASCII)", |g| {
        g.bench("str_find_ascii_cs", str_find_ascii_cs);
        g.bench("str_find_ascii_ci", str_find_ascii_ci);
        g.bench("str_rfind_ascii_cs", str_rfind_ascii_cs);
        g.bench("str_rfind_ascii_ci", str_rfind_ascii_ci);
        g.bench("str_replace_ascii_cs", str_replace_ascii_cs);
        g.bench("str_replace_ascii_ci", str_replace_ascii_ci);
    });

    runner.group::<UnicodeStringSearchContext>("String Search (Unicode)", |g| {
        g.bench("str_find_unicode_ci", str_find_unicode_ci);
        g.bench("str_rfind_unicode_ci", str_rfind_unicode_ci);
        g.bench("str_replace_unicode_ci", str_replace_unicode_ci);
    });
    }
);
