use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;

use anyhow::Context;
use metrics_macros::increment_counter;
use moor_value::AsByteBuffer;
use tracing::{info, span, trace, warn};

use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::Var;

use crate::compiler::codegen::compile;
use crate::db::LoaderInterface;
use crate::textdump::{Object, TextdumpReader};
use moor_value::model::objects::{ObjAttrs, ObjFlag};
use moor_value::model::props::PropFlag;
use moor_value::model::r#match::Preposition;
use moor_value::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
use moor_value::model::verbs::VerbFlag;

struct RProp {
    definer: Objid,
    name: String,
    owner: Objid,
    flags: u8,
    value: Var,
}

fn resolve_prop(omap: &BTreeMap<Objid, Object>, offset: usize, o: &Object) -> Option<RProp> {
    let local_len = o.propdefs.len();
    if offset < local_len {
        let name = o.propdefs[offset].clone();
        let pval = &o.propvals[offset];
        return Some(RProp {
            definer: o.id,
            name,
            owner: pval.owner,
            flags: pval.flags,
            value: pval.value.clone(),
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
        _ => {
            PrepSpec::Other(Preposition::from_repr(vprep as u16).expect("Unsupported preposition"))
        }
    }
}

fn cv_aspec_flag(flags: u16) -> ArgSpec {
    match flags {
        VF_ASPEC_NONE => ArgSpec::None,
        VF_ASPEC_ANY => ArgSpec::Any,
        VF_ASPEC_THIS => ArgSpec::This,
        _ => panic!("Unsupported argsec"),
    }
}

#[tracing::instrument(skip(ldr))]
pub async fn textdump_load(ldr: &mut dyn LoaderInterface, path: &str) -> Result<(), anyhow::Error> {
    let textdump_import_span = span!(tracing::Level::INFO, "textdump_import");
    let _enter = textdump_import_span.enter();

    let jhcore = File::open(path)?;
    let br = BufReader::new(jhcore);
    let mut tdr = TextdumpReader::new(br);
    let td = tdr.read_textdump()?;

    info!("Instantiating objects");
    for (objid, o) in &td.objects {
        let flags: BitEnum<ObjFlag> = BitEnum::from_u8(o.flags);

        trace!(
            objid = ?objid, name=o.name, flags=?flags, "Creating object",
        );
        ldr.create_object(
            Some(*objid),
            ObjAttrs::new()
                // .owner(o.owner)
                // .location(o.location)
                .name(o.name.as_str())
                // .parent(o.parent)
                .flags(flags),
        )
        .await
        .with_context(|| format!("Unable to create object #{}", objid.0))?;

        increment_counter!("textdump.created_objects");
    }

    info!("Setting object attributes (parent/location/owner)");
    for (objid, o) in &td.objects {
        trace!(owner = ?o.owner, parent = ?o.parent, location = ?o.location, "Setting attributes");
        ldr.set_object_owner(*objid, o.owner)
            .await
            .with_context(|| format!("Unable to set owner of {}", *objid))?;
        ldr.set_object_parent(*objid, o.parent)
            .await
            .with_context(|| format!("Unable to set parent of {}", *objid))?;
        ldr.set_object_location(*objid, o.location)
            .await
            .with_context(|| format!("Unable to set location of {}", *objid))?;
    }

    info!("Defining properties...");

    // Define props. This means going through and just adding at the very root, which will create
    // initially-clear state in all the descendants. A second pass will then go through and update
    // flags and values for the children.
    for (objid, o) in &td.objects {
        for (pnum, _p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: BitEnum<PropFlag> = BitEnum::from_u8(resolved.flags);
            if resolved.definer == *objid {
                trace!(definer = ?objid.0, name = resolved.name, "Defining property");
                let value = Some(resolved.value);
                ldr.define_property(
                    resolved.definer,
                    *objid,
                    resolved.name.as_str(),
                    resolved.owner,
                    flags,
                    value,
                )
                .await
                .with_context(|| format!("Unable to define property on {}", objid))?;
            }
            increment_counter!("textdump.defined_properties");
        }
    }

    info!("Setting property values & info");
    for (objid, o) in &td.objects {
        for (pnum, p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: BitEnum<PropFlag> = BitEnum::from_u8(p.flags);
            trace!(objid = ?objid.0, name = resolved.name, flags = ?flags, "Setting property");
            let value = (!p.is_clear).then(|| p.value.clone());

            ldr.set_update_property(*objid, resolved.name.as_str(), p.owner, flags, value)
                .await
                .with_context(|| format!("Unable to set property on {}", objid))?;
            increment_counter!("textdump.set_properties");
        }
    }

    info!("Defining verbs...");
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

            let Some(verb) = td.verbs.get(&(*objid, vn)) else {
                increment_counter!("textdump.missing_programs");
                warn!(
                    "Missing program for defined verb #{}/{} ({:?})",
                    objid.0, vn, names
                );
                continue;
            };

            let program = compile(verb.program.as_str()).with_context(|| {
                format!(
                    "Compile error in #{}/{} ({:?}): {:?}",
                    objid.0, vn, names, verb.program
                )
            })?;

            // Encode the binary (for now using bincode)
            let binary = program.with_byte_buffer(|d| Vec::from(d));

            ldr.add_verb(*objid, names.clone(), v.owner, flags, argspec, binary)
                .await
                .with_context(|| format!("Unable to add verb #{}/{} ({:?})", objid.0, vn, names))?;
            trace!(objid = ?objid.0, name = ?vn, "Added verb");
            increment_counter!("textdump.compiled_verbs");
        }
    }
    info!("Verbs defined.");

    info!("Import complete.");

    Ok(())
}
