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

//! Proptest strategies for generating MOO AST nodes.

use crate::ast::{Arg, BinaryOp, Expr, UnaryOp};
use moor_var::program::names::{VarName, Variable};
use moor_var::{ErrorCode, Obj, Symbol, Var};
use proptest::prelude::*;
use proptest::strategy::BoxedStrategy;

// =============================================================================
// Literal Generators
// =============================================================================

/// Generate an arbitrary integer literal.
pub fn arb_integer() -> impl Strategy<Value = Expr> {
    // Use a reasonable range that won't cause overflow issues
    (-1_000_000i64..1_000_000i64).prop_map(|n| Expr::Value(Var::mk_integer(n)))
}

/// Generate an arbitrary float literal.
pub fn arb_float() -> impl Strategy<Value = Expr> {
    // Avoid special floats (NaN, Inf) and very small numbers that might round to 0
    prop_oneof![
        (-1000.0f64..1000.0).prop_map(|f| Expr::Value(Var::mk_float(f))),
        Just(Expr::Value(Var::mk_float(0.0))),
        Just(Expr::Value(Var::mk_float(1.0))),
        Just(Expr::Value(Var::mk_float(-1.0))),
    ]
}

/// Generate an arbitrary string literal.
pub fn arb_string() -> impl Strategy<Value = Expr> {
    // Generate simple ASCII strings to avoid unicode edge cases initially
    "[a-zA-Z0-9 .,!?;:'-]{0,50}".prop_map(|s| Expr::Value(Var::mk_str(&s)))
}

/// Generate an arbitrary object reference (#n).
pub fn arb_objref() -> impl Strategy<Value = Expr> {
    // Simple system object IDs
    (-10i32..1000i32).prop_map(|n| Expr::Value(Var::mk_object(Obj::mk_id(n))))
}

/// Generate an arbitrary boolean literal.
pub fn arb_bool() -> impl Strategy<Value = Expr> {
    prop_oneof![
        Just(Expr::Value(Var::mk_bool(true))),
        Just(Expr::Value(Var::mk_bool(false))),
    ]
}

/// Generate an arbitrary error literal (E_TYPE, E_PERM, etc.)
pub fn arb_error() -> impl Strategy<Value = Expr> {
    prop_oneof![
        Just(Expr::Error(ErrorCode::E_NONE, None)),
        Just(Expr::Error(ErrorCode::E_TYPE, None)),
        Just(Expr::Error(ErrorCode::E_DIV, None)),
        Just(Expr::Error(ErrorCode::E_PERM, None)),
        Just(Expr::Error(ErrorCode::E_PROPNF, None)),
        Just(Expr::Error(ErrorCode::E_VERBNF, None)),
        Just(Expr::Error(ErrorCode::E_VARNF, None)),
        Just(Expr::Error(ErrorCode::E_INVIND, None)),
        Just(Expr::Error(ErrorCode::E_RECMOVE, None)),
        Just(Expr::Error(ErrorCode::E_MAXREC, None)),
        Just(Expr::Error(ErrorCode::E_RANGE, None)),
        Just(Expr::Error(ErrorCode::E_ARGS, None)),
        Just(Expr::Error(ErrorCode::E_NACC, None)),
        Just(Expr::Error(ErrorCode::E_INVARG, None)),
        Just(Expr::Error(ErrorCode::E_QUOTA, None)),
        Just(Expr::Error(ErrorCode::E_FLOAT, None)),
        Just(Expr::Error(ErrorCode::E_FILE, None)),
        Just(Expr::Error(ErrorCode::E_EXEC, None)),
        Just(Expr::Error(ErrorCode::E_INTRPT, None)),
    ]
}

/// Generate an arbitrary literal value (int, float, string, obj, bool, error).
pub fn arb_literal() -> impl Strategy<Value = Expr> {
    prop_oneof![
        4 => arb_integer(),
        2 => arb_float(),
        2 => arb_string(),
        1 => arb_objref(),
        1 => arb_bool(),
        1 => arb_error(),
    ]
}

// =============================================================================
// Identifier Generators
// =============================================================================

/// MOO keywords that cannot be standalone identifiers.
const MOO_KEYWORDS: &[&str] = &[
    "for", "endfor", "if", "else", "return", "endif", "elseif", "while",
    "endwhile", "continue", "break", "fork", "endfork", "try", "except",
    "endtry", "finally", "in", "let", "fn", "endfn",
];

/// Check if a string is a valid MOO identifier.
/// Returns false if it's exactly a keyword.
fn is_valid_moo_identifier(s: &str) -> bool {
    let lower = s.to_lowercase();
    !MOO_KEYWORDS.contains(&lower.as_str())
}

/// Generate a valid MOO identifier string.
/// MOO identifiers: start with letter or underscore, followed by letters, digits, or underscores.
/// Filters out exact keyword matches.
pub fn arb_identifier_string() -> impl Strategy<Value = String> {
    // First character: letter or underscore
    let first_char = prop_oneof![
        prop::char::range('a', 'z'),
        prop::char::range('A', 'Z'),
        Just('_'),
    ];

    // Rest: letters, digits, or underscores (0-15 more chars)
    let rest_chars = prop::collection::vec(
        prop_oneof![
            prop::char::range('a', 'z'),
            prop::char::range('A', 'Z'),
            prop::char::range('0', '9'),
            Just('_'),
        ],
        0..16,
    );

    (first_char, rest_chars)
        .prop_map(|(first, rest)| {
            let mut s = String::with_capacity(1 + rest.len());
            s.push(first);
            for c in rest {
                s.extend(c.to_lowercase());
            }
            s
        })
        .prop_filter("identifier must not be a keyword", |s| {
            is_valid_moo_identifier(s)
        })
}

/// Generate a Variable with a named identifier.
pub fn arb_variable() -> impl Strategy<Value = Variable> {
    arb_identifier_string().prop_map(|name| Variable {
        id: 0,
        scope_id: 0,
        nr: VarName::Named(Symbol::mk(&name)),
    })
}

// =============================================================================
// Operator Generators
// =============================================================================

/// Generate an arbitrary binary operator.
pub fn arb_binary_op() -> impl Strategy<Value = BinaryOp> {
    prop_oneof![
        Just(BinaryOp::Add),
        Just(BinaryOp::Sub),
        Just(BinaryOp::Mul),
        Just(BinaryOp::Div),
        Just(BinaryOp::Mod),
        Just(BinaryOp::Exp),
        Just(BinaryOp::Eq),
        Just(BinaryOp::NEq),
        Just(BinaryOp::Lt),
        Just(BinaryOp::LtE),
        Just(BinaryOp::Gt),
        Just(BinaryOp::GtE),
        Just(BinaryOp::In),
        Just(BinaryOp::BitAnd),
        Just(BinaryOp::BitOr),
        Just(BinaryOp::BitXor),
        Just(BinaryOp::BitShl),
        Just(BinaryOp::BitShr),
        Just(BinaryOp::BitLShr),
    ]
}

/// Generate an arbitrary unary operator.
pub fn arb_unary_op() -> impl Strategy<Value = UnaryOp> {
    prop_oneof![Just(UnaryOp::Neg), Just(UnaryOp::Not), Just(UnaryOp::BitNot),]
}

// =============================================================================
// Collection Generators
// =============================================================================

/// Generate an Arg (Normal or Splice) wrapping an expression.
/// For simplicity, we only generate Normal args (no splices).
pub fn arb_arg<S: Strategy<Value = Expr> + 'static>(expr_strategy: S) -> impl Strategy<Value = Arg> {
    expr_strategy.prop_map(Arg::Normal)
}

/// Generate a list literal: {expr1, expr2, ...}
pub fn arb_list<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_len: usize,
) -> impl Strategy<Value = Expr> {
    prop::collection::vec(arb_arg(expr_strategy), 0..=max_len).prop_map(Expr::List)
}

/// Generate a map literal: [key1 -> val1, key2 -> val2, ...]
pub fn arb_map<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_len: usize,
) -> impl Strategy<Value = Expr> {
    prop::collection::vec((expr_strategy.clone(), expr_strategy), 0..=max_len).prop_map(Expr::Map)
}

// =============================================================================
// Expression Generators (Layered)
// =============================================================================

/// Layer 1: Literals, binary ops, unary ops.
///
/// The `depth` parameter controls nesting depth to prevent explosion:
/// - depth=0: only literals
/// - depth>0: literals, binary ops (with depth-1 sub-expressions), or unary ops
pub fn arb_expr_layer1(depth: usize) -> impl Strategy<Value = Expr> {
    if depth == 0 {
        arb_literal().boxed()
    } else {
        prop_oneof![
            // 60% literals
            6 => arb_literal(),
            // 30% binary ops with sub-expressions
            3 => (arb_binary_op(), arb_expr_layer1(depth - 1), arb_expr_layer1(depth - 1))
                .prop_map(|(op, left, right)| {
                    Expr::Binary(op, Box::new(left), Box::new(right))
                }),
            // 10% unary ops with sub-expression
            1 => (arb_unary_op(), arb_expr_layer1(depth - 1))
                .prop_map(|(op, expr)| {
                    Expr::Unary(op, Box::new(expr))
                }),
        ]
        .boxed()
    }
}

/// Layer 2: Layer 1 + identifiers, lists, and maps.
///
/// The `depth` parameter controls nesting depth to prevent explosion:
/// - depth=0: only literals and identifiers
/// - depth>0: all of layer 1 plus lists and maps with sub-expressions
pub fn arb_expr_layer2(depth: usize) -> BoxedStrategy<Expr> {
    if depth == 0 {
        // At depth 0: literals and identifiers only
        prop_oneof![8 => arb_literal(), 2 => arb_variable().prop_map(Expr::Id),]
            .boxed()
    } else {
        // Create boxed strategies that can be cloned
        let elem_strategy = arb_expr_layer2(depth - 1);

        prop_oneof![
            // 40% literals
            4 => arb_literal(),
            // 10% identifiers
            1 => arb_variable().prop_map(Expr::Id),
            // 20% binary ops
            2 => (arb_binary_op(), arb_expr_layer2(depth - 1), arb_expr_layer2(depth - 1))
                .prop_map(|(op, left, right)| {
                    Expr::Binary(op, Box::new(left), Box::new(right))
                }),
            // 10% unary ops
            1 => (arb_unary_op(), arb_expr_layer2(depth - 1))
                .prop_map(|(op, expr)| {
                    Expr::Unary(op, Box::new(expr))
                }),
            // 10% lists (0-4 elements)
            1 => arb_list(elem_strategy.clone(), 4),
            // 10% maps (0-3 entries)
            1 => arb_map(elem_strategy, 3),
        ]
        .boxed()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;

    #[test]
    fn test_literal_generates() {
        let mut runner = TestRunner::default();
        let strategy = arb_literal();
        for _ in 0..50 {
            let _ = strategy.new_tree(&mut runner).unwrap().current();
        }
    }

    #[test]
    fn test_identifier_string_valid() {
        let mut runner = TestRunner::default();
        let strategy = arb_identifier_string();
        for _ in 0..100 {
            let ident = strategy.new_tree(&mut runner).unwrap().current();
            let first = ident.chars().next().unwrap();
            assert!(
                first.is_ascii_alphabetic() || first == '_',
                "Invalid first char: {}",
                first
            );
            for c in ident.chars() {
                assert!(
                    c.is_ascii_alphanumeric() || c == '_',
                    "Invalid char: {}",
                    c
                );
            }
        }
    }

    #[test]
    fn test_expr_layer1_depth0_only_literals() {
        let mut runner = TestRunner::default();
        let strategy = arb_expr_layer1(0);
        for _ in 0..20 {
            let expr = strategy.new_tree(&mut runner).unwrap().current();
            assert!(
                matches!(expr, Expr::Value(_) | Expr::Error(_, _)),
                "Expected literal at depth 0, got: {:?}",
                expr
            );
        }
    }

    #[test]
    fn test_expr_layer2_generates_identifiers() {
        let mut runner = TestRunner::default();
        let strategy = arb_expr_layer2(0);
        let mut found_id = false;
        for _ in 0..100 {
            let expr = strategy.new_tree(&mut runner).unwrap().current();
            if matches!(expr, Expr::Id(_)) {
                found_id = true;
                break;
            }
        }
        assert!(found_id, "Layer 2 at depth 0 should generate identifiers");
    }

    #[test]
    fn test_expr_layer2_generates_lists_and_maps() {
        let mut runner = TestRunner::default();
        let strategy = arb_expr_layer2(2);
        let mut found_list = false;
        let mut found_map = false;
        for _ in 0..200 {
            let expr = strategy.new_tree(&mut runner).unwrap().current();
            if matches!(expr, Expr::List(_)) {
                found_list = true;
            }
            if matches!(expr, Expr::Map(_)) {
                found_map = true;
            }
            if found_list && found_map {
                break;
            }
        }
        assert!(found_list, "Layer 2 should generate lists");
        assert!(found_map, "Layer 2 should generate maps");
    }
}
