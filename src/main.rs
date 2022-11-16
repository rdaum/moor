






use rusqlite::Connection;




use crate::db::sqllite::SQLiteTx;
use crate::model::objects::Objects;
use crate::model::props::{Propdef, PropDefs, PropFlag};
use crate::model::var::{Var};




pub mod grammar;
pub mod model;
pub mod textdump;
pub mod compiler;
mod db;

fn main() {
    println!("Hello, world!");

    let mut conn = Connection::open_in_memory().unwrap();
    let tx = conn.transaction().unwrap();
    let mut s = SQLiteTx::new(tx).unwrap();
    s.initialize_schema().unwrap();

    let o = s.create_object().unwrap();

    s.add_propdef(Propdef {
        oid: o,
        pname: String::from("test"),
        owner: o,
        flags: vec![PropFlag::Chown, PropFlag::Read],
        val: Var::Str(String::from("testing"))
    }).unwrap();

    let pds= s.get_propdefs(o).unwrap();
    assert_eq!(pds.len(), 1);
    assert_eq!(pds[0].owner, o);
    assert_eq!(pds[0].pname, "test");

    let c = s.count_propdefs(o).unwrap();
    assert_eq!(c, 1);

    s.delete_propdef(o, "test").unwrap();

    let c = s.count_propdefs(o).unwrap();
    assert_eq!(c, 0);

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
