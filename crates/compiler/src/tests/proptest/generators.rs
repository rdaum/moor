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

use crate::ast::{
    Arg, BinaryOp, CallTarget, CatchCodes, CondArm, ElseArm, ExceptArm, Expr, ScatterItem,
    ScatterKind, Stmt, StmtNode, UnaryOp,
};
use moor_var::program::names::{VarName, Variable};
use moor_var::{ErrorCode, Obj, Symbol, Var, VarType};
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

/// Generate an arbitrary error code (for use in except clauses).
pub fn arb_error_code() -> impl Strategy<Value = ErrorCode> {
    prop_oneof![
        Just(ErrorCode::E_NONE),
        Just(ErrorCode::E_TYPE),
        Just(ErrorCode::E_DIV),
        Just(ErrorCode::E_PERM),
        Just(ErrorCode::E_PROPNF),
        Just(ErrorCode::E_VERBNF),
        Just(ErrorCode::E_VARNF),
        Just(ErrorCode::E_INVIND),
        Just(ErrorCode::E_RECMOVE),
        Just(ErrorCode::E_MAXREC),
        Just(ErrorCode::E_RANGE),
        Just(ErrorCode::E_ARGS),
        Just(ErrorCode::E_NACC),
        Just(ErrorCode::E_INVARG),
        Just(ErrorCode::E_QUOTA),
        Just(ErrorCode::E_FLOAT),
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
    // Must not be a keyword, must not end with underscore (parser requires more chars after _),
    // and must not start with e_ (error code prefix which causes parsing ambiguity)
    !MOO_KEYWORDS.contains(&lower.as_str()) && !s.ends_with('_') && !lower.starts_with("e_")
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
// Indexing Generators
// =============================================================================

/// Generate a single index expression: base[index]
pub fn arb_index<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Expr> {
    (expr_strategy.clone(), expr_strategy).prop_map(|(base, index)| {
        Expr::Index(Box::new(base), Box::new(index))
    })
}

/// Generate a range index expression: base[from..to]
pub fn arb_range<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Expr> {
    (expr_strategy.clone(), expr_strategy.clone(), expr_strategy).prop_map(|(base, from, to)| {
        Expr::Range {
            base: Box::new(base),
            from: Box::new(from),
            to: Box::new(to),
        }
    })
}

// =============================================================================
// Conditional Generators
// =============================================================================

/// Generate a conditional (ternary) expression: condition ? consequence | alternative
pub fn arb_cond<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Expr> {
    (
        expr_strategy.clone(),
        expr_strategy.clone(),
        expr_strategy,
    )
        .prop_map(|(condition, consequence, alternative)| Expr::Cond {
            condition: Box::new(condition),
            consequence: Box::new(consequence),
            alternative: Box::new(alternative),
        })
}

/// Generate an And expression: left && right
pub fn arb_and<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Expr> {
    (expr_strategy.clone(), expr_strategy)
        .prop_map(|(left, right)| Expr::And(Box::new(left), Box::new(right)))
}

/// Generate an Or expression: left || right
pub fn arb_or<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Expr> {
    (expr_strategy.clone(), expr_strategy)
        .prop_map(|(left, right)| Expr::Or(Box::new(left), Box::new(right)))
}

/// Generate a property access expression: location.property
/// Location must be something that could have properties (objects, variables) - not numbers.
/// We use object literals and variables as locations since they're semantically valid.
pub fn arb_prop<S: Strategy<Value = Expr> + Clone + 'static>(
    _expr_strategy: S,
) -> impl Strategy<Value = Expr> {
    // Location should be object-like: object refs or variables
    let location_strategy = prop_oneof![
        arb_objref(),
        arb_variable().prop_map(Expr::Id),
    ];
    (location_strategy, arb_identifier_string()).prop_map(|(location, prop_name)| Expr::Prop {
        location: Box::new(location),
        property: Box::new(Expr::Value(Var::mk_str(&prop_name))),
    })
}

/// Generate a verb call expression: location:verb(args)
/// Location must be something that could have verbs (objects, variables) - not numbers.
/// We use object literals and variables as locations since they're semantically valid.
/// Verb names must be identifiers or string expressions (per LambdaMOO spec).
pub fn arb_verb<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_args: usize,
) -> impl Strategy<Value = Expr> {
    // Location should be object-like: object refs or variables
    let location_strategy = prop_oneof![
        arb_objref(),
        arb_variable().prop_map(Expr::Id),
    ];

    // Verb names can be: identifier strings or dynamic expressions (which must return strings)
    // Per LambdaMOO spec: obj:name() or obj:(expr)() where expr returns a string
    let verb_strategy = prop_oneof![
        // String verb name (most common): obj:foo() - stored as string literal
        arb_identifier_string().prop_map(|s| Expr::Value(Var::mk_str(&s))),
        // Dynamic verb name: obj:(expr)() - use a variable for simplicity
        arb_variable().prop_map(Expr::Id),
    ];

    (
        location_strategy,
        verb_strategy,
        proptest::collection::vec(expr_strategy.prop_map(Arg::Normal), 0..=max_args),
    )
        .prop_map(|(location, verb, args)| Expr::Verb {
            location: Box::new(location),
            verb: Box::new(verb),
            args,
        })
}

/// Generate a builtin function call: func(args)
pub fn arb_call<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_args: usize,
) -> impl Strategy<Value = Expr> {
    // Common builtin function names
    let builtin_names = prop_oneof![
        Just("length"),
        Just("typeof"),
        Just("tostr"),
        Just("toint"),
        Just("tofloat"),
        Just("min"),
        Just("max"),
        Just("abs"),
        Just("sqrt"),
        Just("random"),
        Just("time"),
        Just("ctime"),
        Just("encode_binary"),
        Just("decode_binary"),
        Just("listappend"),
        Just("listinsert"),
        Just("listdelete"),
        Just("listset"),
        Just("setadd"),
        Just("setremove"),
        Just("strcmp"),
        Just("strsub"),
        Just("index"),
        Just("rindex"),
    ];

    let args_strategy = proptest::collection::vec(arb_arg(expr_strategy), 0..=max_args);

    (builtin_names, args_strategy).prop_map(|(name, args)| Expr::Call {
        function: CallTarget::Builtin(Symbol::mk(name)),
        args,
    })
}

/// Generate a try-catch expression: `expr ! codes => fallback`
pub fn arb_try_catch<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Expr> {
    let codes_strategy = prop_oneof![
        // ANY
        Just(CatchCodes::Any),
        // Specific error codes
        proptest::collection::vec(
            prop_oneof![
                Just(ErrorCode::E_TYPE),
                Just(ErrorCode::E_DIV),
                Just(ErrorCode::E_PERM),
                Just(ErrorCode::E_PROPNF),
                Just(ErrorCode::E_VERBNF),
                Just(ErrorCode::E_VARNF),
                Just(ErrorCode::E_INVIND),
                Just(ErrorCode::E_RANGE),
                Just(ErrorCode::E_ARGS),
                Just(ErrorCode::E_NACC),
                Just(ErrorCode::E_INVARG),
            ],
            1..=3
        )
        .prop_map(|codes| {
            CatchCodes::Codes(codes.into_iter().map(|c| Arg::Normal(Expr::Error(c, None))).collect())
        }),
    ];

    prop_oneof![
        // Without fallback: `expr ! codes'
        (expr_strategy.clone(), codes_strategy.clone()).prop_map(|(trye, codes)| Expr::TryCatch {
            trye: Box::new(trye),
            codes,
            except: None,
        }),
        // With fallback: `expr ! codes => fallback'
        (expr_strategy.clone(), codes_strategy, expr_strategy).prop_map(
            |(trye, codes, fallback)| Expr::TryCatch {
                trye: Box::new(trye),
                codes,
                except: Some(Box::new(fallback)),
            }
        ),
    ]
}

/// Generate the length expression: $ (used in range contexts)
/// Note: $ only makes sense inside range expressions like `list[1..$]`.
/// It's not included in layer2_complete since it's a contextual expression.
#[allow(dead_code)]
pub fn arb_length() -> impl Strategy<Value = Expr> {
    Just(Expr::Length)
}

/// Generate a type constant: INT, STR, FLOAT, OBJ, LIST, MAP, ERR, BOOL
pub fn arb_type_constant() -> impl Strategy<Value = Expr> {
    prop_oneof![
        Just(Expr::TypeConstant(VarType::TYPE_INT)),
        Just(Expr::TypeConstant(VarType::TYPE_FLOAT)),
        Just(Expr::TypeConstant(VarType::TYPE_STR)),
        Just(Expr::TypeConstant(VarType::TYPE_OBJ)),
        Just(Expr::TypeConstant(VarType::TYPE_LIST)),
        Just(Expr::TypeConstant(VarType::TYPE_MAP)),
        Just(Expr::TypeConstant(VarType::TYPE_ERR)),
        Just(Expr::TypeConstant(VarType::TYPE_BOOL)),
    ]
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

/// Layer 2b: Layer 2 + indexing and conditionals.
///
/// Adds:
/// - Single indexing: expr[index]
/// - Range indexing: expr[from..to]
/// - Conditional (ternary): condition ? consequence | alternative
pub fn arb_expr_layer2b(depth: usize) -> BoxedStrategy<Expr> {
    if depth == 0 {
        // At depth 0: literals and identifiers only
        prop_oneof![8 => arb_literal(), 2 => arb_variable().prop_map(Expr::Id),]
            .boxed()
    } else {
        // Create boxed strategies that can be cloned
        let elem_strategy = arb_expr_layer2b(depth - 1);

        prop_oneof![
            // 30% literals
            3 => arb_literal(),
            // 10% identifiers
            1 => arb_variable().prop_map(Expr::Id),
            // 15% binary ops
            2 => (arb_binary_op(), arb_expr_layer2b(depth - 1), arb_expr_layer2b(depth - 1))
                .prop_map(|(op, left, right)| {
                    Expr::Binary(op, Box::new(left), Box::new(right))
                }),
            // 5% unary ops
            1 => (arb_unary_op(), arb_expr_layer2b(depth - 1))
                .prop_map(|(op, expr)| {
                    Expr::Unary(op, Box::new(expr))
                }),
            // 10% lists (0-4 elements)
            1 => arb_list(elem_strategy.clone(), 4),
            // 5% maps (0-3 entries)
            1 => arb_map(elem_strategy.clone(), 3),
            // 10% single index
            1 => arb_index(elem_strategy.clone()),
            // 5% range index
            1 => arb_range(elem_strategy.clone()),
            // 10% conditional
            1 => arb_cond(elem_strategy),
        ]
        .boxed()
    }
}

/// Layer 2 Complete: All expression types except statements and lambdas.
///
/// Adds to Layer 2b:
/// - And/Or logical operators
/// - Property access (obj.prop, obj.(expr))
/// - Verb calls (obj:verb(args))
/// - Builtin function calls (func(args))
/// - Try-catch expressions
/// - Type constants (INT, STR, etc.)
/// - Length ($)
pub fn arb_expr_layer2_complete(depth: usize) -> BoxedStrategy<Expr> {
    if depth == 0 {
        // At depth 0: literals, identifiers, and type constants
        prop_oneof![
            8 => arb_literal(),
            2 => arb_variable().prop_map(Expr::Id),
            1 => arb_type_constant(),
        ]
        .boxed()
    } else {
        let elem_strategy = arb_expr_layer2_complete(depth - 1);

        prop_oneof![
            // Base types (20%)
            2 => arb_literal(),
            1 => arb_variable().prop_map(Expr::Id),
            1 => arb_type_constant(),
            // Binary/unary ops (15%)
            1 => (arb_binary_op(), arb_expr_layer2_complete(depth - 1), arb_expr_layer2_complete(depth - 1))
                .prop_map(|(op, left, right)| {
                    Expr::Binary(op, Box::new(left), Box::new(right))
                }),
            1 => (arb_unary_op(), arb_expr_layer2_complete(depth - 1))
                .prop_map(|(op, expr)| {
                    Expr::Unary(op, Box::new(expr))
                }),
            // Logical ops (10%)
            1 => arb_and(elem_strategy.clone()),
            1 => arb_or(elem_strategy.clone()),
            // Collections (10%)
            1 => arb_list(elem_strategy.clone(), 3),
            1 => arb_map(elem_strategy.clone(), 2),
            // Indexing (10%)
            1 => arb_index(elem_strategy.clone()),
            1 => arb_range(elem_strategy.clone()),
            // Conditional (5%)
            1 => arb_cond(elem_strategy.clone()),
            // Property/verb access (10%)
            1 => arb_prop(elem_strategy.clone()),
            1 => arb_verb(elem_strategy.clone(), 3),
            // Function calls (5%)
            1 => arb_call(elem_strategy.clone(), 3),
            // Try-catch (5%)
            1 => arb_try_catch(elem_strategy),
        ]
        .boxed()
    }
}

// =============================================================================
// Scatter Assignment Generators
// =============================================================================

/// Generate a scatter assignment expression: {a, ?b = 1, @rest} = expr
/// Constraints:
/// - Must have at least one item
/// - At most one Rest item (and it's typically last)
/// - Required items come first, then Optional, then Rest
pub fn arb_scatter<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Expr> {
    // Generate 1-4 required items, 0-2 optional items, and optionally a rest item
    let required_items = proptest::collection::vec(
        arb_variable().prop_map(|id| ScatterItem {
            kind: ScatterKind::Required,
            id,
            expr: None,
        }),
        1..=3,
    );
    let optional_items = proptest::collection::vec(
        (arb_variable(), expr_strategy.clone()).prop_map(|(id, default_expr)| ScatterItem {
            kind: ScatterKind::Optional,
            id,
            expr: Some(default_expr),
        }),
        0..=2,
    );
    let rest_item = proptest::option::of(arb_variable().prop_map(|id| ScatterItem {
        kind: ScatterKind::Rest,
        id,
        expr: None,
    }));

    (required_items, optional_items, rest_item, expr_strategy).prop_map(
        |(mut items, opt_items, rest, rhs)| {
            items.extend(opt_items);
            if let Some(rest_item) = rest {
                items.push(rest_item);
            }
            Expr::Scatter(items, Box::new(rhs))
        },
    )
}

// =============================================================================
// Declaration Generators
// =============================================================================

/// Generate a let declaration expression: let x or let x = expr
pub fn arb_decl_let<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Expr> {
    (arb_variable(), proptest::option::of(expr_strategy)).prop_map(|(id, expr)| Expr::Decl {
        id,
        is_const: false,
        expr: expr.map(Box::new),
    })
}

/// Generate a const declaration expression: const x = expr
/// Note: const requires an initializer in MOO
pub fn arb_decl_const<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Expr> {
    (arb_variable(), expr_strategy).prop_map(|(id, init)| Expr::Decl {
        id,
        is_const: true,
        expr: Some(Box::new(init)),
    })
}

/// Generate a declaration statement: let x; or let x = expr; or const x = expr;
pub fn arb_stmt_decl<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Stmt> {
    prop_oneof![
        arb_decl_let(expr_strategy.clone()).prop_map(|decl| make_stmt(StmtNode::Expr(decl))),
        arb_decl_const(expr_strategy).prop_map(|decl| make_stmt(StmtNode::Expr(decl))),
    ]
}

/// Generate a scope with declarations: begin let x = ...; body end
pub fn arb_stmt_scope_with_decls<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_decls: usize,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let decl_strategy = proptest::collection::vec(arb_stmt_decl(expr_strategy.clone()), 1..=max_decls);
    let body_strategy = proptest::collection::vec(arb_stmt_expr(expr_strategy), 1..=max_body_len);

    (decl_strategy, body_strategy).prop_map(|(decls, body)| {
        let mut all_stmts = decls;
        all_stmts.extend(body);
        make_stmt(StmtNode::Scope {
            num_bindings: all_stmts.len(),
            body: all_stmts,
        })
    })
}

// =============================================================================
// Statement Generators
// =============================================================================

/// Create a Stmt wrapper with default line info.
fn make_stmt(node: StmtNode) -> Stmt {
    Stmt {
        node,
        line_col: (1, 1),
        tree_line_no: 0,
    }
}

/// Generate a simple return statement: return expr;
#[allow(dead_code)]
pub fn arb_stmt_return<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Stmt> {
    prop_oneof![
        // pass; (no value)
        Just(make_stmt(StmtNode::Expr(Expr::Pass { args: vec![] }))),
        // expr;
        expr_strategy.prop_map(|expr| make_stmt(StmtNode::Expr(expr))),
    ]
}

/// Generate an expression statement: expr;
pub fn arb_stmt_expr<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Stmt> {
    expr_strategy.prop_map(|expr| make_stmt(StmtNode::Expr(expr)))
}

/// Generate a simple if statement: if (cond) body endif
pub fn arb_stmt_if<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);

    (expr_strategy, body_strategy).prop_map(|(condition, statements)| {
        make_stmt(StmtNode::Cond {
            arms: vec![CondArm {
                condition,
                statements,
                environment_width: 0,
            }],
            otherwise: None,
        })
    })
}

/// Generate an if-else statement: if (cond) body else body endif
pub fn arb_stmt_if_else<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);
    let else_body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);

    (expr_strategy, body_strategy, else_body_strategy).prop_map(
        |(condition, statements, else_statements)| {
            make_stmt(StmtNode::Cond {
                arms: vec![CondArm {
                    condition,
                    statements,
                    environment_width: 0,
                }],
                otherwise: Some(ElseArm {
                    statements: else_statements,
                    environment_width: 0,
                }),
            })
        },
    )
}

/// Generate a for-list statement: for x in (expr) body endfor
pub fn arb_stmt_for_list<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);

    (arb_variable(), expr_strategy, body_strategy).prop_map(|(var, list_expr, body)| {
        make_stmt(StmtNode::ForList {
            value_binding: var,
            key_binding: None,
            expr: list_expr,
            body,
            environment_width: 0,
        })
    })
}

/// Generate a for-list statement with key binding: for x, i in (expr) body endfor
pub fn arb_stmt_for_list_keyed<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);

    (arb_variable(), arb_variable(), expr_strategy, body_strategy).prop_map(
        |(var, key_var, list_expr, body)| {
            make_stmt(StmtNode::ForList {
                value_binding: var,
                key_binding: Some(key_var),
                expr: list_expr,
                body,
                environment_width: 0,
            })
        },
    )
}

/// Generate a for-range statement: for x in [from..to] body endfor
/// Note: $ (Length) is only valid in indexing contexts like list[1..$],
/// not in for-range statements.
pub fn arb_stmt_for_range<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);

    (
        arb_variable(),
        expr_strategy.clone(),
        expr_strategy,
        body_strategy,
    )
        .prop_map(|(var, from, to, body)| {
            make_stmt(StmtNode::ForRange {
                id: var,
                from,
                to,
                body,
                environment_width: 0,
            })
        })
}

/// Generate a while statement: while (cond) body endwhile
pub fn arb_stmt_while<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);

    (expr_strategy, body_strategy).prop_map(|(condition, body)| {
        make_stmt(StmtNode::While {
            id: None,
            condition,
            body,
            environment_width: 0,
        })
    })
}

/// Generate a labeled while statement: while name (cond) body endwhile
pub fn arb_stmt_while_labeled<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);

    (arb_variable(), expr_strategy, body_strategy).prop_map(|(label, condition, body)| {
        make_stmt(StmtNode::While {
            id: Some(label),
            condition,
            body,
            environment_width: 0,
        })
    })
}

/// Generate a fork statement: fork (time) body endfork
pub fn arb_stmt_fork<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);

    (expr_strategy, body_strategy).prop_map(|(time, body)| {
        make_stmt(StmtNode::Fork {
            id: None,
            time,
            body,
        })
    })
}

/// Generate a labeled fork statement: fork name (time) body endfork
pub fn arb_stmt_fork_labeled<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);

    (arb_variable(), expr_strategy, body_strategy).prop_map(|(label, time, body)| {
        make_stmt(StmtNode::Fork {
            id: Some(label),
            time,
            body,
        })
    })
}

/// Generate a try-except statement: try body except handler endtry
pub fn arb_stmt_try_except<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);

    // Generate error code arguments as Arg::Normal(Expr::Error(...))
    let error_args_strategy = proptest::collection::vec(
        arb_error_code().prop_map(|code| Arg::Normal(Expr::Error(code, None))),
        1..=3,
    );

    // Generate 1-2 except arms with different catch codes
    let except_arm_strategy = (
        proptest::option::of(arb_variable()),
        prop_oneof![
            Just(CatchCodes::Any),
            error_args_strategy.prop_map(CatchCodes::Codes),
        ],
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len),
    )
        .prop_map(|(id, codes, statements)| ExceptArm {
            id,
            codes,
            statements,
        });

    let excepts_strategy = proptest::collection::vec(except_arm_strategy, 1..=2);

    (body_strategy, excepts_strategy).prop_map(|(body, excepts)| {
        make_stmt(StmtNode::TryExcept {
            body,
            excepts,
            environment_width: 0,
        })
    })
}

/// Generate a try-finally statement: try body finally handler endtry
pub fn arb_stmt_try_finally<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len);
    let handler_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy), 1..=max_body_len);

    (body_strategy, handler_strategy).prop_map(|(body, handler)| {
        make_stmt(StmtNode::TryFinally {
            body,
            handler,
            environment_width: 0,
        })
    })
}

/// Generate a break statement: break;
pub fn arb_stmt_break() -> impl Strategy<Value = Stmt> {
    Just(make_stmt(StmtNode::Break { exit: None }))
}

/// Generate a continue statement: continue;
pub fn arb_stmt_continue() -> impl Strategy<Value = Stmt> {
    Just(make_stmt(StmtNode::Continue { exit: None }))
}

/// Generate a labeled while loop with a matching labeled break inside.
/// This ensures semantic correctness - break labels must reference an enclosing loop.
pub fn arb_stmt_while_with_labeled_break<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Stmt> {
    (arb_variable(), expr_strategy).prop_map(|(label, cond)| {
        make_stmt(StmtNode::While {
            id: Some(label),
            condition: cond,
            body: vec![make_stmt(StmtNode::Break {
                exit: Some(label),
            })],
            environment_width: 0,
        })
    })
}

/// Generate a labeled while loop with a matching labeled continue inside.
/// This ensures semantic correctness - continue labels must reference an enclosing loop.
pub fn arb_stmt_while_with_labeled_continue<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
) -> impl Strategy<Value = Stmt> {
    (arb_variable(), expr_strategy).prop_map(|(label, cond)| {
        make_stmt(StmtNode::While {
            id: Some(label),
            condition: cond,
            body: vec![make_stmt(StmtNode::Continue {
                exit: Some(label),
            })],
            environment_width: 0,
        })
    })
}

/// Generate an if-elseif-else statement with multiple arms
pub fn arb_stmt_if_elseif<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
    max_elseif_arms: usize,
) -> impl Strategy<Value = Stmt> {
    // Generate 1-N condition arms (first is "if", rest are "elseif")
    // Each arm has its own condition and body
    let arm_strategy = (
        expr_strategy.clone(),
        proptest::collection::vec(arb_stmt_expr(expr_strategy.clone()), 1..=max_body_len),
    )
        .prop_map(|(condition, statements)| CondArm {
            condition,
            statements,
            environment_width: 0,
        });

    let arms_strategy = proptest::collection::vec(arm_strategy, 1..=max_elseif_arms);

    // Optional else arm
    let else_strategy = proptest::option::of(
        proptest::collection::vec(arb_stmt_expr(expr_strategy), 1..=max_body_len).prop_map(
            |statements| ElseArm {
                statements,
                environment_width: 0,
            },
        ),
    );

    (arms_strategy, else_strategy).prop_map(|(arms, otherwise)| {
        make_stmt(StmtNode::Cond { arms, otherwise })
    })
}

/// Generate a scope/begin statement: begin body end
/// This is a moor extension for lexical scoping.
pub fn arb_stmt_scope<S: Strategy<Value = Expr> + Clone + 'static>(
    expr_strategy: S,
    max_body_len: usize,
) -> impl Strategy<Value = Stmt> {
    let body_strategy =
        proptest::collection::vec(arb_stmt_expr(expr_strategy), 1..=max_body_len);

    body_strategy.prop_map(|body| {
        make_stmt(StmtNode::Scope {
            num_bindings: 0,
            body,
        })
    })
}

/// Layer 3: Statements layer - simple statements and scatter assignments.
///
/// Generates:
/// - Expression statements
/// - Scatter assignments
pub fn arb_stmt_layer3(depth: usize) -> BoxedStrategy<Stmt> {
    let expr_strategy = arb_expr_layer2_complete(depth);

    prop_oneof![
        // Expression statement (most common)
        3 => arb_stmt_expr(expr_strategy.clone()),
        // Scatter assignment
        1 => arb_scatter(expr_strategy).prop_map(|scatter| make_stmt(StmtNode::Expr(scatter))),
    ]
    .boxed()
}

/// Layer 4: Control flow statements.
///
/// Generates:
/// - All of Layer 3
/// - If statements (simple and with else)
/// - For loops (list and range)
/// - While loops
pub fn arb_stmt_layer4(depth: usize) -> BoxedStrategy<Stmt> {
    let expr_strategy = arb_expr_layer2_complete(depth);

    prop_oneof![
        // Expression statement
        3 => arb_stmt_expr(expr_strategy.clone()),
        // Scatter assignment
        1 => arb_scatter(expr_strategy.clone()).prop_map(|scatter| make_stmt(StmtNode::Expr(scatter))),
        // If statement (simple)
        1 => arb_stmt_if(expr_strategy.clone(), 2),
        // If-else statement
        1 => arb_stmt_if_else(expr_strategy.clone(), 2),
        // For-list loop
        1 => arb_stmt_for_list(expr_strategy.clone(), 2),
        // For-range loop
        1 => arb_stmt_for_range(expr_strategy.clone(), 2),
        // While loop
        1 => arb_stmt_while(expr_strategy, 2),
    ]
    .boxed()
}

/// Layer 5: All statement types.
///
/// Generates:
/// - All of Layer 4
/// - Fork statements (with and without labels)
/// - Try-except statements
/// - Try-finally statements
/// - Break/continue statements (with and without labels)
/// - Labeled while loops
/// - For-list with key binding
/// - If-elseif-else statements
/// - Scope/begin blocks (with and without declarations)
/// - Declaration statements (let/const)
pub fn arb_stmt_layer5(depth: usize) -> BoxedStrategy<Stmt> {
    let expr_strategy = arb_expr_layer2_complete(depth);

    prop_oneof![
        // Expression statement
        3 => arb_stmt_expr(expr_strategy.clone()),
        // Scatter assignment
        1 => arb_scatter(expr_strategy.clone()).prop_map(|scatter| make_stmt(StmtNode::Expr(scatter))),
        // If statement (simple)
        1 => arb_stmt_if(expr_strategy.clone(), 2),
        // If-else statement
        1 => arb_stmt_if_else(expr_strategy.clone(), 2),
        // If-elseif-else statement
        1 => arb_stmt_if_elseif(expr_strategy.clone(), 2, 3),
        // For-list loop
        1 => arb_stmt_for_list(expr_strategy.clone(), 2),
        // For-list loop with key binding
        1 => arb_stmt_for_list_keyed(expr_strategy.clone(), 2),
        // For-range loop
        1 => arb_stmt_for_range(expr_strategy.clone(), 2),
        // While loop
        1 => arb_stmt_while(expr_strategy.clone(), 2),
        // Labeled while loop
        1 => arb_stmt_while_labeled(expr_strategy.clone(), 2),
        // Fork statement
        1 => arb_stmt_fork(expr_strategy.clone(), 2),
        // Labeled fork statement
        1 => arb_stmt_fork_labeled(expr_strategy.clone(), 2),
        // Try-except statement
        1 => arb_stmt_try_except(expr_strategy.clone(), 2),
        // Try-finally statement
        1 => arb_stmt_try_finally(expr_strategy.clone(), 2),
        // Scope/begin block (simple)
        1 => arb_stmt_scope(expr_strategy.clone(), 2),
        // Scope/begin block with declarations
        1 => arb_stmt_scope_with_decls(expr_strategy.clone(), 2, 2),
        // Declaration statement (let/const)
        1 => arb_stmt_decl(expr_strategy.clone()),
        // Break statement
        1 => arb_stmt_break(),
        // Continue statement
        1 => arb_stmt_continue(),
        // Labeled while loop with labeled break
        1 => arb_stmt_while_with_labeled_break(expr_strategy.clone()),
        // Labeled while loop with labeled continue
        1 => arb_stmt_while_with_labeled_continue(expr_strategy),
    ]
    .boxed()
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

    #[test]
    fn test_expr_layer2b_generates_index_and_cond() {
        let mut runner = TestRunner::default();
        let strategy = arb_expr_layer2b(2);
        let mut found_index = false;
        let mut found_range = false;
        let mut found_cond = false;
        for _ in 0..300 {
            let expr = strategy.new_tree(&mut runner).unwrap().current();
            if matches!(expr, Expr::Index(_, _)) {
                found_index = true;
            }
            if matches!(expr, Expr::Range { .. }) {
                found_range = true;
            }
            if matches!(expr, Expr::Cond { .. }) {
                found_cond = true;
            }
            if found_index && found_range && found_cond {
                break;
            }
        }
        assert!(found_index, "Layer 2b should generate index expressions");
        assert!(found_range, "Layer 2b should generate range expressions");
        assert!(found_cond, "Layer 2b should generate conditional expressions");
    }
}
