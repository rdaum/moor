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

/// Kicks off the Pest parser and converts it into our AST.
/// This is the main entry point for parsing.
use std::cell::{Cell, RefCell};
use std::{rc::Rc, str::FromStr};

use moor_var::{ErrorCode, SYSTEM_OBJECT, Symbol, Var, VarType, v_none};
pub use pest::Parser as PestParser;
use pest::{
    error::LineColLocation,
    iterators::{Pair, Pairs},
    pratt_parser::{Assoc, Op, PrattParser},
    set_error_detail,
};
use std::sync::Once;

use moor_common::builtins::BUILTINS;
use moor_var::{AnonymousObjid, Obj, UuObjid, v_binary, v_float, v_int, v_obj, v_str, v_string};

use crate::{
    ast::{
        Arg,
        Arg::{Normal, Splice},
        BinaryOp, CallTarget, CatchCodes, CondArm, ElseArm, ExceptArm, Expr, ScatterItem,
        ScatterKind, Stmt, StmtNode,
        StmtNode::Scope,
        UnaryOp,
    },
    diagnostics::build_parse_error_details,
    parse::moo::{MooParser, Rule},
    unparse::annotate_line_numbers,
    var_scope::VarScope,
};
use base64::{Engine, engine::general_purpose};
use moor_common::model::{
    CompileContext, CompileError,
    CompileError::{DuplicateVariable, UnknownTypeConstant},
    ParseErrorDetails,
};
use moor_var::program::{DeclType, names::Names};

/// Regular moo grammar
pub mod moo {
    #[derive(Parser)]
    #[grammar = "src/moo.pest"]
    pub struct MooParser;
}

/// Grammar when wrapped up in an objdef file
pub mod objdef {
    #[derive(Parser)]
    #[grammar = "src/moo.pest"]
    #[grammar = "src/objdef.pest"]
    pub struct ObjDefParser;
}

/// Macro to define the Pratt parser configuration from a single source of truth.
///
/// PRECEDENCE ORDER: Earlier levels = lower precedence, later levels = higher precedence
/// (Assignment is lowest, Atomic is highest)
///
/// TO ADD NEW OPERATORS:
/// - Add to existing level: insert Rule name in appropriate infix/prefix/postfix group
/// - Create new level: add new precedence level in correct position in macro call
///
/// SYNTAX:
/// ```text
/// LevelName => [
///   infix(left|right): [rule1, rule2],    // Binary operators
///   prefix(left): [rule3],                // Unary prefix operators
///   postfix(left): [rule4],               // Unary postfix operators
/// ]
/// ```
macro_rules! define_operators {
    ($(
        $level:ident => [
            $($binding:ident($assoc:ident): [$($op:ident),* $(,)?]),* $(,)?
        ]
    ),* $(,)?) => {
        // Creates a Pratt parser definition based on the define_operators! structure.
        fn build_pratt_parser() -> PrattParser<Rule> {
            PrattParser::new()
            $(
                $(
                    .op(define_operators!(@ops_group $binding($assoc): [$($op),*]))
                )*
            )*
        }
    };

    // Generate union of ops for a group
    (@ops_group $binding:ident($assoc:ident): []) => {
        Op::infix(Rule::dummy, Assoc::Left) // Empty fallback - should not be used
    };
    (@ops_group $binding:ident($assoc:ident): [$op:ident]) => {
        define_operators!(@make_op $binding, $assoc, $op)
    };
    (@ops_group $binding:ident($assoc:ident): [$op:ident, $($rest:ident),+]) => {
        define_operators!(@make_op $binding, $assoc, $op) |
        define_operators!(@ops_group $binding($assoc): [$($rest),+])
    };

    // Create the appropriate Op based on binding type
    (@make_op infix, $assoc:ident, $op:ident) => {
        Op::infix(Rule::$op, define_operators!(@assoc $assoc))
    };
    (@make_op prefix, $assoc:ident, $op:ident) => {
        Op::prefix(Rule::$op)
    };
    (@make_op postfix, $assoc:ident, $op:ident) => {
        Op::postfix(Rule::$op)
    };

    // Convert associativity
    (@assoc left) => { Assoc::Left };
    (@assoc right) => { Assoc::Right };
}

// This macro call creates the enum for PrecedentLevel, and also creates build_pratt_parser().
define_operators! {

    Assignment => [
        prefix(left): [scatter_assign],
        postfix(left): [assign],
    ],
    Conditional => [
        postfix(left): [cond_expr],
    ],
    Logical => [
        infix(left): [lor, land],
    ],
    BitwiseOr => [
        infix(left): [bitor],
    ],
    BitwiseXor => [
        infix(left): [bitxor],
    ],
    BitwiseAnd => [
        infix(left): [bitand],
    ],
    Comparison => [
        infix(left): [eq, neq, gt, lt, gte, lte, in_range],
    ],
    BitwiseShift => [
        infix(left): [bitshl, bitlshr, bitshr],
    ],
    Arithmetic => [
        infix(left): [add, sub],
    ],
    Multiplicative => [
        infix(left): [mul, div, modulus],
    ],
    Exponent => [
        infix(right): [pow],
    ],
    Unary => [
        prefix(left): [neg, not, bitnot],
    ],
    Postfix => [
        postfix(left): [index_range, index_single, verb_call, verb_expr_call, prop, prop_expr, call],
    ],
    Atomic => [], // DEFAULT: Everything else
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompileOptions {
    /// Whether we allow lexical scope blocks. begin/end blocks and 'let' and 'global' statements
    pub lexical_scopes: bool,
    /// Whether to support the flyweight type (a delegate object with slots and contents)
    pub flyweight_type: bool, // TODO: future options:
    //      - symbol types
    //      - disable "#" style object references (obscure_references)
    /// Whether to support list and range comprehensions in the compiler
    pub list_comprehensions: bool,
    /// Whether to support boolean types in compilation
    pub bool_type: bool,
    /// Whether to support symbol types ('sym) in compilation
    pub symbol_type: bool,
    /// Whether to support non-standard custom error values.
    pub custom_errors: bool,
    /// Whether to turn unsupported builtins into `call_function` invocations.
    /// Useful for textdump imports from other MOO dialects.
    pub call_unsupported_builtins: bool,
    /// Whether to parse legacy type constant names (INT, OBJ, STR, etc.) as type literals.
    /// When false (default), these become valid variable identifiers.
    /// When true (textdump import mode), these are parsed as type literals.
    /// Note: The new TYPE_* forms (TYPE_INT, TYPE_OBJ, etc.) are always recognized.
    pub legacy_type_constants: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            lexical_scopes: true,
            flyweight_type: true,
            list_comprehensions: true,
            bool_type: true,
            symbol_type: true,
            custom_errors: true,
            call_unsupported_builtins: false,
            legacy_type_constants: false,
        }
    }
}

pub struct TreeTransformer {
    // TODO: this is RefCell because PrattParser has some API restrictions which result in
    //   borrowing issues, see: https://github.com/pest-parser/pest/discussions/1030
    names: RefCell<VarScope>,
    options: CompileOptions,
    lambda_body_depth: Cell<usize>,
    dollars_ok: Cell<usize>,
}

impl TreeTransformer {
    pub fn new(options: CompileOptions) -> Rc<Self> {
        Rc::new(Self {
            names: RefCell::new(VarScope::new()),
            options,
            lambda_body_depth: Cell::new(0),
            dollars_ok: Cell::new(0),
        })
    }

    fn enter_lambda_body(&self) {
        self.lambda_body_depth.set(self.lambda_body_depth.get() + 1);
    }

    fn exit_lambda_body(&self) {
        self.lambda_body_depth.set(self.lambda_body_depth.get() - 1);
    }

    fn in_lambda_body(&self) -> bool {
        self.lambda_body_depth.get() > 0
    }

    fn enter_dollars_ok(&self) {
        self.dollars_ok.set(self.dollars_ok.get() + 1);
    }

    fn exit_dollars_ok(&self) {
        self.dollars_ok.set(self.dollars_ok.get() - 1);
    }

    fn dollars_allowed(&self) -> bool {
        self.dollars_ok.get() > 0
    }

    fn compile_context(&self, pair: &Pair<Rule>) -> CompileContext {
        CompileContext::new(pair.line_col())
    }

    fn parse_atom(self: Rc<Self>, pair: Pair<Rule>) -> Result<Expr, CompileError> {
        match pair.as_rule() {
            Rule::ident => {
                let ident_str = pair.as_str().trim();

                // In legacy mode, check if this identifier is actually a legacy type constant
                if self.options.legacy_type_constants
                    && let Some(type_id) = VarType::parse_legacy(ident_str)
                {
                    return Ok(Expr::TypeConstant(type_id));
                }

                let name = if self.in_lambda_body() {
                    self.names
                        .borrow_mut()
                        .find_or_add_name_scoped(ident_str, DeclType::Unknown)
                        .unwrap()
                } else {
                    self.names
                        .borrow_mut()
                        .find_or_add_name_global(ident_str, DeclType::Unknown)
                        .unwrap()
                };
                Ok(Expr::Id(name))
            }
            Rule::type_constant => {
                let type_str = pair.as_str();
                let Some(type_id) = VarType::parse(type_str) else {
                    return Err(UnknownTypeConstant(
                        self.compile_context(&pair),
                        type_str.into(),
                    ));
                };

                Ok(Expr::TypeConstant(type_id))
            }
            Rule::object => {
                let ostr = &pair.as_str()[1..];
                if ostr.starts_with("anon_")
                    && ostr.len() == 22
                    && ostr.chars().nth(11) == Some('-')
                {
                    // This is an anonymous object: anon_FFFFFF-FFFFFFFFFF
                    let uuid_part = &ostr[5..]; // Skip "anon_" prefix
                    if let Some((first, second)) = uuid_part.split_once('-') {
                        let first_group = u64::from_str_radix(first, 16).unwrap();
                        let epoch_ms = u64::from_str_radix(second, 16).unwrap();
                        let autoincrement = ((first_group >> 6) & 0xFFFF) as u16;
                        let rng = (first_group & 0x3F) as u8;
                        let anonymous_id = AnonymousObjid::new(autoincrement, rng, epoch_ms);
                        let objid = Obj::mk_anonymous(anonymous_id);
                        Ok(Expr::Value(v_obj(objid)))
                    } else {
                        Err(CompileError::StringLexError(
                            self.compile_context(&pair),
                            format!("invalid anonymous object literal '{}'", pair.as_str()),
                        ))
                    }
                } else if ostr.len() == 17 && ostr.chars().nth(6) == Some('-') {
                    // This is an uuobjid probably, so we can safely assemble from there.
                    let uuobjid = UuObjid::from_uuid_string(ostr).unwrap();
                    let objid = Obj::mk_uuobjid(uuobjid);
                    Ok(Expr::Value(v_obj(objid)))
                } else {
                    // Parse as OID - must fit in i32 range
                    match i32::from_str(ostr) {
                        Ok(oid) => {
                            let objid = Obj::mk_id(oid);
                            Ok(Expr::Value(v_obj(objid)))
                        }
                        Err(e) => Err(CompileError::StringLexError(
                            self.compile_context(&pair),
                            format!("invalid object ID '{}': {}", ostr, e),
                        )),
                    }
                }
            }
            Rule::integer => match pair.as_str().parse::<i64>() {
                Ok(int) => Ok(Expr::Value(v_int(int))),
                Err(e) => Err(CompileError::StringLexError(
                    self.compile_context(&pair),
                    format!("invalid integer literal '{}': {e}", pair.as_str()),
                )),
            },
            Rule::boolean => {
                if !self.options.bool_type {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "Booleans".to_string(),
                    ));
                }
                let b = pair.as_str().trim() == "true";
                Ok(Expr::Value(Var::mk_bool(b)))
            }
            Rule::symbol => {
                if !self.options.symbol_type {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "Symbols".to_string(),
                    ));
                }
                let s = Symbol::mk(&pair.as_str()[1..]);
                Ok(Expr::Value(Var::mk_symbol(s)))
            }
            Rule::float => {
                let float = pair.as_str().parse::<f64>().unwrap();
                Ok(Expr::Value(v_float(float)))
            }
            Rule::string => {
                let string = pair.as_str();
                let parsed = match moor_common::util::unquote_str(string) {
                    Ok(str) => str,
                    Err(e) => {
                        return Err(CompileError::StringLexError(
                            self.compile_context(&pair),
                            format!("invalid string literal '{string}': {e}"),
                        ));
                    }
                };
                Ok(Expr::Value(v_str(&parsed)))
            }
            Rule::literal_binary => {
                let binary_literal = pair.as_str();
                // Remove b" and " from the literal to get just the base64 content
                let base64_content = binary_literal
                    .strip_prefix("b\"")
                    .and_then(|s| s.strip_suffix("\""))
                    .ok_or_else(|| {
                        CompileError::StringLexError(
                            self.compile_context(&pair),
                            format!(
                                "invalid binary literal '{binary_literal}': missing b\" prefix or \" suffix"
                            ),
                        )
                    })?;

                // Decode the base64 content
                let decoded = match general_purpose::URL_SAFE.decode(base64_content) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        return Err(CompileError::StringLexError(
                            self.compile_context(&pair),
                            format!(
                                "invalid binary literal '{binary_literal}': invalid base64: {e}"
                            ),
                        ));
                    }
                };

                Ok(Expr::Value(v_binary(decoded)))
            }
            Rule::err => {
                let mut inner = pair.into_inner();
                let pair = inner.next().unwrap();
                let e = pair.as_str();
                let Some(e) = ErrorCode::parse_str(e) else {
                    panic!("invalid error value: {e}");
                };
                if let ErrorCode::ErrCustom(_) = &e
                    && !self.options.custom_errors
                {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "CustomErrors".to_string(),
                    ));
                }
                let mut msg_part = None;
                if let Some(msg) = inner.next() {
                    msg_part = Some(Box::new(self.clone().parse_expr(msg.into_inner())?));
                }

                Ok(Expr::Error(e, msg_part))
            }
            Rule::sysprop => {
                let mut inner = pair.into_inner();
                let property = inner.next().unwrap().as_str();
                Ok(Expr::Prop {
                    location: Box::new(Expr::Value(v_obj(SYSTEM_OBJECT))),
                    property: Box::new(Expr::Value(v_str(property))),
                })
            }
            _ => {
                panic!("Unimplemented atom: {pair:?}");
            }
        }
    }

    fn parse_exprlist(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Vec<Arg>, CompileError> {
        let (lower, _) = pairs.size_hint();
        let mut args = Vec::with_capacity(lower);
        for pair in pairs {
            match pair.as_rule() {
                Rule::argument => {
                    let arg = if pair.as_str().starts_with('@') {
                        Splice(
                            self.clone()
                                .parse_expr(pair.into_inner().next().unwrap().into_inner())?,
                        )
                    } else {
                        Normal(
                            self.clone()
                                .parse_expr(pair.into_inner().next().unwrap().into_inner())?,
                        )
                    };
                    args.push(arg);
                }
                _ => {
                    panic!("Unimplemented exprlist: {pair:?}");
                }
            }
        }
        Ok(args)
    }

    fn parse_arglist(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Vec<Arg>, CompileError> {
        let Some(first) = pairs.peek() else {
            return Ok(vec![]);
        };

        let Rule::exprlist = first.as_rule() else {
            panic!("Unimplemented arglist: {first:?}");
        };

        self.parse_exprlist(first.into_inner())
    }

    fn parse_except_codes(self: Rc<Self>, pairs: Pair<Rule>) -> Result<CatchCodes, CompileError> {
        match pairs.as_rule() {
            Rule::anycode => Ok(CatchCodes::Any),
            Rule::exprlist => Ok(CatchCodes::Codes(self.parse_exprlist(pairs.into_inner())?)),
            _ => {
                panic!("Unimplemented except_codes: {pairs:?}");
            }
        }
    }

    fn parse_expr(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Expr, CompileError> {
        let pratt = build_pratt_parser();

        let primary_self = self.clone();
        let postfix_self = self.clone();

        pratt
            .map_primary(|primary| {
                match primary.as_rule() {
                    Rule::atom => {
                        let mut inner = primary.into_inner();
                        let expr = primary_self.clone().parse_atom(inner.next().unwrap())?;
                        Ok(expr)
                    }
                    Rule::sysprop => {
                        let mut inner = primary.into_inner();
                        let property = inner.next().unwrap().as_str();
                        Ok(Expr::Prop {
                            location: Box::new(Expr::Value(v_obj(SYSTEM_OBJECT))),
                            property: Box::new(Expr::Value(v_str(property))),
                        })
                    }
                    Rule::sysprop_call => {
                        let mut inner = primary.into_inner();
                        let verb = inner.next().unwrap().as_str()[1..].to_string();
                        let args = primary_self
                            .clone()
                            .parse_arglist(inner.next().unwrap().into_inner())?;
                        Ok(Expr::Verb {
                            location: Box::new(Expr::Value(v_obj(SYSTEM_OBJECT))),
                            verb: Box::new(Expr::Value(v_string(verb))),
                            args,
                        })
                    }
                    Rule::lambda => {
                        let mut inner = primary.into_inner();
                        let lambda_params = inner.next().unwrap();
                        let body_part = inner.next().unwrap();

                        // Enter a scope BEFORE parsing lambda parameters.
                        // This prevents DuplicateVariable errors when multiple lambdas
                        // in the same expression use the same parameter name.
                        // e.g., {{s} => s + 1, {s} => s + 2}
                        primary_self.enter_scope();

                        let params = primary_self
                            .clone()
                            .parse_lambda_params(lambda_params.into_inner())?;

                        // Enter a scope for the lambda body to keep locals distinct from params.
                        primary_self.enter_scope();
                        primary_self.enter_lambda_body();

                        let body = match body_part.as_rule() {
                            Rule::begin_statement => {
                                // Parse begin statement directly
                                let line_col = body_part.line_col();
                                let stmt_opt = primary_self.clone().parse_statement(body_part)?;
                                let stmt = stmt_opt.ok_or_else(|| CompileError::ParseError {
                                    error_position: CompileContext::new(line_col),
                                    end_line_col: Some(line_col),
                                    context: "lambda body parsing".to_string(),
                                    message: "Expected statement in lambda body".to_string(),
                                    details: Box::new(ParseErrorDetails::default()),
                                })?;
                                Box::new(stmt)
                            }
                            Rule::expr => {
                                // Parse expression and wrap it in a return statement
                                let line_col = body_part.line_col();
                                let expr =
                                    primary_self.clone().parse_expr(body_part.into_inner())?;
                                let return_stmt = Stmt::new(
                                    StmtNode::Expr(Expr::Return(Some(Box::new(expr)))),
                                    line_col, // Use actual line numbers from body expression
                                );
                                Box::new(return_stmt)
                            }
                            _ => {
                                let line_col = body_part.line_col();
                                return Err(CompileError::ParseError {
                                    error_position: CompileContext::new(line_col),
                                    end_line_col: Some(line_col),
                                    context: "lambda body parsing".to_string(),
                                    message: "Invalid lambda body".to_string(),
                                    details: Box::new(ParseErrorDetails::default()),
                                });
                            }
                        };

                        primary_self.exit_lambda_body();

                        // Exit the lambda's body scope.
                        let _ = primary_self.exit_scope();

                        // Exit the lambda's parameter scope.
                        // Arrow lambdas don't create a Scope node in the AST, so we
                        // discard the binding count. The scope was just for isolation.
                        let _ = primary_self.exit_scope();

                        Ok(Expr::Lambda {
                            params,
                            body,
                            self_name: None,
                        })
                    }
                    Rule::fn_expr => {
                        let mut inner = primary.into_inner();
                        let lambda_params = inner.next().unwrap();
                        let statements_part = inner.next().unwrap();

                        // Enter a scope for parameter isolation.
                        primary_self.enter_scope();

                        let params = primary_self
                            .clone()
                            .parse_lambda_params(lambda_params.into_inner())?;

                        // Parse the statements and wrap them in a scope with proper binding tracking
                        let scope_line_col = statements_part.line_col();
                        primary_self.enter_scope();
                        primary_self.enter_lambda_body();
                        let statements = primary_self
                            .clone()
                            .parse_statements(statements_part.into_inner())?;
                        primary_self.exit_lambda_body();
                        let num_body_bindings = primary_self.exit_scope();

                        // Exit the parameter isolation scope.
                        let _ = primary_self.exit_scope();
                        let body = Box::new(Stmt::new(
                            StmtNode::Scope {
                                num_bindings: num_body_bindings,
                                body: statements,
                            },
                            scope_line_col, // Use actual line numbers from statements
                        ));

                        Ok(Expr::Lambda {
                            params,
                            body,
                            self_name: None,
                        })
                    }
                    Rule::list => {
                        let mut inner = primary.into_inner();
                        if let Some(arglist) = inner.next() {
                            let args = primary_self.clone().parse_exprlist(arglist.into_inner())?;
                            Ok(Expr::List(args))
                        } else {
                            Ok(Expr::List(vec![]))
                        }
                    }
                    Rule::map => {
                        let mut inner = primary.into_inner();
                        let (lower, _) = inner.size_hint();
                        let mut map_pairs = Vec::with_capacity(lower / 2);
                        loop {
                            let Some(key_rule) = inner.next() else {
                                break;
                            };
                            let value_rule = inner.next().unwrap();
                            let key = primary_self
                                .clone()
                                .parse_expr(key_rule.into_inner())
                                .unwrap();
                            let value = primary_self
                                .clone()
                                .parse_expr(value_rule.into_inner())
                                .unwrap();
                            map_pairs.push((key, value));
                        }
                        Ok(Expr::Map(map_pairs))
                    }
                    Rule::flyweight => {
                        if !self.options.flyweight_type {
                            return Err(CompileError::DisabledFeature(
                                self.compile_context(&primary),
                                "Flyweights".to_string(),
                            ));
                        }
                        let mut parts = primary.into_inner();

                        // Three components:
                        // 1. The delegate object
                        // 2. The slots
                        // 3. The contents
                        let delegate = primary_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;

                        let mut slots = vec![];
                        let mut contents = None;

                        // Parse the remaining parts: optional slots, optional contents
                        for next in parts {
                            match next.as_rule() {
                                Rule::flyweight_slot => {
                                    let mut inner = next.into_inner();
                                    let slot_ident = inner.next().unwrap();
                                    let slot_name = Symbol::mk(slot_ident.as_str());

                                    if slot_name == Symbol::mk("delegate")
                                        || slot_name == Symbol::mk("slots")
                                    {
                                        return Err(CompileError::BadSlotName(
                                            self.compile_context(&slot_ident),
                                            slot_name.to_string(),
                                        ));
                                    }

                                    let expr_pair = inner.next().unwrap();
                                    let slot_expr =
                                        primary_self.clone().parse_expr(expr_pair.into_inner())?;
                                    slots.push((slot_name, slot_expr));
                                }
                                Rule::expr => {
                                    // This is the contents expression
                                    let expr =
                                        primary_self.clone().parse_expr(next.into_inner())?;
                                    contents = Some(Box::new(expr));
                                }
                                _ => {
                                    panic!("Unexpected rule: {:?}", next.as_rule());
                                }
                            };
                        }
                        Ok(Expr::Flyweight(Box::new(delegate), slots, contents))
                    }
                    Rule::builtin_call => {
                        let mut inner = primary.into_inner();
                        let bf = inner.next().unwrap().as_str();
                        let args = primary_self
                            .clone()
                            .parse_arglist(inner.next().unwrap().into_inner())?;
                        let function_name = Symbol::mk(bf);

                        // Determine if this is a builtin or variable call
                        let function = if BUILTINS.find_builtin(function_name).is_some() {
                            CallTarget::Builtin(function_name)
                        } else {
                            // Unknown function - could be lambda variable
                            CallTarget::Expr(Box::new(Expr::Id(
                                self.names
                                    .borrow_mut()
                                    .find_or_add_name_global(bf, DeclType::Unknown)
                                    .unwrap(),
                            )))
                        };

                        Ok(Expr::Call { function, args })
                    }
                    Rule::pass_expr => {
                        let mut inner = primary.into_inner();
                        let args = if let Some(arglist) = inner.next() {
                            primary_self.clone().parse_exprlist(arglist.into_inner())?
                        } else {
                            vec![]
                        };
                        Ok(Expr::Pass { args })
                    }
                    Rule::range_end => {
                        if !primary_self.dollars_allowed() {
                            let line_col = primary.line_col();
                            return Err(CompileError::ParseError {
                                error_position: CompileContext::new(line_col),
                                end_line_col: Some(line_col),
                                context: "range expression".to_string(),
                                message: "Illegal context for `$' expression.".to_string(),
                                details: Box::new(ParseErrorDetails::default()),
                            });
                        }
                        Ok(Expr::Length)
                    }
                    Rule::try_expr => {
                        let mut inner = primary.into_inner();
                        let try_expr = primary_self
                            .clone()
                            .parse_expr(inner.next().unwrap().into_inner())?;
                        let codes = inner.next().unwrap();
                        let catch_codes = primary_self
                            .clone()
                            .parse_except_codes(codes.into_inner().next().unwrap())?;
                        let except = inner.next().map(|e| {
                            Box::new(primary_self.clone().parse_expr(e.into_inner()).unwrap())
                        });
                        Ok(Expr::TryCatch {
                            trye: Box::new(try_expr),
                            codes: catch_codes,
                            except,
                        })
                    }

                    Rule::paren_expr => {
                        let mut inner = primary.into_inner();
                        let expr = primary_self
                            .clone()
                            .parse_expr(inner.next().unwrap().into_inner())?;
                        Ok(expr)
                    }
                    Rule::integer => match primary.as_str().parse::<i64>() {
                        Ok(int) => Ok(Expr::Value(v_int(int))),
                        Err(e) => Err(CompileError::StringLexError(
                            self.compile_context(&primary),
                            format!("invalid integer literal '{}': {e}", primary.as_str()),
                        )),
                    },
                    Rule::range_comprehension => {
                        if !self.options.list_comprehensions {
                            return Err(CompileError::DisabledFeature(
                                self.compile_context(&primary),
                                "ListComprehension".to_string(),
                            ));
                        }
                        let mut inner = primary.into_inner();

                        let producer_portion = inner.next().unwrap().into_inner();

                        let variable_ident = inner.next().unwrap();
                        let varname = variable_ident.as_str().trim();
                        let variable = match variable_ident.as_rule() {
                            Rule::ident => {
                                let mut names = self.names.borrow_mut();
                                let Some(name) = names.declare_name(varname, DeclType::For) else {
                                    return Err(DuplicateVariable(
                                        self.compile_context(&variable_ident),
                                        varname.into(),
                                    ));
                                };
                                name
                            }
                            _ => {
                                panic!("Unexpected rule: {:?}", variable_ident.as_rule());
                            }
                        };
                        let producer_expr = primary_self.clone().parse_expr(producer_portion)?;
                        let clause = inner.next().unwrap();

                        match clause.as_rule() {
                            Rule::for_range_clause => {
                                self.enter_scope();
                                let mut clause_inner = clause.into_inner();
                                let from_rule = clause_inner.next().unwrap();
                                let to_rule = clause_inner.next().unwrap();
                                let from = self.clone().parse_expr(from_rule.into_inner())?;
                                let to = self.clone().parse_expr(to_rule.into_inner())?;
                                let end_of_range_register =
                                    self.names.borrow_mut().declare_register()?;
                                self.exit_scope();
                                Ok(Expr::ComprehendRange {
                                    variable,
                                    end_of_range_register,
                                    producer_expr: Box::new(producer_expr),
                                    from: Box::new(from),
                                    to: Box::new(to),
                                })
                            }
                            Rule::for_in_clause => {
                                self.enter_scope();

                                let mut clause_inner = clause.into_inner();
                                let in_rule = clause_inner.next().unwrap();
                                let expr = self.clone().parse_expr(in_rule.into_inner())?;
                                let position_register =
                                    self.names.borrow_mut().declare_register()?;
                                let list_register = self.names.borrow_mut().declare_register()?;
                                self.exit_scope();
                                Ok(Expr::ComprehendList {
                                    list_register,
                                    variable,
                                    position_register,
                                    producer_expr: Box::new(producer_expr),
                                    list: Box::new(expr),
                                })
                            }
                            _ => {
                                todo!("unhandled rule: {:?}", clause.as_rule())
                            }
                        }
                    }
                    Rule::return_expr => {
                        let mut inner = primary.into_inner();
                        let rhs = match inner.next() {
                            Some(e) => Some(Box::new(self.clone().parse_expr(e.into_inner())?)),
                            None => None,
                        };
                        Ok(Expr::Return(rhs))
                    }

                    _ => todo!("Unimplemented primary: {:?}", primary.as_rule()),
                }
            })
            .map_infix(|lhs, op, rhs| match op.as_rule() {
                Rule::add => Ok(Expr::Binary(
                    BinaryOp::Add,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::sub => Ok(Expr::Binary(
                    BinaryOp::Sub,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::mul => Ok(Expr::Binary(
                    BinaryOp::Mul,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::div => Ok(Expr::Binary(
                    BinaryOp::Div,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::pow => Ok(Expr::Binary(
                    BinaryOp::Exp,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::modulus => Ok(Expr::Binary(
                    BinaryOp::Mod,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::eq => Ok(Expr::Binary(
                    BinaryOp::Eq,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::neq => Ok(Expr::Binary(
                    BinaryOp::NEq,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::lt => Ok(Expr::Binary(
                    BinaryOp::Lt,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::lte => Ok(Expr::Binary(
                    BinaryOp::LtE,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::gt => Ok(Expr::Binary(
                    BinaryOp::Gt,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::gte => Ok(Expr::Binary(
                    BinaryOp::GtE,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),

                Rule::land => Ok(Expr::And(Box::new(lhs?), Box::new(rhs.unwrap()))),
                Rule::lor => Ok(Expr::Or(Box::new(lhs?), Box::new(rhs.unwrap()))),
                Rule::in_range => Ok(Expr::Binary(
                    BinaryOp::In,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::bitand => Ok(Expr::Binary(
                    BinaryOp::BitAnd,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::bitor => Ok(Expr::Binary(
                    BinaryOp::BitOr,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::bitxor => Ok(Expr::Binary(
                    BinaryOp::BitXor,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::bitshl => Ok(Expr::Binary(
                    BinaryOp::BitShl,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::bitshr => Ok(Expr::Binary(
                    BinaryOp::BitShr,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::bitlshr => Ok(Expr::Binary(
                    BinaryOp::BitLShr,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                _ => todo!("Unimplemented infix: {:?}", op.as_rule()),
            })
            .map_prefix(|op, rhs| match op.as_rule() {
                Rule::scatter_assign => {
                    let rhs = rhs?;

                    self.clone().parse_scatter_assign(op, rhs, false, false)
                }
                Rule::not => Ok(Expr::Unary(UnaryOp::Not, Box::new(rhs?))),
                Rule::neg => Ok(Expr::Unary(UnaryOp::Neg, Box::new(rhs?))),
                Rule::bitnot => Ok(Expr::Unary(UnaryOp::BitNot, Box::new(rhs?))),
                _ => todo!("Unimplemented prefix: {:?}", op.as_rule()),
            })
            .map_postfix(|lhs, op| {
                match op.as_rule() {
                    Rule::verb_call => {
                        let mut parts = op.into_inner();
                        let ident = parts.next().unwrap().as_str();
                        let args_expr = parts.next().unwrap();
                        let args = postfix_self.clone().parse_arglist(args_expr.into_inner())?;
                        Ok(Expr::Verb {
                            location: Box::new(lhs?),
                            verb: Box::new(Expr::Value(v_str(ident))),
                            args,
                        })
                    }
                    Rule::verb_expr_call => {
                        let mut parts = op.into_inner();
                        let expr = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        let args_expr = parts.next().unwrap();
                        let args = postfix_self.clone().parse_arglist(args_expr.into_inner())?;
                        Ok(Expr::Verb {
                            location: Box::new(lhs?),
                            verb: Box::new(expr),
                            args,
                        })
                    }
                    Rule::prop => {
                        let mut parts = op.into_inner();
                        let ident = parts.next().unwrap().as_str();
                        Ok(Expr::Prop {
                            location: Box::new(lhs?),
                            property: Box::new(Expr::Value(v_str(ident))),
                        })
                    }
                    Rule::prop_expr => {
                        let mut parts = op.into_inner();
                        let expr = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        Ok(Expr::Prop {
                            location: Box::new(lhs?),
                            property: Box::new(expr),
                        })
                    }
                    Rule::assign => {
                        let mut parts = op.clone().into_inner();
                        let right = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        let lhs = lhs?;
                        if let Expr::Id(name) = &lhs {
                            let mut names = self.names.borrow_mut();
                            let decl = names.decl_for_mut(name);

                            // If the variable referenced was not introduced by a let/const clause,
                            // and doesn't have a prior declaration, mark this as its declaration.
                            if decl.decl_type == DeclType::Unknown {
                                decl.decl_type = DeclType::Assign;
                            }

                            // If the variable referenced in the LHS is const, we can't assign to it.
                            // TODO this likely needs to recurse down this tree.
                            if decl.constant {
                                return Err(CompileError::AssignToConst(
                                    self.compile_context(&op),
                                    decl.identifier.to_symbol(),
                                ));
                            }
                        }
                        Ok(Expr::Assign {
                            left: Box::new(lhs),
                            right: Box::new(right),
                        })
                    }
                    Rule::index_single => {
                        let mut parts = op.into_inner();
                        postfix_self.enter_dollars_ok();
                        let index_result = (|| {
                            let index = postfix_self
                                .clone()
                                .parse_expr(parts.next().unwrap().into_inner())?;
                            Ok(Expr::Index(Box::new(lhs?), Box::new(index)))
                        })();
                        postfix_self.exit_dollars_ok();
                        index_result
                    }
                    Rule::index_range => {
                        let mut parts = op.into_inner();
                        postfix_self.enter_dollars_ok();
                        let range_result = (|| {
                            let start = postfix_self
                                .clone()
                                .parse_expr(parts.next().unwrap().into_inner())?;
                            let end = postfix_self
                                .clone()
                                .parse_expr(parts.next().unwrap().into_inner())?;
                            Ok(Expr::Range {
                                base: Box::new(lhs?),
                                from: Box::new(start),
                                to: Box::new(end),
                            })
                        })();
                        postfix_self.exit_dollars_ok();
                        range_result
                    }
                    Rule::cond_expr => {
                        let mut parts = op.into_inner();
                        let true_expr = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        let false_expr = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        Ok(Expr::Cond {
                            condition: Box::new(lhs?),
                            consequence: Box::new(true_expr),
                            alternative: Box::new(false_expr),
                        })
                    }
                    Rule::call => {
                        // Call an expression as a function, e.g. make_getter()()
                        let mut parts = op.into_inner();
                        let args_expr = parts.next().unwrap();
                        let args = postfix_self.clone().parse_arglist(args_expr.into_inner())?;
                        Ok(Expr::Call {
                            function: CallTarget::Expr(Box::new(lhs?)),
                            args,
                        })
                    }
                    _ => todo!("Unimplemented postfix: {:?}", op.as_rule()),
                }
            })
            .parse(pairs)
    }

    fn parse_statement(self: Rc<Self>, pair: Pair<Rule>) -> Result<Option<Stmt>, CompileError> {
        let line_col = pair.line_col();
        let context = self.compile_context(&pair);
        match pair.as_rule() {
            Rule::expr_statement => {
                let mut inner = pair.into_inner();
                if let Some(rule) = inner.next() {
                    let expr = self.parse_expr(rule.into_inner())?;
                    return Ok(Some(Stmt::new(StmtNode::Expr(expr), line_col)));
                }
                Ok(None)
            }
            Rule::while_statement => {
                let mut parts = pair.into_inner();
                let condition = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let environment_width = self.exit_scope();
                Ok(Some(Stmt::new(
                    StmtNode::While {
                        id: None,
                        condition,
                        body,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::labelled_while_statement => {
                let mut parts = pair.into_inner();
                let varname = parts.next().unwrap().as_str();
                let Some(id) = self
                    .names
                    .borrow_mut()
                    .declare_name(varname, DeclType::WhileLabel)
                else {
                    return Err(DuplicateVariable(context, varname.into()));
                };
                let condition = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let environment_width = self.exit_scope();
                Ok(Some(Stmt::new(
                    StmtNode::While {
                        id: Some(id),
                        condition,
                        body,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::if_statement => {
                let mut parts = pair.into_inner();
                let mut arms = vec![];
                let mut otherwise = None;
                let condition = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let environment_width = { self.exit_scope() };
                arms.push(CondArm {
                    condition,
                    statements: body,
                    environment_width,
                });
                for remainder in parts {
                    match remainder.as_rule() {
                        Rule::endif_clause => {
                            continue;
                        }
                        Rule::elseif_clause => {
                            self.enter_scope();
                            let mut parts = remainder.into_inner();
                            let condition = self
                                .clone()
                                .parse_expr(parts.next().unwrap().into_inner())?;
                            let body = self
                                .clone()
                                .parse_statements(parts.next().unwrap().into_inner())?;
                            let environment_width = self.exit_scope();
                            arms.push(CondArm {
                                condition,
                                statements: body,
                                environment_width,
                            });
                        }
                        Rule::else_clause => {
                            self.enter_scope();
                            let mut parts = remainder.into_inner();
                            let otherwise_statements = self
                                .clone()
                                .parse_statements(parts.next().unwrap().into_inner())?;
                            let otherwise_environment_width = self.exit_scope();
                            let otherwise_arm = ElseArm {
                                statements: otherwise_statements,
                                environment_width: otherwise_environment_width,
                            };
                            otherwise = Some(otherwise_arm);
                        }
                        _ => panic!("Unimplemented if clause: {remainder:?}"),
                    }
                }
                Ok(Some(Stmt::new(
                    StmtNode::Cond { arms, otherwise },
                    line_col,
                )))
            }
            Rule::break_statement => {
                let mut parts = pair.clone().into_inner();
                let label = match parts.next() {
                    None => None,
                    Some(s) => {
                        let label = s.as_str();
                        let Some(label) = self.names.borrow_mut().find_name(label) else {
                            return Err(CompileError::UnknownLoopLabel(
                                self.compile_context(&pair),
                                label.to_string(),
                            ));
                        };
                        Some(label)
                    }
                };
                Ok(Some(Stmt::new(StmtNode::Break { exit: label }, line_col)))
            }
            Rule::continue_statement => {
                let mut parts = pair.clone().into_inner();
                let label = match parts.next() {
                    None => None,
                    Some(s) => {
                        let label = s.as_str();
                        let Some(label) = self.names.borrow_mut().find_name(label) else {
                            return Err(CompileError::UnknownLoopLabel(
                                self.compile_context(&pair),
                                label.to_string(),
                            ));
                        };
                        Some(label)
                    }
                };
                Ok(Some(Stmt::new(
                    StmtNode::Continue { exit: label },
                    line_col,
                )))
            }
            Rule::for_range_statement => {
                let mut parts = pair.into_inner();

                let varname = parts.next().unwrap().as_str();
                let value_binding = {
                    let mut names = self.names.borrow_mut();

                    names.declare_or_use_name(varname, DeclType::For)
                };
                let clause = parts.next().unwrap();
                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                match clause.as_rule() {
                    Rule::for_range_clause => {
                        let mut clause_inner = clause.into_inner();
                        let from_rule = clause_inner.next().unwrap();
                        let to_rule = clause_inner.next().unwrap();
                        let from = self.clone().parse_expr(from_rule.into_inner())?;
                        let to = self.clone().parse_expr(to_rule.into_inner())?;
                        let environment_width = self.exit_scope();
                        Ok(Some(Stmt::new(
                            StmtNode::ForRange {
                                id: value_binding,
                                from,
                                to,
                                body,
                                environment_width,
                            },
                            line_col,
                        )))
                    }
                    _ => panic!("Unimplemented for clause: {clause:?}"),
                }
            }
            Rule::for_in_statement => {
                let mut parts = pair.into_inner();

                let mut index_clause = parts.next().unwrap().into_inner();
                // index_clause can have 1 or 2 elements, if there's 2 then there's a "key" as well as a "value" var
                let first_index_rule = index_clause.next().unwrap();
                let varname = first_index_rule.as_str();
                let value_binding = {
                    let mut names = self.names.borrow_mut();

                    names.declare_or_use_name(varname, DeclType::For)
                };
                let key_binding = match index_clause.next() {
                    None => None,
                    Some(s) => {
                        let varname = s.as_str();
                        let key_var = self
                            .names
                            .borrow_mut()
                            .declare_or_use_name(varname, DeclType::For);
                        Some(key_var)
                    }
                };
                self.enter_scope();

                let clause = parts.next().unwrap();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                match clause.as_rule() {
                    Rule::for_in_clause => {
                        let mut clause_inner = clause.into_inner();
                        let in_rule = clause_inner.next().unwrap();
                        let expr = self.clone().parse_expr(in_rule.into_inner())?;
                        let environment_width = self.exit_scope();
                        Ok(Some(Stmt::new(
                            StmtNode::ForList {
                                value_binding,
                                key_binding,
                                expr,
                                body,
                                environment_width,
                            },
                            line_col,
                        )))
                    }
                    _ => panic!("Unimplemented for clause: {clause:?}"),
                }
            }
            Rule::try_finally_statement => {
                self.enter_scope();
                let mut parts = pair.into_inner();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let handler = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let environment_width = self.exit_scope();
                Ok(Some(Stmt::new(
                    StmtNode::TryFinally {
                        body,
                        handler,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::try_except_statement => {
                self.enter_scope();
                let mut parts = pair.into_inner();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let mut excepts = vec![];
                let environment_width = self.exit_scope();

                for except in parts {
                    match except.as_rule() {
                        Rule::except => {
                            let mut except_clause_parts = except.into_inner();
                            let clause = except_clause_parts.next().unwrap();
                            let (id, codes) = match clause.as_rule() {
                                Rule::labelled_except => {
                                    let mut my_parts = clause.into_inner();
                                    let exception = my_parts.next().map(|id| {
                                        let mut names = self.names.borrow_mut();
                                        names.declare_or_use_name(id.as_str(), DeclType::Except)
                                    });

                                    let codes = self.clone().parse_except_codes(
                                        my_parts.next().unwrap().into_inner().next().unwrap(),
                                    )?;
                                    (exception, codes)
                                }
                                Rule::unlabelled_except => {
                                    let mut my_parts = clause.into_inner();
                                    let codes = self.clone().parse_except_codes(
                                        my_parts.next().unwrap().into_inner().next().unwrap(),
                                    )?;
                                    (None, codes)
                                }
                                _ => panic!("Unimplemented except clause: {clause:?}"),
                            };
                            let statements = self.clone().parse_statements(
                                except_clause_parts.next().unwrap().into_inner(),
                            )?;
                            excepts.push(ExceptArm {
                                id,
                                codes,
                                statements,
                            });
                        }
                        _ => panic!("Unimplemented except clause: {except:?}"),
                    }
                }

                Ok(Some(Stmt::new(
                    StmtNode::TryExcept {
                        body,
                        excepts,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::fork_statement => {
                let mut parts = pair.into_inner();
                let time = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
                let body = self.parse_statements(parts.next().unwrap().into_inner())?;
                Ok(Some(Stmt::new(
                    StmtNode::Fork {
                        id: None,
                        time,
                        body,
                    },
                    line_col,
                )))
            }
            Rule::labelled_fork_statement => {
                let mut parts = pair.into_inner();
                let varname = parts.next().unwrap().as_str();
                let Some(id) = self
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(varname, DeclType::ForkLabel)
                else {
                    return Err(DuplicateVariable(context, varname.into()));
                };
                let time = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
                let body = self.parse_statements(parts.next().unwrap().into_inner())?;
                Ok(Some(Stmt::new(
                    StmtNode::Fork {
                        id: Some(id),
                        time,
                        body,
                    },
                    line_col,
                )))
            }
            Rule::begin_statement => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "lexical_scopes".to_string(),
                    ));
                }
                let mut parts = pair.into_inner();

                // Skip begin_keyword
                parts.next();

                self.enter_scope();

                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let num_total_bindings = self.exit_scope();
                // end_keyword is implicitly consumed by the grammar
                Ok(Some(Stmt::new(
                    Scope {
                        num_bindings: num_total_bindings,
                        body,
                    },
                    line_col,
                )))
            }
            Rule::local_assignment | Rule::const_assignment => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "lexical_scopes".to_string(),
                    ));
                }

                // Scatter, or local, we'll then go match on that...
                let parts = pair.into_inner().next().unwrap();
                match parts.as_rule() {
                    Rule::local_assign_single | Rule::const_assign_single => Ok(Some(Stmt::new(
                        self.clone().parse_decl_assign(parts)?,
                        line_col,
                    ))),
                    Rule::local_assign_scatter | Rule::const_assign_scatter => {
                        let is_const = parts.as_rule() == Rule::const_assign_scatter;
                        let mut parts = parts.into_inner();
                        let op = parts.next().unwrap();
                        let rhs = parts.next().unwrap();
                        let rhs = self.clone().parse_expr(rhs.into_inner())?;
                        let expr = self.parse_scatter_assign(op, rhs, true, is_const)?;
                        Ok(Some(Stmt::new(StmtNode::Expr(expr), line_col)))
                    }
                    _ => {
                        unimplemented!("Unimplemented assignment: {:?}", parts.as_rule())
                    }
                }
            }

            Rule::global_assignment => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "lexical_scopes".to_string(),
                    ));
                }

                // An explicit global-declaration.
                // global x, or global x = y
                let mut parts = pair.into_inner();
                let first = parts.next().unwrap();
                let varname = if first.as_rule() == Rule::global_keyword {
                    parts.next().unwrap().as_str()
                } else {
                    first.as_str()
                };
                let id = {
                    let mut names = self.names.borrow_mut();
                    let Some(id) = names.find_or_add_name_global(varname, DeclType::Global) else {
                        return Err(DuplicateVariable(context, varname.into()));
                    };
                    names.decl_for_mut(&id).decl_type = DeclType::Global;
                    id
                };
                let expr = parts
                    .next()
                    .map(|e| self.parse_expr(e.into_inner()).unwrap());

                // Produces an assignment expression as usual, but
                // the decompiler will need to look and see that
                //      a) the statement is just an assignment on its own
                //      b) the variable being assigned to is in scope 0 (global)
                // and then it's a global declaration.
                // Note that this well have the effect of turning most existing MOO decompilations
                // into global declarations, which is fine, if that feature is turned on.
                Ok(Some(Stmt::new(
                    StmtNode::Expr(Expr::Assign {
                        left: Box::new(Expr::Id(id)),
                        right: Box::new(expr.unwrap_or(Expr::Value(v_none()))),
                    }),
                    line_col,
                )))
            }
            Rule::empty_return => Ok(Some(Stmt::new(StmtNode::mk_return_none(), line_col))),
            Rule::fn_statement => {
                let inner = pair.into_inner().next().unwrap();
                match inner.as_rule() {
                    Rule::fn_named => {
                        // fn name(params) statements endfn
                        // This is like: let name = fn(params) statements endfn;
                        let mut parts = inner.clone().into_inner();
                        let func_name = parts.next().unwrap().as_str();
                        let params_part = parts.next().unwrap();
                        let statements_part = parts.next().unwrap();

                        // Enter a scope for parameter isolation.
                        self.enter_scope();

                        // Parse the lambda parameters
                        let params = self.clone().parse_lambda_params(params_part.into_inner())?;

                        // Parse the function body with proper scope tracking
                        let scope_line_col = statements_part.line_col();
                        self.enter_scope();
                        self.enter_lambda_body();
                        let statements = self
                            .clone()
                            .parse_statements(statements_part.into_inner())?;
                        self.exit_lambda_body();
                        let num_body_bindings = self.exit_scope();

                        // Exit the parameter isolation scope.
                        let _ = self.exit_scope();
                        let body = Box::new(Stmt::new(
                            StmtNode::Scope {
                                num_bindings: num_body_bindings,
                                body: statements,
                            },
                            scope_line_col, // Use actual line numbers from statements
                        ));

                        // Create a declaration for the function name
                        let id = {
                            let mut names = self.names.borrow_mut();
                            names.declare_or_use_name(func_name, DeclType::Let)
                        };

                        // Create a lambda expression with self-reference
                        let lambda_expr = Expr::Lambda {
                            params,
                            body,
                            self_name: Some(id),
                        };
                        Ok(Some(Stmt::new(
                            StmtNode::Expr(Expr::Decl {
                                id,
                                expr: Some(Box::new(lambda_expr)),
                                is_const: false,
                            }),
                            line_col,
                        )))
                    }
                    Rule::fn_assignment => {
                        // name = fn(params) statements endfn;
                        // Parse this as: variable = fn_expr
                        let mut parts = inner.clone().into_inner();
                        let var_name = parts.next().unwrap().as_str();
                        let func_expr_part = parts.next().unwrap(); // This is the fn_expr rule

                        // Parse the fn expression manually (similar to fn_expr case above)
                        let mut func_parts = func_expr_part.into_inner();
                        let lambda_params = func_parts.next().unwrap();
                        let statements_part = func_parts.next().unwrap();

                        // Enter a scope for parameter isolation.
                        self.enter_scope();

                        let params = self
                            .clone()
                            .parse_lambda_params(lambda_params.into_inner())?;

                        // Parse the function body with proper scope tracking
                        let scope_line_col = statements_part.line_col();
                        self.enter_scope();
                        self.enter_lambda_body();
                        let statements = self
                            .clone()
                            .parse_statements(statements_part.into_inner())?;
                        self.exit_lambda_body();
                        let num_body_bindings = self.exit_scope();

                        // Exit the parameter isolation scope.
                        let _ = self.exit_scope();
                        let body = Box::new(Stmt::new(
                            StmtNode::Scope {
                                num_bindings: num_body_bindings,
                                body: statements,
                            },
                            scope_line_col, // Use actual line numbers from statements
                        ));

                        // Create the lambda expression
                        let lambda_expr = Expr::Lambda {
                            params,
                            body,
                            self_name: None,
                        };

                        // Create assignment or declaration
                        let maybe_id = self.names.borrow().find_name(var_name);
                        let assign_expr = match maybe_id {
                            Some(id) => {
                                // Variable exists, create assignment
                                Expr::Assign {
                                    left: Box::new(Expr::Id(id)),
                                    right: Box::new(lambda_expr),
                                }
                            }
                            None => {
                                // Variable doesn't exist, declare it
                                let id = {
                                    let mut names = self.names.borrow_mut();
                                    let Some(id) =
                                        names.declare(var_name, false, false, DeclType::Let)
                                    else {
                                        return Err(DuplicateVariable(
                                            self.compile_context(&inner),
                                            var_name.into(),
                                        ));
                                    };
                                    id
                                };
                                Expr::Decl {
                                    id,
                                    expr: Some(Box::new(lambda_expr)),
                                    is_const: false,
                                }
                            }
                        };

                        Ok(Some(Stmt::new(StmtNode::Expr(assign_expr), line_col)))
                    }
                    _ => panic!("Unexpected fn statement rule: {:?}", inner.as_rule()),
                }
            }
            _ => panic!("Unimplemented statement: {:?}", pair.as_rule()),
        }
    }

    fn parse_statements(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Vec<Stmt>, CompileError> {
        let (lower, _) = pairs.size_hint();
        let mut statements = Vec::with_capacity(lower);
        for pair in pairs {
            match pair.as_rule() {
                Rule::statement => {
                    let stmt = self
                        .clone()
                        .parse_statement(pair.into_inner().next().unwrap())?;
                    if let Some(stmt) = stmt {
                        statements.push(stmt);
                    }
                }
                _ => {
                    panic!("Unexpected rule: {:?}", pair.as_rule());
                }
            }
        }
        Ok(statements)
    }

    fn parse_decl_assign(self: Rc<Self>, pair: Pair<Rule>) -> Result<StmtNode, CompileError> {
        let context = self.compile_context(&pair);
        let is_const = pair.as_rule() == Rule::const_assign_single;

        // An assignment declaration that introduces a locally lexically scoped variable.
        // May be of form `let x = expr` or just `let x`
        let mut parts = pair.into_inner();

        let varname = parts.next().unwrap().as_str();
        let id = {
            let mut names = self.names.borrow_mut();
            let Some(id) = names.declare(varname, is_const, false, DeclType::Let) else {
                return Err(DuplicateVariable(context, varname.into()));
            };
            id
        };
        let expr = parts
            .next()
            .map(|e| self.parse_expr(e.into_inner()).unwrap());

        // Create a proper Decl expression for let/const declarations
        Ok(StmtNode::Expr(Expr::Decl {
            id,
            is_const,
            expr: expr.map(Box::new),
        }))
    }

    fn parse_scatter_assign(
        self: Rc<Self>,
        op: Pair<Rule>,
        rhs: Expr,
        local_scope: bool,
        is_const: bool,
    ) -> Result<Expr, CompileError> {
        let context = self.compile_context(&op);
        let inner = op.into_inner();
        let (lower, _) = inner.size_hint();
        let mut items = Vec::with_capacity(lower);
        let mut seen_rest = false;
        for scatter_item in inner {
            match scatter_item.as_rule() {
                Rule::scatter_optional => {
                    let mut inner = scatter_item.into_inner();
                    let id = inner.next().unwrap().as_str();
                    let Some(id) = self.clone().names.borrow_mut().declare(
                        id,
                        is_const,
                        !local_scope,
                        DeclType::Assign,
                    ) else {
                        return Err(DuplicateVariable(context, id.into()));
                    };

                    let expr = inner
                        .next()
                        .map(|e| self.clone().parse_expr(e.into_inner()).unwrap());
                    items.push(ScatterItem {
                        kind: ScatterKind::Optional,
                        id,
                        expr,
                    });
                }
                Rule::scatter_target => {
                    let mut inner = scatter_item.into_inner();
                    let id = inner.next().unwrap().as_str();
                    let Some(id) = self.clone().names.borrow_mut().declare(
                        id,
                        is_const,
                        !local_scope,
                        DeclType::Assign,
                    ) else {
                        return Err(DuplicateVariable(context, id.into()));
                    };
                    items.push(ScatterItem {
                        kind: ScatterKind::Required,
                        id,
                        expr: None,
                    });
                }
                Rule::scatter_rest => {
                    if seen_rest {
                        return Err(CompileError::ParseError {
                            error_position: context,
                            end_line_col: None,
                            context: "scattering assignment validation".to_string(),
                            message: "More than one `@' target in scattering assignment."
                                .to_string(),
                            details: Box::new(ParseErrorDetails::default()),
                        });
                    }
                    seen_rest = true;
                    let mut inner = scatter_item.into_inner();
                    let id = inner.next().unwrap().as_str();
                    let Some(id) = self.clone().names.borrow_mut().declare(
                        id,
                        is_const,
                        !local_scope,
                        DeclType::Assign,
                    ) else {
                        return Err(DuplicateVariable(context, id.into()));
                    };
                    items.push(ScatterItem {
                        kind: ScatterKind::Rest,
                        id,
                        expr: None,
                    });
                }
                _ => {
                    panic!("Unimplemented scatter_item: {scatter_item:?}");
                }
            }
        }

        Ok(Expr::Scatter(items, Box::new(rhs)))
    }

    fn parse_lambda_params(
        self: Rc<Self>,
        params: Pairs<Rule>,
    ) -> Result<Vec<ScatterItem>, CompileError> {
        let (lower, _) = params.size_hint();
        let mut items = Vec::with_capacity(lower);
        let mut seen_rest = false;
        for param in params {
            match param.as_rule() {
                Rule::lambda_param => {
                    let inner_param = param.into_inner().next().unwrap();
                    match inner_param.as_rule() {
                        Rule::scatter_optional => {
                            let context = self.compile_context(&inner_param);
                            let mut inner = inner_param.into_inner();
                            let id_str = inner.next().unwrap().as_str();
                            let Some(id) = self.clone().names.borrow_mut().declare(
                                id_str,
                                false,
                                false,
                                DeclType::Assign,
                            ) else {
                                return Err(DuplicateVariable(context, id_str.into()));
                            };

                            let expr = inner
                                .next()
                                .map(|e| self.clone().parse_expr(e.into_inner()).unwrap());
                            items.push(ScatterItem {
                                kind: ScatterKind::Optional,
                                id,
                                expr,
                            });
                        }
                        Rule::scatter_target => {
                            let context = self.compile_context(&inner_param);
                            let mut inner = inner_param.into_inner();
                            let id_str = inner.next().unwrap().as_str();
                            let Some(id) = self.clone().names.borrow_mut().declare(
                                id_str,
                                false,
                                false,
                                DeclType::Assign,
                            ) else {
                                return Err(DuplicateVariable(context, id_str.into()));
                            };

                            items.push(ScatterItem {
                                kind: ScatterKind::Required,
                                id,
                                expr: None,
                            });
                        }
                        Rule::scatter_rest => {
                            if seen_rest {
                                return Err(CompileError::ParseError {
                                    error_position: CompileContext::new((0, 0)),
                                    end_line_col: None,
                                    context: "lambda parameter validation".to_string(),
                                    message: "More than one `@' target in scattering assignment."
                                        .to_string(),
                                    details: Box::new(ParseErrorDetails::default()),
                                });
                            }
                            seen_rest = true;
                            let context = self.compile_context(&inner_param);
                            let mut inner = inner_param.into_inner();
                            let id_str = inner.next().unwrap().as_str();
                            let Some(id) = self.clone().names.borrow_mut().declare(
                                id_str,
                                false,
                                false,
                                DeclType::Assign,
                            ) else {
                                return Err(DuplicateVariable(context, id_str.into()));
                            };

                            items.push(ScatterItem {
                                kind: ScatterKind::Rest,
                                id,
                                expr: None,
                            });
                        }
                        _ => {
                            panic!("Unimplemented inner lambda_param: {inner_param:?}");
                        }
                    }
                }
                _ => {
                    panic!("Unimplemented lambda_param: {param:?}");
                }
            }
        }

        Ok(items)
    }

    fn transform_tree(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Parse, CompileError> {
        let mut program = Vec::new();
        for pair in pairs {
            match pair.as_rule() {
                Rule::program => {
                    let inna = pair.into_inner().next().unwrap();

                    match inna.as_rule() {
                        Rule::statements => {
                            let parsed_statements =
                                self.clone().parse_statements(inna.into_inner())?;
                            program.extend(parsed_statements);
                        }

                        _ => {
                            panic!("Unexpected rule: {:?}", inna.as_rule());
                        }
                    }
                }
                _ => {
                    panic!("Unexpected rule: {:?}", pair.as_rule());
                }
            }
        }

        self.do_transform(program)
    }

    fn do_transform(self: Rc<Self>, mut program: Vec<Stmt>) -> Result<Parse, CompileError> {
        let unbound_names = self.names.borrow_mut();
        // Annotate the "true" line numbers of the AST nodes.
        annotate_line_numbers(1, &mut program);
        let names = unbound_names.bind();
        Ok(Parse {
            stmts: program,
            variables: unbound_names.clone(),
            names,
        })
    }

    fn enter_scope(&self) {
        if self.options.lexical_scopes {
            self.names.borrow_mut().enter_new_scope();
        }
    }

    fn exit_scope(&self) -> usize {
        if self.options.lexical_scopes {
            return self.names.borrow_mut().exit_scope();
        }
        0
    }
}

/// The emitted parse tree from the parse phase of the compiler.
#[derive(Debug)]
pub struct Parse {
    pub stmts: Vec<Stmt>,
    pub variables: VarScope,
    pub names: Names,
}

pub fn parse_program(program_text: &str, options: CompileOptions) -> Result<Parse, CompileError> {
    ensure_pest_error_detail();

    let pairs = match MooParser::parse(Rule::program, program_text) {
        Ok(pairs) => pairs,
        Err(e) => {
            let ((mut line, mut column), mut end_line_col) = match e.line_col {
                LineColLocation::Pos(lc) => (lc, None),
                LineColLocation::Span(begin, end) => (begin, Some(end)),
            };

            let (summary, details) = build_parse_error_details(program_text, &e);

            if let Some((start, end)) = details.span {
                let (computed_line, computed_col) = offset_to_line_col(program_text, start);
                line = computed_line;
                column = computed_col;

                let end_offset = if end > start { end - 1 } else { start };
                let (end_line, end_col) = offset_to_line_col(program_text, end_offset);
                end_line_col = Some((end_line, end_col));
            }

            let context = CompileContext::new((line, column));
            return Err(CompileError::ParseError {
                error_position: context,
                end_line_col,
                context: e.line().to_string(),
                message: summary,
                details: Box::new(details),
            });
        }
    };

    // TODO: this is in Rc because of borrowing issues in the Pratt parser
    let tree_transform = TreeTransformer::new(options);
    tree_transform.transform_tree(pairs)
}

fn ensure_pest_error_detail() {
    static INIT: Once = Once::new();
    INIT.call_once(|| set_error_detail(true));
}

fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    let clamped = offset.min(source.len());

    for ch in source[..clamped].chars() {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
}
