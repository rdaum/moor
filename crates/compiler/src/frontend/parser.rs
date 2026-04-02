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

use rowan::GreenNode;

use crate::{SyntaxKind, Token, lex};

use super::{
    cursor::{ParseError, TokenCursor},
    syntax::{CstBuilder, SyntaxNode},
};

pub fn parse_to_cst(source: &str) -> (GreenNode, Vec<ParseError>) {
    let tokens = lex(source);
    Parser::new(source, &tokens).parse_program()
}

pub fn parse_to_syntax_node(source: &str) -> (SyntaxNode, Vec<ParseError>) {
    let (green, errors) = parse_to_cst(source);
    (SyntaxNode::new_root(green), errors)
}

struct Parser<'a> {
    source: &'a str,
    tokens: &'a [Token],
    cursor: TokenCursor<'a>,
    builder: CstBuilder,
    emitted: usize,
    expr_stops: Vec<SyntaxKind>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, tokens: &'a [Token]) -> Self {
        Self {
            source,
            tokens,
            cursor: TokenCursor::new(source, tokens),
            builder: CstBuilder::new(),
            emitted: 0,
            expr_stops: Vec::new(),
        }
    }

    fn parse_program(mut self) -> (GreenNode, Vec<ParseError>) {
        self.builder.start_node(SyntaxKind::Program);
        self.parse_stmt_list_until(&[SyntaxKind::Eof]);

        self.emit_until(self.tokens.len().saturating_sub(1));
        self.bump_significant();
        self.builder.finish_node();

        let mut errors = self.cursor.into_errors();
        let green = self.builder.finish();
        errors.sort_by(|lhs, rhs| lhs.span.start.cmp(&rhs.span.start));
        (green, errors)
    }

    fn parse_stmt_list_until(&mut self, stops: &[SyntaxKind]) {
        self.builder.start_node(SyntaxKind::StmtList);
        while !self.cursor.at(SyntaxKind::Eof) && !stops.contains(&self.cursor.current_kind()) {
            self.parse_statement();
        }
        self.builder.finish_node();
    }

    fn parse_statement(&mut self) {
        if self.cursor.at(SyntaxKind::Semi) {
            self.builder.start_node(SyntaxKind::ExprStmt);
            self.bump_significant();
            self.builder.finish_node();
            return;
        }

        match self.cursor.current_kind() {
            SyntaxKind::LetKw if self.looks_like_decl_statement(SyntaxKind::LetKw) => {
                self.parse_decl_statement(SyntaxKind::LetStmt);
                return;
            }
            SyntaxKind::ConstKw if self.looks_like_decl_statement(SyntaxKind::ConstKw) => {
                self.parse_decl_statement(SyntaxKind::ConstStmt);
                return;
            }
            SyntaxKind::GlobalKw if self.looks_like_decl_statement(SyntaxKind::GlobalKw) => {
                self.parse_decl_statement(SyntaxKind::GlobalStmt);
                return;
            }
            SyntaxKind::FnKw if self.looks_like_fn_statement() => {
                self.parse_fn_statement();
                return;
            }
            SyntaxKind::ReturnKw => {
                self.parse_return_statement();
                return;
            }
            SyntaxKind::ForKw => {
                self.parse_for_statement();
                return;
            }
            SyntaxKind::ForkKw => {
                self.parse_fork_statement();
                return;
            }
            SyntaxKind::BreakKw => {
                self.parse_jump_statement(SyntaxKind::BreakStmt);
                return;
            }
            SyntaxKind::ContinueKw => {
                self.parse_jump_statement(SyntaxKind::ContinueStmt);
                return;
            }
            SyntaxKind::IfKw => {
                self.parse_if_statement();
                return;
            }
            SyntaxKind::WhileKw => {
                self.parse_while_statement();
                return;
            }
            SyntaxKind::TryKw => {
                self.parse_try_statement();
                return;
            }
            _ => {}
        }

        if self.looks_like_begin_statement() {
            self.parse_begin_statement();
            return;
        }

        self.builder.start_node(SyntaxKind::ExprStmt);
        if self.starts_expr() {
            self.parse_expr();
            if !self.cursor.bump_if(SyntaxKind::Semi) {
                self.cursor.push_error("expected ';' after expression");
                self.consume_statement_error_tail();
            } else {
                self.emit_to_cursor();
            }
            self.builder.finish_node();
            return;
        }

        self.consume_unsupported_statement();
        self.builder.finish_node();
    }

    /// `decl-stmt = ('let' | 'const' | 'global') (ident | scatter-pattern) ['=' expr] ';'`
    fn parse_decl_statement(&mut self, stmt_kind: SyntaxKind) {
        self.builder.start_node(stmt_kind);
        self.bump_significant();

        if matches!(stmt_kind, SyntaxKind::LetStmt | SyntaxKind::ConstStmt)
            && self.cursor.at(SyntaxKind::LBrace)
        {
            self.parse_scatter_decl_body();
        } else {
            self.parse_simple_decl_body();
        }

        if self.cursor.bump_if(SyntaxKind::Semi) {
            self.emit_to_cursor();
            self.builder.finish_node();
            return;
        }

        let keyword = match stmt_kind {
            SyntaxKind::LetStmt => "let",
            SyntaxKind::ConstStmt => "const",
            SyntaxKind::GlobalStmt => "global",
            _ => "declaration",
        };
        self.cursor
            .push_error(format!("expected ';' after {keyword}"));
        self.consume_statement_error_tail();
        self.builder.finish_node();
    }

    fn parse_scatter_decl_body(&mut self) {
        self.parse_scatter_pattern();
        if !self.cursor.bump_if(SyntaxKind::Eq) {
            self.cursor
                .push_error("expected '=' after scatter declaration");
        } else {
            self.emit_to_cursor();
        }

        if self.starts_expr() {
            self.parse_expr();
            return;
        }

        self.cursor
            .push_error("expected expression after scatter declaration");
        self.consume_error_node_until(&[SyntaxKind::Semi]);
    }

    fn parse_simple_decl_body(&mut self) {
        if !self.cursor.bump_if(SyntaxKind::Ident) {
            self.cursor.push_error("expected identifier in declaration");
        } else {
            self.emit_to_cursor();
        }

        if !self.cursor.bump_if(SyntaxKind::Eq) {
            return;
        }

        self.emit_to_cursor();
        if self.starts_expr() {
            self.parse_expr();
            return;
        }

        self.cursor
            .push_error("expected initializer expression after '='");
        self.consume_error_node_until(&[SyntaxKind::Semi]);
    }

    fn parse_fn_statement(&mut self) {
        self.builder.start_node(SyntaxKind::FnStmt);
        self.bump_significant();
        if !self.cursor.bump_if(SyntaxKind::Ident) {
            self.cursor.push_error("expected function name after fn");
        } else {
            self.emit_to_cursor();
        }
        self.parse_fn_signature_and_body();
        self.builder.finish_node();
    }

    fn parse_return_statement(&mut self) {
        self.builder.start_node(SyntaxKind::ReturnStmt);
        self.bump_significant();
        if !self.cursor.at(SyntaxKind::Semi) && self.starts_expr() {
            self.parse_expr();
        }
        if !self.cursor.bump_if(SyntaxKind::Semi) {
            self.cursor.push_error("expected ';' after return");
            self.consume_statement_error_tail();
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_jump_statement(&mut self, stmt_kind: SyntaxKind) {
        self.builder.start_node(stmt_kind);
        self.bump_significant();
        if self.cursor.at(SyntaxKind::Ident) {
            self.bump_significant();
        }
        if !self.cursor.bump_if(SyntaxKind::Semi) {
            let keyword = match stmt_kind {
                SyntaxKind::BreakStmt => "break",
                SyntaxKind::ContinueStmt => "continue",
                _ => "jump",
            };
            self.cursor
                .push_error(format!("expected ';' after {keyword}"));
            self.consume_statement_error_tail();
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_if_statement(&mut self) {
        self.builder.start_node(SyntaxKind::IfStmt);
        self.bump_significant();
        self.parse_paren_condition("if");
        self.parse_stmt_list_until(&[
            SyntaxKind::ElseIfKw,
            SyntaxKind::ElseKw,
            SyntaxKind::EndIfKw,
        ]);

        while self.cursor.at(SyntaxKind::ElseIfKw) {
            self.builder.start_node(SyntaxKind::ElseIfClause);
            self.bump_significant();
            self.parse_paren_condition("elseif");
            self.parse_stmt_list_until(&[
                SyntaxKind::ElseIfKw,
                SyntaxKind::ElseKw,
                SyntaxKind::EndIfKw,
            ]);
            self.builder.finish_node();
        }

        if self.cursor.at(SyntaxKind::ElseKw) {
            self.builder.start_node(SyntaxKind::ElseClause);
            self.bump_significant();
            self.parse_stmt_list_until(&[SyntaxKind::EndIfKw]);
            self.builder.finish_node();
        }

        if !self.cursor.bump_if(SyntaxKind::EndIfKw) {
            self.cursor.push_error("expected endif");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_for_statement(&mut self) {
        let stmt_kind = if self.looks_like_for_range() {
            SyntaxKind::ForRangeStmt
        } else {
            SyntaxKind::ForInStmt
        };
        self.builder.start_node(stmt_kind);
        self.bump_significant();

        if !self.cursor.bump_if(SyntaxKind::Ident) {
            self.cursor.push_error("expected loop variable after for");
        } else {
            self.emit_to_cursor();
            if stmt_kind == SyntaxKind::ForInStmt && self.cursor.bump_if(SyntaxKind::Comma) {
                self.emit_to_cursor();
                if !self.cursor.bump_if(SyntaxKind::Ident) {
                    self.cursor.push_error("expected key variable after ','");
                } else {
                    self.emit_to_cursor();
                }
            }
        }

        if !self.cursor.bump_if(SyntaxKind::InKw) {
            self.cursor.push_error("expected in");
        } else {
            self.emit_to_cursor();
        }

        if stmt_kind == SyntaxKind::ForRangeStmt {
            self.parse_range_clause("for");
        } else {
            self.parse_paren_condition("for");
        }

        self.parse_stmt_list_until(&[SyntaxKind::EndForKw]);
        if !self.cursor.bump_if(SyntaxKind::EndForKw) {
            self.cursor.push_error("expected endfor");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_while_statement(&mut self) {
        self.builder.start_node(SyntaxKind::WhileStmt);
        self.bump_significant();
        if self.cursor.at(SyntaxKind::Ident) && self.cursor.nth_kind(1) == SyntaxKind::LParen {
            self.bump_significant();
        }
        self.parse_paren_condition("while");
        self.parse_stmt_list_until(&[SyntaxKind::EndWhileKw]);
        if !self.cursor.bump_if(SyntaxKind::EndWhileKw) {
            self.cursor.push_error("expected endwhile");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_fork_statement(&mut self) {
        self.builder.start_node(SyntaxKind::ForkStmt);
        self.bump_significant();
        if self.cursor.at(SyntaxKind::Ident) && self.cursor.nth_kind(1) == SyntaxKind::LParen {
            self.bump_significant();
        }
        self.parse_paren_condition("fork");
        self.parse_stmt_list_until(&[SyntaxKind::EndForkKw]);
        if !self.cursor.bump_if(SyntaxKind::EndForkKw) {
            self.cursor.push_error("expected endfork");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    /// `try-stmt = 'try' stmt-list { 'except' '(' catch-codes ')' stmt-list } [ 'finally' stmt-list ] 'endtry'`
    fn parse_try_statement(&mut self) {
        self.builder.start_node(SyntaxKind::TryExceptStmt);
        self.bump_significant();
        self.parse_stmt_list_until(&[
            SyntaxKind::ExceptKw,
            SyntaxKind::FinallyKw,
            SyntaxKind::EndTryKw,
        ]);

        if self.cursor.at(SyntaxKind::FinallyKw) {
            self.parse_try_finally_clause();
        } else {
            self.parse_try_except_clauses();
        }

        if !self.cursor.bump_if(SyntaxKind::EndTryKw) {
            self.cursor.push_error("expected endtry");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_try_except_clauses(&mut self) {
        while self.cursor.at(SyntaxKind::ExceptKw) {
            self.parse_except_clause();
        }
    }

    fn parse_try_finally_clause(&mut self) {
        self.builder.start_node(SyntaxKind::TryFinallyStmt);
        self.bump_significant();
        self.parse_stmt_list_until(&[SyntaxKind::EndTryKw]);
        self.builder.finish_node();
    }

    /// `except-clause = 'except' '(' catch-codes ')' stmt-list`
    fn parse_except_clause(&mut self) {
        self.builder.start_node(SyntaxKind::ExceptClause);
        self.bump_significant();

        if self.cursor.at(SyntaxKind::Ident) && self.cursor.nth_kind(1) == SyntaxKind::LParen {
            self.bump_significant();
        }

        if !self.cursor.bump_if(SyntaxKind::LParen) {
            self.cursor.push_error("expected '(' after except");
            self.parse_stmt_list_until(&[
                SyntaxKind::ExceptKw,
                SyntaxKind::FinallyKw,
                SyntaxKind::EndTryKw,
            ]);
            self.builder.finish_node();
            return;
        }

        self.emit_to_cursor();
        self.parse_catch_codes();
        if !self.cursor.bump_if(SyntaxKind::RParen) {
            self.cursor.push_error("expected ')'");
        } else {
            self.emit_to_cursor();
        }

        self.parse_stmt_list_until(&[
            SyntaxKind::ExceptKw,
            SyntaxKind::FinallyKw,
            SyntaxKind::EndTryKw,
        ]);
        self.builder.finish_node();
    }

    fn parse_catch_codes(&mut self) {
        if self.cursor.at(SyntaxKind::AnyKw) {
            self.bump_significant();
            return;
        }

        if !self.starts_expr() {
            self.cursor.push_error("expected catch codes");
            return;
        }

        loop {
            if self.cursor.at(SyntaxKind::At) {
                self.bump_significant();
            }

            if self.starts_expr() {
                self.parse_expr();
            } else {
                self.cursor.push_error("expected catch code expression");
                self.consume_error_node_until(&[
                    SyntaxKind::Comma,
                    SyntaxKind::RParen,
                    SyntaxKind::FinallyKw,
                    SyntaxKind::ExceptKw,
                ]);
                return;
            }

            if !self.cursor.bump_if(SyntaxKind::Comma) {
                return;
            }
            self.emit_to_cursor();
        }
    }

    fn parse_begin_statement(&mut self) {
        self.builder.start_node(SyntaxKind::BeginStmt);
        self.bump_significant();
        self.parse_stmt_list_until_contextual_end("end");
        if !self.at_contextual_ident("end") {
            self.cursor.push_error("expected end");
        } else {
            self.bump_significant();
        }
        self.builder.finish_node();
    }

    fn parse_paren_condition(&mut self, keyword: &str) {
        if !self.cursor.bump_if(SyntaxKind::LParen) {
            self.cursor
                .push_error(format!("expected '(' after {keyword}"));
            return;
        }
        self.emit_to_cursor();
        if self.starts_expr() {
            self.parse_expr();
        } else {
            self.cursor
                .push_error(format!("expected condition expression after {keyword}("));
            self.consume_error_node_until(&[SyntaxKind::RParen]);
        }
        if !self.cursor.bump_if(SyntaxKind::RParen) {
            self.cursor.push_error("expected ')'");
        } else {
            self.emit_to_cursor();
        }
    }

    fn parse_fn_signature_and_body(&mut self) {
        self.builder.start_node(SyntaxKind::ParamList);
        if !self.cursor.bump_if(SyntaxKind::LParen) {
            self.cursor.push_error("expected '(' after fn");
        } else {
            self.emit_to_cursor();
            if !self.cursor.at(SyntaxKind::RParen) {
                loop {
                    self.parse_param_item();
                    if self.cursor.bump_if(SyntaxKind::Comma) {
                        self.emit_to_cursor();
                        continue;
                    }
                    break;
                }
            }
            if !self.cursor.bump_if(SyntaxKind::RParen) {
                self.cursor.push_error("expected ')'");
            } else {
                self.emit_to_cursor();
            }
        }
        self.builder.finish_node();

        self.parse_stmt_list_until(&[SyntaxKind::EndFnKw]);
        if !self.cursor.bump_if(SyntaxKind::EndFnKw) {
            self.cursor.push_error("expected endfn");
        } else {
            self.emit_to_cursor();
        }
    }

    fn parse_param_item(&mut self) {
        self.builder.start_node(SyntaxKind::ScatterItem);
        if self.cursor.at(SyntaxKind::Question) || self.cursor.at(SyntaxKind::At) {
            self.bump_significant();
        }
        if !self.bump_ident_like_name() {
            self.cursor.push_error("expected parameter name");
        }
        if self.cursor.bump_if(SyntaxKind::Eq) {
            self.emit_to_cursor();
            if self.starts_expr() {
                self.parse_expr();
            } else {
                self.cursor
                    .push_error("expected default parameter expression");
                self.consume_error_node_until(&[SyntaxKind::Comma, SyntaxKind::RParen]);
            }
        }
        self.builder.finish_node();
    }

    fn parse_scatter_pattern(&mut self) {
        self.builder.start_node(SyntaxKind::ScatterExpr);
        if !self.cursor.bump_if(SyntaxKind::LBrace) {
            self.cursor
                .push_error("expected '{' to start scatter pattern");
            self.builder.finish_node();
            return;
        }
        self.emit_to_cursor();

        if !self.cursor.at(SyntaxKind::RBrace) {
            loop {
                self.builder.start_node(SyntaxKind::ScatterItem);
                if self.cursor.at(SyntaxKind::Question) || self.cursor.at(SyntaxKind::At) {
                    self.bump_significant();
                }
                if !self.bump_ident_like_name() {
                    self.cursor.push_error("expected scatter target");
                }
                if self.cursor.bump_if(SyntaxKind::Eq) {
                    self.emit_to_cursor();
                    if self.starts_expr() {
                        self.parse_expr();
                    } else {
                        self.cursor
                            .push_error("expected default expression in scatter item");
                        self.consume_error_node_until(&[SyntaxKind::Comma, SyntaxKind::RBrace]);
                    }
                }
                self.builder.finish_node();

                if self.cursor.bump_if(SyntaxKind::Comma) {
                    self.emit_to_cursor();
                    continue;
                }
                break;
            }
        }

        if !self.cursor.bump_if(SyntaxKind::RBrace) {
            self.cursor
                .push_error("expected '}' to end scatter pattern");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_range_clause(&mut self, keyword: &str) {
        if !self.cursor.bump_if(SyntaxKind::LBracket) {
            self.cursor
                .push_error(format!("expected '[' after {keyword}"));
            return;
        }
        self.emit_to_cursor();
        if self.starts_expr() {
            self.parse_expr();
        } else {
            self.cursor.push_error("expected range start expression");
            self.consume_error_node_until(&[SyntaxKind::DotDot, SyntaxKind::RBracket]);
        }

        if !self.cursor.bump_if(SyntaxKind::DotDot) {
            self.cursor.push_error("expected '..'");
        } else {
            self.emit_to_cursor();
        }

        if self.starts_expr() {
            self.parse_expr();
        } else {
            self.cursor.push_error("expected range end expression");
            self.consume_error_node_until(&[SyntaxKind::RBracket]);
        }

        if !self.cursor.bump_if(SyntaxKind::RBracket) {
            self.cursor.push_error("expected ']'");
        } else {
            self.emit_to_cursor();
        }
    }

    fn parse_expr(&mut self) {
        self.parse_expr_bp(1);
    }

    fn bump_ident_like_name(&mut self) -> bool {
        match self.cursor.current_kind() {
            kind if is_name_like_token(kind) => {
                self.bump_significant();
                true
            }
            _ => false,
        }
    }

    fn parse_expr_with_stops(&mut self, stops: &[SyntaxKind]) {
        let saved_len = self.expr_stops.len();
        self.expr_stops.extend_from_slice(stops);
        self.parse_expr();
        self.expr_stops.truncate(saved_len);
    }

    fn parse_expr_bp(&mut self, min_bp: u8) {
        let checkpoint = self.builder.checkpoint();
        self.parse_prefix();
        self.parse_expr_suffix(checkpoint, min_bp);
    }

    fn parse_prefix(&mut self) {
        if self.cursor.at(SyntaxKind::LBrace)
            && matches!(self.classify_brace_form(), BraceForm::ScatterAssign)
        {
            let checkpoint = self.builder.checkpoint();
            self.builder
                .start_node_at(checkpoint, SyntaxKind::ScatterExpr);
            self.parse_scatter_pattern();
            if !self.cursor.bump_if(SyntaxKind::Eq) {
                self.cursor
                    .push_error("expected '=' after scatter assignment");
            } else {
                self.emit_to_cursor();
            }
            if self.starts_expr() {
                self.parse_expr_bp(1);
            } else {
                self.cursor
                    .push_error("expected expression after scatter assignment");
                self.consume_error_node_until(&[
                    SyntaxKind::Semi,
                    SyntaxKind::RParen,
                    SyntaxKind::RBracket,
                    SyntaxKind::RBrace,
                ]);
            }
            self.builder.finish_node();
            return;
        }

        if matches!(
            self.cursor.current_kind(),
            SyntaxKind::Minus | SyntaxKind::Bang | SyntaxKind::Tilde
        ) {
            let checkpoint = self.builder.checkpoint();
            self.bump_significant();
            self.builder
                .start_node_at(checkpoint, SyntaxKind::UnaryExpr);
            self.parse_expr_bp(12);
            self.builder.finish_node();
            return;
        }

        self.parse_primary();
    }

    /// `primary = atom | paren-expr | list | map | lambda | fn-expr | try-expr | sysprop | pass-call`
    fn parse_primary(&mut self) {
        match self.cursor.current_kind() {
            SyntaxKind::LParen => {
                self.builder.start_node(SyntaxKind::ParenExpr);
                self.bump_significant();
                if self.starts_expr() {
                    self.parse_expr();
                } else {
                    self.cursor.push_error("expected expression after '('");
                    self.consume_error_node_until(&[SyntaxKind::RParen, SyntaxKind::Semi]);
                }
                if !self.cursor.bump_if(SyntaxKind::RParen) {
                    self.cursor.push_error("expected ')'");
                } else {
                    self.emit_to_cursor();
                }
                self.builder.finish_node();
            }
            SyntaxKind::LBrace => match self.classify_brace_form() {
                BraceForm::Lambda => self.parse_lambda_literal(),
                BraceForm::Comprehension => self.parse_comprehension_or_list(true),
                BraceForm::List | BraceForm::ScatterAssign => {
                    self.parse_comprehension_or_list(false);
                }
            },
            SyntaxKind::LBracket => {
                self.parse_map_literal();
            }
            SyntaxKind::Lt => {
                self.parse_flyweight_literal();
            }
            SyntaxKind::Backtick => {
                self.parse_try_expr();
            }
            SyntaxKind::Dollar => {
                if !is_name_like_token(self.cursor.nth_kind(1)) {
                    self.bump_significant();
                    return;
                }
                self.builder.start_node(SyntaxKind::SysPropExpr);
                self.bump_significant();
                if self.bump_ident_like_name() {
                    self.emit_to_cursor();
                } else {
                    self.cursor.push_error("expected identifier after '$'");
                }
                self.builder.finish_node();
            }
            SyntaxKind::PassKw => {
                if self.cursor.nth_kind(1) != SyntaxKind::LParen {
                    self.bump_significant();
                    return;
                }
                let checkpoint = self.builder.checkpoint();
                self.bump_significant();
                self.builder.start_node_at(checkpoint, SyntaxKind::PassExpr);
                self.parse_call_arg_list();
                self.builder.finish_node();
            }
            SyntaxKind::FnKw => {
                let checkpoint = self.builder.checkpoint();
                self.bump_significant();
                self.builder
                    .start_node_at(checkpoint, SyntaxKind::LambdaExpr);
                self.parse_fn_signature_and_body();
                self.builder.finish_node();
            }
            SyntaxKind::ReturnKw => {
                self.builder.start_node(SyntaxKind::ReturnStmt);
                self.bump_significant();
                if self.starts_expr() {
                    self.parse_expr();
                }
                self.builder.finish_node();
            }
            kind if is_atom_token(kind) => {
                self.bump_significant();
            }
            _ => {
                self.cursor.push_error("expected expression");
                self.consume_error_node_until(&[SyntaxKind::Semi, SyntaxKind::RParen]);
            }
        }
    }

    fn parse_lambda_literal(&mut self) {
        let checkpoint = self.builder.checkpoint();
        self.builder
            .start_node_at(checkpoint, SyntaxKind::LambdaExpr);
        self.parse_braced_param_list();
        if !self.cursor.bump_if(SyntaxKind::FatArrow) {
            self.cursor.push_error("expected '=>'");
        } else {
            self.emit_to_cursor();
        }
        if self.starts_expr() {
            self.parse_expr();
        } else {
            self.cursor
                .push_error("expected expression after lambda arrow");
            self.consume_error_node_until(&[
                SyntaxKind::Semi,
                SyntaxKind::Comma,
                SyntaxKind::RParen,
                SyntaxKind::RBracket,
                SyntaxKind::RBrace,
            ]);
        }
        self.builder.finish_node();
    }

    fn parse_braced_param_list(&mut self) {
        self.builder.start_node(SyntaxKind::ParamList);
        if !self.cursor.bump_if(SyntaxKind::LBrace) {
            self.cursor.push_error("expected '{' after lambda");
            self.builder.finish_node();
            return;
        }
        self.emit_to_cursor();
        if !self.cursor.at(SyntaxKind::RBrace) {
            loop {
                self.parse_param_item();
                if self.cursor.bump_if(SyntaxKind::Comma) {
                    self.emit_to_cursor();
                    continue;
                }
                break;
            }
        }
        if !self.cursor.bump_if(SyntaxKind::RBrace) {
            self.cursor
                .push_error("expected '}' after lambda parameters");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_comprehension_or_list(&mut self, is_comprehension: bool) {
        let checkpoint = self.builder.checkpoint();
        let expr_kind = if is_comprehension {
            SyntaxKind::ComprehensionExpr
        } else {
            SyntaxKind::ListExpr
        };
        self.builder.start_node_at(checkpoint, expr_kind);
        self.bump_significant();

        if self.cursor.at(SyntaxKind::RBrace) {
            self.bump_significant();
            self.builder.finish_node();
            return;
        }

        if is_comprehension {
            self.parse_expr();
            if !self.cursor.bump_if(SyntaxKind::ForKw) {
                self.cursor.push_error("expected for in comprehension");
            } else {
                self.emit_to_cursor();
            }
            if !self.cursor.bump_if(SyntaxKind::Ident) {
                self.cursor
                    .push_error("expected loop variable in comprehension");
            } else {
                self.emit_to_cursor();
            }
            if !self.cursor.bump_if(SyntaxKind::InKw) {
                self.cursor.push_error("expected in");
            } else {
                self.emit_to_cursor();
            }
            if self.cursor.at(SyntaxKind::LBracket) {
                self.parse_range_clause("for");
            } else if self.cursor.at(SyntaxKind::LParen) {
                self.parse_paren_condition("for");
            } else {
                self.cursor
                    .push_error("expected range or source clause in comprehension");
                self.consume_error_node_until(&[SyntaxKind::RBrace]);
            }
        } else {
            self.parse_list_item();
            while self.cursor.bump_if(SyntaxKind::Comma) {
                self.emit_to_cursor();
                self.parse_list_item();
            }
        }

        if !self.cursor.bump_if(SyntaxKind::RBrace) {
            self.cursor.push_error("expected '}'");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_list_item(&mut self) {
        if self.cursor.at(SyntaxKind::At) {
            self.bump_significant();
        }
        if self.starts_expr() {
            self.parse_expr();
            return;
        }
        self.cursor.push_error("expected list item expression");
        self.consume_error_node_until(&[SyntaxKind::Comma, SyntaxKind::RBrace]);
    }

    fn parse_map_literal(&mut self) {
        let checkpoint = self.builder.checkpoint();
        self.builder.start_node_at(checkpoint, SyntaxKind::MapExpr);
        self.bump_significant();
        if self.cursor.at(SyntaxKind::RBracket) {
            self.bump_significant();
            self.builder.finish_node();
            return;
        }

        loop {
            if self.starts_expr() {
                self.parse_expr();
            } else {
                self.cursor.push_error("expected map key expression");
                self.consume_error_node_until(&[
                    SyntaxKind::Arrow,
                    SyntaxKind::Comma,
                    SyntaxKind::RBracket,
                ]);
            }

            if !self.cursor.bump_if(SyntaxKind::Arrow) {
                self.cursor.push_error("expected '->' in map literal");
            } else {
                self.emit_to_cursor();
            }

            if self.starts_expr() {
                self.parse_expr();
            } else {
                self.cursor.push_error("expected map value expression");
                self.consume_error_node_until(&[SyntaxKind::Comma, SyntaxKind::RBracket]);
            }

            if self.cursor.bump_if(SyntaxKind::Comma) {
                self.emit_to_cursor();
                continue;
            }
            break;
        }

        if !self.cursor.bump_if(SyntaxKind::RBracket) {
            self.cursor.push_error("expected ']'");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_flyweight_literal(&mut self) {
        let checkpoint = self.builder.checkpoint();
        self.builder
            .start_node_at(checkpoint, SyntaxKind::FlyweightExpr);
        self.bump_significant();

        if self.starts_expr() {
            self.parse_expr_with_stops(&[SyntaxKind::Gt]);
        } else {
            self.cursor
                .push_error("expected flyweight delegate expression");
            self.consume_error_node_until(&[SyntaxKind::Comma, SyntaxKind::Gt]);
        }

        while self.cursor.bump_if(SyntaxKind::Comma) {
            self.emit_to_cursor();
            if self.cursor.at(SyntaxKind::Dot) {
                self.bump_significant();
                if !self.cursor.bump_if(SyntaxKind::Ident) {
                    self.cursor.push_error("expected flyweight slot name");
                } else {
                    self.emit_to_cursor();
                }
                if !self.cursor.bump_if(SyntaxKind::Eq) {
                    self.cursor.push_error("expected '=' after flyweight slot");
                } else {
                    self.emit_to_cursor();
                }
                if self.starts_expr() {
                    self.parse_expr_with_stops(&[SyntaxKind::Gt]);
                } else {
                    self.cursor.push_error("expected flyweight slot value");
                    self.consume_error_node_until(&[SyntaxKind::Comma, SyntaxKind::Gt]);
                }
                continue;
            }

            if self.starts_expr() {
                self.parse_expr_with_stops(&[SyntaxKind::Gt]);
            } else {
                self.cursor
                    .push_error("expected flyweight contents expression");
                self.consume_error_node_until(&[SyntaxKind::Gt]);
            }
            break;
        }

        if !self.cursor.bump_if(SyntaxKind::Gt) {
            self.cursor.push_error("expected '>'");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_try_expr(&mut self) {
        let checkpoint = self.builder.checkpoint();
        self.builder.start_node_at(checkpoint, SyntaxKind::TryExpr);
        self.bump_significant();

        if self.starts_expr() {
            self.parse_expr();
        } else {
            self.cursor.push_error("expected expression after '`'");
            self.consume_error_node_until(&[SyntaxKind::Bang, SyntaxKind::Apostrophe]);
        }

        if !self.cursor.bump_if(SyntaxKind::Bang) {
            self.cursor.push_error("expected '!'");
        } else {
            self.emit_to_cursor();
        }

        if self.cursor.at(SyntaxKind::AnyKw) {
            self.bump_significant();
        } else if self.starts_expr() {
            loop {
                if self.cursor.at(SyntaxKind::At) {
                    self.bump_significant();
                }
                if self.starts_expr() {
                    self.parse_expr();
                } else {
                    self.cursor.push_error("expected catch code expression");
                    self.consume_error_node_until(&[
                        SyntaxKind::Comma,
                        SyntaxKind::FatArrow,
                        SyntaxKind::Apostrophe,
                    ]);
                    break;
                }
                if !self.cursor.bump_if(SyntaxKind::Comma) {
                    break;
                }
                self.emit_to_cursor();
            }
        } else {
            self.cursor.push_error("expected catch codes");
        }

        if self.cursor.bump_if(SyntaxKind::FatArrow) {
            self.emit_to_cursor();
            if self.starts_expr() {
                self.parse_expr();
            } else {
                self.cursor
                    .push_error("expected fallback expression after '=>'");
                self.consume_error_node_until(&[SyntaxKind::Apostrophe]);
            }
        }

        if !self.cursor.bump_if(SyntaxKind::Apostrophe) {
            self.cursor
                .push_error("expected closing apostrophe for try expression");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    /// `expr-suffix = { postfix | infix }`
    fn parse_expr_suffix(&mut self, checkpoint: rowan::Checkpoint, min_bp: u8) {
        loop {
            if let Some(postfix) = self.postfix_kind() {
                let (left_bp, expr_kind) = postfix_binding_power(postfix);
                if left_bp < min_bp {
                    break;
                }
                match postfix {
                    PostfixOp::Call => self.parse_call_postfix(checkpoint, expr_kind),
                    PostfixOp::Property => self.parse_property_postfix(checkpoint, expr_kind),
                    PostfixOp::Index => self.parse_index_or_range(checkpoint),
                    PostfixOp::VerbCall => self.parse_verb_call_postfix(checkpoint, expr_kind),
                }
                continue;
            }

            if let Some((left_bp, right_bp, expr_kind, op_kind)) = self.infix_binding_power() {
                if left_bp < min_bp {
                    break;
                }

                self.builder.start_node_at(checkpoint, expr_kind);
                self.bump_significant();

                if matches!(op_kind, InfixOp::Conditional) {
                    if self.starts_expr() {
                        self.parse_expr_bp(1);
                    } else {
                        self.cursor
                            .push_error("expected consequence expression after '?'");
                        self.consume_error_node_until(&[SyntaxKind::Pipe, SyntaxKind::Semi]);
                    }

                    if !self.cursor.bump_if(SyntaxKind::Pipe) {
                        self.cursor.push_error("expected '|'");
                    } else {
                        self.emit_to_cursor();
                    }

                    if self.starts_expr() {
                        self.parse_expr_bp(right_bp);
                    } else {
                        self.cursor
                            .push_error("expected alternative expression after '|'");
                        self.consume_error_node_until(&[SyntaxKind::Semi, SyntaxKind::RParen]);
                    }
                    self.builder.finish_node();
                    continue;
                }

                if self.starts_expr() {
                    self.parse_expr_bp(right_bp);
                } else {
                    self.cursor.push_error("expected expression after operator");
                    self.consume_error_node_until(&[SyntaxKind::Semi, SyntaxKind::RParen]);
                }
                self.builder.finish_node();
                continue;
            }

            break;
        }
    }

    fn parse_call_postfix(&mut self, checkpoint: rowan::Checkpoint, expr_kind: SyntaxKind) {
        self.builder.start_node_at(checkpoint, expr_kind);
        self.parse_call_arg_list();
        self.builder.finish_node();
    }

    fn parse_property_postfix(&mut self, checkpoint: rowan::Checkpoint, expr_kind: SyntaxKind) {
        self.builder.start_node_at(checkpoint, expr_kind);
        self.bump_significant();
        if self.bump_ident_like_name() {
            self.builder.finish_node();
            return;
        }

        if !self.cursor.bump_if(SyntaxKind::LParen) {
            self.cursor.push_error("expected property name after '.'");
            self.builder.finish_node();
            return;
        }

        self.emit_to_cursor();
        if self.starts_expr() {
            self.parse_expr_bp(1);
        } else {
            self.cursor
                .push_error("expected property expression after '.('");
            self.consume_error_node_until(&[SyntaxKind::RParen, SyntaxKind::Semi]);
        }
        if !self.cursor.bump_if(SyntaxKind::RParen) {
            self.cursor
                .push_error("expected ')' after property expression");
        } else {
            self.emit_to_cursor();
        }
        self.builder.finish_node();
    }

    fn parse_verb_call_postfix(&mut self, checkpoint: rowan::Checkpoint, expr_kind: SyntaxKind) {
        self.builder.start_node_at(checkpoint, expr_kind);
        self.bump_significant();
        if self.bump_ident_like_name() {
            // direct verb name
        } else if self.cursor.bump_if(SyntaxKind::LParen) {
            self.emit_to_cursor();
            if self.starts_expr() {
                self.parse_expr_bp(1);
            } else {
                self.cursor
                    .push_error("expected verb expression after ':('");
                self.consume_error_node_until(&[SyntaxKind::RParen, SyntaxKind::Semi]);
            }
            if !self.cursor.bump_if(SyntaxKind::RParen) {
                self.cursor.push_error("expected ')' after verb expression");
            } else {
                self.emit_to_cursor();
            }
        } else {
            self.cursor.push_error("expected verb name after ':'");
        }

        if self.cursor.at(SyntaxKind::LParen) {
            self.parse_call_arg_list();
        } else {
            self.cursor
                .push_error("expected argument list after verb call");
        }
        self.builder.finish_node();
    }

    /// `index = '[' expr [ '..' expr ] ']'`
    fn parse_index_or_range(&mut self, checkpoint: rowan::Checkpoint) {
        self.bump_significant();
        if self.starts_expr() {
            self.parse_expr_bp(1);
        } else {
            self.cursor.push_error("expected expression after '['");
            self.consume_error_node_until(&[
                SyntaxKind::DotDot,
                SyntaxKind::RBracket,
                SyntaxKind::Semi,
            ]);
        }

        let expr_kind = if self.cursor.bump_if(SyntaxKind::DotDot) {
            self.emit_to_cursor();
            if self.starts_expr() {
                self.parse_expr_bp(1);
            } else {
                self.cursor.push_error("expected range end expression");
                self.consume_error_node_until(&[SyntaxKind::RBracket, SyntaxKind::Semi]);
            }
            SyntaxKind::RangeExpr
        } else {
            SyntaxKind::IndexExpr
        };

        if !self.cursor.bump_if(SyntaxKind::RBracket) {
            self.cursor.push_error("expected ']'");
        } else {
            self.emit_to_cursor();
        }

        self.builder.start_node_at(checkpoint, expr_kind);
        self.builder.finish_node();
    }

    fn postfix_kind(&self) -> Option<PostfixOp> {
        if self.expr_stops.contains(&self.cursor.current_kind()) {
            return None;
        }
        match self.cursor.current_kind() {
            SyntaxKind::LParen => Some(PostfixOp::Call),
            SyntaxKind::Dot => Some(PostfixOp::Property),
            SyntaxKind::LBracket => Some(PostfixOp::Index),
            SyntaxKind::Colon => Some(PostfixOp::VerbCall),
            _ => None,
        }
    }

    fn infix_binding_power(&self) -> Option<(u8, u8, SyntaxKind, InfixOp)> {
        if self.expr_stops.contains(&self.cursor.current_kind()) {
            return None;
        }
        match self.cursor.current_kind() {
            SyntaxKind::Eq => Some((1, 1, SyntaxKind::AssignExpr, InfixOp::Assign)),
            SyntaxKind::Question => Some((2, 2, SyntaxKind::CondExpr, InfixOp::Conditional)),
            SyntaxKind::PipePipe => Some((3, 4, SyntaxKind::BinExpr, InfixOp::Binary)),
            SyntaxKind::AmpAmp => Some((3, 4, SyntaxKind::BinExpr, InfixOp::Binary)),
            SyntaxKind::PipeDot => Some((4, 5, SyntaxKind::BinExpr, InfixOp::Binary)),
            SyntaxKind::CaretDot => Some((5, 6, SyntaxKind::BinExpr, InfixOp::Binary)),
            SyntaxKind::AmpDot => Some((6, 7, SyntaxKind::BinExpr, InfixOp::Binary)),
            SyntaxKind::EqEq
            | SyntaxKind::BangEq
            | SyntaxKind::Lt
            | SyntaxKind::Gt
            | SyntaxKind::LtEq
            | SyntaxKind::GtEq
            | SyntaxKind::InKw => Some((7, 8, SyntaxKind::BinExpr, InfixOp::Binary)),
            SyntaxKind::Shl | SyntaxKind::Shr | SyntaxKind::LShr => {
                Some((8, 9, SyntaxKind::BinExpr, InfixOp::Binary))
            }
            SyntaxKind::Plus | SyntaxKind::Minus => {
                Some((9, 10, SyntaxKind::BinExpr, InfixOp::Binary))
            }
            SyntaxKind::Star | SyntaxKind::Slash | SyntaxKind::Percent => {
                Some((10, 11, SyntaxKind::BinExpr, InfixOp::Binary))
            }
            SyntaxKind::Caret => Some((11, 11, SyntaxKind::BinExpr, InfixOp::Binary)),
            _ => None,
        }
    }

    fn parse_call_arg_list(&mut self) {
        self.bump_significant();

        if self.cursor.at(SyntaxKind::RParen) {
            self.bump_significant();
            return;
        }

        loop {
            if self.cursor.at(SyntaxKind::At) {
                self.bump_significant();
            }

            if self.starts_expr() {
                self.parse_expr();
            } else {
                self.cursor.push_error("expected argument expression");
                self.consume_error_node_until(&[
                    SyntaxKind::Comma,
                    SyntaxKind::RParen,
                    SyntaxKind::Semi,
                ]);
            }

            if self.cursor.bump_if(SyntaxKind::Comma) {
                self.emit_to_cursor();
                continue;
            }

            if self.cursor.bump_if(SyntaxKind::RParen) {
                self.emit_to_cursor();
                return;
            }

            self.cursor.push_error("expected ',' or ')'");
            self.consume_error_node_until(&[
                SyntaxKind::Comma,
                SyntaxKind::RParen,
                SyntaxKind::Semi,
            ]);

            if self.cursor.bump_if(SyntaxKind::Comma) {
                self.emit_to_cursor();
                continue;
            }
            if self.cursor.bump_if(SyntaxKind::RParen) {
                self.emit_to_cursor();
                return;
            }
            return;
        }
    }

    fn consume_unsupported_statement(&mut self) {
        self.cursor.push_error(format!(
            "unsupported statement start: {:?}",
            self.cursor.current_kind()
        ));
        self.consume_error_node_until(&[SyntaxKind::Semi]);
        if self.cursor.bump_if(SyntaxKind::Semi) {
            self.emit_to_cursor();
        }
    }

    fn consume_statement_error_tail(&mut self) {
        self.consume_error_node_until(&[SyntaxKind::Semi]);
        if self.cursor.bump_if(SyntaxKind::Semi) {
            self.emit_to_cursor();
        }
    }

    fn consume_error_node_until(&mut self, stops: &[SyntaxKind]) {
        self.builder.start_node(SyntaxKind::Error);
        while !self.cursor.at(SyntaxKind::Eof) && !stops.contains(&self.cursor.current_kind()) {
            self.bump_significant();
        }
        self.builder.finish_node();
    }

    fn bump_significant(&mut self) {
        let _ = self.cursor.bump();
        self.emit_to_cursor();
    }

    fn emit_to_cursor(&mut self) {
        let target = self.cursor.raw_index();
        self.emit_until(target);
    }

    fn emit_until(&mut self, exclusive_end: usize) {
        while self.emitted < exclusive_end {
            let token = &self.tokens[self.emitted];
            self.builder
                .token(token.kind, &self.source[token.span.clone()]);
            self.emitted += 1;
        }
    }

    fn parse_stmt_list_until_contextual_end(&mut self, stop_ident: &str) {
        self.builder.start_node(SyntaxKind::StmtList);
        while !self.cursor.at(SyntaxKind::Eof) && !self.at_contextual_ident(stop_ident) {
            self.parse_statement();
        }
        self.builder.finish_node();
    }

    fn at_contextual_ident(&self, ident: &str) -> bool {
        self.cursor.at(SyntaxKind::Ident) && self.cursor.current_text().eq_ignore_ascii_case(ident)
    }

    fn classify_brace_form(&self) -> BraceForm {
        let open_idx = self.cursor.current_raw_index();
        let Some(close_idx) =
            self.find_matching_delim(open_idx, SyntaxKind::LBrace, SyntaxKind::RBrace)
        else {
            return BraceForm::List;
        };

        match self.peek_significant_kind_from(close_idx + 1) {
            Some(SyntaxKind::FatArrow) => return BraceForm::Lambda,
            Some(SyntaxKind::Eq) => return BraceForm::ScatterAssign,
            _ => {}
        }

        let mut idx = open_idx + 1;
        let mut paren_depth = 0usize;
        let mut brace_depth = 0usize;
        let mut bracket_depth = 0usize;
        while idx < close_idx {
            let kind = self.tokens[idx].kind;
            if kind.is_trivia() {
                idx += 1;
                continue;
            }
            match kind {
                SyntaxKind::LParen => paren_depth += 1,
                SyntaxKind::RParen => paren_depth = paren_depth.saturating_sub(1),
                SyntaxKind::LBrace => brace_depth += 1,
                SyntaxKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
                SyntaxKind::LBracket => bracket_depth += 1,
                SyntaxKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                SyntaxKind::ForKw if paren_depth == 0 && brace_depth == 0 && bracket_depth == 0 => {
                    return BraceForm::Comprehension;
                }
                _ => {}
            }
            idx += 1;
        }

        BraceForm::List
    }

    fn peek_significant_kind_from(&self, start: usize) -> Option<SyntaxKind> {
        self.tokens[start..]
            .iter()
            .find(|token| !token.kind.is_trivia())
            .map(|token| token.kind)
    }

    fn find_matching_delim(
        &self,
        start: usize,
        open: SyntaxKind,
        close: SyntaxKind,
    ) -> Option<usize> {
        let mut depth = 0usize;
        for (idx, token) in self.tokens.iter().enumerate().skip(start) {
            if token.kind.is_trivia() {
                continue;
            }
            if token.kind == open {
                depth += 1;
                continue;
            }
            if token.kind == close {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx);
                }
            }
        }
        None
    }

    fn looks_like_for_range(&self) -> bool {
        self.cursor.nth_kind(1) == SyntaxKind::Ident
            && self.cursor.nth_kind(2) == SyntaxKind::InKw
            && self.cursor.nth_kind(3) == SyntaxKind::LBracket
    }

    fn looks_like_decl_statement(&self, keyword: SyntaxKind) -> bool {
        match keyword {
            SyntaxKind::LetKw | SyntaxKind::ConstKw => {
                matches!(
                    self.cursor.nth_kind(1),
                    SyntaxKind::Ident | SyntaxKind::LBrace
                )
            }
            SyntaxKind::GlobalKw => self.cursor.nth_kind(1) == SyntaxKind::Ident,
            _ => false,
        }
    }

    fn looks_like_fn_statement(&self) -> bool {
        self.cursor.nth_kind(1) == SyntaxKind::Ident
            && self.cursor.nth_kind(2) == SyntaxKind::LParen
    }

    fn looks_like_begin_statement(&self) -> bool {
        if !self.at_contextual_ident("begin") {
            return false;
        }
        !matches!(
            self.cursor.nth_kind(1),
            SyntaxKind::Semi
                | SyntaxKind::Eq
                | SyntaxKind::Dot
                | SyntaxKind::Colon
                | SyntaxKind::LParen
                | SyntaxKind::LBracket
                | SyntaxKind::Question
        )
    }

    fn starts_expr(&self) -> bool {
        matches!(
            self.cursor.current_kind(),
            SyntaxKind::Ident
                | SyntaxKind::IntLit
                | SyntaxKind::FloatLit
                | SyntaxKind::StringLit
                | SyntaxKind::ObjectLit
                | SyntaxKind::ErrorLit
                | SyntaxKind::SymbolLit
                | SyntaxKind::BinaryLit
                | SyntaxKind::TypeConstant
                | SyntaxKind::TrueKw
                | SyntaxKind::FalseKw
                | SyntaxKind::PassKw
                | SyntaxKind::ReturnKw
                | SyntaxKind::FnKw
                | SyntaxKind::GlobalKw
                | SyntaxKind::Dollar
                | SyntaxKind::LParen
                | SyntaxKind::LBrace
                | SyntaxKind::LBracket
                | SyntaxKind::Lt
                | SyntaxKind::Backtick
                | SyntaxKind::Minus
                | SyntaxKind::Bang
                | SyntaxKind::Tilde
        )
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PostfixOp {
    Call,
    Property,
    Index,
    VerbCall,
}

fn postfix_binding_power(op: PostfixOp) -> (u8, SyntaxKind) {
    match op {
        PostfixOp::Call => (12, SyntaxKind::CallExpr),
        PostfixOp::Property => (12, SyntaxKind::PropExpr),
        PostfixOp::Index => (12, SyntaxKind::IndexExpr),
        PostfixOp::VerbCall => (12, SyntaxKind::VerbCallExpr),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InfixOp {
    Assign,
    Conditional,
    Binary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BraceForm {
    Lambda,
    ScatterAssign,
    Comprehension,
    List,
}

fn is_atom_token(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Ident
            | SyntaxKind::IntLit
            | SyntaxKind::FloatLit
            | SyntaxKind::StringLit
            | SyntaxKind::ObjectLit
            | SyntaxKind::ErrorLit
            | SyntaxKind::SymbolLit
            | SyntaxKind::BinaryLit
            | SyntaxKind::TypeConstant
            | SyntaxKind::TrueKw
            | SyntaxKind::FalseKw
            | SyntaxKind::GlobalKw
    )
}

#[cfg(test)]
mod tests {
    use crate::SyntaxKind;

    use super::parse_to_syntax_node;

    #[test]
    fn parses_empty_statement_program() {
        let (root, errors) = parse_to_syntax_node(";");
        assert!(errors.is_empty());
        assert_eq!(root.kind(), SyntaxKind::Program);
        let stmt_list = root.first_child().unwrap();
        assert_eq!(stmt_list.kind(), SyntaxKind::StmtList);
        let stmt = stmt_list.first_child().unwrap();
        assert_eq!(stmt.kind(), SyntaxKind::ExprStmt);
    }

    #[test]
    fn parses_call_property_and_index_chain() {
        let (root, errors) = parse_to_syntax_node("foo(1).bar[2];");
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::CallExpr));
        assert!(kinds.contains(&SyntaxKind::PropExpr));
        assert!(kinds.contains(&SyntaxKind::IndexExpr));
    }

    #[test]
    fn parses_binary_precedence_and_unary() {
        let (root, errors) = parse_to_syntax_node("-a + b * c;");
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::UnaryExpr));
        assert!(
            kinds
                .iter()
                .filter(|kind| **kind == SyntaxKind::BinExpr)
                .count()
                >= 2
        );
    }

    #[test]
    fn parses_assignment_and_conditional() {
        let (root, errors) = parse_to_syntax_node("a = b ? c | d;");
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::AssignExpr));
        assert!(kinds.contains(&SyntaxKind::CondExpr));
    }

    #[test]
    fn parses_verb_call_and_range_index() {
        let (root, errors) = parse_to_syntax_node("obj:verb(x)[1..2];");
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::VerbCallExpr));
        assert!(kinds.contains(&SyntaxKind::RangeExpr));
    }

    #[test]
    fn parses_return_break_and_continue_statements() {
        let (root, errors) = parse_to_syntax_node("return x; break loop; continue;");
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::ReturnStmt));
        assert!(kinds.contains(&SyntaxKind::BreakStmt));
        assert!(kinds.contains(&SyntaxKind::ContinueStmt));
    }

    #[test]
    fn parses_if_elseif_else_blocks() {
        let source = "if (a) return b; elseif (c) return d; else return e; endif";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::IfStmt));
        assert!(kinds.contains(&SyntaxKind::ElseIfClause));
        assert!(kinds.contains(&SyntaxKind::ElseClause));
    }

    #[test]
    fn parses_labelled_while_block() {
        let source = "while loop (a < b) x = x + 1; endwhile";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::WhileStmt));
        assert!(kinds.contains(&SyntaxKind::AssignExpr));
        assert!(kinds.contains(&SyntaxKind::BinExpr));
    }

    #[test]
    fn parses_for_range_and_for_in_blocks() {
        let source = "for x in [1..5] return x; endfor for a, b in (items) return a; endfor";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::ForRangeStmt));
        assert!(kinds.contains(&SyntaxKind::ForInStmt));
    }

    #[test]
    fn parses_fork_statement() {
        let source = "fork timer (5) return x; endfork";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::ForkStmt));
        assert!(kinds.contains(&SyntaxKind::ReturnStmt));
    }

    #[test]
    fn parses_try_except_and_try_finally_blocks() {
        let except_src = "try return x; except (ANY) return y; endtry";
        let (except_root, except_errors) = parse_to_syntax_node(except_src);
        assert!(except_errors.is_empty(), "{except_errors:?}");
        let except_kinds: Vec<_> = except_root.descendants().map(|node| node.kind()).collect();
        assert!(except_kinds.contains(&SyntaxKind::TryExceptStmt));
        assert!(except_kinds.contains(&SyntaxKind::ExceptClause));

        let finally_src = "try return x; finally return y; endtry";
        let (finally_root, finally_errors) = parse_to_syntax_node(finally_src);
        assert!(finally_errors.is_empty(), "{finally_errors:?}");
        let finally_kinds: Vec<_> = finally_root.descendants().map(|node| node.kind()).collect();
        assert!(finally_kinds.contains(&SyntaxKind::TryExceptStmt));
        assert!(finally_kinds.contains(&SyntaxKind::TryFinallyStmt));
    }

    #[test]
    fn parses_contextual_begin_end_block() {
        let source = "begin return x; end";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::BeginStmt));
        assert!(kinds.contains(&SyntaxKind::ReturnStmt));
    }

    #[test]
    fn parses_let_const_and_global_declarations() {
        let source = "let x = 1; const y = 2; global z = 3;";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::LetStmt));
        assert!(kinds.contains(&SyntaxKind::ConstStmt));
        assert!(kinds.contains(&SyntaxKind::GlobalStmt));
    }

    #[test]
    fn parses_scatter_declarations() {
        let source = "let {?a = 1, @rest} = value; const {x, y} = pair;";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::ScatterExpr));
        assert!(
            kinds
                .iter()
                .filter(|kind| **kind == SyntaxKind::ScatterItem)
                .count()
                >= 3
        );
    }

    #[test]
    fn parses_named_fn_statement_and_fn_expression() {
        let source = "fn add(a, ?b = 1, @rest) return a + b; endfn value = fn(x) return x; endfn;";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::FnStmt));
        assert!(kinds.contains(&SyntaxKind::LambdaExpr));
        assert!(kinds.contains(&SyntaxKind::ParamList));
    }

    #[test]
    fn parses_remaining_primary_expression_forms() {
        let source = "{?x = 1, @rest} => x; {1, @items}; {x for item in (items)}; [a -> 1, b -> 2]; <parent, .slot = value, contents>; `foo ! ANY => bar';";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(kinds.contains(&SyntaxKind::LambdaExpr));
        assert!(kinds.contains(&SyntaxKind::ListExpr));
        assert!(kinds.contains(&SyntaxKind::ComprehensionExpr));
        assert!(kinds.contains(&SyntaxKind::MapExpr));
        assert!(kinds.contains(&SyntaxKind::FlyweightExpr));
        assert!(kinds.contains(&SyntaxKind::TryExpr));
    }

    #[test]
    fn parses_scatter_assignment_and_range_end() {
        let source = "{x, ?y = 1, @rest} = value; list[1..$];";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        let kinds: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert!(
            kinds
                .iter()
                .filter(|kind| **kind == SyntaxKind::ScatterExpr)
                .count()
                >= 2
        );
        assert!(kinds.contains(&SyntaxKind::RangeExpr));
    }

    #[test]
    fn missing_endif_reports_error() {
        let (_root, errors) = parse_to_syntax_node("if (a) return b;");
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|error| error.message.contains("endif")));
    }

    #[test]
    fn preserves_trivia_losslessly() {
        let source = "foo /*x*/ (1);\n";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(root.to_string(), source);
    }

    #[test]
    fn unsupported_statement_produces_error_node_and_error() {
        let (root, errors) = parse_to_syntax_node("elseif (x) return y;");
        assert!(!errors.is_empty());
        let has_error = root
            .descendants()
            .any(|node| node.kind() == SyntaxKind::Error);
        assert!(has_error);
    }

    #[test]
    fn missing_semi_reports_error() {
        let (_root, errors) = parse_to_syntax_node("foo(1)");
        assert!(!errors.is_empty());
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("expected ';'"))
        );
    }

    #[test]
    fn parses_sysprop_and_pass_calls() {
        let (root, errors) = parse_to_syntax_node("$foo; pass(1, @bar);");
        assert!(errors.is_empty(), "{errors:?}");
        let has_sysprop = root
            .descendants()
            .any(|node| node.kind() == SyntaxKind::SysPropExpr);
        let has_pass = root
            .descendants()
            .any(|node| node.kind() == SyntaxKind::PassExpr);
        assert!(has_sysprop);
        assert!(has_pass);
    }

    #[test]
    fn typed_root_round_trips() {
        let (root, errors) = parse_to_syntax_node("(foo);");
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(root.kind(), SyntaxKind::Program);
        let parens = root
            .descendants()
            .find(|node| node.kind() == SyntaxKind::ParenExpr);
        assert!(parens.is_some());
    }
}
