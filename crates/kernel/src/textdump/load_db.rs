// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use tracing::{info, span, trace};

use moor_compiler::compile;
use moor_compiler::Program;
use moor_db::loader::LoaderInterface;
use moor_values::model::Preposition;
use moor_values::model::PropFlag;
use moor_values::model::VerbFlag;
use moor_values::model::{ArgSpec, PrepSpec, VerbArgsSpec};
use moor_values::model::{ObjAttrs, ObjFlag};
use moor_values::util::BitEnum;
use moor_values::var::Objid;
use moor_values::var::Var;
use moor_values::{AsByteBuffer, NOTHING};

use crate::textdump::read::TextdumpReaderError;
use crate::textdump::{
    Object, TextdumpReader, PREP_ANY, PREP_NONE, VF_ASPEC_ANY, VF_ASPEC_NONE, VF_ASPEC_THIS,
    VF_DEBUG, VF_DOBJSHIFT, VF_EXEC, VF_IOBJSHIFT, VF_OBJMASK, VF_PERMMASK, VF_READ, VF_WRITE,
};

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
pub fn textdump_load(
    ldr: Rc<dyn LoaderInterface>,
    path: PathBuf,
) -> Result<(), TextdumpReaderError> {
    let textdump_import_span = span!(tracing::Level::INFO, "textdump_import");
    let _enter = textdump_import_span.enter();

    let corefile =
        File::open(path).map_err(|e| TextdumpReaderError::CouldNotOpenFile(e.to_string()))?;

    let br = BufReader::new(corefile);

    read_textdump(ldr, br)?
}

pub fn read_textdump<T: io::Read>(
    loader: Rc<dyn LoaderInterface>,
    reader: BufReader<T>,
) -> Result<Result<(), TextdumpReaderError>, TextdumpReaderError> {
    let mut tdr = TextdumpReader::new(reader);
    let td = tdr.read_textdump()?;

    info!("Instantiating objects");
    for (objid, o) in &td.objects {
        let flags: BitEnum<ObjFlag> = BitEnum::from_u8(o.flags);

        trace!(
            objid = ?objid, name=o.name, flags=?flags, "Creating object",
        );
        loader
            .create_object(
                Some(*objid),
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, flags, &o.name),
            )
            .unwrap();
    }

    info!("Setting object attributes (parent/location/owner)");
    for (objid, o) in &td.objects {
        trace!(owner = ?o.owner, parent = ?o.parent, location = ?o.location, "Setting attributes");
        loader.set_object_owner(*objid, o.owner).map_err(|e| {
            TextdumpReaderError::LoadError(format!("setting owner of {}", objid), e.clone())
        })?;
        loader.set_object_parent(*objid, o.parent).map_err(|e| {
            TextdumpReaderError::LoadError(format!("setting parent of {}", objid), e.clone())
        })?;
        loader.set_object_location(*objid, o.location).unwrap();
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
                loader
                    .define_property(
                        resolved.definer,
                        *objid,
                        resolved.name.as_str(),
                        resolved.owner,
                        flags,
                        value,
                    )
                    .unwrap();
            }
        }
    }

    info!("Setting property values & info");
    for (objid, o) in &td.objects {
        for (pnum, p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: BitEnum<PropFlag> = BitEnum::from_u8(p.flags);
            trace!(objid = ?objid.0, name = resolved.name, flags = ?flags, "Setting property");
            let value = (!p.is_clear).then(|| p.value.clone());

            loader
                .set_property(*objid, resolved.name.as_str(), p.owner, flags, value)
                .unwrap();
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

            let program = match td.verbs.get(&(*objid, vn)) {
                Some(verb) if verb.program.is_some() => {
                    compile(verb.program.clone().unwrap().as_str()).map_err(|e| {
                        TextdumpReaderError::VerbCompileError(
                            format!("compiling verb #{}/{} ({:?})", objid.0, vn, names),
                            e.clone(),
                        )
                    })?
                }
                // If the verb program is missing, then it's an empty program, and we'll put in
                // an empty binary.
                _ => Program {
                    literals: vec![],
                    jump_labels: vec![],
                    var_names: Default::default(),
                    main_vector: Arc::new(vec![]),
                    fork_vectors: vec![],
                    line_number_spans: vec![],
                },
            };

            let binary =
                // Encode the binary (for now using bincode)
                program.with_byte_buffer(|d| Vec::from(d)).expect("Failed to encode program");

            loader
                .add_verb(*objid, names.clone(), v.owner, flags, argspec, binary)
                .map_err(|e| {
                    TextdumpReaderError::LoadError(
                        format!("adding verb #{}/{} ({:?})", objid.0, vn, names),
                        e.clone(),
                    )
                })?;
            trace!(objid = ?objid.0, name = ?vn, "Added verb");
        }
    }
    info!("Verbs defined.");

    info!("Import complete.");

    Ok(Ok(()))
}
