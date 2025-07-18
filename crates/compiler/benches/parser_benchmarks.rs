// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com>
// GPL v3.0 License

//! Benchmarks comparing parsing performance between different approaches:
//! - Original PEST parser (legacy)
//! - PEST CST parser 
//! - Tree-sitter CST parser
//! - Tree-sitter Semantic Walker

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use moor_compiler::{CompileOptions, compile, parse_program_cst};

#[cfg(feature = "tree-sitter-parser")]
use moor_compiler::{compile_with_tree_sitter, parse_with_semantic_walker, parse_program_with_tree_sitter};

fn benchmark_parsers(c: &mut Criterion) {
    let test_cases = vec![
        ("Simple", "return 42;"),
        ("Assignment", "x = player.name + \" says hello\";"),
        ("If Statement", r#"if (x > 0) notify(player, "positive"); else notify(player, "non-positive"); endif"#),
        ("For Loop", "for i in [1..10] total = total + i; endfor"),
        ("Function Call", r#"result = $string_utils:capitalize($object_utils:name(player));"#),
        ("Try-Except", r#"try result = 1/0; except (e) notify(player, "Error: " + tostr(e)); endtry"#),
        ("Scatter Assignment", r#"let {things, ?nothingstr = "nothing", @rest} = args;"#),
        ("Complex Expression", r#"return $object_utils:ancestors(this)[1]:(verbs()[random(length(verbs()))])(player, player.location);"#),
        ("Large Program", r#"
            if (caller != this)
                return E_PERM;
            endif
            
            let {cmd, @args} = args;
            
            try
                player:tell("Processing command: " + cmd);
                
                if (cmd == "help")
                    for line in help_text
                        player:tell(line);
                    endfor
                elseif (cmd == "status")
                    player:tell("Status: " + tostr($time_utils:now()));
                    for obj in connected_players()
                        player:tell("Player: " + obj.name);
                    endfor
                else
                    player:tell("Unknown command: " + cmd);
                endif
                
            except e (ANY)
                player:tell("Error executing command: " + tostr(e));
                $error_manager:log_error(e, this, verb, args);
            endtry
        "#),
    ];
    
    let mut group = c.benchmark_group("Parser Comparison");
    
    for (test_name, test_code) in test_cases.iter() {
        // Original PEST parser (legacy AST)
        group.bench_with_input(
            BenchmarkId::new("PEST Legacy", test_name), 
            test_code,
            |b, code| {
                b.iter(|| {
                    black_box(compile(black_box(code), CompileOptions::default()))
                })
            }
        );
        
        // PEST CST parser
        group.bench_with_input(
            BenchmarkId::new("PEST CST", test_name), 
            test_code,
            |b, code| {
                b.iter(|| {
                    black_box(parse_program_cst(black_box(code), CompileOptions::default()))
                })
            }
        );
        
        #[cfg(feature = "tree-sitter-parser")]
        {
            // Tree-sitter CST parser
            group.bench_with_input(
                BenchmarkId::new("Tree-sitter CST", test_name), 
                test_code,
                |b, code| {
                    b.iter(|| {
                        black_box(parse_program_with_tree_sitter(black_box(code), CompileOptions::default()))
                    })
                }
            );
            
            // Tree-sitter full compilation
            group.bench_with_input(
                BenchmarkId::new("Tree-sitter Compile", test_name), 
                test_code,
                |b, code| {
                    b.iter(|| {
                        black_box(compile_with_tree_sitter(black_box(code), CompileOptions::default()))
                    })
                }
            );
            
            // Tree-sitter Semantic Walker
            group.bench_with_input(
                BenchmarkId::new("Tree-sitter Semantic", test_name), 
                test_code,
                |b, code| {
                    b.iter(|| {
                        black_box(parse_with_semantic_walker(black_box(code)))
                    })
                }
            );
        }
    }
    
    group.finish();
}

fn benchmark_scatter_assignment_specifically(c: &mut Criterion) {
    let scatter_cases = vec![
        ("Basic Scatter", "{x, y} = {1, 2};"),
        ("Optional Scatter", r#"let {cmd, ?arg = "default"} = args;"#),
        ("Rest Scatter", "let {first, @rest} = items;"),
        ("Complex Scatter", r#"let {things, ?nothingstr = "nothing", @rest} = $object_utils:inventory(player);"#),
        ("Nested Scatter", r#"let {{inner_x, inner_y}, z} = {{1, 2}, 3};"#),
    ];
    
    let mut group = c.benchmark_group("Scatter Assignment Focus");
    
    for (test_name, test_code) in scatter_cases.iter() {
        // PEST CST
        group.bench_with_input(
            BenchmarkId::new("PEST CST", test_name), 
            test_code,
            |b, code| {
                b.iter(|| {
                    black_box(parse_program_cst(black_box(code), CompileOptions::default()))
                })
            }
        );
        
        #[cfg(feature = "tree-sitter-parser")]
        {
            // Tree-sitter CST
            group.bench_with_input(
                BenchmarkId::new("Tree-sitter CST", test_name), 
                test_code,
                |b, code| {
                    b.iter(|| {
                        black_box(parse_program_with_tree_sitter(black_box(code), CompileOptions::default()))
                    })
                }
            );
            
            // Tree-sitter Semantic Walker
            group.bench_with_input(
                BenchmarkId::new("Tree-sitter Semantic", test_name), 
                test_code,
                |b, code| {
                    b.iter(|| {
                        black_box(parse_with_semantic_walker(black_box(code)))
                    })
                }
            );
        }
    }
    
    group.finish();
}

criterion_group!(benches, benchmark_parsers, benchmark_scatter_assignment_specifically);
criterion_main!(benches);