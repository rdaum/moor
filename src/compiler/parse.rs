use std::rc::Rc;
use crate::compiler::ast::{Arg, CondArm, ExceptArm, Expr, LoopKind, Stmt};
use crate::grammar::moolexer::mooLexer;
use crate::grammar::mooparser::{mooParser, mooParserContextType, ElseifsContext, ElseifsContextAttrs, IfContext, IfContextAttrs, ProgramContext, ProgramContextAttrs, StatementsContext, StatementsContextAttrs, ForExprContext, ForExprContextAttrs, ForRangeContext, ForRangeContextAttrs, WhileContext, WhileContextAttrs, ForkContext, ForkContextAttrs, ExprContextAll, StatementsContextAll, ExprStmtContext, ExprStmtContextAttrs, BreakContext, BreakContextAttrs, ContinueContext, ContinueContextAttrs, ReturnContext, ReturnContextAttrs, TryExceptContext, TryExceptContextAttrs, ExceptsContext, ExceptsContextAttrs, ExceptContext, ExceptContextAttrs, CodesContext, CodesContextAttrs, Ne_arglistContext, ArgContext, SpliceArgContext, ArgContextAttrs, SpliceArgContextAttrs, Ne_arglistContextAttrs, TryFinallyContext, TryFinallyContextAttrs, StringContext};
use crate::grammar::moovisitor::mooVisitor;
use antlr_rust::common_token_stream::CommonTokenStream;
use antlr_rust::error_listener::ErrorListener;
use antlr_rust::errors::ANTLRError;
use antlr_rust::recognizer::Recognizer;
use antlr_rust::token_factory::TokenFactory;
use antlr_rust::tree::{ParseTree, ParseTreeVisitor, TerminalNode, Visitable};
use antlr_rust::{InputStream, Parser};
use anyhow::anyhow;

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

struct LoopEntry {
    name : Option<String>,
    is_barrier: bool
}

pub struct Name(usize);
struct Names {
    names : Vec<String>
}
impl Default for Names {
    fn default() -> Self {
        Self {
            names: vec![]
        }
    }
}
impl Names {
    pub fn find_or_add_name(&mut self, name : &String) -> Name {
        match self.names.iter().position(|n| {
            n.as_str() == name
        }) {
            None => {
                let pos = self.names.len();
                self.names.push(String::from(name));
                Name(pos)
            }
            Some(n) => {
                Name(n)
            }
        }
    }
}

pub struct ASTGenVisitor {
     _statement_stack: Vec<Vec<Stmt>>,
     _expr_stack: Vec<Expr>,
     _cond_arm_stack: Vec<Vec<CondArm>>,
     _loop_stack: Vec<LoopEntry>,
    _excepts_stack: Vec<Vec<ExceptArm>>,
    _args_stack: Vec<Vec<Arg>>,
    _names: Names,
}

impl ASTGenVisitor {
    pub fn new() -> Self {
        Self {
            _statement_stack: Default::default(),
            _expr_stack: Default::default(),
            _cond_arm_stack: Default::default(),
            _loop_stack: Default::default(),
            _excepts_stack: Default::default(),
            _args_stack: Default::default(),
            _names: Default::default(),
        }
    }
}

enum LoopExitKind {
    Break,
    Continue
}
impl ASTGenVisitor {
    // Loop scope management
    fn push_loop_name(&mut self, name : Option<&String>) {
        self._loop_stack.push(LoopEntry {
            name: name.map(String::from),
            is_barrier: false
        })
    }
    fn resume_loop_scope(&mut self) -> LoopEntry {
        let last_entry = self._loop_stack.pop();
        match last_entry {
            None => {
                // TODO should be a recoverable error?
                panic!("PARSER: Empty loop stack in RESUME_LOOP_SCOPE!")
            }
            Some(loop_entry) if !loop_entry.is_barrier => {
                // TODO should be a recoverable error?
                panic!("PARSER: Tried to resume non-loop-scope barrier!")
            }
            Some(loop_entry) => loop_entry
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
            Some(loop_entry) => loop_entry
        }
    }
    fn suspend_loop_scope(&mut self) {
        self._loop_stack.push(LoopEntry {
            name: None,
            is_barrier: true
        })
    }
    fn check_loop_name(&mut self, name : Option<&String>, kind: LoopExitKind) -> Result<(), anyhow::Error>{
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
                    if e.is_barrier { return false }
                    if let Some(name) = &e.name {
                        if name == n {
                            return true;
                        }
                    }
                    false
                });
                if entry.is_some() {
                    return Ok(())
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
    fn find_id(&mut self, name : &String) -> Name {
        self._names.find_or_add_name(name)
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

    fn get_id(id : &Option<Rc<TerminalNode<mooParserContextType>>>) -> String {
        id.as_ref().unwrap().get_text()
    }

    fn get_opt_id(id : &Option<Rc<TerminalNode<mooParserContextType>>>) -> Option<String> {
        id.as_ref().map(|s| {
            String::from(s.get_text().as_str())
        })
    }

}

impl<'node> ParseTreeVisitor<'node, mooParserContextType> for ASTGenVisitor {}

impl<'node> mooVisitor<'node> for ASTGenVisitor {

    fn visit_program(&mut self, ctx: &ProgramContext<'node>) {
        ctx.statements().iter().for_each(|item| item.accept(self));
    }

    fn visit_statements(&mut self, ctx: &StatementsContext<'node>) {
        self._statement_stack.push(vec![]);
        ctx.statement_all().iter().for_each(|item| item.accept(self));
    }

    fn visit_If(&mut self, ctx: &IfContext<'node>) {
        let condition = self.reduce_expr(&ctx.expr());
        let statements = self.reduce_statements(&ctx.statements(0));

        self._cond_arm_stack.push(vec![
            CondArm {
                condition,
                statements
            }
        ]);
        for ei in ctx.elseifs().iter() {
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

    fn visit_elseifs(&mut self, ctx: &ElseifsContext<'node>) {
        let condition = self.reduce_expr(&ctx.expr());
        let statements = self.reduce_statements(&ctx.statements());

        let cond_arm = CondArm {
            condition,
            statements
        };
        self._cond_arm_stack.last_mut().unwrap().push(cond_arm);
    }


    fn visit_ForExpr(&mut self, ctx: &ForExprContext<'node>) {
        let id = Self::get_id(&ctx.ID());
        self.push_loop_name(Some(&id));
        let _name = self.find_id(&id);

        let expr_node = self.reduce_expr(&ctx.expr());

        let body = self.reduce_statements(&ctx.statements());

        let stmt = Stmt::List {
            expr: expr_node,
            body
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
        let stmt = Stmt::Range {
            id,
            from,
            to,
            body
        };
        self.pop_loop_name();
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_While(&mut self, ctx: &WhileContext<'node>) {
        // Handle ID's while loops as well as non-ID'd
        let id = Self::get_opt_id(&ctx.ID());
        self.push_loop_name(id.as_ref());
        let id = id.map(|id| {
            self.find_id(&id)
        });

        let condition = self.reduce_expr(&ctx.condition);
        let body = self.reduce_statements(&ctx.statements());

        let stmt = Stmt::Loop {
            kind: LoopKind::While,
            id,
            condition,
            body
        };

        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_Fork(&mut self, ctx: &ForkContext<'node>) {
        self.suspend_loop_scope();
        let id = Self::get_opt_id(&ctx.ID());
        self.push_loop_name(id.as_ref());
        let id = id.map(|id| {
            self.find_id(&id)
        });

        let time = self.reduce_expr(&ctx.time);

        let body = self.reduce_statements(&ctx.statements());

        let stmt = Stmt::Fork {
            id,
            time,
            body
        };
        self._statement_stack.last_mut().unwrap().push(stmt);
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

    fn visit_Break(&mut self, ctx: &BreakContext<'node>) {
        let id = Self::get_opt_id(&ctx.ID());
        // TODO propagate error correctly
        self.check_loop_name(id.as_ref(), LoopExitKind::Break).expect("Bad break");

        let exit = id.as_ref().map(|id| {
            self.find_id(id)
        });
        let stmt = Stmt::Break {
            exit
        };
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_Continue(&mut self, ctx: &ContinueContext<'node>) {
        let id = Self::get_opt_id(&ctx.ID());
        // TODO propagate error correctly
        self.check_loop_name(id.as_ref(), LoopExitKind::Continue).expect("Bad break");

        let exit = id.as_ref().map(|id| {
            self.find_id(id)
        });
        let stmt = Stmt::Continue {
            exit
        };
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_Return(&mut self, ctx: &ReturnContext<'node>) {
        let expr = self.reduce_opt_expr(&ctx.expr());
        let stmt = Stmt::Return {expr};
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_TryExcept(&mut self, ctx: &TryExceptContext<'node>) {
        let body = self.reduce_statements(&ctx.statements());
        self._excepts_stack.push(vec![]);
        ctx.excepts().as_ref().iter().for_each(|e| {
            e.accept(self)
        });
        let excepts = self._excepts_stack.pop().unwrap();
        let stmt = Stmt::Catch {
            body,
            excepts
        };
        self._statement_stack.last_mut().unwrap().push(stmt);
    }

    fn visit_TryFinally(&mut self, ctx: &TryFinallyContext<'node>) {
        let body = self.reduce_statements(&ctx.statements(0));
        let handler = self.reduce_statements(&ctx.statements(1));
        self._statement_stack.last_mut().unwrap().push(Stmt::Finally {
            body,
            handler
        })
    }

    fn visit_excepts(&mut self, ctx: &ExceptsContext<'node>) {
        // Just visit each 'except' arm, and that will fill the _excepts_stack
        ctx.except_all().iter().for_each(|e| {
            e.accept(self)
        });
    }

    fn visit_except(&mut self, ctx: &ExceptContext<'node>) {
        // Produce an except arm
        let id = Self::get_opt_id(&ctx.ID());
        let id = id.map(|id| {
            self.find_id(&id)
        });

        self._args_stack.push(vec![]);
        let codes = self._args_stack.pop().unwrap();
        let statements = self.reduce_statements(&ctx.statements());
        let except_arm = ExceptArm {
            id,
            codes,
            statements
        };
        self._excepts_stack.last_mut().unwrap().push(except_arm);
    }

    fn visit_codes(&mut self, ctx: &CodesContext<'node>) {
        // Push to the arglist.
        ctx.ne_arglist().iter().for_each(|al| {
            al.accept(self)
        });

    }

    fn visit_ne_arglist(&mut self, ctx: &Ne_arglistContext<'node>) {
        self._args_stack.push(vec![]);
        ctx.argument_all().iter().for_each(|a| {
            a.accept(self)
        });
        let _arguments = self._args_stack.pop().unwrap();
    }

    fn visit_Arg(&mut self, ctx: &ArgContext<'node>) {
        let expr = self.reduce_expr(&ctx.expr());
        self._args_stack.last_mut().unwrap().push(Arg::Normal(expr));
    }

    fn visit_SpliceArg(&mut self, ctx: &SpliceArgContext<'node>) {
        let expr = self.reduce_expr(&ctx.expr());
        self._args_stack.last_mut().unwrap().push(Arg::Splice(expr));
    }

    fn visit_String(&mut self, ctx: &StringContext<'node>) {
        let _string = ctx.get_text().as_str();
        
    }
}

pub fn parse(program: &str) {
    let is = InputStream::new(program);
    let lexer = mooLexer::new(is);
    let source = CommonTokenStream::new(lexer);
    let mut parser = mooParser::new(source);
    println!("Compiled");

    let err_listener = Box::new(VerbCompileErrorListener {
        program: String::from(program),
    });

    parser.add_error_listener(err_listener);
    let program_context = parser.program().unwrap();

    let mut astgen_visitor = ASTGenVisitor::new();

    program_context.accept(&mut astgen_visitor);
    // let tree = program_context.to_string_tree(&*parser);
}
