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

//! Lower the handwritten frontend CST into the existing AST and semantic state.

use rowan::{NodeOrToken, TextRange, TextSize, ast::AstNode};

use moor_common::builtins::BUILTINS;
use moor_common::model::{CompileContext, CompileError, ParseErrorDetails};
use moor_var::{
    AnonymousObjid, Obj, SYSTEM_OBJECT, Symbol, UuObjid, Var, VarType, program::DeclType, v_binary,
    v_float, v_int, v_none, v_obj, v_str,
};

use crate::{
    FrontendParseError, SyntaxKind, VarScope,
    ast::{
        Arg,
        Arg::{Normal, Splice},
        BinaryOp, CallTarget, CatchCodes, CondArm, ElseArm, ExceptArm, Expr,
        ScatterItem as AstScatterItem, ScatterKind, Stmt, StmtNode, UnaryOp,
    },
    compile_options::CompileOptions,
    frontend::{
        cst::{
            BeginStmt, BreakStmt, ConstStmt, ContinueStmt, ExprStmt, Expression, FnStmt, ForInStmt,
            ForRangeStmt, ForkStmt, GlobalStmt, IfStmt, LambdaExpr, LetStmt, ParamList, Program,
            ReturnStmt, ScatterExpr, ScatterItem, Statement, TryExceptStmt, WhileStmt,
        },
        parser::parse_to_syntax_node,
        syntax::{SyntaxElement, SyntaxNode, SyntaxToken},
    },
    parse_tree::Parse,
    unparse::annotate_line_numbers,
};

use base64::{Engine, engine::general_purpose};

pub fn parse_program_frontend(
    program_text: &str,
    options: CompileOptions,
) -> Result<Parse, CompileError> {
    let (root, errors) = parse_to_syntax_node(program_text);
    if let Some(error) = errors.first() {
        return Err(frontend_error_to_compile_error(program_text, error));
    }

    let Some(program) = Program::cast(root) else {
        return Err(CompileError::ParseError {
            error_position: CompileContext::new((1, 1)),
            context: "frontend lowering".to_string(),
            end_line_col: Some((1, 1)),
            message: "frontend parser did not produce a program root".to_string(),
            details: Box::new(ParseErrorDetails::default()),
        });
    };

    Lowerer::new(program_text, options).lower_program(program)
}

struct Lowerer<'a> {
    source: &'a str,
    options: CompileOptions,
    names: VarScope,
    lambda_body_depth: usize,
    dollars_ok: usize,
}

impl<'a> Lowerer<'a> {
    fn new(source: &'a str, options: CompileOptions) -> Self {
        Self {
            source,
            options,
            names: VarScope::new(),
            lambda_body_depth: 0,
            dollars_ok: 0,
        }
    }

    fn lower_program(mut self, program: Program) -> Result<Parse, CompileError> {
        let Some(statements) = program.statements() else {
            return Err(self.make_parse_error(
                TextRange::new(TextSize::from(0), TextSize::from(0)),
                "frontend lowering",
                "program is missing a statement list",
            ));
        };

        let mut stmts = Vec::new();
        for statement in statements {
            if let Some(stmt) = self.lower_statement(statement)? {
                stmts.push(stmt);
            }
        }

        annotate_line_numbers(1, &mut stmts);
        let names = self.names.bind();
        Ok(Parse {
            stmts,
            variables: self.names,
            names,
        })
    }

    fn lower_statement(&mut self, statement: Statement) -> Result<Option<Stmt>, CompileError> {
        match statement {
            Statement::If(stmt) => self.lower_if_stmt(stmt).map(Some),
            Statement::ForIn(stmt) => self.lower_for_in_stmt(stmt).map(Some),
            Statement::ForRange(stmt) => self.lower_for_range_stmt(stmt).map(Some),
            Statement::While(stmt) => self.lower_while_stmt(stmt).map(Some),
            Statement::Fork(stmt) => self.lower_fork_stmt(stmt).map(Some),
            Statement::TryExcept(stmt) => self.lower_try_stmt(stmt).map(Some),
            Statement::Expr(stmt) => self.lower_expr_stmt(stmt),
            Statement::Return(stmt) => self.lower_return_stmt(stmt).map(Some),
            Statement::Break(stmt) => self.lower_break_stmt(stmt).map(Some),
            Statement::Continue(stmt) => self.lower_continue_stmt(stmt).map(Some),
            Statement::Fn(stmt) => self.lower_fn_stmt(stmt).map(Some),
            Statement::Let(stmt) => self.lower_decl_stmt(stmt, false).map(Some),
            Statement::Const(stmt) => self.lower_const_stmt(stmt).map(Some),
            Statement::Global(stmt) => self.lower_global_stmt(stmt).map(Some),
            Statement::Begin(stmt) => self.lower_begin_stmt(stmt).map(Some),
            _ => {
                Err(self.unsupported_node(statement.syntax(), "frontend lowering statement subset"))
            }
        }
    }

    fn lower_expr_stmt(&mut self, stmt: ExprStmt) -> Result<Option<Stmt>, CompileError> {
        let Some(content) = stmt.content() else {
            return Ok(None);
        };
        if let NodeOrToken::Node(node) = &content
            && let Some(expr) = Expression::cast(node.clone())
            && let Expression::Assign(assign) = expr
            && let Some(stmt) = self.lower_fn_assignment_stmt(stmt.syntax(), assign.syntax())?
        {
            return Ok(Some(stmt));
        }
        let expr = self.lower_expr_element(content)?;
        Ok(Some(Stmt::new(
            StmtNode::Expr(expr),
            self.line_col(stmt.syntax().text_range()),
        )))
    }

    fn lower_return_stmt(&mut self, stmt: ReturnStmt) -> Result<Stmt, CompileError> {
        let elements = significant_elements(stmt.syntax());
        let expr = if elements.len() > 2 {
            Some(self.lower_expr_element(expect_exprish(&elements, 1)?)?)
        } else {
            None
        };
        Ok(Stmt::new(
            StmtNode::Expr(Expr::Return(expr.map(Box::new))),
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn if_condition(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        self.lower_expr_element(expect_exprish(&elements, 2)?)
    }

    fn jump_label(
        &mut self,
        node: &SyntaxNode,
    ) -> Result<Option<moor_var::program::names::Variable>, CompileError> {
        let elements = significant_elements(node);
        if let Some(NodeOrToken::Token(token)) = elements.get(1)
            && token.kind() == SyntaxKind::Ident
        {
            let Some(label) = self.names.find_name(token.text()) else {
                return Err(CompileError::UnknownLoopLabel(
                    self.compile_context(token.text_range()),
                    token.text().to_string(),
                ));
            };
            return Ok(Some(label));
        }
        Ok(None)
    }

    fn lower_except_head(
        &mut self,
        node: &SyntaxNode,
    ) -> Result<(Option<moor_var::program::names::Variable>, CatchCodes), CompileError> {
        let elements = significant_elements(node);
        let mut idx = 1usize;
        let id = if matches!(elements.get(idx), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Ident)
            && !matches!(elements.get(idx + 1), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::LParen)
        {
            None
        } else if matches!(elements.get(idx), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Ident)
        {
            let ident = expect_token(&elements, idx)?;
            idx += 1;
            Some(
                self.names
                    .declare_or_use_name(ident.text(), DeclType::Except),
            )
        } else {
            None
        };

        while idx < elements.len() {
            if matches!(elements.get(idx), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::LParen)
            {
                break;
            }
            idx += 1;
        }
        idx += 1;

        let codes = if matches!(elements.get(idx), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::AnyKw)
        {
            CatchCodes::Any
        } else {
            let mut args = Vec::new();
            while idx < elements.len() {
                match elements.get(idx) {
                    Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::RParen => break,
                    Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Comma => {
                        idx += 1;
                    }
                    Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::At => {
                        let expr = self.lower_expr_element(expect_exprish(&elements, idx + 1)?)?;
                        args.push(Splice(expr));
                        idx += 2;
                    }
                    Some(element) => {
                        args.push(Normal(self.lower_expr_element(element.clone())?));
                        idx += 1;
                    }
                    None => break,
                }
            }
            CatchCodes::Codes(args)
        };
        Ok((id, codes))
    }

    fn lower_if_stmt(&mut self, stmt: IfStmt) -> Result<Stmt, CompileError> {
        let mut arms = Vec::new();

        let condition = self.if_condition(stmt.syntax())?;
        self.enter_scope();
        let body = self.lower_stmt_list(stmt.body().ok_or_else(|| {
            self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering if",
                "if body is missing",
            )
        })?)?;
        let environment_width = self.exit_scope();
        arms.push(CondArm {
            condition,
            statements: body,
            environment_width,
        });

        for clause in stmt.elseif_clauses() {
            let condition = self.if_condition(clause.syntax())?;
            self.enter_scope();
            let body = self.lower_stmt_list(clause.body().ok_or_else(|| {
                self.make_parse_error(
                    clause.syntax().text_range(),
                    "frontend lowering elseif",
                    "elseif body is missing",
                )
            })?)?;
            let environment_width = self.exit_scope();
            arms.push(CondArm {
                condition,
                statements: body,
                environment_width,
            });
        }

        let otherwise = if let Some(clause) = stmt.else_clause() {
            self.enter_scope();
            let statements = self.lower_stmt_list(clause.body().ok_or_else(|| {
                self.make_parse_error(
                    clause.syntax().text_range(),
                    "frontend lowering else",
                    "else body is missing",
                )
            })?)?;
            let environment_width = self.exit_scope();
            Some(ElseArm {
                statements,
                environment_width,
            })
        } else {
            None
        };

        Ok(Stmt::new(
            StmtNode::Cond { arms, otherwise },
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_while_stmt(&mut self, stmt: WhileStmt) -> Result<Stmt, CompileError> {
        let elements = significant_elements(stmt.syntax());
        let mut idx = 1usize;
        let id = if matches!(elements.get(1), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Ident)
            && matches!(elements.get(2), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::LParen)
        {
            idx = 2;
            let label = expect_token(&elements, 1)?;
            let Some(id) = self.names.declare_name(label.text(), DeclType::WhileLabel) else {
                return Err(CompileError::DuplicateVariable(
                    self.compile_context(label.text_range()),
                    Symbol::mk(label.text()),
                ));
            };
            Some(id)
        } else {
            None
        };

        let condition = self.lower_expr_element(expect_exprish(&elements, idx + 1)?)?;
        self.enter_scope();
        let body = self.lower_stmt_list(stmt.body().ok_or_else(|| {
            self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering while",
                "while body is missing",
            )
        })?)?;
        let environment_width = self.exit_scope();

        Ok(Stmt::new(
            StmtNode::While {
                id,
                condition,
                body,
                environment_width,
            },
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_fork_stmt(&mut self, stmt: ForkStmt) -> Result<Stmt, CompileError> {
        let elements = significant_elements(stmt.syntax());
        let mut idx = 1usize;
        let id = if matches!(elements.get(1), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Ident)
            && matches!(elements.get(2), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::LParen)
        {
            idx = 2;
            let label = expect_token(&elements, 1)?;
            let Some(id) = self
                .names
                .find_or_add_name_global(label.text(), DeclType::ForkLabel)
            else {
                return Err(CompileError::DuplicateVariable(
                    self.compile_context(label.text_range()),
                    Symbol::mk(label.text()),
                ));
            };
            Some(id)
        } else {
            None
        };

        let time = self.lower_expr_element(expect_exprish(&elements, idx + 1)?)?;
        let body = self.lower_stmt_list(stmt.body().ok_or_else(|| {
            self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering fork",
                "fork body is missing",
            )
        })?)?;

        Ok(Stmt::new(
            StmtNode::Fork { id, time, body },
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_for_range_stmt(&mut self, stmt: ForRangeStmt) -> Result<Stmt, CompileError> {
        let elements = significant_elements(stmt.syntax());
        let ident = expect_token(&elements, 1)?;
        let id = self.names.declare_or_use_name(ident.text(), DeclType::For);
        let from = self.lower_expr_element(expect_exprish(&elements, 4)?)?;
        let to = self.lower_expr_element(expect_exprish(&elements, 6)?)?;
        self.enter_scope();
        let body = self.lower_stmt_list(stmt.body().ok_or_else(|| {
            self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering for-range",
                "for-range body is missing",
            )
        })?)?;
        let environment_width = self.exit_scope();

        Ok(Stmt::new(
            StmtNode::ForRange {
                id,
                from,
                to,
                body,
                environment_width,
            },
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_for_in_stmt(&mut self, stmt: ForInStmt) -> Result<Stmt, CompileError> {
        let elements = significant_elements(stmt.syntax());
        let value_name = expect_token(&elements, 1)?;
        let value_binding = self
            .names
            .declare_or_use_name(value_name.text(), DeclType::For);

        let mut idx = 2usize;
        let key_binding = if matches!(elements.get(idx), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Comma)
        {
            let key_name = expect_token(&elements, idx + 1)?;
            idx += 2;
            Some(
                self.names
                    .declare_or_use_name(key_name.text(), DeclType::For),
            )
        } else {
            None
        };

        let expr = self.lower_expr_element(expect_exprish(&elements, idx + 2)?)?;
        self.enter_scope();
        let body = self.lower_stmt_list(stmt.body().ok_or_else(|| {
            self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering for-in",
                "for-in body is missing",
            )
        })?)?;
        let environment_width = self.exit_scope();

        Ok(Stmt::new(
            StmtNode::ForList {
                value_binding,
                key_binding,
                expr,
                body,
                environment_width,
            },
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_try_stmt(&mut self, stmt: TryExceptStmt) -> Result<Stmt, CompileError> {
        self.enter_scope();
        let body = self.lower_stmt_list(stmt.body().ok_or_else(|| {
            self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering try",
                "try body is missing",
            )
        })?)?;
        let environment_width = self.exit_scope();

        if let Some(finally_clause) = stmt.finally_clause() {
            let handler = self.lower_stmt_list(finally_clause.body().ok_or_else(|| {
                self.make_parse_error(
                    finally_clause.syntax().text_range(),
                    "frontend lowering finally",
                    "finally body is missing",
                )
            })?)?;
            return Ok(Stmt::new(
                StmtNode::TryFinally {
                    body,
                    handler,
                    environment_width,
                },
                self.line_col(stmt.syntax().text_range()),
            ));
        }

        let mut excepts = Vec::new();
        for clause in stmt.except_clauses() {
            let (id, codes) = self.lower_except_head(clause.syntax())?;
            let statements = self.lower_stmt_list(clause.body().ok_or_else(|| {
                self.make_parse_error(
                    clause.syntax().text_range(),
                    "frontend lowering except",
                    "except body is missing",
                )
            })?)?;
            excepts.push(ExceptArm {
                id,
                codes,
                statements,
            });
        }

        Ok(Stmt::new(
            StmtNode::TryExcept {
                body,
                excepts,
                environment_width,
            },
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_break_stmt(&mut self, stmt: BreakStmt) -> Result<Stmt, CompileError> {
        let exit = self.jump_label(stmt.syntax())?;
        Ok(Stmt::new(
            StmtNode::Break { exit },
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_continue_stmt(&mut self, stmt: ContinueStmt) -> Result<Stmt, CompileError> {
        let exit = self.jump_label(stmt.syntax())?;
        Ok(Stmt::new(
            StmtNode::Continue { exit },
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_fn_stmt(&mut self, stmt: FnStmt) -> Result<Stmt, CompileError> {
        let Some(name_token) = stmt.name_token() else {
            return Err(self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering fn",
                "missing function name",
            ));
        };

        self.enter_scope();
        let params = self.lower_lambda_params(stmt.params().ok_or_else(|| {
            self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering fn",
                "missing function parameters",
            )
        })?)?;

        let scope_line_col = self.line_col(
            stmt.body()
                .map(|body| body.syntax().text_range())
                .unwrap_or(stmt.syntax().text_range()),
        );
        self.enter_scope();
        self.lambda_body_depth += 1;
        let statements = self.lower_stmt_list(stmt.body().ok_or_else(|| {
            self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering fn",
                "missing function body",
            )
        })?)?;
        self.lambda_body_depth = self.lambda_body_depth.saturating_sub(1);
        let num_body_bindings = self.exit_scope();
        let _ = self.exit_scope();

        let body = Box::new(Stmt::new(
            StmtNode::Scope {
                num_bindings: num_body_bindings,
                body: statements,
            },
            scope_line_col,
        ));

        let id = self
            .names
            .declare_or_use_name(name_token.text(), DeclType::Let);
        let lambda_expr = Expr::Lambda {
            params,
            body,
            self_name: Some(id),
        };

        Ok(Stmt::new(
            StmtNode::Expr(Expr::Decl {
                id,
                expr: Some(Box::new(lambda_expr)),
                is_const: false,
            }),
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_fn_assignment_stmt(
        &mut self,
        stmt_syntax: &SyntaxNode,
        assign_syntax: &SyntaxNode,
    ) -> Result<Option<Stmt>, CompileError> {
        let elements = significant_elements(assign_syntax);
        let lhs = match elements.first() {
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Ident => token.clone(),
            _ => return Ok(None),
        };
        let rhs_node = match elements.get(2) {
            Some(NodeOrToken::Node(node)) => node.clone(),
            _ => return Ok(None),
        };
        let Some(lambda) = LambdaExpr::cast(rhs_node) else {
            return Ok(None);
        };
        if lambda.body().is_none() {
            return Ok(None);
        }

        let lambda_expr = self.lower_lambda_expr(lambda)?;
        let assign_expr = match self.names.find_name(lhs.text()) {
            Some(id) => Expr::Assign {
                left: Box::new(Expr::Id(id)),
                right: Box::new(lambda_expr),
            },
            None => {
                let Some(id) = self.names.declare(lhs.text(), false, false, DeclType::Let) else {
                    return Err(CompileError::DuplicateVariable(
                        self.compile_context(lhs.text_range()),
                        Symbol::mk(lhs.text()),
                    ));
                };
                Expr::Decl {
                    id,
                    expr: Some(Box::new(lambda_expr)),
                    is_const: false,
                }
            }
        };

        Ok(Some(Stmt::new(
            StmtNode::Expr(assign_expr),
            self.line_col(stmt_syntax.text_range()),
        )))
    }

    fn lower_decl_stmt(&mut self, stmt: LetStmt, is_const: bool) -> Result<Stmt, CompileError> {
        if !self.options.lexical_scopes {
            return Err(CompileError::DisabledFeature(
                self.compile_context(stmt.syntax().text_range()),
                "lexical_scopes".to_string(),
            ));
        }

        if let Some(scatter) = stmt.scatter() {
            let rhs = self.scatter_rhs(stmt.syntax())?;
            let expr = self.lower_scatter_assign(scatter, rhs, true, is_const)?;
            return Ok(Stmt::new(
                StmtNode::Expr(expr),
                self.line_col(stmt.syntax().text_range()),
            ));
        }

        let Some(name_token) = stmt.name_token() else {
            return Err(self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering let",
                "missing declaration name",
            ));
        };

        let context = self.compile_context(name_token.text_range());
        let Some(id) = self
            .names
            .declare(name_token.text(), is_const, false, DeclType::Let)
        else {
            return Err(CompileError::DuplicateVariable(
                context,
                Symbol::mk(name_token.text()),
            ));
        };

        let expr = self
            .rhs_expr_from_decl(stmt.syntax())?
            .map(Box::new);
        Ok(Stmt::new(
            StmtNode::Expr(Expr::Decl { id, is_const, expr }),
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_const_stmt(&mut self, stmt: ConstStmt) -> Result<Stmt, CompileError> {
        if !self.options.lexical_scopes {
            return Err(CompileError::DisabledFeature(
                self.compile_context(stmt.syntax().text_range()),
                "lexical_scopes".to_string(),
            ));
        }

        if let Some(scatter) = stmt.scatter() {
            let rhs = self.scatter_rhs(stmt.syntax())?;
            let expr = self.lower_scatter_assign(scatter, rhs, true, true)?;
            return Ok(Stmt::new(
                StmtNode::Expr(expr),
                self.line_col(stmt.syntax().text_range()),
            ));
        }

        let Some(name_token) = stmt.name_token() else {
            return Err(self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering const",
                "missing declaration name",
            ));
        };

        let context = self.compile_context(name_token.text_range());
        let Some(id) = self.names.declare_const(name_token.text(), DeclType::Let) else {
            return Err(CompileError::DuplicateVariable(
                context,
                Symbol::mk(name_token.text()),
            ));
        };

        let expr = self
            .rhs_expr_from_decl(stmt.syntax())?
            .map(Box::new);
        Ok(Stmt::new(
            StmtNode::Expr(Expr::Decl {
                id,
                is_const: true,
                expr,
            }),
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_global_stmt(&mut self, stmt: GlobalStmt) -> Result<Stmt, CompileError> {
        if !self.options.lexical_scopes {
            return Err(CompileError::DisabledFeature(
                self.compile_context(stmt.syntax().text_range()),
                "lexical_scopes".to_string(),
            ));
        }

        let Some(name_token) = stmt.name_token() else {
            return Err(self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering global",
                "missing global name",
            ));
        };

        let context = self.compile_context(name_token.text_range());
        let Some(id) = self
            .names
            .find_or_add_name_global(name_token.text(), DeclType::Global)
        else {
            return Err(CompileError::DuplicateVariable(
                context,
                Symbol::mk(name_token.text()),
            ));
        };
        self.names.decl_for_mut(&id).decl_type = DeclType::Global;

        let rhs = self
            .rhs_expr_from_decl(stmt.syntax())?
            .unwrap_or_else(|| Expr::Value(v_none()));

        Ok(Stmt::new(
            StmtNode::Expr(Expr::Assign {
                left: Box::new(Expr::Id(id)),
                right: Box::new(rhs),
            }),
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_begin_stmt(&mut self, stmt: BeginStmt) -> Result<Stmt, CompileError> {
        if !self.options.lexical_scopes {
            return Err(CompileError::DisabledFeature(
                self.compile_context(stmt.syntax().text_range()),
                "lexical_scopes".to_string(),
            ));
        }

        self.enter_scope();
        let body = self.lower_stmt_list(stmt.body().ok_or_else(|| {
            self.make_parse_error(
                stmt.syntax().text_range(),
                "frontend lowering begin",
                "begin block is missing a statement list",
            )
        })?)?;
        let num_bindings = self.exit_scope();
        Ok(Stmt::new(
            StmtNode::Scope { num_bindings, body },
            self.line_col(stmt.syntax().text_range()),
        ))
    }

    fn lower_stmt_list(
        &mut self,
        stmt_list: crate::frontend::cst::StmtList,
    ) -> Result<Vec<Stmt>, CompileError> {
        let mut stmts = Vec::new();
        for statement in stmt_list.statements() {
            if let Some(stmt) = self.lower_statement(statement)? {
                stmts.push(stmt);
            }
        }
        Ok(stmts)
    }

    fn lower_expr_element(&mut self, element: SyntaxElement) -> Result<Expr, CompileError> {
        match element {
            NodeOrToken::Node(node) => {
                if let Some(return_stmt) = ReturnStmt::cast(node.clone()) {
                    return self.lower_return_expr(return_stmt);
                }
                self.lower_expr_node(node)
            }
            NodeOrToken::Token(token) => self.lower_atom_token(token),
        }
    }

    fn lower_return_expr(&mut self, stmt: ReturnStmt) -> Result<Expr, CompileError> {
        let elements = significant_elements(stmt.syntax());
        let expr = if elements.len() > 1 {
            Some(Box::new(
                self.lower_expr_element(expect_exprish(&elements, 1)?)?,
            ))
        } else {
            None
        };
        Ok(Expr::Return(expr))
    }

    fn lower_expr_node(&mut self, node: SyntaxNode) -> Result<Expr, CompileError> {
        let Some(expr) = Expression::cast(node.clone()) else {
            return Err(self.unsupported_node(&node, "frontend lowering expression subset"));
        };

        match expr {
            Expression::Paren(expr) => self.single_child_expr(expr.syntax()),
            Expression::Unary(expr) => self.lower_unary_expr(expr.syntax()),
            Expression::Binary(expr) => self.lower_binary_expr(expr.syntax()),
            Expression::Assign(expr) => self.lower_assign_expr(expr.syntax()),
            Expression::Conditional(expr) => self.lower_cond_expr(expr.syntax()),
            Expression::Index(expr) => self.lower_index_expr(expr.syntax()),
            Expression::Range(expr) => self.lower_range_expr(expr.syntax()),
            Expression::Call(expr) => self.lower_call_expr(expr.syntax()),
            Expression::VerbCall(expr) => self.lower_verb_call_expr(expr.syntax()),
            Expression::Property(expr) => self.lower_prop_expr(expr.syntax()),
            Expression::List(expr) => self.lower_list_expr(expr.syntax()),
            Expression::Map(expr) => self.lower_map_expr(expr.syntax()),
            Expression::Flyweight(expr) => self.lower_flyweight_expr(expr.syntax()),
            Expression::Pass(expr) => self.lower_pass_expr(expr.syntax()),
            Expression::SysProp(expr) => self.lower_sysprop_expr(expr.syntax()),
            Expression::Try(expr) => self.lower_try_expr(expr.syntax()),
            Expression::Comprehension(expr) => self.lower_comprehension_expr(expr.syntax()),
            Expression::Scatter(expr) => {
                let rhs = self.scatter_rhs(expr.syntax())?;
                let pattern = inner_scatter_pattern(expr);
                self.lower_scatter_assign(pattern, rhs, false, false)
            }
            Expression::Lambda(expr) => self.lower_lambda_expr(expr),
        }
    }

    fn lower_lambda_expr(&mut self, expr: LambdaExpr) -> Result<Expr, CompileError> {
        self.enter_scope();
        let params = self.lower_lambda_params(expr.params().ok_or_else(|| {
            self.make_parse_error(
                expr.syntax().text_range(),
                "frontend lowering lambda",
                "missing lambda parameters",
            )
        })?)?;

        if let Some(body_list) = expr.body() {
            let scope_line_col = self.line_col(body_list.syntax().text_range());
            self.enter_scope();
            self.lambda_body_depth += 1;
            let statements = self.lower_stmt_list(body_list)?;
            self.lambda_body_depth = self.lambda_body_depth.saturating_sub(1);
            let num_body_bindings = self.exit_scope();
            let _ = self.exit_scope();
            return Ok(Expr::Lambda {
                params,
                body: Box::new(Stmt::new(
                    StmtNode::Scope {
                        num_bindings: num_body_bindings,
                        body: statements,
                    },
                    scope_line_col,
                )),
                self_name: None,
            });
        }

        let elements = significant_elements(expr.syntax());
        let arrow_idx = elements
            .iter()
            .position(|element| matches!(element, NodeOrToken::Token(token) if token.kind() == SyntaxKind::FatArrow))
            .ok_or_else(|| {
                self.make_parse_error(
                    expr.syntax().text_range(),
                    "frontend lowering lambda",
                    "missing lambda arrow",
                )
            })?;
        self.enter_scope();
        self.lambda_body_depth += 1;
        let body_expr = self.lower_expr_element(expect_exprish(&elements, arrow_idx + 1)?)?;
        self.lambda_body_depth = self.lambda_body_depth.saturating_sub(1);
        let _ = self.exit_scope();
        let _ = self.exit_scope();
        let return_stmt = Stmt::new(
            StmtNode::Expr(Expr::Return(Some(Box::new(body_expr)))),
            self.line_col(expr.syntax().text_range()),
        );
        Ok(Expr::Lambda {
            params,
            body: Box::new(return_stmt),
            self_name: None,
        })
    }

    fn lower_unary_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let op = expect_token(&elements, 0)?;
        let rhs = self.lower_expr_element(expect_exprish(&elements, 1)?)?;
        let op = match op.kind() {
            SyntaxKind::Minus => UnaryOp::Neg,
            SyntaxKind::Bang => UnaryOp::Not,
            SyntaxKind::Tilde => UnaryOp::BitNot,
            _ => return Err(self.unsupported_token(&op, "unary operator")),
        };
        Ok(Expr::Unary(op, Box::new(rhs)))
    }

    fn lower_binary_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let lhs = self.lower_expr_element(expect_exprish(&elements, 0)?)?;
        let op = expect_token(&elements, 1)?;
        let rhs = self.lower_expr_element(expect_exprish(&elements, 2)?)?;
        let expr = match op.kind() {
            SyntaxKind::Plus => Expr::Binary(BinaryOp::Add, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::Minus => Expr::Binary(BinaryOp::Sub, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::Star => Expr::Binary(BinaryOp::Mul, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::Slash => Expr::Binary(BinaryOp::Div, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::Percent => Expr::Binary(BinaryOp::Mod, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::Caret => Expr::Binary(BinaryOp::Exp, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::EqEq => Expr::Binary(BinaryOp::Eq, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::BangEq => Expr::Binary(BinaryOp::NEq, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::Lt => Expr::Binary(BinaryOp::Lt, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::LtEq => Expr::Binary(BinaryOp::LtE, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::Gt => Expr::Binary(BinaryOp::Gt, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::GtEq => Expr::Binary(BinaryOp::GtE, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::InKw => Expr::Binary(BinaryOp::In, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::AmpAmp => Expr::And(Box::new(lhs), Box::new(rhs)),
            SyntaxKind::PipePipe => Expr::Or(Box::new(lhs), Box::new(rhs)),
            SyntaxKind::AmpDot => Expr::Binary(BinaryOp::BitAnd, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::PipeDot => Expr::Binary(BinaryOp::BitOr, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::CaretDot => Expr::Binary(BinaryOp::BitXor, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::Shl => Expr::Binary(BinaryOp::BitShl, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::Shr => Expr::Binary(BinaryOp::BitShr, Box::new(lhs), Box::new(rhs)),
            SyntaxKind::LShr => Expr::Binary(BinaryOp::BitLShr, Box::new(lhs), Box::new(rhs)),
            _ => return Err(self.unsupported_token(&op, "binary operator")),
        };
        Ok(expr)
    }

    fn lower_assign_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let lhs = self.lower_expr_element(expect_exprish(&elements, 0)?)?;
        let rhs = self.lower_expr_element(expect_exprish(&elements, 2)?)?;
        if let Expr::Id(name) = &lhs {
            let mut const_symbol = None;
            {
                let decl = self.names.decl_for_mut(name);
                if decl.decl_type == DeclType::Unknown {
                    decl.decl_type = DeclType::Assign;
                }
                if decl.constant {
                    const_symbol = Some(decl.identifier.to_symbol());
                }
            }
            if let Some(symbol) = const_symbol {
                return Err(CompileError::AssignToConst(
                    self.compile_context(node.text_range()),
                    symbol,
                ));
            }
        }
        Ok(Expr::Assign {
            left: Box::new(lhs),
            right: Box::new(rhs),
        })
    }

    fn lower_cond_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        Ok(Expr::Cond {
            condition: Box::new(self.lower_expr_element(expect_exprish(&elements, 0)?)?),
            consequence: Box::new(self.lower_expr_element(expect_exprish(&elements, 2)?)?),
            alternative: Box::new(self.lower_expr_element(expect_exprish(&elements, 4)?)?),
        })
    }

    fn lower_index_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let base = self.lower_expr_element(expect_exprish(&elements, 0)?)?;
        self.enter_dollars_ok();
        let index = self.lower_expr_element(expect_exprish(&elements, 2)?)?;
        self.exit_dollars_ok();
        Ok(Expr::Index(Box::new(base), Box::new(index)))
    }

    fn lower_range_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let base = self.lower_expr_element(expect_exprish(&elements, 0)?)?;
        self.enter_dollars_ok();
        let from = self.lower_expr_element(expect_exprish(&elements, 2)?)?;
        let to = self.lower_expr_element(expect_exprish(&elements, 4)?)?;
        self.exit_dollars_ok();
        Ok(Expr::Range {
            base: Box::new(base),
            from: Box::new(from),
            to: Box::new(to),
        })
    }

    fn lower_call_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let callee = expect_exprish(&elements, 0)?;
        let args = self.lower_args(&elements[2..elements.len().saturating_sub(1)])?;

        match callee {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::ErrorLit => {
                let Expr::Error(error, None) = self.lower_error_literal_token(&token)? else {
                    unreachable!("error literal token did not lower to Expr::Error");
                };
                let message = match args.as_slice() {
                    [] => None,
                    [Normal(expr)] => Some(Box::new(expr.clone())),
                    _ => {
                        return Err(self.make_parse_error(
                            token.text_range(),
                            "frontend lowering error literal",
                            "error literals accept at most one non-spliced argument",
                        ));
                    }
                };
                Ok(Expr::Error(error, message))
            }
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Ident => {
                let function_name = Symbol::mk(token.text());
                let function = if BUILTINS.find_builtin(function_name).is_some() {
                    CallTarget::Builtin(function_name)
                } else {
                    let id = self
                        .names
                        .find_or_add_name_global(token.text(), DeclType::Unknown)
                        .unwrap();
                    CallTarget::Expr(Box::new(Expr::Id(id)))
                };
                Ok(Expr::Call { function, args })
            }
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::SysPropExpr => {
                let callee_expr = self.lower_sysprop_expr(&node)?;
                let Expr::Prop { location, property } = callee_expr else {
                    return Err(self.unsupported_node(&node, "sysprop call lowering"));
                };
                Ok(Expr::Verb {
                    location,
                    verb: property,
                    args,
                })
            }
            _ => Ok(Expr::Call {
                function: CallTarget::Expr(Box::new(self.lower_expr_element(callee)?)),
                args,
            }),
        }
    }

    fn lower_prop_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let location = self.lower_expr_element(expect_exprish(&elements, 0)?)?;
        let property = match expect_exprish(&elements, 2)? {
            NodeOrToken::Token(token) if is_name_like_token(token.kind()) => {
                Expr::Value(v_str(token.text()))
            }
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::LParen => {
                self.lower_expr_element(expect_exprish(&elements, 3)?)?
            }
            other => self.lower_expr_element(other)?,
        };
        Ok(Expr::Prop {
            location: Box::new(location),
            property: Box::new(property),
        })
    }

    fn lower_verb_call_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let location = self.lower_expr_element(expect_exprish(&elements, 0)?)?;
        let colon_idx = elements
            .iter()
            .position(|element| matches!(element, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Colon))
            .ok_or_else(|| {
                self.make_parse_error(
                    node.text_range(),
                    "frontend lowering verb call",
                    "missing ':' in verb call",
                )
            })?;

        let (verb, args_start) = match elements.get(colon_idx + 1) {
            Some(NodeOrToken::Token(token)) if is_name_like_token(token.kind()) => {
                (Expr::Value(v_str(token.text())), colon_idx + 2)
            }
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::LParen => (
                self.lower_expr_element(expect_exprish(&elements, colon_idx + 2)?)?,
                colon_idx + 4,
            ),
            _ => {
                return Err(self.make_parse_error(
                    node.text_range(),
                    "frontend lowering verb call",
                    "missing verb target",
                ));
            }
        };

        let args = self.lower_args(&elements[args_start + 1..elements.len().saturating_sub(1)])?;
        Ok(Expr::Verb {
            location: Box::new(location),
            verb: Box::new(verb),
            args,
        })
    }

    fn lower_pass_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let args = self.lower_args(&elements[2..elements.len().saturating_sub(1)])?;
        Ok(Expr::Pass { args })
    }

    fn lower_sysprop_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let Some(NodeOrToken::Token(name)) = elements.get(1) else {
            return Err(self.make_parse_error(
                node.text_range(),
                "frontend lowering sysprop",
                "missing system property name",
            ));
        };
        Ok(Expr::Prop {
            location: Box::new(Expr::Value(v_obj(SYSTEM_OBJECT))),
            property: Box::new(Expr::Value(v_str(name.text()))),
        })
    }

    fn lower_list_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let args = self.lower_args(&elements[1..elements.len().saturating_sub(1)])?;
        Ok(Expr::List(args))
    }

    fn lower_map_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let mut entries = Vec::new();
        let mut idx = 1;
        let limit = elements.len().saturating_sub(1);
        while idx < limit {
            let key = self.lower_expr_element(expect_exprish(&elements, idx)?)?;
            idx += 2;
            let value = self.lower_expr_element(expect_exprish(&elements, idx)?)?;
            entries.push((key, value));
            idx += 1;
            if idx < limit
                && matches!(elements[idx], NodeOrToken::Token(ref token) if token.kind() == SyntaxKind::Comma)
            {
                idx += 1;
            }
        }
        Ok(Expr::Map(entries))
    }

    fn lower_flyweight_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        if !self.options.flyweight_type {
            return Err(CompileError::DisabledFeature(
                self.compile_context(node.text_range()),
                "Flyweights".to_string(),
            ));
        }

        let elements = significant_elements(node);
        let delegate = self.lower_expr_element(expect_exprish(&elements, 1)?)?;
        let mut slots = Vec::new();
        let mut contents = None;
        let mut idx = 2usize;
        let limit = elements.len().saturating_sub(1);

        while idx < limit {
            if !matches!(elements.get(idx), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Comma)
            {
                idx += 1;
                continue;
            }
            idx += 1;
            if idx >= limit {
                break;
            }

            if matches!(elements.get(idx), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Dot)
            {
                let slot_name = expect_token(&elements, idx + 1)?;
                let symbol = Symbol::mk(slot_name.text());
                if symbol == Symbol::mk("delegate") || symbol == Symbol::mk("slots") {
                    return Err(CompileError::BadSlotName(
                        self.compile_context(slot_name.text_range()),
                        symbol.to_string(),
                    ));
                }
                let value = self.lower_expr_element(expect_exprish(&elements, idx + 3)?)?;
                slots.push((symbol, value));
                idx += 4;
                continue;
            }

            contents = Some(Box::new(
                self.lower_expr_element(expect_exprish(&elements, idx)?)?,
            ));
            break;
        }

        Ok(Expr::Flyweight(Box::new(delegate), slots, contents))
    }

    fn lower_try_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        let bang_idx = elements
            .iter()
            .position(|element| matches!(element, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Bang))
            .ok_or_else(|| {
                self.make_parse_error(
                    node.text_range(),
                    "frontend lowering try expression",
                    "missing '!' in try expression",
                )
            })?;
        let apostrophe_idx = elements
            .iter()
            .rposition(|element| matches!(element, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Apostrophe))
            .ok_or_else(|| {
                self.make_parse_error(
                    node.text_range(),
                    "frontend lowering try expression",
                    "missing closing apostrophe in try expression",
                )
            })?;
        let fat_arrow_idx = elements.iter().position(
            |element| matches!(element, NodeOrToken::Token(token) if token.kind() == SyntaxKind::FatArrow),
        );

        let trye = self.lower_expr_element(expect_exprish(&elements, 1)?)?;
        let codes_end = fat_arrow_idx.unwrap_or(apostrophe_idx);
        let codes = if matches!(elements.get(bang_idx + 1), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::AnyKw)
        {
            CatchCodes::Any
        } else {
            CatchCodes::Codes(self.lower_args(&elements[bang_idx + 1..codes_end])?)
        };
        let except = if let Some(fat_arrow_idx) = fat_arrow_idx {
            Some(Box::new(self.lower_expr_element(expect_exprish(
                &elements,
                fat_arrow_idx + 1,
            )?)?))
        } else {
            None
        };

        Ok(Expr::TryCatch {
            trye: Box::new(trye),
            codes,
            except,
        })
    }

    fn lower_comprehension_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        if !self.options.list_comprehensions {
            return Err(CompileError::DisabledFeature(
                self.compile_context(node.text_range()),
                "ListComprehension".to_string(),
            ));
        }

        let elements = significant_elements(node);
        let variable_ident = expect_token(&elements, 3)?;
        let Some(variable) = self
            .names
            .declare_name(variable_ident.text(), DeclType::For)
        else {
            return Err(CompileError::DuplicateVariable(
                self.compile_context(variable_ident.text_range()),
                Symbol::mk(variable_ident.text()),
            ));
        };
        let producer_expr = self.lower_expr_element(expect_exprish(&elements, 1)?)?;

        match elements.get(5) {
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::LBracket => {
                self.enter_scope();
                let from = self.lower_expr_element(expect_exprish(&elements, 6)?)?;
                let to = self.lower_expr_element(expect_exprish(&elements, 8)?)?;
                let end_of_range_register = self.names.declare_register()?;
                let _ = self.exit_scope();
                Ok(Expr::ComprehendRange {
                    variable,
                    end_of_range_register,
                    producer_expr: Box::new(producer_expr),
                    from: Box::new(from),
                    to: Box::new(to),
                })
            }
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::LParen => {
                self.enter_scope();
                let list = self.lower_expr_element(expect_exprish(&elements, 6)?)?;
                let position_register = self.names.declare_register()?;
                let list_register = self.names.declare_register()?;
                let _ = self.exit_scope();
                Ok(Expr::ComprehendList {
                    variable,
                    position_register,
                    list_register,
                    producer_expr: Box::new(producer_expr),
                    list: Box::new(list),
                })
            }
            _ => Err(self.make_parse_error(
                node.text_range(),
                "frontend lowering comprehension",
                "unsupported comprehension clause",
            )),
        }
    }

    fn single_child_expr(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let child = node
            .children_with_tokens()
            .find(|element| {
                element.kind() != SyntaxKind::LParen
                    && element.kind() != SyntaxKind::RParen
                    && !element.kind().is_trivia()
            })
            .ok_or_else(|| {
                self.make_parse_error(
                    node.text_range(),
                    "frontend lowering",
                    "missing nested expression",
                )
            })?;
        self.lower_expr_element(child)
    }

    fn lower_scatter_assign(
        &mut self,
        scatter: ScatterExpr,
        rhs: Expr,
        local_scope: bool,
        is_const: bool,
    ) -> Result<Expr, CompileError> {
        let mut items = Vec::new();
        let mut seen_rest = false;
        for item in scatter.items() {
            let ast_item = self.lower_scatter_item(item, local_scope, is_const, &mut seen_rest)?;
            items.push(ast_item);
        }
        Ok(Expr::Scatter(items, Box::new(rhs)))
    }

    fn lower_scatter_item(
        &mut self,
        item: ScatterItem,
        local_scope: bool,
        is_const: bool,
        seen_rest: &mut bool,
    ) -> Result<AstScatterItem, CompileError> {
        let elements = significant_elements(item.syntax());
        let mut idx = 0;
        let kind = match elements.first() {
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Question => {
                idx += 1;
                ScatterKind::Optional
            }
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::At => {
                if *seen_rest {
                    return Err(self.make_parse_error(
                        item.syntax().text_range(),
                        "frontend lowering scatter",
                        "More than one `@' target in scattering assignment.",
                    ));
                }
                *seen_rest = true;
                idx += 1;
                ScatterKind::Rest
            }
            _ => ScatterKind::Required,
        };

        let name = expect_token(&elements, idx)?;
        let Some(id) = self
            .names
            .declare(name.text(), is_const, !local_scope, DeclType::Assign)
        else {
            return Err(CompileError::DuplicateVariable(
                self.compile_context(name.text_range()),
                Symbol::mk(name.text()),
            ));
        };
        idx += 1;

        let expr = if idx < elements.len()
            && matches!(elements[idx], NodeOrToken::Token(ref token) if token.kind() == SyntaxKind::Eq)
        {
            Some(self.lower_expr_element(expect_exprish(&elements, idx + 1)?)?)
        } else {
            None
        };

        Ok(AstScatterItem { kind, id, expr })
    }

    fn lower_lambda_params(
        &mut self,
        params: ParamList,
    ) -> Result<Vec<AstScatterItem>, CompileError> {
        let mut items = Vec::new();
        let mut seen_rest = false;
        for item in params.items() {
            let ast_item = self.lower_lambda_param_item(item, &mut seen_rest)?;
            items.push(ast_item);
        }
        Ok(items)
    }

    fn lower_lambda_param_item(
        &mut self,
        item: ScatterItem,
        seen_rest: &mut bool,
    ) -> Result<AstScatterItem, CompileError> {
        let elements = significant_elements(item.syntax());
        let mut idx = 0usize;
        let kind = match elements.first() {
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Question => {
                idx += 1;
                ScatterKind::Optional
            }
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::At => {
                if *seen_rest {
                    return Err(self.make_parse_error(
                        item.syntax().text_range(),
                        "frontend lowering lambda parameters",
                        "More than one `@' target in scattering assignment.",
                    ));
                }
                *seen_rest = true;
                idx += 1;
                ScatterKind::Rest
            }
            _ => ScatterKind::Required,
        };

        let name = expect_token(&elements, idx)?;
        let context = self.compile_context(name.text_range());
        let Some(id) = self
            .names
            .declare(name.text(), false, false, DeclType::Assign)
        else {
            return Err(CompileError::DuplicateVariable(
                context,
                Symbol::mk(name.text()),
            ));
        };
        idx += 1;

        let expr = if idx < elements.len()
            && matches!(elements[idx], NodeOrToken::Token(ref token) if token.kind() == SyntaxKind::Eq)
        {
            Some(self.lower_expr_element(expect_exprish(&elements, idx + 1)?)?)
        } else {
            None
        };

        Ok(AstScatterItem { kind, id, expr })
    }

    fn lower_args(&mut self, elements: &[SyntaxElement]) -> Result<Vec<Arg>, CompileError> {
        let mut args = Vec::new();
        let mut splice = false;
        for element in elements {
            match element {
                NodeOrToken::Token(token) if token.kind().is_trivia() => {}
                NodeOrToken::Token(token) if token.kind() == SyntaxKind::Comma => {}
                NodeOrToken::Token(token) if token.kind() == SyntaxKind::At => {
                    splice = true;
                }
                _ => {
                    let expr = self.lower_expr_element(element.clone())?;
                    args.push(if splice { Splice(expr) } else { Normal(expr) });
                    splice = false;
                }
            }
        }
        Ok(args)
    }

    fn rhs_expr_from_decl(&mut self, node: &SyntaxNode) -> Result<Option<Expr>, CompileError> {
        let elements = significant_elements(node);
        for (idx, element) in elements.iter().enumerate() {
            if matches!(element, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Eq) {
                return Ok(Some(
                    self.lower_expr_element(expect_exprish(&elements, idx + 1)?)?,
                ));
            }
        }
        Ok(None)
    }

    fn scatter_rhs(&mut self, node: &SyntaxNode) -> Result<Expr, CompileError> {
        let elements = significant_elements(node);
        for (idx, element) in elements.iter().enumerate() {
            if matches!(element, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Eq) {
                return self.lower_expr_element(expect_exprish(&elements, idx + 1)?);
            }
        }
        Err(self.make_parse_error(
            node.text_range(),
            "frontend lowering scatter",
            "scatter assignment is missing a right-hand side",
        ))
    }

    fn lower_atom_token(&mut self, token: SyntaxToken) -> Result<Expr, CompileError> {
        match token.kind() {
            SyntaxKind::Ident | SyntaxKind::GlobalKw | SyntaxKind::PassKw | SyntaxKind::AnyKw => {
                let ident = token.text();
                if self.options.legacy_type_constants
                    && let Some(type_id) = VarType::parse_legacy(ident)
                {
                    return Ok(Expr::TypeConstant(type_id));
                }
                let id = if self.lambda_body_depth > 0 {
                    self.names
                        .find_or_add_name_scoped(ident, DeclType::Unknown)
                        .unwrap()
                } else {
                    self.names
                        .find_or_add_name_global(ident, DeclType::Unknown)
                        .unwrap()
                };
                Ok(Expr::Id(id))
            }
            SyntaxKind::IntLit => Ok(Expr::Value(v_int(token.text().parse::<i64>().map_err(
                |e| {
                    CompileError::StringLexError(
                        self.compile_context(token.text_range()),
                        format!("invalid integer literal '{}': {e}", token.text()),
                    )
                },
            )?))),
            SyntaxKind::FloatLit => Ok(Expr::Value(v_float(token.text().parse::<f64>().map_err(
                |e| {
                    CompileError::StringLexError(
                        self.compile_context(token.text_range()),
                        format!("invalid float literal '{}': {e}", token.text()),
                    )
                },
            )?))),
            SyntaxKind::StringLit => {
                let parsed = moor_common::util::unquote_str(token.text()).map_err(|e| {
                    CompileError::StringLexError(
                        self.compile_context(token.text_range()),
                        format!("invalid string literal '{}': {e}", token.text()),
                    )
                })?;
                Ok(Expr::Value(v_str(&parsed)))
            }
            SyntaxKind::BinaryLit => {
                let content = token
                    .text()
                    .strip_prefix("b\"")
                    .and_then(|s| s.strip_suffix('"'))
                    .ok_or_else(|| {
                        CompileError::StringLexError(
                            self.compile_context(token.text_range()),
                            format!("invalid binary literal '{}'", token.text()),
                        )
                    })?;
                let decoded = general_purpose::URL_SAFE.decode(content).map_err(|e| {
                    CompileError::StringLexError(
                        self.compile_context(token.text_range()),
                        format!(
                            "invalid binary literal '{}': invalid base64: {e}",
                            token.text()
                        ),
                    )
                })?;
                Ok(Expr::Value(v_binary(decoded)))
            }
            SyntaxKind::ErrorLit => self.lower_error_literal_token(&token),
            SyntaxKind::ObjectLit => self.lower_object_literal(token),
            SyntaxKind::TypeConstant => {
                let type_id = VarType::parse(token.text()).ok_or_else(|| {
                    CompileError::UnknownTypeConstant(
                        self.compile_context(token.text_range()),
                        token.text().to_string(),
                    )
                })?;
                Ok(Expr::TypeConstant(type_id))
            }
            SyntaxKind::TrueKw => {
                if !self.options.bool_type {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(token.text_range()),
                        "Booleans".to_string(),
                    ));
                }
                Ok(Expr::Value(Var::mk_bool(true)))
            }
            SyntaxKind::FalseKw => {
                if !self.options.bool_type {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(token.text_range()),
                        "Booleans".to_string(),
                    ));
                }
                Ok(Expr::Value(Var::mk_bool(false)))
            }
            SyntaxKind::SymbolLit => {
                if !self.options.symbol_type {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(token.text_range()),
                        "Symbols".to_string(),
                    ));
                }
                Ok(Expr::Value(Var::mk_symbol(Symbol::mk(
                    token.text().trim_start_matches('\''),
                ))))
            }
            SyntaxKind::Dollar => {
                if self.dollars_ok == 0 {
                    return Err(self.make_parse_error(
                        token.text_range(),
                        "range expression",
                        "Illegal context for `$' expression.",
                    ));
                }
                Ok(Expr::Length)
            }
            _ => Err(self.unsupported_token(&token, "frontend atom lowering")),
        }
    }

    fn lower_error_literal_token(&self, token: &SyntaxToken) -> Result<Expr, CompileError> {
        let Some(error) = moor_var::ErrorCode::parse_str(token.text()) else {
            return Err(self.make_parse_error(
                token.text_range(),
                "frontend lowering error literal",
                "invalid error literal",
            ));
        };
        if let moor_var::ErrorCode::ErrCustom(_) = &error
            && !self.options.custom_errors
        {
            return Err(CompileError::DisabledFeature(
                self.compile_context(token.text_range()),
                "CustomErrors".to_string(),
            ));
        }
        Ok(Expr::Error(error, None))
    }

    fn lower_object_literal(&self, token: SyntaxToken) -> Result<Expr, CompileError> {
        let ostr = &token.text()[1..];
        if ostr.starts_with("anon_") && ostr.len() == 22 && ostr.chars().nth(11) == Some('-') {
            let uuid_part = &ostr[5..];
            if let Some((first, second)) = uuid_part.split_once('-') {
                let first_group = u64::from_str_radix(first, 16).unwrap();
                let epoch_ms = u64::from_str_radix(second, 16).unwrap();
                let autoincrement = ((first_group >> 6) & 0xFFFF) as u16;
                let rng = (first_group & 0x3F) as u8;
                let anonymous_id = AnonymousObjid::new(autoincrement, rng, epoch_ms);
                return Ok(Expr::Value(v_obj(Obj::mk_anonymous(anonymous_id))));
            }
        }

        if ostr.len() == 17 && ostr.chars().nth(6) == Some('-') {
            return Ok(Expr::Value(v_obj(Obj::mk_uuobjid(
                UuObjid::from_uuid_string(ostr).unwrap(),
            ))));
        }

        let oid = ostr.parse::<i32>().map_err(|e| {
            CompileError::StringLexError(
                self.compile_context(token.text_range()),
                format!("invalid object ID '{}': {e}", ostr),
            )
        })?;
        Ok(Expr::Value(v_obj(Obj::mk_id(oid))))
    }

    fn enter_scope(&mut self) {
        if self.options.lexical_scopes {
            self.names.enter_new_scope();
        }
    }

    fn exit_scope(&mut self) -> usize {
        if self.options.lexical_scopes {
            return self.names.exit_scope();
        }
        0
    }

    fn enter_dollars_ok(&mut self) {
        self.dollars_ok += 1;
    }

    fn exit_dollars_ok(&mut self) {
        self.dollars_ok = self.dollars_ok.saturating_sub(1);
    }

    fn compile_context(&self, range: TextRange) -> CompileContext {
        CompileContext::new(self.line_col(range))
    }

    fn line_col(&self, range: TextRange) -> (usize, usize) {
        offset_to_line_col(self.source, range.start().into())
    }

    fn make_parse_error(
        &self,
        range: impl Into<TextRange>,
        context: &str,
        message: &str,
    ) -> CompileError {
        let range = range.into();
        let start = usize::from(range.start());
        let end = usize::from(range.end());
        CompileError::ParseError {
            error_position: self.compile_context(range),
            context: context.to_string(),
            end_line_col: Some(offset_to_line_col(self.source, end.saturating_sub(1))),
            message: message.to_string(),
            details: Box::new(ParseErrorDetails {
                span: Some((start, end)),
                expected_tokens: vec![],
                notes: vec![],
            }),
        }
    }

    fn unsupported_node(&self, node: &SyntaxNode, context: &str) -> CompileError {
        self.make_parse_error(
            node.text_range(),
            context,
            &format!("unsupported frontend syntax: {:?}", node.kind()),
        )
    }

    fn unsupported_token(&self, token: &SyntaxToken, context: &str) -> CompileError {
        self.make_parse_error(
            token.text_range(),
            context,
            &format!("unsupported frontend token: {:?}", token.kind()),
        )
    }
}

fn frontend_error_to_compile_error(source: &str, error: &FrontendParseError) -> CompileError {
    let start = error.span.start;
    let end = error.span.end.max(error.span.start);
    CompileError::ParseError {
        error_position: CompileContext::new(offset_to_line_col(source, start)),
        context: "frontend parser".to_string(),
        end_line_col: Some(offset_to_line_col(source, end.saturating_sub(1))),
        message: error.message.clone(),
        details: Box::new(ParseErrorDetails {
            span: Some((start, end)),
            expected_tokens: vec![],
            notes: vec![],
        }),
    }
}

fn inner_scatter_pattern(expr: ScatterExpr) -> ScatterExpr {
    if expr.items().next().is_some() {
        return expr;
    }

    expr.syntax()
        .children()
        .find_map(ScatterExpr::cast)
        .unwrap_or(expr)
}

fn is_name_like_token(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Ident
            | SyntaxKind::IfKw
            | SyntaxKind::ElseKw
            | SyntaxKind::ElseIfKw
            | SyntaxKind::EndIfKw
            | SyntaxKind::ForKw
            | SyntaxKind::EndForKw
            | SyntaxKind::WhileKw
            | SyntaxKind::EndWhileKw
            | SyntaxKind::ForkKw
            | SyntaxKind::EndForkKw
            | SyntaxKind::InKw
            | SyntaxKind::ReturnKw
            | SyntaxKind::BreakKw
            | SyntaxKind::ContinueKw
            | SyntaxKind::TryKw
            | SyntaxKind::ExceptKw
            | SyntaxKind::FinallyKw
            | SyntaxKind::EndTryKw
            | SyntaxKind::FnKw
            | SyntaxKind::EndFnKw
            | SyntaxKind::LetKw
            | SyntaxKind::ConstKw
            | SyntaxKind::GlobalKw
            | SyntaxKind::PassKw
            | SyntaxKind::AnyKw
            | SyntaxKind::TrueKw
            | SyntaxKind::FalseKw
    )
}

fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;
    for ch in source[..offset.min(source.len())].chars() {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn significant_elements(node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|element| !element.kind().is_trivia())
        .collect()
}

fn expect_exprish(elements: &[SyntaxElement], idx: usize) -> Result<SyntaxElement, CompileError> {
    elements
        .get(idx)
        .cloned()
        .ok_or_else(|| CompileError::ParseError {
            error_position: CompileContext::new((1, 1)),
            context: "frontend lowering".to_string(),
            end_line_col: Some((1, 1)),
            message: "missing expression element".to_string(),
            details: Box::new(ParseErrorDetails::default()),
        })
}

fn expect_token(elements: &[SyntaxElement], idx: usize) -> Result<SyntaxToken, CompileError> {
    match elements.get(idx) {
        Some(NodeOrToken::Token(token)) => Ok(token.clone()),
        _ => Err(CompileError::ParseError {
            error_position: CompileContext::new((1, 1)),
            context: "frontend lowering".to_string(),
            end_line_col: Some((1, 1)),
            message: "missing token".to_string(),
            details: Box::new(ParseErrorDetails::default()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_program_frontend;
    use crate::{CompileOptions, ast::render_parse_shape};

    #[test]
    fn lowers_declarations_and_scope_to_canonical_shape() {
        let source = r#"let x = 1 + 2;
const y = ["a" -> x];
global z = x;
x = y["a"];
begin
  let xs = {1, @rest};
  return $foo[1..$];
end"#;
        let parse = parse_program_frontend(source, CompileOptions::default()).unwrap();
        let shape = render_parse_shape(&parse);
        let expected = r#"
(stmts
  (expr
    (decl kind=let id=x
      (binary +
        (value 1)
        (value 2)
      )
    )
  )
  (expr
    (decl kind=const id=y
      (map
        (entry
          (value "a")
          (id x)
        )
      )
    )
  )
  (expr
    (assign
      (id z)
      (id x)
    )
  )
  (expr
    (assign
      (id x)
      (index
        (id y)
        (value "a")
      )
    )
  )
  (scope bindings=1
    (stmts
      (expr
        (decl kind=let id=xs
          (list
            (args
              (arg
                (value 1)
              )
              (splice
                (id rest)
              )
            )
          )
        )
      )
      (expr
        (return
          (range
            (prop
              (value #0)
              (value "foo")
            )
            (value 1)
            (length)
          )
        )
      )
    )
  )
)"#;
        assert_eq!(shape, expected.trim());
    }

    #[test]
    fn lowers_control_flow_to_canonical_shape() {
        let source = "if (a) while loop (b) break loop; endwhile elseif (c) continue; else for x, y in (items) return x; endfor endif";
        let parse = parse_program_frontend(source, CompileOptions::default()).unwrap();
        let shape = render_parse_shape(&parse);
        let expected = r#"
(stmts
  (if
    (arm env=1
      (id a)
      (stmts
        (while label=loop env=0
          (id b)
          (stmts
            (break loop)
          )
        )
      )
    )
    (arm env=0
      (id c)
      (stmts
        (continue _)
      )
    )
    (else env=2
      (stmts
        (for-list value=x key=y env=0
          (id items)
          (stmts
            (expr
              (return
                (id x)
              )
            )
          )
        )
      )
    )
  )
)"#;
        assert_eq!(shape, expected.trim());
    }

    #[test]
    fn lowers_fork_and_try_forms_to_canonical_shape() {
        let source = "fork timer (5) return x; endfork try return x; except err (ANY) return y; except (codes) return z; endtry try return x; finally return y; endtry";
        let parse = parse_program_frontend(source, CompileOptions::default()).unwrap();
        let shape = render_parse_shape(&parse);
        let expected = r#"
(stmts
  (fork label=timer
    (value 5)
    (stmts
      (expr
        (return
          (id x)
        )
      )
    )
  )
  (try-except env=0
    (stmts
      (expr
        (return
          (id x)
        )
      )
    )
    (except id=err
      (codes any)
      (stmts
        (expr
          (return
            (id y)
          )
        )
      )
    )
    (except id=_
      (codes
        (args
          (arg
            (id codes)
          )
        )
      )
      (stmts
        (expr
          (return
            (id z)
          )
        )
      )
    )
  )
  (try-finally env=0
    (body
      (stmts
        (expr
          (return
            (id x)
          )
        )
      )
    )
    (handler
      (stmts
        (expr
          (return
            (id y)
          )
        )
      )
    )
  )
)"#;
        assert_eq!(shape, expected.trim());
    }

    #[test]
    fn lowers_fn_and_lambda_forms_to_canonical_shape() {
        let source = "fn add(a, ?b = 1, @rest) return a + b; endfn value = fn(x) return x; endfn; let f = {?x = 1, @rest} => x + 1; return f;";
        let parse = parse_program_frontend(source, CompileOptions::default()).unwrap();
        let shape = render_parse_shape(&parse);
        let expected = r#"
(stmts
  (expr
    (decl kind=let id=add
      (lambda self=add
        (scatter-items
          (item kind=required id=a
          )
          (item kind=optional id=b
            (value 1)
          )
          (item kind=rest id=rest@1
          )
        )
        (scope bindings=0
          (stmts
            (expr
              (return
                (binary +
                  (id a)
                  (id b)
                )
              )
            )
          )
        )
      )
    )
  )
  (expr
    (decl kind=let id=value
      (lambda self=_
        (scatter-items
          (item kind=required id=x@3
          )
        )
        (scope bindings=0
          (stmts
            (expr
              (return
                (id x@3)
              )
            )
          )
        )
      )
    )
  )
  (expr
    (decl kind=let id=f
      (lambda self=_
        (scatter-items
          (item kind=optional id=x@5
            (value 1)
          )
          (item kind=rest id=rest@5
          )
        )
        (expr
          (return
            (binary +
              (id x@5)
              (value 1)
            )
          )
        )
      )
    )
  )
  (expr
    (return
      (id f)
    )
  )
)"#;
        assert_eq!(shape, expected.trim());
    }

    #[test]
    fn lowers_specialized_expressions_to_canonical_shape() {
        let source = "return foo:bar(1, @args) + `x ! E_PERM, @codes => y ' + <#1, .name = \"x\", .value = 1, {1, 2}>;";
        let parse = parse_program_frontend(source, CompileOptions::default()).unwrap();
        let shape = render_parse_shape(&parse);
        let expected = r#"
(stmts
  (expr
    (return
      (binary +
        (binary +
          (verb
            (id foo)
            (value "bar")
            (args
              (arg
                (value 1)
              )
              (splice
                (id args)
              )
            )
          )
          (try-expr
            (id x)
            (codes
              (args
                (arg
                  (error E_PERM
                  )
                )
                (splice
                  (id codes)
                )
              )
            )
            (except
              (id y)
            )
          )
        )
        (flyweight
          (value #1)
          (slot name
            (value "x")
          )
          (slot value
            (value 1)
          )
          (contents
            (list
              (args
                (arg
                  (value 1)
                )
                (arg
                  (value 2)
                )
              )
            )
          )
        )
      )
    )
  )
)"#;
        assert_eq!(shape, expected.trim());
    }

    #[test]
    fn lowers_comprehensions_to_canonical_shape() {
        let source = "return {item * 2 for item in (items)} + {n for n in [1..limit]};";
        let parse = parse_program_frontend(source, CompileOptions::default()).unwrap();
        let shape = render_parse_shape(&parse);
        let expected = r#"
(stmts
  (expr
    (return
      (binary +
        (comprehend-list var=item pos=<register_0> list-reg=<register_1>
          (binary *
            (id item)
            (value 2)
          )
          (id items)
        )
        (comprehend-range var=n end-reg=<register_2>
          (id n)
          (value 1)
          (id limit)
        )
      )
    )
  )
)"#;
        assert_eq!(shape, expected.trim());
    }
}
