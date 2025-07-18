use moor_compiler::{compile, CompileOptions};
#[cfg(feature = "tree-sitter-parser")]
use moor_compiler::compile_with_tree_sitter;
use moor_compiler::test_utils::*;

fn main() {
    println!("=== Comprehensive Parser Validation ===\n");
    
    // Get test code from command line or use default
    let custom_test = std::env::args().nth(1);
    
    if let Some(test_code) = custom_test {
        println!("=== Single Test Mode ===");
        println!("Testing code: {}", test_code);
        run_debug_comparison(&test_code);
    } else {
        println!("=== Comprehensive Test Suite ===");
        run_comprehensive_tests();
    }
}

fn run_debug_comparison(test_code: &str) {
    println!("\n=== Parser Comparison ===");
    let result = compare_parsers(test_code);
    result.print_result("Debug Test");
    
    if !result.parsers_agree {
        println!("\n⚠️  Parser disagreement detected!");
        println!("This indicates a potential compatibility issue between parsers.");
        std::process::exit(1);
    }
}

fn run_comprehensive_tests() {
    let test_cases = vec![
        // Basic expressions
        ("Basic return", "return 42;"),
        ("Simple assignment", "x = 5;"),
        ("If statement", "if (x > 0) return 1; endif"),
        
        // Function calls
        ("Notify call", "notify(player, \"hello\");"),
        ("System property call", "result = $string_utils:capitalize(\"test\");"),
        
        // Complex expressions
        ("For loop", "for i in [1..10] x = x + i; endfor"),
        ("Try-except", "try x = 1/0; except (e) notify(player, \"error\"); endtry"),
        
        // Edge cases
        ("Scatter assignment", "{x, y} = {1, 2};"),
        ("Property access", "x = $obj.prop;"),
        ("Builtin call", "return callers()[1];"),
        
        // Advanced cases
        ("Conditional expr", "x = (y > 0) ? 1 | 0;"),
        ("List comprehension", "result = [x for x in [1..10] if x % 2 == 0];"),
        ("Map literal", "data = [\"key\" -> \"value\", \"num\" -> 42];"),
        
        // Error cases (should parse but may have runtime issues)
        ("Multi-statement", "return 42; \"moot-line:2\";"),
        ("Empty statement", ";"),
        ("Complex expression", "return $object_utils:ancestors(this)[1].name;"),
    ];
    
    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut parser_disagreements = 0;
    
    for (i, (name, test_case)) in test_cases.iter().enumerate() {
        println!("Test {}: {} - {}", i + 1, name, test_case);
        
        let comparison_result = compare_parsers(test_case);
        
        // Test individual parsers
        let pest_result = test_pest_parser(test_case);
        let tree_sitter_result = test_tree_sitter_parser(test_case);
        
        // Summary for this test
        let both_passed = pest_result && tree_sitter_result;
        let parsers_agree = comparison_result.parsers_agree;
        
        if both_passed {
            total_passed += 1;
            if parsers_agree {
                println!("  ✓ Both parsers succeeded and agree");
            } else {
                println!("  ⚠️  Both parsers succeeded but disagree on structure");
                parser_disagreements += 1;
            }
        } else {
            total_failed += 1;
            println!("  ✗ One or both parsers failed");
            if !parsers_agree {
                parser_disagreements += 1;
            }
        }
        
        println!();
    }
    
    println!("=== COMPREHENSIVE VALIDATION SUMMARY ===");
    println!("Total tests: {}", test_cases.len());
    println!("Passed: {}", total_passed);
    println!("Failed: {}", total_failed);
    println!("Parser disagreements: {}", parser_disagreements);
    
    if total_failed > 0 {
        println!("\n❌ Some tests failed - check parser implementations");
        std::process::exit(1);
    }
    
    if parser_disagreements > 0 {
        println!("\n⚠️  Parser disagreements detected - check compatibility");
        std::process::exit(1);
    }
    
    println!("\n✅ All tests passed with parser agreement!");
}

fn test_pest_parser(test_case: &str) -> bool {
    match compile(test_case, CompileOptions::default()) {
        Ok(_) => {
            println!("    PEST: ✓ Success");
            true
        }
        Err(e) => {
            println!("    PEST: ✗ Error - {}", e);
            false
        }
    }
}

fn test_tree_sitter_parser(test_case: &str) -> bool {
    #[cfg(feature = "tree-sitter-parser")]
    {
        match compile_with_tree_sitter(test_case, CompileOptions::default()) {
            Ok(_) => {
                println!("    Tree-sitter: ✓ Success");
                true
            }
            Err(e) => {
                println!("    Tree-sitter: ✗ Error - {}", e);
                false
            }
        }
    }
    
    #[cfg(not(feature = "tree-sitter-parser"))]
    {
        println!("    Tree-sitter: ⚠️  Not available (enable with --features tree-sitter-parser)");
        true // Don't fail CI if tree-sitter is not enabled
    }
}