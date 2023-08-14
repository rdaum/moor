use crate::compiler::ast;
use crate::compiler::ast::Stmt;
use crate::compiler::parse::Parse;

fn unparse_expr(_expr: &ast::Expr) -> String {
    unimplemented!()
}

fn unparse_stmt(stmt: &ast::Stmt, indent: usize) -> String {
    let indent = indent + 4;
    let mut base_str = " ".repeat(indent).to_string();
    match stmt {
        Stmt::Cond { arms, otherwise } => {
            let cond_frag = unparse_expr(&arms[0].condition);
            let stmt_frag = unparse_stmts(&arms[0].statements, indent + 4);
            base_str.push_str(format!("if ({})\n{}\n", cond_frag, stmt_frag).as_str());
            for arm in arms.iter().skip(1) {
                let cond_frag = unparse_expr(&arm.condition);
                let stmt_frag = unparse_stmts(&arm.statements, indent + 4);
                base_str.push_str(format!("else if ({})\n{}\n", cond_frag, stmt_frag).as_str());
            }
            if !otherwise.is_empty() {
                let stmt_frag = unparse_stmts(otherwise, indent + 4);
                base_str.push_str(format!(" else\n{}", stmt_frag).as_str());
            }
            base_str.push_str("endif\n");
            base_str
        }
        _ => {
            unimplemented!("unparse_stmt: {:?}", stmt)
        }
    }
}

pub fn unparse_stmts(stms: &[Stmt], indent: usize) -> String {
    let mut buffer = String::new();
    for s in stms {
        buffer.push_str(&unparse_stmt(s, indent));
        buffer.push('\n');
    }
    buffer
}

pub fn unparse(tree: &Parse) -> String {
    let mut buffer = String::new();
    buffer.push_str(unparse_stmts(&tree.stmts, 0).as_str());
    buffer
}