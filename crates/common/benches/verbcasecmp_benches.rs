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

use moor_bench_utils::{BenchContext, black_box};
use moor_common::util::verbcasecmp;

// Context for exact match benchmarks
struct ExactMatchContext {
    patterns: Vec<&'static str>,
    words: Vec<&'static str>,
}

impl BenchContext for ExactMatchContext {
    fn prepare(_num_chunks: usize) -> Self {
        ExactMatchContext {
            patterns: vec!["look", "get", "drop", "give", "examine", "inventory", "who"],
            words: vec!["look", "get", "drop", "give", "examine", "inventory", "who"],
        }
    }
}

// Context for case insensitive match benchmarks
struct CaseMatchContext {
    patterns: Vec<&'static str>,
    words: Vec<&'static str>,
}

impl BenchContext for CaseMatchContext {
    fn prepare(_num_chunks: usize) -> Self {
        CaseMatchContext {
            patterns: vec!["look", "get", "drop", "give", "examine", "inventory", "who"],
            words: vec!["LOOK", "Get", "DROP", "Give", "EXAMINE", "Inventory", "WHO"],
        }
    }
}

// Context for wildcard match benchmarks
struct WildcardMatchContext {
    patterns: Vec<&'static str>,
    words: Vec<&'static str>,
}

impl BenchContext for WildcardMatchContext {
    fn prepare(_num_chunks: usize) -> Self {
        WildcardMatchContext {
            patterns: vec!["l*", "ex*", "inv*", "wh*", "foo*bar", "g*ive", "dr*op"],
            words: vec![
                "look",
                "examine",
                "inventory",
                "who",
                "foobar",
                "give",
                "drop",
            ],
        }
    }
}

// Context for pronoun pattern benchmarks (the case we fixed)
struct PronounMatchContext {
    patterns: Vec<&'static str>,
    words: Vec<&'static str>,
}

impl BenchContext for PronounMatchContext {
    fn prepare(_num_chunks: usize) -> Self {
        PronounMatchContext {
            patterns: vec!["ps*c", "po*c", "pr*c", "pp*c", "pq*c", "psu", "pru"],
            words: vec!["psc", "poc", "prc", "ppc", "pqc", "psu", "pru"],
        }
    }
}

// Context for mismatch benchmarks (should be fast fails)
struct MismatchContext {
    patterns: Vec<&'static str>,
    words: Vec<&'static str>,
}

impl BenchContext for MismatchContext {
    fn prepare(_num_chunks: usize) -> Self {
        MismatchContext {
            patterns: vec!["look", "get", "drop", "give", "examine"],
            words: vec!["talk", "run", "jump", "sleep", "think"],
        }
    }
}

// Context for leading asterisk benchmarks (the broken case we investigated)
struct LeadingAsteriskContext {
    patterns: Vec<&'static str>,
    words: Vec<&'static str>,
}

impl BenchContext for LeadingAsteriskContext {
    fn prepare(_num_chunks: usize) -> Self {
        LeadingAsteriskContext {
            patterns: vec!["*p", "*xyz", "**test", "*foo*bar"],
            words: vec!["p", "xyz", "test", "foobar"],
        }
    }
}

// Context for long string benchmarks
struct LongStringContext {
    patterns: Vec<String>,
    words: Vec<String>,
}

impl BenchContext for LongStringContext {
    fn prepare(_num_chunks: usize) -> Self {
        let long_patterns: Vec<String> = vec![
            "very_long_verb_name_with_many_characters".to_string(),
            "another*extremely*long*pattern*with*wildcards".to_string(),
            "super_duper_extra_long_command_name_here".to_string(),
        ];
        let long_words: Vec<String> = vec![
            "very_long_verb_name_with_many_characters".to_string(),
            "another_extremely_long_pattern_with_wildcards".to_string(),
            "super_duper_extra_long_command_name_here".to_string(),
        ];

        LongStringContext {
            patterns: long_patterns,
            words: long_words,
        }
    }
}

fn exact_match_bench(ctx: &mut ExactMatchContext, chunk_size: usize, _chunk_num: usize) {
    let patterns = &ctx.patterns;
    let words = &ctx.words;

    for i in 0..chunk_size {
        let pattern_idx = i % patterns.len();
        let word_idx = i % words.len();
        let result = verbcasecmp(patterns[pattern_idx], words[word_idx]);
        black_box(result);
    }
}

fn case_insensitive_match_bench(ctx: &mut CaseMatchContext, chunk_size: usize, _chunk_num: usize) {
    let patterns = &ctx.patterns;
    let words = &ctx.words;

    for i in 0..chunk_size {
        let pattern_idx = i % patterns.len();
        let word_idx = i % words.len();
        let result = verbcasecmp(patterns[pattern_idx], words[word_idx]);
        black_box(result);
    }
}

fn wildcard_match_bench(ctx: &mut WildcardMatchContext, chunk_size: usize, _chunk_num: usize) {
    let patterns = &ctx.patterns;
    let words = &ctx.words;

    for i in 0..chunk_size {
        let pattern_idx = i % patterns.len();
        let word_idx = i % words.len();
        let result = verbcasecmp(patterns[pattern_idx], words[word_idx]);
        black_box(result);
    }
}

fn pronoun_match_bench(ctx: &mut PronounMatchContext, chunk_size: usize, _chunk_num: usize) {
    let patterns = &ctx.patterns;
    let words = &ctx.words;

    for i in 0..chunk_size {
        let pattern_idx = i % patterns.len();
        let word_idx = i % words.len();
        let result = verbcasecmp(patterns[pattern_idx], words[word_idx]);
        black_box(result);
    }
}

fn mismatch_bench(ctx: &mut MismatchContext, chunk_size: usize, _chunk_num: usize) {
    let patterns = &ctx.patterns;
    let words = &ctx.words;

    for i in 0..chunk_size {
        let pattern_idx = i % patterns.len();
        let word_idx = i % words.len();
        let result = verbcasecmp(patterns[pattern_idx], words[word_idx]);
        black_box(result);
    }
}

fn leading_asterisk_bench(ctx: &mut LeadingAsteriskContext, chunk_size: usize, _chunk_num: usize) {
    let patterns = &ctx.patterns;
    let words = &ctx.words;

    for i in 0..chunk_size {
        let pattern_idx = i % patterns.len();
        let word_idx = i % words.len();
        let result = verbcasecmp(patterns[pattern_idx], words[word_idx]);
        black_box(result);
    }
}

fn long_string_bench(ctx: &mut LongStringContext, chunk_size: usize, _chunk_num: usize) {
    let patterns = &ctx.patterns;
    let words = &ctx.words;

    for i in 0..chunk_size {
        let pattern_idx = i % patterns.len();
        let word_idx = i % words.len();
        let result = verbcasecmp(&patterns[pattern_idx], &words[word_idx]);
        black_box(result);
    }
}

// Mixed workload benchmark - cycles through different match types
struct MixedWorkloadContext {
    exact_patterns: Vec<&'static str>,
    exact_words: Vec<&'static str>,
    wildcard_patterns: Vec<&'static str>,
    wildcard_words: Vec<&'static str>,
    mismatch_patterns: Vec<&'static str>,
    mismatch_words: Vec<&'static str>,
}

impl BenchContext for MixedWorkloadContext {
    fn prepare(_num_chunks: usize) -> Self {
        MixedWorkloadContext {
            exact_patterns: vec!["look", "get", "drop", "give"],
            exact_words: vec!["look", "get", "drop", "give"],
            wildcard_patterns: vec!["l*", "ex*", "foo*bar", "g*ive"],
            wildcard_words: vec!["look", "examine", "foobar", "give"],
            mismatch_patterns: vec!["look", "get", "drop"],
            mismatch_words: vec!["talk", "run", "jump"],
        }
    }
}

fn mixed_workload_bench(ctx: &mut MixedWorkloadContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let result = match i % 3 {
            0 => {
                // Exact match
                let idx = i % ctx.exact_patterns.len();
                verbcasecmp(ctx.exact_patterns[idx], ctx.exact_words[idx])
            }
            1 => {
                // Wildcard match
                let idx = i % ctx.wildcard_patterns.len();
                verbcasecmp(ctx.wildcard_patterns[idx], ctx.wildcard_words[idx])
            }
            _ => {
                // Mismatch
                let idx = i % ctx.mismatch_patterns.len();
                verbcasecmp(ctx.mismatch_patterns[idx], ctx.mismatch_words[idx])
            }
        };
        black_box(result);
    }
}

pub fn main() {
    use moor_bench_utils::{BenchmarkDef, generate_session_summary, run_benchmark_group};
    use std::env;

    #[cfg(target_os = "linux")]
    {
        use moor_bench_utils::perf_event::{Builder, events::Hardware};
        // Check if we can do perf events, and if not warn but continue with timing-only benchmarks
        if Builder::new(Hardware::INSTRUCTIONS).build().is_err() {
            eprintln!(
                "⚠️  Perf events are not available on this system (insufficient permissions or kernel support)."
            );
            eprintln!("   Continuing with timing-only benchmarks (performance counters disabled).");
            eprintln!();
        }
    }

    let args: Vec<String> = env::args().collect();
    // Look for filter arguments after "--"
    let filter = if let Some(separator_pos) = args.iter().position(|arg| arg == "--") {
        // Filter is the first argument after "--"
        args.get(separator_pos + 1).map(|s| s.as_str())
    } else {
        // Fallback: look for any non-flag argument that's not our binary name
        args.iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--") && !args[0].contains(arg.as_str()))
            .map(|s| s.as_str())
    };

    if let Some(f) = filter {
        eprintln!("Running verbcasecmp benchmarks matching filter: '{f}'");
        eprintln!(
            "Available filters: all, exact, case, wildcard, pronoun, mismatch, leading, long, mixed, or any benchmark name substring"
        );
        eprintln!();
    }

    // Define all benchmark groups declaratively
    let exact_benchmarks = [BenchmarkDef {
        name: "exact_match",
        group: "exact",
        func: exact_match_bench,
    }];

    let case_benchmarks = [BenchmarkDef {
        name: "case_insensitive_match",
        group: "case",
        func: case_insensitive_match_bench,
    }];

    let wildcard_benchmarks = [BenchmarkDef {
        name: "wildcard_match",
        group: "wildcard",
        func: wildcard_match_bench,
    }];

    let pronoun_benchmarks = [BenchmarkDef {
        name: "pronoun_match",
        group: "pronoun",
        func: pronoun_match_bench,
    }];

    let mismatch_benchmarks = [BenchmarkDef {
        name: "mismatch",
        group: "mismatch",
        func: mismatch_bench,
    }];

    let leading_benchmarks = [BenchmarkDef {
        name: "leading_asterisk",
        group: "leading",
        func: leading_asterisk_bench,
    }];

    let long_benchmarks = [BenchmarkDef {
        name: "long_string_match",
        group: "long",
        func: long_string_bench,
    }];

    let mixed_benchmarks = [BenchmarkDef {
        name: "mixed_workload",
        group: "mixed",
        func: mixed_workload_bench,
    }];

    // Run benchmark groups
    run_benchmark_group::<ExactMatchContext>(&exact_benchmarks, "Exact Match Benchmarks", filter);
    run_benchmark_group::<CaseMatchContext>(
        &case_benchmarks,
        "Case Insensitive Match Benchmarks",
        filter,
    );
    run_benchmark_group::<WildcardMatchContext>(
        &wildcard_benchmarks,
        "Wildcard Match Benchmarks",
        filter,
    );
    run_benchmark_group::<PronounMatchContext>(
        &pronoun_benchmarks,
        "Pronoun Pattern Benchmarks",
        filter,
    );
    run_benchmark_group::<MismatchContext>(&mismatch_benchmarks, "Mismatch Benchmarks", filter);
    run_benchmark_group::<LeadingAsteriskContext>(
        &leading_benchmarks,
        "Leading Asterisk Benchmarks",
        filter,
    );
    run_benchmark_group::<LongStringContext>(&long_benchmarks, "Long String Benchmarks", filter);
    run_benchmark_group::<MixedWorkloadContext>(
        &mixed_benchmarks,
        "Mixed Workload Benchmarks",
        filter,
    );

    if filter.is_some() {
        eprintln!("\nVerbcasecmp benchmark filtering complete.");
    }

    // Generate session summary with regression analysis
    generate_session_summary();
}
