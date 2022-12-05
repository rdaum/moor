extern crate core;

use enumset::EnumSet;
use int_enum::IntEnum;
use rusqlite::Connection;
use std::collections::HashMap;
use std::env::args;
use std::fs::File;
use std::io::BufReader;
use bincode::config;
use crate::compiler::codegen;
use crate::compiler::parse::parse_program;

use crate::db::sqllite::SQLiteTx;
use crate::model::ObjDB;
use crate::model::objects::{ObjAttrs, ObjFlag, Objects};
use crate::model::props::{PropDefs, PropFlag, Propdef, Properties};
use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
use crate::model::var::{Objid, Var};
use crate::model::verbs::{Program, VerbFlag, Verbs};
use crate::textdump::{Object, TextdumpReader, Verb};
use crate::vm::opcode::Binary;

pub mod compiler;
mod db;
pub mod grammar;
pub mod model;
pub mod textdump;
pub mod vm;

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

const VF_READ: u16 = 01;
const VF_WRITE: u16 = 02;
const VF_EXEC: u16 = 04;
const VF_DEBUG: u16 = 010;
const VF_PERMMASK : u16 = 0xf;
const VF_DOBJSHIFT : u16 = 4;
const VF_IOBJSHIFT : u16 = 4;
const VF_OBJMASK :u16 = 0x3;

const VF_ASPEC_NONE : u16 = 0;
const VF_ASPEC_ANY : u16 = 1;
const VF_ASPEC_THIS : u16 = 2;

const PREP_ANY: i16 = -2;
const PREP_NONE: i16 = -1;

fn cv_prep_flag(vprep: i16) -> PrepSpec {
    match vprep {
        PREP_ANY => PrepSpec::Any,
        PREP_NONE => PrepSpec::None,
        _ => PrepSpec::Other(vprep as u16),
    }
}

fn cv_aspec_flag(flags: u16) -> ArgSpec {
    match flags {
        VF_ASPEC_NONE => ArgSpec::None,
        VF_ASPEC_ANY => ArgSpec::Any,
        VF_ASPEC_THIS => ArgSpec::This,
        _ => panic!("Unsupported argpsec")
    }
}
fn main() {
    println!("Hello, world!");

    let mut conn = Connection::open("test.db").unwrap();
    let mut s : &mut dyn ObjDB   = &mut SQLiteTx::new(&mut conn).unwrap();
    s.initialize().unwrap();

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
            }
        }
    }

    println!("Defined props\nSetting props...");
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
        }
    }

    println!("Set props\nDefining verbs...");
    // Pass 4 define verbs
    for (objid, o) in &td.objects {
        for (vn, v) in o.verbdefs.iter().enumerate() {
            let mut flags: EnumSet<VerbFlag> = EnumSet::new();
            let permflags = v.flags & VF_PERMMASK;
            if permflags & VF_READ != 0 {
                flags |= VerbFlag::Read;
            }
            if permflags & VF_WRITE != 0 {
                flags |= VerbFlag::Write;
            }
            if permflags & VF_EXEC != 0 {
                flags |= VerbFlag::Exec;
            }
            let dobjflags = (v.flags >> VF_DOBJSHIFT) & VF_OBJMASK;
            let iobjflags = (v.flags >> VF_DOBJSHIFT) & VF_OBJMASK;

            let mut argspec = VerbArgsSpec {
                dobj: cv_aspec_flag(dobjflags),
                prep: cv_prep_flag(v.prep),
                iobj: cv_aspec_flag(iobjflags),
            };

            let names = v.name.split(" ").collect();

            let verb = match td.verbs.get(&(*objid, vn)) {
                None => {
                    println!("Could not find verb #{}/{} ({:?}", objid.0, vn, names);
                    continue;
                }
                Some(v) => {v}
            };

            let ast = parse_program(verb.program.as_str()).expect("Could not parse");
            let mut cg_state = codegen::State::new();
            for x in ast {
                cg_state.generate_stmt(&x).expect("Could not codegen");
            }

            // let binary = Binary {
            //     first_lineno: 0,
            //     ref_count: 0,
            //     literals: cg_state.literals,
            //     jump_labels: cg_state.jumps.iter().map(|jl| {
            //         jl.id
            //     }).collect(),
            //     var_names: cg_state.vars.iter().map(|vn| vn.name.0).collect(),
            //     main_vector: cg_state.ops,
            // };
            let prg = bytes::Bytes::new();
            // bincode::serde::encode_into_writer(binary, &prg, config::standard()).expect("Could not serialize program");
            s.add_verb(
                *objid,
                names,
                v.owner,
                flags,
                argspec,
                Program(prg),
            )
            .unwrap();
        }
    }
    println!("Verbs defined.\nImport complete.");

    s.commit().unwrap();


}
