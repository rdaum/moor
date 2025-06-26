// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::read::TextdumpReaderError;
use crate::{
    Object, PREP_ANY, PREP_NONE, TextdumpReader, TextdumpVersion, VF_ASPEC_ANY, VF_ASPEC_NONE,
    VF_ASPEC_THIS, VF_DEBUG, VF_DOBJSHIFT, VF_EXEC, VF_IOBJSHIFT, VF_OBJMASK, VF_PERMMASK, VF_READ,
    VF_WRITE,
};
use moor_common::matching::Preposition;
use moor_common::model::PropFlag;
use moor_common::model::VerbFlag;
use moor_common::model::loader::LoaderInterface;
use moor_common::model::{ArgSpec, PrepSpec, VerbArgsSpec};
use moor_common::model::{ObjAttrs, ObjFlag};
use moor_common::program::ProgramType;
use moor_common::util::BitEnum;
use moor_compiler::compile;
use moor_compiler::{CompileOptions, Program};
use moor_var::Obj;
use moor_var::Var;
use moor_var::{NOTHING, Symbol};
use semver::Version;
use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;
use tracing::{info, span, trace, warn};

struct RProp {
    definer: Obj,
    name: Symbol,
    owner: Obj,
    flags: u8,
    value: Var,
}

fn resolve_prop(omap: &BTreeMap<Obj, Object>, offset: usize, o: &Object) -> Option<RProp> {
    let local_len = o.propdefs.len();
    if offset < local_len {
        let name = o.propdefs[offset];
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

pub fn textdump_load(
    ldr: &mut dyn LoaderInterface,
    path: PathBuf,
    moor_version: Version,
    features_config: CompileOptions,
) -> Result<(), TextdumpReaderError> {
    let textdump_import_span = span!(tracing::Level::INFO, "textdump_import");
    let _enter = textdump_import_span.enter();

    let corefile =
        File::open(path).map_err(|e| TextdumpReaderError::CouldNotOpenFile(e.to_string()))?;

    let br = BufReader::new(corefile);

    read_textdump(ldr, br, moor_version, features_config)
}

/// Returns true if the compile options are compatible with another configuration, for the purposes
/// of textdump loading.
///
/// Which means that if the other configuration has a feature enabled, this configuration
/// must also have it enabled.
/// The other way around is fine.
pub fn is_textdump_compatible(a: &CompileOptions, other: &CompileOptions) -> bool {
    (!other.lexical_scopes || a.lexical_scopes)
        && (!other.map_type || a.map_type)
        && (!other.bool_type || a.bool_type)
        && (!other.flyweight_type || a.flyweight_type)
        && (!other.symbol_type || a.symbol_type)
        && (!other.list_comprehensions || a.list_comprehensions)
        && (!other.custom_errors || a.custom_errors)
}

pub fn read_textdump<T: io::Read>(
    loader: &mut dyn LoaderInterface,
    reader: BufReader<T>,
    moo_version: Version,
    compile_options: CompileOptions,
) -> Result<(), TextdumpReaderError> {
    let mut tdr = TextdumpReader::new(reader)?;
    // Validate the textdumps' version string against the configuration of the server.
    match &tdr.version {
        TextdumpVersion::LambdaMOO(v) => {
            if (*v as u16) > 4 {
                return Err(TextdumpReaderError::VersionError(format!(
                    "Unsupported LambdaMOO DB version: {v}"
                )));
            }
        }
        TextdumpVersion::ToastStunt(v) => {
            // We don't support a lot of "Toast" features, but we'll try to import the textdump
            // as best we can and then things fail at the compile or runtime level for features
            // we don't support.
            warn!(
                "Importing a ToastStunt textdump version ({v}), which may contain features, builtins,\
                     and datatypes unsupported by mooR. This may cause errors requiring manual intervention."
            );
        }
        TextdumpVersion::Moor(v, other_options, _encoding) => {
            // Semver major versions must match.
            // TODO: We will let minor and patch versions slide, but may need to get stricter
            //   about minor in the future.
            if v.major != moo_version.major {
                return Err(TextdumpReaderError::VersionError(
                    "Incompatible major moor version".to_string(),
                ));
            }

            // Features mut be compatible
            if !is_textdump_compatible(&compile_options, other_options) {
                return Err(TextdumpReaderError::VersionError(
                    "Incompatible compiler features".to_string(),
                ));
            }
        }
    }

    let td = tdr.read_textdump()?;

    // For textdump imports we wrap unknown functions up in `call_function`...
    let mut compile_options = compile_options.clone();
    compile_options.call_unsupported_builtins = true;

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
        loader.set_object_owner(objid, &o.owner).map_err(|e| {
            TextdumpReaderError::LoadError(format!("setting owner of {objid}"), e.clone())
        })?;
        loader.set_object_parent(objid, &o.parent).map_err(|e| {
            TextdumpReaderError::LoadError(format!("setting parent of {objid}"), e.clone())
        })?;
        loader.set_object_location(objid, &o.location).unwrap();
    }

    info!("Defining properties...");

    // Define props. This means going through and just adding at the very root, which will create
    // initially-clear state in all the descendants. A second pass will then go through and update
    // flags and common for the children.
    for (objid, o) in &td.objects {
        for (pnum, _p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: BitEnum<PropFlag> = BitEnum::from_u8(resolved.flags);
            if resolved.definer == *objid {
                let value = Some(resolved.value);
                loader
                    .define_property(
                        &resolved.definer,
                        objid,
                        resolved.name,
                        &resolved.owner,
                        flags,
                        value,
                    )
                    .unwrap();
            }
        }
    }

    info!("Setting property common & info");
    for (objid, o) in &td.objects {
        for (pnum, p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: BitEnum<PropFlag> = BitEnum::from_u8(p.flags);
            let value = (!p.is_clear).then(|| p.value.clone());

            loader
                .set_property(objid, resolved.name, Some(p.owner), Some(flags), value)
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

            let names: Vec<_> = v.name.split(' ').map(Symbol::mk).collect();

            let program = match td.verbs.get(&(*objid, vn)) {
                Some(verb) if verb.program.is_some() => compile(
                    verb.program.clone().unwrap().as_str(),
                    compile_options.clone(),
                )
                .map_err(|e| {
                    TextdumpReaderError::VerbCompileError(
                        format!("compiling verb #{objid}/{vn} ({names:?})"),
                        e.clone(),
                    )
                })?,
                // If the verb program is missing, then it's an empty program, and we'll put in
                // an empty binary.
                _ => Program::new(),
            };

            loader
                .add_verb(
                    objid,
                    &names,
                    &v.owner,
                    flags,
                    argspec,
                    ProgramType::MooR(program),
                )
                .map_err(|e| {
                    TextdumpReaderError::LoadError(
                        format!("adding verb #{objid}/{vn} ({names:?})"),
                        e.clone(),
                    )
                })?;
            trace!(objid = ?objid, name = ?vn, "Added verb");
        }
    }
    info!("Verbs defined.");

    info!("Import complete.");

    Ok(())
}
