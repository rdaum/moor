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

//! Source-side conformance corpus for the persistence-oriented codec.
//!
//! §8 and §15.2 of `pluggable-persistence-and-postgresql.md` require canonical decompiled MOO
//! source that compiles back to equivalent behavior. This file establishes the compiler-visible
//! half of that target: canonical source must be a fixed point (compile → decompile → recompile →
//! decompile produces identical text). Lambda *values* and behavioral equivalence after reload
//! need a runtime lambda, so those cases live with the kernel/VM tests that accompany the
//! fallible codec in §16 step 4.

use moor_compiler::{CompileOptions, compile, program_to_tree, unparse};

/// Compile MOO source and return its canonical decompiled form.
fn canonical_source(source: &str) -> Result<String, String> {
    let program = compile(source, CompileOptions::default())
        .map_err(|error| format!("compile failed: {error:?}"))?;
    let tree = program_to_tree(&program).map_err(|error| format!("decompile failed: {error:?}"))?;
    let lines =
        unparse(&tree, false, true).map_err(|error| format!("unparse failed: {error:?}"))?;
    Ok(lines.join("\n"))
}

#[track_caller]
fn assert_source_fixed_point(source: &str) {
    let first = canonical_source(source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    let second =
        canonical_source(&first).unwrap_or_else(|error| panic!("recompiling {first:?}: {error}"));
    assert_eq!(
        first, second,
        "canonical source is not a fixed point for {source:?}"
    );
}

#[test]
fn representative_programs_decompile_to_a_fixed_point() {
    let programs = [
        // Control flow.
        "if (1) return 2; elseif (2) return 3; else return 4; endif",
        "while labelled (1) if (1 == 2) break labelled; else continue labelled; endif endwhile",
        "for x in [1..5] return 2; endfor",
        "try return 1; except a (E_INVARG) return 2; except b (E_PROPNF) return 3; endtry",
        "try return 1; finally return 2; endtry",
        // Expressions.
        "return {1,2,3,@{1,2,3},4};",
        "return -(1 + 2 * (3 - 4) / 5 % 6);",
        "return 1 == 2 != 3 < 4 <= 5 > 6 >= 7;",
        "return 1 && 2 || 3 && 4;",
        "return x[1..2];",
        r#"return x:("y")(1,2,3);"#,
        r#"return $ansi:(this.some_function)();"#,
        "1 ? 2 | 3;",
        r#"options="test"; return #0.(options);"#,
        r#"[ 1 -> 2, 3 -> 4 ];"#,
        // Assignments and scatters, including optional defaults.
        "{connection, player, ?arg3, @arg4} = args;",
        "a[1..2] = {3,4};",
        r#"{?package = 5} = args;"#,
        r#"{?package = $nothing} = args;"#,
        "let {first, ?second = 2, @rest} = args; first = 3; return {first, second, rest};",
        "x = `x + 1 ! e_propnf, E_PERM => 17';",
        // Forks and named functions.
        r#"5; fork (5) 1; endfork 2;"#,
        r#"5; fork tst (5) 1; endfork 2;"#,
        "const captured = 1; fn get() const local = captured; return local; endfn return get();",
        "fn outer() const x = 1; fork (0) let y = x; fork (0) const z = y; z; endfork endfork return x; endfn return outer();",
        // Lambda expressions with captures.
        "const base = 2; f = {x} => x + base; return f(3);",
    ];

    for source in programs {
        assert_source_fixed_point(source);
    }
}
