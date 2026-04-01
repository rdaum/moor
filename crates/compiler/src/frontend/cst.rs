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

//! Typed CST wrappers for the handwritten frontend parser.

use rowan::{
    NodeOrToken,
    ast::{AstChildren, AstNode, support},
};

use crate::SyntaxKind;

use super::syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

macro_rules! define_ast_node {
    ($name:ident, $kind:path) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            syntax: SyntaxNode,
        }

        impl AstNode for $name {
            type Language = super::syntax::MooLanguage;

            fn can_cast(kind: SyntaxKind) -> bool {
                kind == $kind
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                if Self::can_cast(syntax.kind()) {
                    Some(Self { syntax })
                } else {
                    None
                }
            }

            fn syntax(&self) -> &SyntaxNode {
                &self.syntax
            }
        }
    };
}

define_ast_node!(Program, SyntaxKind::Program);
define_ast_node!(StmtList, SyntaxKind::StmtList);
define_ast_node!(IfStmt, SyntaxKind::IfStmt);
define_ast_node!(ElseIfClause, SyntaxKind::ElseIfClause);
define_ast_node!(ElseClause, SyntaxKind::ElseClause);
define_ast_node!(ForInStmt, SyntaxKind::ForInStmt);
define_ast_node!(ForRangeStmt, SyntaxKind::ForRangeStmt);
define_ast_node!(WhileStmt, SyntaxKind::WhileStmt);
define_ast_node!(ForkStmt, SyntaxKind::ForkStmt);
define_ast_node!(TryExceptStmt, SyntaxKind::TryExceptStmt);
define_ast_node!(TryFinallyStmt, SyntaxKind::TryFinallyStmt);
define_ast_node!(ExceptClause, SyntaxKind::ExceptClause);
define_ast_node!(ReturnStmt, SyntaxKind::ReturnStmt);
define_ast_node!(BreakStmt, SyntaxKind::BreakStmt);
define_ast_node!(ContinueStmt, SyntaxKind::ContinueStmt);
define_ast_node!(ExprStmt, SyntaxKind::ExprStmt);
define_ast_node!(BeginStmt, SyntaxKind::BeginStmt);
define_ast_node!(FnStmt, SyntaxKind::FnStmt);
define_ast_node!(LetStmt, SyntaxKind::LetStmt);
define_ast_node!(ConstStmt, SyntaxKind::ConstStmt);
define_ast_node!(GlobalStmt, SyntaxKind::GlobalStmt);
define_ast_node!(BinExpr, SyntaxKind::BinExpr);
define_ast_node!(UnaryExpr, SyntaxKind::UnaryExpr);
define_ast_node!(ParenExpr, SyntaxKind::ParenExpr);
define_ast_node!(CondExpr, SyntaxKind::CondExpr);
define_ast_node!(IndexExpr, SyntaxKind::IndexExpr);
define_ast_node!(RangeExpr, SyntaxKind::RangeExpr);
define_ast_node!(CallExpr, SyntaxKind::CallExpr);
define_ast_node!(VerbCallExpr, SyntaxKind::VerbCallExpr);
define_ast_node!(PropExpr, SyntaxKind::PropExpr);
define_ast_node!(AssignExpr, SyntaxKind::AssignExpr);
define_ast_node!(ScatterExpr, SyntaxKind::ScatterExpr);
define_ast_node!(ListExpr, SyntaxKind::ListExpr);
define_ast_node!(MapExpr, SyntaxKind::MapExpr);
define_ast_node!(FlyweightExpr, SyntaxKind::FlyweightExpr);
define_ast_node!(LambdaExpr, SyntaxKind::LambdaExpr);
define_ast_node!(TryExpr, SyntaxKind::TryExpr);
define_ast_node!(PassExpr, SyntaxKind::PassExpr);
define_ast_node!(SysPropExpr, SyntaxKind::SysPropExpr);
define_ast_node!(ComprehensionExpr, SyntaxKind::ComprehensionExpr);
define_ast_node!(ParamList, SyntaxKind::ParamList);
define_ast_node!(ScatterItem, SyntaxKind::ScatterItem);

macro_rules! define_ast_enum {
    ($name:ident { $($variant:ident($ty:ident)),+ $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant($ty)),+
        }

        impl AstNode for $name {
            type Language = super::syntax::MooLanguage;

            fn can_cast(kind: SyntaxKind) -> bool {
                $(<$ty as AstNode>::can_cast(kind))||+
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                $(
                    if let Some(node) = <$ty as AstNode>::cast(syntax.clone()) {
                        return Some(Self::$variant(node));
                    }
                )+
                None
            }

            fn syntax(&self) -> &SyntaxNode {
                match self {
                    $(Self::$variant(node) => node.syntax()),+
                }
            }
        }
    };
}

define_ast_enum!(Statement {
    If(IfStmt),
    ForIn(ForInStmt),
    ForRange(ForRangeStmt),
    While(WhileStmt),
    Fork(ForkStmt),
    TryExcept(TryExceptStmt),
    TryFinally(TryFinallyStmt),
    Return(ReturnStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
    Expr(ExprStmt),
    Begin(BeginStmt),
    Fn(FnStmt),
    Let(LetStmt),
    Const(ConstStmt),
    Global(GlobalStmt),
});

define_ast_enum!(Expression {
    Binary(BinExpr),
    Unary(UnaryExpr),
    Paren(ParenExpr),
    Conditional(CondExpr),
    Index(IndexExpr),
    Range(RangeExpr),
    Call(CallExpr),
    VerbCall(VerbCallExpr),
    Property(PropExpr),
    Assign(AssignExpr),
    Scatter(ScatterExpr),
    List(ListExpr),
    Map(MapExpr),
    Flyweight(FlyweightExpr),
    Lambda(LambdaExpr),
    Try(TryExpr),
    Pass(PassExpr),
    SysProp(SysPropExpr),
    Comprehension(ComprehensionExpr),
});

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    support::token(node, kind)
}

fn child<N: AstNode<Language = super::syntax::MooLanguage>>(node: &SyntaxNode) -> Option<N> {
    support::child(node)
}

fn children<N: AstNode<Language = super::syntax::MooLanguage>>(
    node: &SyntaxNode,
) -> AstChildren<N> {
    support::children(node)
}

fn first_non_trivia_content(node: &SyntaxNode) -> Option<SyntaxElement> {
    node.children_with_tokens().find(|element| match element {
        NodeOrToken::Node(_) => true,
        NodeOrToken::Token(token) => {
            let kind = token.kind();
            !kind.is_trivia() && kind != SyntaxKind::Semi
        }
    })
}

impl Program {
    pub fn stmt_list(&self) -> Option<StmtList> {
        child(self.syntax())
    }

    pub fn statements(&self) -> Option<AstChildren<Statement>> {
        Some(self.stmt_list()?.statements())
    }
}

impl StmtList {
    pub fn statements(&self) -> AstChildren<Statement> {
        children(self.syntax())
    }
}

impl IfStmt {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }

    pub fn elseif_clauses(&self) -> AstChildren<ElseIfClause> {
        children(self.syntax())
    }

    pub fn else_clause(&self) -> Option<ElseClause> {
        child(self.syntax())
    }
}

impl ElseIfClause {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }
}

impl ElseClause {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }
}

impl ForInStmt {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }
}

impl ForRangeStmt {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }
}

impl WhileStmt {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }
}

impl ForkStmt {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }
}

impl TryExceptStmt {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }

    pub fn except_clauses(&self) -> AstChildren<ExceptClause> {
        children(self.syntax())
    }

    pub fn finally_clause(&self) -> Option<TryFinallyStmt> {
        child(self.syntax())
    }
}

impl TryFinallyStmt {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }
}

impl ExceptClause {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }
}

impl ExprStmt {
    pub fn expr(&self) -> Option<Expression> {
        child(self.syntax())
    }

    pub fn content(&self) -> Option<SyntaxElement> {
        first_non_trivia_content(self.syntax())
    }
}

impl ReturnStmt {
    pub fn expr(&self) -> Option<Expression> {
        child(self.syntax())
    }

    pub fn content(&self) -> Option<SyntaxElement> {
        first_non_trivia_content(self.syntax())
    }
}

impl BeginStmt {
    pub fn body(&self) -> Option<StmtList> {
        child(self.syntax())
    }
}

impl FnStmt {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token(self.syntax(), SyntaxKind::Ident)
    }

    pub fn params(&self) -> Option<ParamList> {
        child(self.syntax())
    }

    pub fn body(&self) -> Option<StmtList> {
        children(self.syntax()).next()
    }
}

impl LetStmt {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token(self.syntax(), SyntaxKind::Ident)
    }

    pub fn scatter(&self) -> Option<ScatterExpr> {
        child(self.syntax())
    }
}

impl ConstStmt {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token(self.syntax(), SyntaxKind::Ident)
    }

    pub fn scatter(&self) -> Option<ScatterExpr> {
        child(self.syntax())
    }
}

impl GlobalStmt {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token(self.syntax(), SyntaxKind::Ident)
    }
}

impl ScatterExpr {
    pub fn items(&self) -> AstChildren<ScatterItem> {
        children(self.syntax())
    }
}

impl LambdaExpr {
    pub fn params(&self) -> Option<ParamList> {
        child(self.syntax())
    }

    pub fn body(&self) -> Option<StmtList> {
        children(self.syntax()).next()
    }
}

impl ParamList {
    pub fn items(&self) -> AstChildren<ScatterItem> {
        children(self.syntax())
    }
}

impl SysPropExpr {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        self.syntax()
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| token.kind() == SyntaxKind::Ident)
    }
}

#[cfg(test)]
mod tests {
    use rowan::ast::AstNode;

    use super::{Expression, LambdaExpr, Program, Statement};
    use crate::frontend::parser::parse_to_syntax_node;

    #[test]
    fn casts_program_and_iterates_statements() {
        let (root, errors) = parse_to_syntax_node("let x = 1; return x;");
        assert!(errors.is_empty(), "{errors:?}");

        let program = Program::cast(root).unwrap();
        let stmt_list = program.stmt_list().unwrap();
        let statements: Vec<_> = stmt_list.statements().collect();
        assert_eq!(statements.len(), 2);
        assert!(matches!(statements[0], Statement::Let(_)));
        assert!(matches!(statements[1], Statement::Return(_)));
    }

    #[test]
    fn exposes_clause_and_body_wrappers() {
        let source = "if (a) return b; elseif (c) return d; else return e; endif";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");

        let program = Program::cast(root).unwrap();
        let stmt = program.statements().unwrap().next().unwrap();
        let Statement::If(if_stmt) = stmt else {
            panic!("expected if statement");
        };

        assert!(if_stmt.body().is_some());
        assert_eq!(if_stmt.elseif_clauses().count(), 1);
        assert!(if_stmt.else_clause().is_some());
    }

    #[test]
    fn exposes_function_and_lambda_wrappers() {
        let source = "fn add(a, ?b = 1) return a + b; endfn value = {?x = 1, @rest} => x;";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");

        let program = Program::cast(root).unwrap();
        let mut statements = program.statements().unwrap();

        let Statement::Fn(fn_stmt) = statements.next().unwrap() else {
            panic!("expected function statement");
        };
        assert_eq!(fn_stmt.name_token().unwrap().text(), "add");
        assert_eq!(fn_stmt.params().unwrap().items().count(), 2);
        assert!(fn_stmt.body().is_some());

        let Statement::Expr(expr_stmt) = statements.next().unwrap() else {
            panic!("expected expression statement");
        };
        let Expression::Assign(assign) = expr_stmt.expr().unwrap() else {
            panic!("expected assignment expression");
        };
        let lambda = assign
            .syntax()
            .children()
            .find_map(LambdaExpr::cast)
            .unwrap();
        assert_eq!(lambda.params().unwrap().items().count(), 2);
    }

    #[test]
    fn exposes_try_statement_and_expr_content() {
        let source = "try return x; except (ANY) return y; endtry foo;";
        let (root, errors) = parse_to_syntax_node(source);
        assert!(errors.is_empty(), "{errors:?}");

        let program = Program::cast(root).unwrap();
        let mut statements = program.statements().unwrap();

        let Statement::TryExcept(try_stmt) = statements.next().unwrap() else {
            panic!("expected try statement");
        };
        assert!(try_stmt.body().is_some());
        assert_eq!(try_stmt.except_clauses().count(), 1);

        let Statement::Expr(expr_stmt) = statements.next().unwrap() else {
            panic!("expected expression statement");
        };
        assert_eq!(expr_stmt.content().unwrap().to_string(), "foo");
    }

    #[test]
    fn sysprop_wrapper_exposes_name_token() {
        let (root, errors) = parse_to_syntax_node("$player;");
        assert!(errors.is_empty(), "{errors:?}");

        let program = Program::cast(root).unwrap();
        let Statement::Expr(expr_stmt) = program.statements().unwrap().next().unwrap() else {
            panic!("expected expression statement");
        };
        let Expression::SysProp(sysprop) = expr_stmt.expr().unwrap() else {
            panic!("expected sysprop expression");
        };
        assert_eq!(sysprop.name_token().unwrap().text(), "player");
    }
}
