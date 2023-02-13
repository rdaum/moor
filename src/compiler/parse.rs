use std::rc::Rc;
use std::str::FromStr;

use antlr_rust::common_token_stream::CommonTokenStream;
use antlr_rust::error_listener::ErrorListener;
use antlr_rust::errors::ANTLRError;
use antlr_rust::parser_rule_context::ParserRuleContext;
use antlr_rust::recognizer::Recognizer;
use antlr_rust::token::Token;
use antlr_rust::token_factory::TokenFactory;
use antlr_rust::tree::{ParseTree, ParseTreeVisitor, TerminalNode, Tree, Visitable};
use antlr_rust::{InputStream, Parser};
use anyhow::anyhow;
use decorum::R64;
use paste::paste;
use serde_derive::{Deserialize, Serialize};

use crate::compiler::ast::{
    Arg, BinaryOp, CondArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt, UnaryOp,
};
use crate::compiler::ast::Expr::VarExpr;
use crate::grammar::moolexer::mooLexer;
use crate::grammar::mooparser::*;
use crate::grammar::moovisitor::mooVisitor;
use crate::model::var::Var::{Obj, Str};
use crate::model::var::{Error, Objid, Var};

pub struct VerbCompileErrorListener {
    pub program: String,
}

impl<'a, T: Recognizer<'a>> ErrorListener<'a, T> for VerbCompileErrorListener {
    fn syntax_error(
        &self,
        _recognizer: &T,
        offending_symbol: Option<&<T::TF as TokenFactory<'a>>::Inner>,
        line: isize,
        column: isize,
        msg: &str,
        _e: Option<&ANTLRError>,
    ) {
        if let Some(_of) = offending_symbol {
            let lines: Vec<&str> = self.program.lines().collect();
            eprintln!("Error {} in:\n{}", msg, lines[line as usize - 1]);
            eprintln!("{}^", (0..column).map(|_| " ").collect::<String>());
            panic!("Compilation fail.");
        }
    }
}

#[derive(Debug)]
struct LoopEntry {
    name: Option<String>,
    is_barrier: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct Name(pub usize);

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Names {
    pub names: Vec<String>,
}

impl Default for Names {
    fn default() -> Self {
        Self { names: vec![] }
    }
}

impl Names {
    pub fn new() -> Self {
        let mut names = Self { names: vec![] };

        names.find_or_add_name(&String::from("NUM"));
        names.find_or_add_name(&String::from("OBJ"));
        names.find_or_add_name(&String::from("STR"));
        names.find_or_add_name(&String::from("LIST"));
        names.find_or_add_name(&String::from("ERR"));
        names.find_or_add_name(&String::from("INT"));
        names.find_or_add_name(&String::from("FLOAT"));
        names.find_or_add_name(&String::from("player"));
        names.find_or_add_name(&String::from("this"));
        names.find_or_add_name(&String::from("caller"));
        names.find_or_add_name(&String::from("verb"));
        names.find_or_add_name(&String::from("args"));
        names.find_or_add_name(&String::from("argstr"));
        names.find_or_add_name(&String::from("dobj"));
        names.find_or_add_name(&String::from("dobjstr"));
        names.find_or_add_name(&String::from("prepstr"));
        names.find_or_add_name(&String::from("iobj"));
        names.find_or_add_name(&String::from("iobjstr"));
        names
    }

    pub fn find_or_add_name(&mut self, name: &String) -> Name {
        match self
            .names
            .iter()
            .position(|n| n.to_lowercase().as_str() == name.to_lowercase())
        {
            None => {
                let pos = self.names.len();
                self.names.push(String::from(name));
                Name(pos)
            }
            Some(n) => Name(n),
        }
    }

    pub fn find_name(&self, name: &str) -> Option<Name> {
        self.find_name_offset(name).map(|n| Name(n))
    }

    pub fn find_name_offset(&self, name: &str) -> Option<usize> {
        self.names
            .iter()
            .position(|x| x.to_lowercase() == name.to_lowercase())
    }
    pub fn width(&self) -> usize {
        return self.names.len();
    }
}

pub struct ASTGenVisitor {
    pub program: Vec<Stmt>,
    pub names: Names,
    _statement_stack: Vec<Vec<Stmt>>,
    _expr_stack: Vec<Expr>,
    _cond_arm_stack: Vec<Vec<CondArm>>,
    _loop_stack: Vec<LoopEntry>,
    _excepts_stack: Vec<Vec<ExceptArm>>,
    _args_stack: Vec<Vec<Arg>>,
    _scatter_stack: Vec<Vec<ScatterItem>>,
}

impl ASTGenVisitor {
    pub fn new(names: Names) -> Self {
        Self {
            program: Default::default(),
            names,
            _statement_stack: Default::default(),
            _expr_stack: Default::default(),
            _cond_arm_stack: Default::default(),
            _loop_stack: Default::default(),
            _excepts_stack: Default::default(),
            _args_stack: Default::default(),
            _scatter_stack: Default::default(),
        }
    }
}

enum LoopExitKind {
    Break,
    Continue,
}

impl ASTGenVisitor {
    // Loop scope management
    fn push_loop_name(&mut self, name: Option<&String>) {
        self._loop_stack.push(LoopEntry {
            name: name.map(String::from),
            is_barrier: false,
        })
    }
    fn resume_loop_scope(&mut self) {
        let last_entry = self._loop_stack.last();
        match last_entry {
            None => {
                // TODO should be a recoverable error?
                panic!("PARSER: Empty loop stack in RESUME_LOOP_SCOPE!")
            }
            Some(loop_entry) if !loop_entry.is_barrier => {
                // TODO should be a recoverable error?
                panic!(
                    "PARSER: Tried to resume non-loop-scope barrier! (current loop: {:?}",
                    loop_entry
                )
            }
            Some(_) => {
                self._loop_stack.pop();
            }
        }
    }
    fn pop_loop_name(&mut self) -> LoopEntry {
        let last_entry = self._loop_stack.pop();
        match last_entry {
            None => {
                // TODO should be a recoverable error?
                panic!("PARSER: Empty loop stack in POP_LOOP_NAME!")
            }
            Some(loop_entry) if loop_entry.is_barrier => {
                // TODO should be a recoverable error?
                panic!("PARSER: Tried to pop loop-scope barrier!")
            }
            Some(loop_entry) => loop_entry,
        }
    }
    fn suspend_loop_scope(&mut self) {
        self._loop_stack.push(LoopEntry {
            name: None,
            is_barrier: true,
        })
    }
    fn check_loop_name(
        &mut self,
        name: Option<&String>,
        kind: LoopExitKind,
    ) -> Result<(), anyhow::Error> {
        match name {
            None => {
                let last = self._loop_stack.last();
                if last.is_none() || last.unwrap().is_barrier {
                    match kind {
                        LoopExitKind::Break => {
                            return Err(anyhow!("No enclosing loop for `break' statement"));
                        }
                        LoopExitKind::Continue => {
                            return Err(anyhow!("No enclosing loop for `continue' statement"));
                        }
                    }
                }
                Ok(())
            }
            Some(n) => {
                let entry = self._loop_stack.iter().rev().find(|e| {
                    if e.is_barrier {
                        return false;
                    }
                    if let Some(name) = &e.name {
                        if name == n {
                            return true;
                        }
                    }
                    false
                });
                if entry.is_some() {
                    return Ok(());
                }
                match kind {
                    LoopExitKind::Break => {
                        Err(anyhow!("Invalid loop name in `break` statement: {}", n))
                    }
                    LoopExitKind::Continue => {
                        Err(anyhow!("Invalid loop name in `continue` statement: {}", n))
                    }
                }
            }
        }
    }

    // Local names slot mgmt. Find or create.
    fn find_id(&mut self, name: &String) -> Name {
        self.names.find_or_add_name(name)
    }

    fn reduce_expr(&mut self, node: &Option<Rc<ExprContextAll>>) -> Expr {
        node.as_ref().unwrap().accept(self);
        self._expr_stack.pop().unwrap()
    }

    fn reduce_opt_expr(&mut self, node: &Option<Rc<ExprContextAll>>) -> Option<Expr> {
        match node.as_ref() {
            None => None,
            Some(node) => {
                node.accept(self);
                Some(self._expr_stack.pop().unwrap())
            }
        }
    }

    fn reduce_statements(&mut self, node: &Option<Rc<StatementsContextAll>>) -> Vec<Stmt> {
        self._statement_stack.push(vec![]);
        node.iter().for_each(|stmt| stmt.accept(self));
        self._statement_stack.pop().unwrap()
    }

    fn get_id(id: &Option<Rc<TerminalNode<mooParserContextType>>>) -> String {
        id.as_ref().unwrap().get_text()
    }

    fn get_opt_id(id: &Option<Rc<TerminalNode<mooParserContextType>>>) -> Option<String> {
        id.as_ref().map(|s| String::from(s.get_text().as_str()))
    }
}

macro_rules! binary_expr {
    ( $op:ident ) => {
        paste! {
            fn [<visit_ $op Expr>](&mut self, ctx: &[<$op ExprContext>]<'node>) {
                let left = self.reduce_expr(&ctx.expr(0));
                let right = self.reduce_expr(&ctx.expr(1));
                self._expr_stack
                    .push(Expr::Binary(BinaryOp::$op, Box::new(left), Box::new(right)));
            }
        }
    };
}

macro_rules! unary_expr {
    ( $op:ident ) => {
        paste! {
            fn [<visit_ $op Expr>](&mut self, ctx: &[<$op ExprContext>]<'node>) {
                let expr = self.reduce_expr(&ctx.expr());
                self._expr_stack
                    .push(Expr::Unary(UnaryOp::$op, Box::new(expr)));
            }
        }
    };
}
impl<'node> ParseTreeVisitor<'node, mooParserContextType> for ASTGenVisitor {}

impl<'node> mooVisitor<'node> for ASTGenVisitor {
    fn visit_program(&mut self, ctx: &ProgramContext<'node>) {
        self._statement_stack.push(vec![]);
        ctx.statements().iter().for_each(|item| item.accept(self));
        self.program = self._statement_stack.pop().unwrap();
    }

    fn visit_statements(&mut self, ctx: &StatementsContext<'node>) {
        ctx.statement_all()
            .iter()
            .for_each(|item| item.accept(self));
    }

    fn visit_If(&mut self, ctx: &IfContext<'node>) {
        let condition = self.reduce_expr(&ctx.expr());
        let statements = self.reduce_statements(&ctx.statements(0));

        self._cond_arm_stack.push(vec![CondArm {
            condition,
            statements,
        }]);
        for ei in ctx.elseif_all().iter() {
            ei.accept(self);
        }
        let cond_arms = self._cond_arm_stack.pop().unwrap();

        let otherwise = if ctx.elsepart.is_some() {
            self.reduce_statements(&ctx.elsepart)
        } else {
            vec![]
        };

        let cond = Stmt::Cond {
            arms: cond_arms,
            otherwise,
        };
        self._statement_stack.last_mut().unwrap().push(cond);
    }

    fn visit_ForExpr(&mut self, ctx: &ForExprContext<'node>) {
        let id = Self::get_id(&ctx.ID());
        self.push_loop_name(Some(&id));
        let id = self.find_id(&id);

        let expr_node = self.reduce_expr(&ctx.expr());

        let body = self.reduce_statements(&ctx.statements());

        let stmt = Stmt::ForList {
            id,
            expr: expr_node,
            body,
        };
        self.pop_loop_name();
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_ForRange(&mut self, ctx: &ForRangeContext<'node>) {
        let id = Self::get_id(&ctx.ID());
        self.push_loop_name(Some(&id));

        let id = self.find_id(&id);
        let from = self.reduce_expr(&ctx.from);
        let to = self.reduce_expr(&ctx.to);

        let body = self.reduce_statements(&ctx.statements());
        let stmt = Stmt::ForRange { id, from, to, body };
        self.pop_loop_name();
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_While(&mut self, ctx: &WhileContext<'node>) {
        // Handle ID's while loops as well as non-ID'd
        let id = Self::get_opt_id(&ctx.ID());
        self.push_loop_name(id.as_ref());
        let id = id.map(|id| self.find_id(&id));

        let condition = self.reduce_expr(&ctx.condition);
        let body = self.reduce_statements(&ctx.statements());
        let stmt = Stmt::While {
            id,
            condition,
            body,
        };
        self.pop_loop_name();

        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_Fork(&mut self, ctx: &ForkContext<'node>) {
        self.suspend_loop_scope();
        let id = Self::get_opt_id(&ctx.ID());
        let id = id.map(|id| self.find_id(&id));

        let time = self.reduce_expr(&ctx.time);
        let body = self.reduce_statements(&ctx.statements());

        let stmt = Stmt::Fork { id, time, body };
        self._statement_stack.last_mut().unwrap().push(stmt);
        self.resume_loop_scope();
    }

    fn visit_Break(&mut self, ctx: &BreakContext<'node>) {
        let id = Self::get_opt_id(&ctx.ID());
        // TODO propagate error correctly
        self.check_loop_name(id.as_ref(), LoopExitKind::Break)
            .expect("Bad break");

        let exit = id.as_ref().map(|id| self.find_id(id));
        let stmt = Stmt::Break { exit };
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_Continue(&mut self, ctx: &ContinueContext<'node>) {
        let id = Self::get_opt_id(&ctx.ID());
        // TODO propagate error correctly
        self.check_loop_name(id.as_ref(), LoopExitKind::Continue)
            .expect("Bad break");

        let exit = id.as_ref().map(|id| self.find_id(id));
        let stmt = Stmt::Continue { exit };
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_Return(&mut self, ctx: &ReturnContext<'node>) {
        let expr = self.reduce_opt_expr(&ctx.expr());
        let stmt = Stmt::Return { expr };
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_TryExcept(&mut self, ctx: &TryExceptContext<'node>) {
        let body = self.reduce_statements(&ctx.statements());
        self._excepts_stack.push(vec![]);
        ctx.excepts().as_ref().iter().for_each(|e| e.accept(self));
        let excepts = self._excepts_stack.pop().unwrap();
        let stmt = Stmt::TryExcept { body, excepts };
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_TryFinally(&mut self, ctx: &TryFinallyContext<'node>) {
        let body = self.reduce_statements(&ctx.statements(0));
        let handler = self.reduce_statements(&ctx.statements(1));
        self._statement_stack
            .last_mut()
            .unwrap()
            .push(Stmt::TryFinally { body, handler })
    }

    fn visit_ExprStmt(&mut self, ctx: &ExprStmtContext<'node>) {
        match self.reduce_opt_expr(&ctx.expr()) {
            None => {}
            Some(expr) => {
                let stmt = Stmt::Expr(expr);
                self._statement_stack.last_mut().unwrap().push(stmt);
            }
        }
    }

    fn visit_elseif(&mut self, ctx: &ElseifContext<'node>) {
        let condition = self.reduce_expr(&ctx.condition);
        let statements = self.reduce_statements(&ctx.statements());

        let cond_arm = CondArm {
            condition,
            statements,
        };
        self._cond_arm_stack.last_mut().unwrap().push(cond_arm);
    }

    fn visit_excepts(&mut self, ctx: &ExceptsContext<'node>) {
        // Just visit each 'except' arm, and that will fill the _excepts_stack
        ctx.except_all().iter().for_each(|e| e.accept(self));
    }

    fn visit_except(&mut self, ctx: &ExceptContext<'node>) {
        // Produce an except arm
        let id = Self::get_opt_id(&ctx.ID());
        let id = id.map(|id| self.find_id(&id));

        self._args_stack.push(vec![]);
        ctx.codes().iter().for_each(|c| c.accept(self));
        let codes = self._args_stack.pop().unwrap();
        let statements = self.reduce_statements(&ctx.statements());
        let except_arm = ExceptArm {
            id,
            codes,
            statements,
        };
        self._excepts_stack.last_mut().unwrap().push(except_arm);
    }

    fn visit_Int(&mut self, ctx: &IntContext<'node>) {
        let i = i64::from_str(ctx.get_text().as_str()).unwrap();
        self._expr_stack.push(VarExpr(Var::Int(i)));
    }

    fn visit_Float(&mut self, ctx: &FloatContext<'node>) {
        let f = f64::from_str(ctx.get_text().as_str()).unwrap();
        self._expr_stack.push(VarExpr(Var::Float(R64::from(f))));
    }

    fn visit_String(&mut self, ctx: &StringContext<'node>) {
        let string = ctx.get_text();
        let string = string.as_str().clone();
        // TODO error handling.
        let string = snailquote::unescape(string).unwrap();
        self._expr_stack
            .push(VarExpr(Var::Str(String::from(string))));
    }

    fn visit_Object(&mut self, ctx: &ObjectContext<'node>) {
        let oid_txt = ctx.get_text();
        let i = i64::from_str(&oid_txt.as_str()[1..]).unwrap();
        self._expr_stack.push(VarExpr(Var::Obj(Objid(i))));
    }

    fn visit_Error(&mut self, ctx: &ErrorContext<'node>) {
        let e = ctx.get_text();
        let e = match e.to_lowercase().as_str() {
            "e_type" => Var::Err(Error::E_TYPE),
            "e_div" => Var::Err(Error::E_DIV),
            "e_perm" => Var::Err(Error::E_PERM),
            "e_propnf" => Var::Err(Error::E_PROPNF),
            "e_verbnf" => Var::Err(Error::E_VERBNF),
            "e_varnf" => Var::Err(Error::E_VARNF),
            "e_invind" => Var::Err(Error::E_INVIND),
            "e_recmove" => Var::Err(Error::E_RECMOVE),
            "e_maxrec" => Var::Err(Error::E_MAXREC),
            "e_range" => Var::Err(Error::E_RANGE),
            "e_args" => Var::Err(Error::E_ARGS),
            "e_nacc" => Var::Err(Error::E_NACC),
            "e_invarg" => Var::Err(Error::E_INVARG),
            "e_quota" => Var::Err(Error::E_QUOTA),
            "e_float" => Var::Err(Error::E_FLOAT),
            &_ => {
                panic!("unknown error")
            }
        };
        self._expr_stack.push(VarExpr(e));
    }

    fn visit_Identifier(&mut self, ctx: &IdentifierContext<'node>) {
        let id = self.find_id(&ctx.get_text());
        self._expr_stack.push(Expr::Id(id))
    }

    fn visit_PropertyExprReference(&mut self, ctx: &PropertyExprReferenceContext<'node>) {
        let expr = self.reduce_expr(&ctx.location);
        let property_expr = self.reduce_expr(&ctx.property);
        self._expr_stack.push(Expr::Prop {
            location: Box::new(expr),
            property: Box::new(property_expr),
        })
    }

    fn visit_IndexRangeExpr(&mut self, ctx: &IndexRangeExprContext<'node>) {
        let expr = self.reduce_expr(&ctx.expr(0));
        let start = self.reduce_expr(&ctx.expr(1));
        let end = self.reduce_expr(&ctx.expr(2));
        self._expr_stack.push(Expr::Range {
            base: Box::new(expr),
            from: Box::new(start),
            to: Box::new(end),
        })
    }

    fn visit_RangeEnd(&mut self, _ctx: &RangeEndContext<'node>) {
        self._expr_stack.push(Expr::Length);
    }

    fn visit_AtomExpr(&mut self, ctx: &AtomExprContext<'node>) {
        ctx.get_children().for_each(|c| c.accept(self))
    }

    fn visit_ListExpr(&mut self, ctx: &ListExprContext<'node>) {
        self._args_stack.push(vec![]);
        ctx.arglist().iter().for_each(|c| c.accept(self));
        let list = self._args_stack.pop().unwrap();
        self._expr_stack.push(Expr::List(list));
    }

    fn visit_VerbExprCall(&mut self, ctx: &VerbExprCallContext<'node>) {
        let expr = self.reduce_expr(&ctx.location);
        let verb = self.reduce_expr(&ctx.verb);

        self._args_stack.push(vec![]);
        ctx.arglist().iter().for_each(|c| c.accept(self));
        let args = self._args_stack.pop().unwrap();

        self._expr_stack.push(Expr::Verb {
            location: Box::new(expr),
            verb: Box::new(verb),
            args,
        });
    }

    fn visit_SysProp(&mut self, ctx: &SysPropContext<'node>) {
        let prop_id = ctx.id.as_ref().unwrap();
        let property = String::from(prop_id.get_text());
        let obj = Objid(0);
        self._expr_stack.push(Expr::Prop {
            location: Box::new(VarExpr(Var::Obj(obj))),
            property: Box::new(VarExpr(Var::Str(property))),
        });
    }

    fn visit_SysVerb(&mut self, ctx: &SysVerbContext<'node>) {
        let verb_id = ctx.id.as_ref().unwrap();
        let verb = String::from(verb_id.get_text());
        let obj = Objid(0);

        self._args_stack.push(vec![]);
        ctx.arglist().iter().for_each(|c| c.accept(self));
        let args = self._args_stack.pop().unwrap();

        self._expr_stack.push(Expr::Verb {
            location: Box::new(VarExpr(Obj(obj))),
            verb: Box::new(VarExpr(Str(verb))),
            args,
        });
    }

    fn visit_PropertyReference(&mut self, ctx: &PropertyReferenceContext<'node>) {
        let expr = self.reduce_expr(&ctx.location);
        let prop_id = &ctx.property.as_ref().unwrap();
        let property = Var::Str(String::from(prop_id.get_text()));
        self._expr_stack.push(Expr::Prop {
            location: Box::new(expr),
            property: Box::new(VarExpr(property)),
        })
    }

    fn visit_ErrorEscape(&mut self, ctx: &ErrorEscapeContext<'node>) {
        let try_expr = self.reduce_expr(&ctx.try_e);
        self._args_stack.push(vec![]);
        ctx.codes().iter().for_each(|c| c.accept(self));
        let codes = self._args_stack.pop().unwrap();
        let except = self.reduce_opt_expr(&ctx.except_expr).map(Box::new);
        self._expr_stack.push(Expr::Catch {
            trye: Box::new(try_expr),
            codes,
            except,
        })
    }

    fn visit_BuiltinCall(&mut self, ctx: &BuiltinCallContext<'node>) {
        let builtin_id = &ctx.builtin.as_ref().unwrap();
        let builtin_id = String::from(builtin_id.get_text());

        self._args_stack.push(vec![]);
        ctx.arglist().iter().for_each(|c| c.accept(self));
        let args = self._args_stack.pop().unwrap();

        self._expr_stack.push(Expr::Call {
            function: builtin_id,
            args,
        });
    }

    fn visit_VerbCall(&mut self, ctx: &VerbCallContext<'node>) {
        let expr = self.reduce_expr(&ctx.location);
        let verb_id = &ctx.verb.as_ref().unwrap();
        let verb = Str(String::from(verb_id.get_text()));

        self._args_stack.push(vec![]);
        ctx.arglist().iter().for_each(|c| c.accept(self));
        let args = self._args_stack.pop().unwrap();

        self._expr_stack.push(Expr::Verb {
            location: Box::new(expr),
            verb: Box::new(VarExpr(verb)),
            args,
        });
    }

    fn visit_codes(&mut self, ctx: &CodesContext<'node>) {
        // Push to the arglist.
        ctx.ne_arglist().iter().for_each(|al| al.accept(self));
    }

    fn visit_arglist(&mut self, ctx: &ArglistContext<'node>) {
        ctx.ne_arglist().iter().for_each(|al| al.accept(self));
    }

    fn visit_ne_arglist(&mut self, ctx: &Ne_arglistContext<'node>) {
        ctx.argument_all().iter().for_each(|a| a.accept(self));
    }

    fn visit_Arg(&mut self, ctx: &ArgContext<'node>) {
        let expr = self.reduce_expr(&ctx.expr());
        self._args_stack.last_mut().unwrap().push(Arg::Normal(expr));
    }

    fn visit_SpliceArg(&mut self, ctx: &SpliceArgContext<'node>) {
        let expr = self.reduce_expr(&ctx.expr());
        self._args_stack.last_mut().unwrap().push(Arg::Splice(expr));
    }

    fn visit_ScatterExpr(&mut self, ctx: &ScatterExprContext<'node>) {
        self._scatter_stack.push(vec![]);
        ctx.scatter().iter().for_each(|s| s.accept(self));
        let scatters = self._scatter_stack.pop().unwrap();
        let rhs = self.reduce_expr(&ctx.expr());
        self._expr_stack
            .push(Expr::Scatter(scatters, Box::new(rhs)));
    }

    fn visit_scatter(&mut self, ctx: &ScatterContext<'node>) {
        ctx.scatter_item_all().iter().for_each(|si| si.accept(self));
    }

    fn visit_ScatterOptionalTarget(&mut self, ctx: &ScatterOptionalTargetContext<'node>) {
        let expr = self.reduce_opt_expr(&ctx.expr());
        let id = self.find_id(&String::from(ctx.sid.as_ref().unwrap().get_text()));
        let sd = ScatterItem {
            kind: ScatterKind::Optional,
            id,
            expr,
        };
        self._scatter_stack.last_mut().unwrap().push(sd);
    }

    fn visit_ScatterTarget(&mut self, ctx: &ScatterTargetContext<'node>) {
        let id = self.find_id(&String::from(ctx.sid.as_ref().unwrap().get_text()));
        let sd = ScatterItem {
            kind: ScatterKind::Required,
            id,
            expr: None,
        };
        self._scatter_stack.last_mut().unwrap().push(sd);
    }

    fn visit_ScatterRestTarget(&mut self, ctx: &ScatterRestTargetContext<'node>) {
        let id = self.find_id(&String::from(ctx.sid.as_ref().unwrap().get_text()));
        let sd = ScatterItem {
            kind: ScatterKind::Rest,
            id,
            expr: None,
        };
        self._scatter_stack.last_mut().unwrap().push(sd);
    }

    fn visit_AndExpr(&mut self, ctx: &AndExprContext<'node>) {
        let left = self.reduce_expr(&ctx.expr(0));
        let right = self.reduce_expr(&ctx.expr(1));
        self._expr_stack
            .push(Expr::And(Box::new(left), Box::new(right)));
    }

    fn visit_OrExpr(&mut self, ctx: &OrExprContext<'node>) {
        let left = self.reduce_expr(&ctx.expr(0));
        let right = self.reduce_expr(&ctx.expr(1));
        self._expr_stack
            .push(Expr::Or(Box::new(left), Box::new(right)));
    }

    fn visit_IndexExpr(&mut self, ctx: &IndexExprContext<'node>) {
        let left = self.reduce_expr(&ctx.expr(0));
        let right = self.reduce_expr(&ctx.expr(1));
        self._expr_stack
            .push(Expr::Index(Box::new(left), Box::new(right)));
    }

    fn visit_AssignExpr(&mut self, ctx: &AssignExprContext<'node>) {
        let left = self.reduce_expr(&ctx.expr(0));
        let right = self.reduce_expr(&ctx.expr(1));
        self._expr_stack.push(Expr::Assign {
            left: Box::new(left),
            right: Box::new(right),
        });
    }

    binary_expr!(Mul);
    binary_expr!(Div);
    binary_expr!(Add);
    binary_expr!(Sub);
    binary_expr!(Lt);
    binary_expr!(LtE);
    binary_expr!(Gt);
    binary_expr!(GtE);
    binary_expr!(Exp);
    binary_expr!(In);
    binary_expr!(Eq);
    binary_expr!(NEq);

    unary_expr!(Not);
    unary_expr!(Neg);

    fn visit_CondExpr(&mut self, ctx: &CondExprContext<'node>) {
        let cond = self.reduce_expr(&ctx.expr(0));
        let left = self.reduce_expr(&ctx.expr(1));
        let right = self.reduce_expr(&ctx.expr(2));
        self._expr_stack.push(Expr::Cond {
            condition: Box::new(cond),
            consequence: Box::new(left),
            alternative: Box::new(right),
        });
    }
}

pub struct Parse {
    pub stmts: Vec<Stmt>,
    pub names: Names,
}

pub fn parse_program(program: &str) -> Result<Parse, anyhow::Error> {
    let is = InputStream::new(program);
    let lexer = mooLexer::new(is);
    let source = CommonTokenStream::new(lexer);
    let mut parser = mooParser::new(source);

    let err_listener = Box::new(VerbCompileErrorListener {
        program: String::from(program),
    });

    parser.add_error_listener(err_listener);
    let program_context = parser.program().unwrap();

    let mut names = Names::new();
    let mut astgen_visitor = ASTGenVisitor::new(names);
    program_context.accept(&mut astgen_visitor);

    Ok(Parse {
        stmts: astgen_visitor.program,
        names: astgen_visitor.names,
    })
}

#[cfg(test)]
mod tests {
    use crate::compiler::ast::Expr::{Id, Prop, VarExpr};
    use crate::compiler::ast::{Arg, BinaryOp, CondArm, Expr, ScatterItem, ScatterKind, Stmt};
    use crate::compiler::parse::{parse_program, Name};
    use crate::model::var::Var::Str;
    use crate::model::var::{Objid, Var};

    #[test]
    fn test_parse_simple_var_assignment_precedence() {
        let program = "a = 1 + 2;";
        let parse = parse_program(program).unwrap();
        let a = parse.names.find_name("a").unwrap();

        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            parse.stmts[0],
            Stmt::Expr(Expr::Assign {
                left: Box::new(Id(a)),
                right: Box::new(Expr::Binary(
                    BinaryOp::Add,
                    Box::new(VarExpr(Var::Int(1))),
                    Box::new(VarExpr(Var::Int(2))),
                )),
            })
        );
    }

    #[test]
    fn test_parse_if_stmt() {
        let program = "if (1 == 2) return 5; elseif (2 == 3) return 3; else return 6; endif";
        let parse = parse_program(program).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            parse.stmts[0],
            Stmt::Cond {
                arms: vec![
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(Var::Int(1))),
                            Box::new(VarExpr(Var::Int(2))),
                        ),
                        statements: vec![Stmt::Return {
                            expr: Some(VarExpr(Var::Int(5))),
                        }],
                    },
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(Var::Int(2))),
                            Box::new(VarExpr(Var::Int(3))),
                        ),
                        statements: vec![Stmt::Return {
                            expr: Some(VarExpr(Var::Int(3))),
                        }],
                    },
                ],

                otherwise: vec![Stmt::Return {
                    expr: Some(VarExpr(Var::Int(6))),
                }],
            }
        );
    }

    #[test]
    fn test_parse_for_loop() {
        let program = "for x in ({1,2,3}) b = x + 5; endfor";
        let parse = parse_program(program).unwrap();
        let x = parse.names.find_name("x").unwrap();
        let b = parse.names.find_name("b").unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            parse.stmts[0],
            Stmt::ForList {
                id: x,
                expr: Expr::List(vec![
                    Arg::Normal(VarExpr(Var::Int(1))),
                    Arg::Normal(VarExpr(Var::Int(2))),
                    Arg::Normal(VarExpr(Var::Int(3))),
                ]),
                body: vec![Stmt::Expr(Expr::Assign {
                    left: Box::new(Expr::Id(b)),
                    right: Box::new(Expr::Binary(
                        BinaryOp::Add,
                        Box::new(Expr::Id(x)),
                        Box::new(VarExpr(Var::Int(5))),
                    )),
                })],
            }
        )
    }

    #[test]
    fn test_parse_for_range() {
        let program = "for x in [1..5] b = x + 5; endfor";
        let parse = parse_program(program).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        let x = parse.names.find_name("x").unwrap();
        let b = parse.names.find_name("b").unwrap();
        assert_eq!(
            parse.stmts[0],
            Stmt::ForRange {
                id: x,
                from: VarExpr(Var::Int(1)),
                to: VarExpr(Var::Int(5)),
                body: vec![Stmt::Expr(Expr::Assign {
                    left: Box::new(Id(b)),
                    right: Box::new(Expr::Binary(
                        BinaryOp::Add,
                        Box::new(Id(x)),
                        Box::new(VarExpr(Var::Int(5))),
                    )),
                })],
            }
        )
    }

    #[test]
    fn test_indexed_range_len() {
        let program = "a = {1, 2, 3}; b = a[2..$];";
        let parse = parse_program(program).unwrap();
        let (a, b) = (
            parse.names.find_name("a").unwrap(),
            parse.names.find_name("b").unwrap(),
        );
        assert_eq!(
            parse.stmts,
            vec![
                Stmt::Expr(Expr::Assign {
                    left: Box::new(Expr::Id(a)),
                    right: Box::new(Expr::List(vec![
                        Arg::Normal(VarExpr(Var::Int(1))),
                        Arg::Normal(VarExpr(Var::Int(2))),
                        Arg::Normal(VarExpr(Var::Int(3))),
                    ])),
                }),
                Stmt::Expr(Expr::Assign {
                    left: Box::new(Expr::Id(b)),
                    right: Box::new(Expr::Range {
                        base: Box::new(Expr::Id(a)),
                        from: Box::new(VarExpr(Var::Int(2))),
                        to: Box::new(Expr::Length),
                    }),
                }),
            ]
        );
    }

    #[test]
    fn test_parse_while() {
        let program = "while (1) x = x + 1; if (x > 5) break; endif endwhile";
        let parse = parse_program(program).unwrap();
        let x = parse.names.find_name("x").unwrap();

        assert_eq!(
            parse.stmts,
            vec![Stmt::While {
                id: None,
                condition: VarExpr(Var::Int(1)),
                body: vec![
                    Stmt::Expr(Expr::Assign {
                        left: Box::new(Expr::Id(x)),
                        right: Box::new(Expr::Binary(
                            BinaryOp::Add,
                            Box::new(Expr::Id(x)),
                            Box::new(VarExpr(Var::Int(1))),
                        )),
                    }),
                    Stmt::Cond {
                        arms: vec![CondArm {
                            condition: Expr::Binary(
                                BinaryOp::Gt,
                                Box::new(Expr::Id(x)),
                                Box::new(VarExpr(Var::Int(5))),
                            ),
                            statements: vec![Stmt::Break { exit: None }],
                        }],
                        otherwise: vec![],
                    },
                ],
            }]
        )
    }

    #[test]
    fn test_parse_labelled_while() {
        let program = "while chuckles (1) x = x + 1; if (x > 5) break chuckles; endif endwhile";
        let parse = parse_program(program).unwrap();
        let chuckles = parse.names.find_name("chuckles").unwrap();
        let x = parse.names.find_name("x").unwrap();

        assert_eq!(
            parse.stmts,
            vec![Stmt::While {
                id: Some(chuckles),
                condition: VarExpr(Var::Int(1)),
                body: vec![
                    Stmt::Expr(Expr::Assign {
                        left: Box::new(Id(x)),
                        right: Box::new(Expr::Binary(
                            BinaryOp::Add,
                            Box::new(Id(x)),
                            Box::new(VarExpr(Var::Int(1))),
                        )),
                    }),
                    Stmt::Cond {
                        arms: vec![CondArm {
                            condition: Expr::Binary(
                                BinaryOp::Gt,
                                Box::new(Id(x)),
                                Box::new(VarExpr(Var::Int(5))),
                            ),
                            statements: vec![Stmt::Break {
                                exit: Some(chuckles)
                            }],
                        }],
                        otherwise: vec![],
                    },
                ],
            }]
        )
    }

    #[test]
    fn test_sysobjref() {
        let program = "$string_utils:from_list(test_string);";
        let parse = parse_program(program).unwrap();
        let test_string = parse
            .names
            .find_name(&"test_string".to_string())
            .unwrap()
            .clone();
        assert_eq!(
            parse.stmts,
            vec![Stmt::Expr(Expr::Verb {
                location: Box::new(Prop {
                    location: Box::new(VarExpr(Var::Obj(Objid(0)))),
                    property: Box::new(VarExpr(Str("string_utils".to_string()))),
                }),
                verb: Box::new(VarExpr(Var::Str("from_list".to_string()))),
                args: vec![Arg::Normal(Id(test_string))],
            })]
        );
    }

    #[test]
    fn test_scatter_assign() {
        let program = "{connection} = args;";
        let parse = parse_program(program).unwrap();
        let connection = parse
            .names
            .find_name(&"connection".to_string())
            .unwrap()
            .clone();
        let args = parse.names.find_name(&"args".to_string()).unwrap().clone();

        let scatter_items = vec![ScatterItem {
            kind: ScatterKind::Required,
            id: connection,
            expr: None,
        }];
        let scatter_right = Box::new(Id(args));
        assert_eq!(
            parse.stmts,
            vec![Stmt::Expr(Expr::Scatter(scatter_items, scatter_right))]
        );
    }

    #[test]
    fn test_indexed_assign() {
        let program = "this.stack[5] = 5;";
        let parse = parse_program(program).unwrap();
        let this = parse.names.find_name(&"this".to_string()).unwrap().clone();
        assert_eq!(
            parse.stmts,
            vec![Stmt::Expr(Expr::Assign {
                left: Box::new(Expr::Index(
                    Box::new(Prop {
                        location: Box::new(Id(this)),
                        property: Box::new(VarExpr(Str("stack".to_string()))),
                    }),
                    Box::new(VarExpr(Var::Int(5))),
                )),
                right: Box::new(VarExpr(Var::Int(5))),
            })]
        );
    }
}
