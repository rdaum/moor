use moor_compiler::{compile, CompileOptions};
#[cfg(feature = "tree-sitter-parser")]
use moor_compiler::compile_with_tree_sitter;

fn main() {
    println!("Parser Error Reporting Comparison");
    println!("=================================\n");

    let test_source = "x = @invalid_token";
    
    println!("Testing with source: {test_source}");
    println!();
    
    // Test PEST CST parser
    println!("--- PEST CST Parser ---");
    match compile(test_source, CompileOptions::default()) {
        Ok(_) => println!("✓ Unexpectedly succeeded!"),
        Err(error) => {
            println!("✗ Error Report:");
            println!("{error}");
        }
    }
    
    println!();
    
    // Test Tree-sitter parser
    #[cfg(feature = "tree-sitter-parser")]
    {
        println!("--- Tree-sitter Parser ---");
        match compile_with_tree_sitter(test_source, CompileOptions::default()) {
            Ok(_) => println!("✓ Unexpectedly succeeded!"),
            Err(error) => {
                println!("✗ Enhanced Error Report:");
                println!("{}", error);
            }
        }
    }
    
    #[cfg(not(feature = "tree-sitter-parser"))]
    {
        println!("--- Tree-sitter Parser ---");
        println!("Tree-sitter parser not available. Enable with --features tree-sitter-parser");
    }
}