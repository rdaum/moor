extern crate core;

use rusqlite::Connection;

use crate::db::sqllite::SQLiteTx;
use crate::model::objects::Objects;
use crate::model::props::{PropDefs, PropFlag, Propdef};
use crate::model::var::Var;

pub mod compiler;
mod db;
pub mod grammar;
pub mod model;
pub mod textdump;

fn main() {
    println!("Hello, world!");

    // let jhcore = File::open("JHCore-DEV-2.db").unwrap();
    // let br = BufReader::new(jhcore);
    // let mut tdr = TextdumpReader::new(br);
    // let td=  tdr.read_textdump().unwrap();
    // // Now iterate and compile each verb...
    // for v in &td.verbs {
    //     println!("Compiling verb {}:{}", v.objid.0, v.verbnum);
    //     let program = compile_program(&v.program).unwrap();
    //     println!("Compiled to AST {:?}", program);
    // }
}
