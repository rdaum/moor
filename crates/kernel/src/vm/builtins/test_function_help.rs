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

#[cfg(test)]
mod tests {
    use crate::vm::builtins::docs::BUILTIN_DOCS;

    #[test]
    fn test_builtin_docs_generated() {
        assert!(!BUILTIN_DOCS.is_empty(), "BUILTIN_DOCS should not be empty");
    }

    #[test]
    fn test_abs_has_docs() {
        let docs = BUILTIN_DOCS.get("abs");
        assert!(docs.is_some(), "abs builtin should have documentation");

        let lines = docs.unwrap();
        assert!(
            !lines.is_empty(),
            "abs documentation should have at least one line"
        );

        // Check that the first line contains the MOO signature
        assert!(lines[0].contains("abs"), "First line should mention abs");
    }

    #[test]
    fn test_min_has_docs() {
        let docs = BUILTIN_DOCS.get("min");
        assert!(docs.is_some(), "min builtin should have documentation");

        let lines = docs.unwrap();
        assert!(
            !lines.is_empty(),
            "min documentation should have at least one line"
        );
    }

    #[test]
    fn test_nonexistent_builtin() {
        let docs = BUILTIN_DOCS.get("this_builtin_does_not_exist");
        assert!(docs.is_none(), "Non-existent builtin should return None");
    }

    #[test]
    fn test_multiple_builtins_extracted() {
        // Should have extracted docs from multiple files
        assert!(
            BUILTIN_DOCS.len() > 10,
            "Should have docs for many builtins"
        );

        // Check a few from different categories
        assert!(BUILTIN_DOCS.get("abs").is_some()); // bf_num.rs
        assert!(BUILTIN_DOCS.get("length").is_some()); // bf_values.rs
        assert!(BUILTIN_DOCS.get("notify").is_some()); // bf_server.rs
    }
}
#[test]
fn print_sample_docs() {
    use crate::vm::builtins::docs::BUILTIN_DOCS;

    println!("\n=== Sample Documentation ===");

    if let Some(docs) = BUILTIN_DOCS.get("abs") {
        println!("\nfunction_help(\"abs\") returns:");
        for line in docs {
            println!("  {line}");
        }
    }

    if let Some(docs) = BUILTIN_DOCS.get("min") {
        println!("\nfunction_help(\"min\") returns:");
        for line in docs {
            println!("  {line}");
        }
    }

    println!("\nTotal builtins documented: {}", BUILTIN_DOCS.len());
}
