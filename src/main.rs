



use std::fs::{File};

use std::io::BufReader;









use symbol_table::{SymbolTable};
use crate::compiler::parse::{compile_program};



use crate::textdump::{TextdumpReader};

pub mod grammar;
pub mod model;
pub mod textdump;
pub mod compiler;

fn main() {
    println!("Hello, world!");

    let jhcore = File::open("JHCore-DEV-2.db").unwrap();

    let br = BufReader::new(jhcore);

    let _symtab = SymbolTable::new();
    let mut tdr = TextdumpReader::new(br);

    let td=  tdr.read_textdump().unwrap();

    // Now iterate and compile each verb...
    for v in &td.verbs {
        println!("Compiling verb {}:{}", v.objid.0, v.verbnum);
        let program = compile_program(&v.program).unwrap();
        println!("Compiled to AST {:?}", program);
    }
}
