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
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, tokens: &'a [Token]) -> Self {
        Self {
            source,
            tokens,
            cursor: TokenCursor::new(source, tokens),
            builder: CstBuilder::new(),
            emitted: 0,
        }
    }

    fn parse_program(mut self) -> (GreenNode, Vec<ParseError>) {
        self.builder.start_node(SyntaxKind::Program);
        self.builder.start_node(SyntaxKind::StmtList);

        while !self.cursor.at(SyntaxKind::Eof) {
            self.parse_statement();
        }

        self.emit_until(self.tokens.len().saturating_sub(1));
        self.builder.finish_node();
        self.bump_significant();
        self.builder.finish_node();

        let mut errors = self.cursor.into_errors();
        let green = self.builder.finish();
        errors.sort_by(|lhs, rhs| lhs.span.start.cmp(&rhs.span.start));
        (green, errors)
    }

    fn parse_statement(&mut self) {
        self.builder.start_node(SyntaxKind::ExprStmt);

        if self.cursor.at(SyntaxKind::Semi) {
            self.bump_significant();
            self.builder.finish_node();
            return;
        }

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

    fn parse_expr(&mut self) {
        self.parse_expr_bp(1);
    }

    fn parse_expr_bp(&mut self, min_bp: u8) {
        let checkpoint = self.builder.checkpoint();
        self.parse_prefix();
        self.parse_expr_suffix(checkpoint, min_bp);
    }

    fn parse_prefix(&mut self) {
        if matches!(
            self.cursor.current_kind(),
            SyntaxKind::Minus | SyntaxKind::Bang | SyntaxKind::Tilde
        ) {
            let checkpoint = self.builder.checkpoint();
            self.bump_significant();
            self.builder.start_node_at(checkpoint, SyntaxKind::UnaryExpr);
            self.parse_prefix();
            self.builder.finish_node();
            return;
        }

        self.parse_primary();
    }

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
            SyntaxKind::Dollar => {
                self.builder.start_node(SyntaxKind::SysPropExpr);
                self.bump_significant();
                if !self.cursor.bump_if(SyntaxKind::Ident) {
                    self.cursor.push_error("expected identifier after '$'");
                } else {
                    self.emit_to_cursor();
                }
                self.builder.finish_node();
            }
            SyntaxKind::PassKw => {
                let checkpoint = self.builder.checkpoint();
                self.bump_significant();
                if self.cursor.at(SyntaxKind::LParen) {
                    self.builder.start_node_at(checkpoint, SyntaxKind::PassExpr);
                    self.parse_call_arg_list();
                    self.builder.finish_node();
                }
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

    fn parse_expr_suffix(&mut self, checkpoint: rowan::Checkpoint, min_bp: u8) {
        loop {
            if let Some(postfix) = self.postfix_kind() {
                let (left_bp, expr_kind) = postfix_binding_power(postfix);
                if left_bp < min_bp {
                    break;
                }
                match postfix {
                    PostfixOp::Call => {
                        self.builder.start_node_at(checkpoint, expr_kind);
                        self.parse_call_arg_list();
                        self.builder.finish_node();
                    }
                    PostfixOp::Property => {
                        self.builder.start_node_at(checkpoint, expr_kind);
                        self.bump_significant();
                        if self.cursor.bump_if(SyntaxKind::Ident) {
                            self.emit_to_cursor();
                        } else if self.cursor.bump_if(SyntaxKind::LParen) {
                            self.emit_to_cursor();
                            if self.starts_expr() {
                                self.parse_expr_bp(1);
                            } else {
                                self.cursor
                                    .push_error("expected property expression after '.('");
                                self.consume_error_node_until(&[SyntaxKind::RParen, SyntaxKind::Semi]);
                            }
                            if !self.cursor.bump_if(SyntaxKind::RParen) {
                                self.cursor.push_error("expected ')' after property expression");
                            } else {
                                self.emit_to_cursor();
                            }
                        } else {
                            self.cursor.push_error("expected property name after '.'");
                        }
                        self.builder.finish_node();
                    }
                    PostfixOp::Index => self.parse_index_or_range(checkpoint),
                    PostfixOp::VerbCall => {
                        self.builder.start_node_at(checkpoint, expr_kind);
                        self.bump_significant();
                        if self.cursor.bump_if(SyntaxKind::Ident) {
                            self.emit_to_cursor();
                        } else if self.cursor.bump_if(SyntaxKind::LParen) {
                            self.emit_to_cursor();
                            if self.starts_expr() {
                                self.parse_expr_bp(1);
                            } else {
                                self.cursor.push_error("expected verb expression after ':('");
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
                            self.cursor.push_error("expected argument list after verb call");
                        }
                        self.builder.finish_node();
                    }
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
        match self.cursor.current_kind() {
            SyntaxKind::LParen => Some(PostfixOp::Call),
            SyntaxKind::Dot => Some(PostfixOp::Property),
            SyntaxKind::LBracket => Some(PostfixOp::Index),
            SyntaxKind::Colon => Some(PostfixOp::VerbCall),
            _ => None,
        }
    }

    fn infix_binding_power(&self) -> Option<(u8, u8, SyntaxKind, InfixOp)> {
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
            SyntaxKind::Plus | SyntaxKind::Minus => Some((9, 10, SyntaxKind::BinExpr, InfixOp::Binary)),
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
                | SyntaxKind::Dollar
                | SyntaxKind::LParen
                | SyntaxKind::Minus
                | SyntaxKind::Bang
                | SyntaxKind::Tilde
        )
    }
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
        assert!(kinds.iter().filter(|kind| **kind == SyntaxKind::BinExpr).count() >= 2);
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
    fn preserves_trivia_losslessly() {
        let source = "foo /*x*/ (1);\n";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(root.to_string(), source);
    }

    #[test]
    fn unsupported_statement_produces_error_node_and_error() {
        let (root, errors) = parse_to_syntax_node("if (x) y; endif");
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
        assert!(errors.iter().any(|error| error.message.contains("expected ';'")));
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
