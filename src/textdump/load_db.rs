use crate::compiler::codegen::compile;
use crate::db::inmem_db::ImDB;
use crate::model::objects::{ObjAttrs, ObjFlag};
use crate::model::props::PropFlag;
use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
use crate::model::var::{Objid, Var};
use crate::model::verbs::VerbFlag;
use crate::textdump::{Object, TextdumpReader};
use crate::util::bitenum::BitEnum;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use tracing::{debug, info, span, warn};

struct RProp {
    definer: Objid,
    name: String,
    owner: Objid,
    flags: u8,
    _val: Var,
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
            _val: pval.value.clone(),
        });
    }

    let offset = offset - local_len;

    let parent = omap.get(&o.parent)?;
    resolve_prop(omap, offset, parent)
}

const VF_READ: u16 = 1;
const VF_WRITE: u16 = 2;
const VF_EXEC: u16 = 4;
const VF_DEBUG: u16 = 10;
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

pub fn textdump_load(s: &mut ImDB, path: &str) -> Result<(), anyhow::Error> {
    let textdump_import_span = span!(tracing::Level::INFO, "textdump_import");
    let _enter = textdump_import_span.enter();

    let jhcore = File::open(path)?;
    let br = BufReader::new(jhcore);
    let mut tdr = TextdumpReader::new(br);
    let td = tdr.read_textdump()?;

    let mut tx = s.do_begin_tx().expect("Unable to start transaction");

    // Pass 1 Create objects
    info!("Instantiating objects...");
    for (objid, o) in &td.objects {
        let flags: BitEnum<ObjFlag> = BitEnum::from_u8(o.flags);

        s.create_object(
            &mut tx,
            Some(*objid),
            ObjAttrs::new()
                .owner(o.owner)
                .location(o.location)
                .name(o.name.as_str())
                .parent(o.parent)
                .flags(flags),
        )?;
    }
    info!("Instantiated objects");
    info!("Defining props...");

    // Pass 2 define props
    for (objid, o) in &td.objects {
        for (pnum, _) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: BitEnum<PropFlag> = BitEnum::from_u8(resolved.flags);
            if resolved.definer == *objid {
                debug!("Defining prop: #{}.{}", objid.0, resolved.name);
                let res = s.add_propdef(
                    &mut tx,
                    *objid,
                    resolved.name.as_str(),
                    resolved.owner,
                    flags,
                    None,
                );
                if res.is_err() {
                    warn!(
                        "Unable to define property {}.{}: {:?}",
                        objid.0, resolved.name, res
                    );
                }
            }
        }
    }

    info!("Defined props");
    info!("Setting props...");
    // Pass 3 set props
    for (objid, o) in &td.objects {
        for (pnum, p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: BitEnum<PropFlag> = BitEnum::from_u8(resolved.flags);
            debug!("Setting prop: #{}.{}", objid.0, resolved.name);

            let result = s.get_propdef(&mut tx, resolved.definer, resolved.name.as_str());
            let Ok(pdf) = result else {
                warn!("Unable to find property {}.{}: {:?}", objid.0, resolved.name, result);
                continue;
            };
            let result = s.set_property(
                &mut tx,
                pdf.pid,
                *objid,
                p.value.clone(),
                resolved.owner,
                flags,
            );
            if result.is_err() {
                warn!(
                    "Unable to set property {}.{}: {:?}",
                    objid.0, resolved.name, result
                );
            }
        }
    }

    info!("Set props");
    info!("Defining verbs...");
    // Pass 4 define verbs
    for (objid, o) in &td.objects {
        for (vn, v) in o.verbdefs.iter().enumerate() {
            let mut flags: BitEnum<VerbFlag> = BitEnum::new();
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
            if permflags & VF_DEBUG != 0 {
                flags |= VerbFlag::Debug;
            }
            let dobjflags = (v.flags >> VF_DOBJSHIFT) & VF_OBJMASK;
            let iobjflags = (v.flags >> VF_IOBJSHIFT) & VF_OBJMASK;

            let argspec = VerbArgsSpec {
                dobj: cv_aspec_flag(dobjflags),
                prep: cv_prep_flag(v.prep),
                iobj: cv_aspec_flag(iobjflags),
            };

            let names: Vec<&str> = v.name.split(' ').collect();

            let verb = match td.verbs.get(&(*objid, vn)) {
                None => {
                    warn!("Could not find verb #{}/{} ({:?}", objid.0, vn, names);
                    continue;
                }
                Some(v) => v,
            };

            debug!(
                "Compiling #{}:{} (#{}:{})...",
                objid.0, names[0], objid.0, vn
            );

            let binary = match compile(verb.program.as_str()) {
                Ok(b) => b,
                Err(e) => {
                    panic!("Compile error in #{}/{}: {:?}", objid.0, names[0], e);
                }
            };

            let av = s.add_verb(
                &mut tx,
                *objid,
                names.clone(),
                v.owner,
                flags,
                argspec,
                binary,
            );
            if av.is_err() {
                warn!(
                    "Unable to add verb: #{}:{}: {:?}",
                    objid.0,
                    names.first().unwrap(),
                    av
                );
            }
        }
    }
    info!("Verbs defined.");
    info!("Import complete.");

    s.do_commit_tx(&mut tx)?;

    Ok(())
}
