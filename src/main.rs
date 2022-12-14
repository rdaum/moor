extern crate core;

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use bincode::config;
use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use enumset::EnumSet;
use int_enum::IntEnum;
use rusqlite::Connection;

use crate::compiler::codegen::compile;
use crate::db::sqllite::SQLiteTx;
use crate::model::objects::{ObjAttrs, ObjFlag, Objects};
use crate::model::props::{PropDefs, PropFlag, Properties};
use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
use crate::model::var::{Objid, Var};
use crate::model::verbs::{Program, VerbFlag, Verbs};
use crate::model::ObjDB;
use crate::textdump::{Object, TextdumpReader};
use crate::vm::execute::{ExecutionResult, VM};
use crate::vm::state::ObjDBState;

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

    let parent = omap.get(&o.parent)?;
    resolve_prop(omap, offset, parent)
}

const VF_READ: u16 = 01;
const VF_WRITE: u16 = 02;
const VF_EXEC: u16 = 04;
const VF_DEBUG: u16 = 010;
const VF_PERMMASK: u16 = 0xf;
const VF_DOBJSHIFT: u16 = 4;
const VF_IOBJSHIFT: u16 = 4;
const VF_OBJMASK: u16 = 0x3;

const VF_ASPEC_NONE: u16 = 0;
const VF_ASPEC_ANY: u16 = 1;
const VF_ASPEC_THIS: u16 = 2;

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
        _ => panic!("Unsupported argpsec"),
    }
}

fn textdump_load(conn: &mut Connection, path: &str) -> Result<(), anyhow::Error> {
    let mut s: &mut dyn ObjDB = &mut SQLiteTx::new(conn)?;
    s.initialize()?;

    let jhcore = File::open(path)?;
    let br = BufReader::new(jhcore);
    let mut tdr = TextdumpReader::new(br);
    let td = tdr.read_textdump()?;

    // Pass 1 Create objects
    eprintln!("Instantiating objects...");
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
        )?;
    }
    eprintln!("Instantiated objects\nDefining props...");

    // Pass 2 define props
    for (objid, o) in &td.objects {
        for (pnum, p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: EnumSet<PropFlag> = EnumSet::from_u8(resolved.flags);
            if resolved.definer == *objid {
                let pid =
                    s.add_propdef(*objid, resolved.name.as_str(), resolved.owner, flags, None)?;
            }
        }
    }

    eprintln!("Defined props\nSetting props...");
    // Pass 3 set props
    for (objid, o) in &td.objects {
        for (pnum, p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: EnumSet<PropFlag> = EnumSet::from_u8(resolved.flags);
            let pdf = s.get_propdef(resolved.definer, resolved.name.as_str())?;
            s.set_property(pdf.pid, *objid, p.value.clone(), resolved.owner, flags)?;
        }
    }

    eprintln!("Set props\nDefining verbs...");
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

            let names: Vec<&str> = v.name.split(" ").collect();

            let verb = match td.verbs.get(&(*objid, vn)) {
                None => {
                    println!("Could not find verb #{}/{} ({:?}", objid.0, vn, names);
                    continue;
                }
                Some(v) => v,
            };

            eprintln!(
                "Compiling #{}/{} (#{}:{})...",
                objid.0, names[0], objid.0, vn
            );

            let binary = match compile(verb.program.as_str()) {
                Ok(b) => b,
                Err(e) => {
                    panic!("Compile error in #{}/{}: {:?}", objid.0, names[0], e);
                }
            };

            let prg = bincode::serde::encode_to_vec(binary, config::standard())
                .expect("Could not serialize program");
            s.add_verb(
                *objid,
                names,
                v.owner,
                flags,
                argspec,
                Program(bytes::Bytes::from(prg)),
            )?;
        }
    }
    eprintln!("Verbs defined.\nImport complete.");

    s.commit()?;

    Ok(())
}

#[derive(Parser, Debug)] // requires `derive` feature
struct Args {
    #[arg(value_name = "DB", help = "Path to database file to use or create", value_hint = ValueHint::FilePath)]
    db: std::path::PathBuf,

    #[arg(value_name = "Textdump", help = "Path to textdump to import", value_hint = ValueHint::FilePath)]
    textdump: Option<std::path::PathBuf>,
}

fn main() {
    let args: Args = Args::parse();

    eprintln!("Moor");

    let mut conn = Connection::open(args.db).unwrap();
    if let Some(textdump) = args.textdump {
        eprintln!("Loading textdump...");
        textdump_load(&mut conn, textdump.to_str().unwrap()).unwrap();
    }

    let mut tx = SQLiteTx::new(&mut conn).unwrap();
    let mut odb_state = ObjDBState { db: &tx };

    let mut vm = VM::new();
    eprintln!("Calling #0:do_login_command...");
    vm.prepare_call_verb(
        &mut odb_state,
        Objid(0),
        "do_login_command",
        false,
        Objid(0),
        Objid(0),
        ObjFlag::Wizard | ObjFlag::Programmer,
        Objid(0),
        vec![],
    )
    .unwrap();
    loop {
        let result = vm.exec(&mut odb_state).unwrap();
        match result {
            ExecutionResult::Complete => {
                eprintln!("Done.");
                break;
            }
            ExecutionResult::More => {}
        }
    }
}
