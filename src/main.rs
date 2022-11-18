extern crate core;

use enumset::EnumSet;
use rusqlite::Connection;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use crate::db::sqllite::SQLiteTx;
use crate::model::objects::{ObjAttrs, ObjFlag, Objects};
use crate::model::props::{PropDefs, PropFlag, Propdef, Properties};
use crate::model::var::{Objid, Var};
use crate::textdump::{Object, TextdumpReader};

pub mod compiler;
mod db;
pub mod grammar;
pub mod model;
pub mod textdump;

struct RProp {
    definer: Objid,
    name: String,
    owner: Objid,
    flags: u8,
    val: Var,
}
fn resolve_prop(omap: &HashMap<Objid, Object>, offset: usize, o: &Object) -> Option<RProp> {
    let local_len = o.propdefs.len();
    if offset < local_len {
        let name = o.propdefs[offset].clone();
        let pval = &o.propvals[offset];
        return Some(RProp {
            definer: o.id,
            name,
            owner: pval.owner,
            flags: pval.flags,
            val: pval.value.clone(),
        });
    }

    let offset = offset - local_len;

    let parent = omap.get(&o.parent).unwrap();
    resolve_prop(omap, offset, parent)
}

fn main() {
    println!("Hello, world!");

    let mut conn = Connection::open("test.db").unwrap();
    let tx = conn.transaction().unwrap();
    let mut s = SQLiteTx::new(tx).unwrap();
    s.initialize_schema().unwrap();

    let jhcore = File::open("JHCore-DEV-2.db").unwrap();
    let br = BufReader::new(jhcore);
    let mut tdr = TextdumpReader::new(br);
    let td = tdr.read_textdump().unwrap();

    // Pass 1 Create objects
    println!("Instantiating objects...");
    for (objid, o) in &td.objects {
        let flags: EnumSet<ObjFlag> = EnumSet::from_u8(o.flags);

        s.create_object(
            Some(*objid),
            ObjAttrs::new()
                .owner(o.owner)
                .location(o.location)
                .name(o.name.as_str())
                .parent(o.parent)
                .flags(flags),
        )
        .unwrap();
    }
    println!("Instantiated objects\nDefining props...");

    // Pass 2 define props
    for (objid, o) in &td.objects {
        for (pnum, p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: EnumSet<PropFlag> = EnumSet::from_u8(resolved.flags);
            if resolved.definer == *objid {
                let pid = s
                    .add_propdef(*objid, resolved.name.as_str(), resolved.owner, flags, None)
                    .unwrap();
                println!("Defined #{}.{} as {}", objid.0, resolved.name, pid.0)
            }
        }
    }

    println!("Defined props, setting props...\n");
    // Pass 3 set props
    for (objid, o) in &td.objects {
        for (pnum, p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: EnumSet<PropFlag> = EnumSet::from_u8(resolved.flags);
            let pdf = s
                .get_propdef(resolved.definer, resolved.name.as_str())
                .unwrap();
            s.set_property(pdf.pid, *objid, p.value.clone(), resolved.owner, flags)
                .unwrap();
            println!("Set pid {} on {} to {:?}", pdf.pid.0, objid.0, p.value.clone())
        }
    }

    s.tx.commit().unwrap();
}
