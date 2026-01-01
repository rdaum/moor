// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::{
    Object, PREP_ANY, PREP_NONE, TextdumpReader, TextdumpVersion, VF_ASPEC_ANY, VF_ASPEC_NONE,
    VF_ASPEC_THIS, VF_DEBUG, VF_DOBJSHIFT, VF_EXEC, VF_IOBJSHIFT, VF_OBJMASK, VF_PERMMASK, VF_READ,
    VF_WRITE, read::TextdumpReaderError,
};
use moor_common::model::CompileError;
use moor_common::{
    matching::Preposition,
    model::{
        ArgSpec, ObjAttrs, ObjFlag, ObjectKind, PrepSpec, PropFlag, VerbArgsSpec, VerbFlag,
        loader::LoaderInterface,
    },
    util::BitEnum,
};
use moor_compiler::{CompileOptions, Program, compile};
use moor_var::{NOTHING, Obj, SYSTEM_OBJECT, Symbol, Var, program::ProgramType, v_str};
use semver::Version;
use std::{collections::BTreeMap, fs::File, io, io::BufReader, path::PathBuf};
use tracing::{info, span, trace, warn};

/// Options for textdump import behavior
#[derive(Debug, Clone, Default)]
pub struct TextdumpImportOptions {
    /// If true, continue importing even when verbs fail to compile.
    /// Failed verbs will be created with empty programs.
    /// If false (default), any compile error will abort the import.
    pub continue_on_compile_errors: bool,
}

/// Result of compiling a verb's source code
enum VerbCompileResult {
    /// Successfully compiled program
    Ok(Program),
    /// Compilation failed but we're continuing - use empty program
    SkippedWithWarning,
    /// Compilation failed and we should abort
    Error(TextdumpReaderError),
}

/// Compile a verb's source code, handling errors according to import options.
fn compile_verb_source(
    source: &str,
    compile_options: CompileOptions,
    import_options: &TextdumpImportOptions,
    objid: &Obj,
    vn: usize,
    names_str: &str,
    start_line: usize,
) -> VerbCompileResult {
    // Enable legacy type constants for all textdump imports, since textdumps
    // come from LambdaMOO/ToastStunt which use the old INT, OBJ, STR, etc. forms.
    let mut compile_options = compile_options;
    compile_options.legacy_type_constants = true;

    match compile(source, compile_options) {
        Ok(program) => VerbCompileResult::Ok(program),
        Err(e) => {
            if !import_options.continue_on_compile_errors {
                return VerbCompileResult::Error(make_compile_error(
                    &e, objid, vn, names_str, start_line,
                ));
            }
            log_compile_warning(&e, objid, names_str, start_line);
            VerbCompileResult::SkippedWithWarning
        }
    }
}

fn make_compile_error(
    e: &CompileError,
    objid: &Obj,
    vn: usize,
    names_str: &str,
    start_line: usize,
) -> TextdumpReaderError {
    match e {
        CompileError::InvalidTypeLiteralAssignment(t, c) => TextdumpReaderError::VerbCompileError(
            format!(
                "Compiling verb {objid}/{vn} ({names_str}) starting at line {}; \
                (*Note*: assignment to type literal {t} is valid in LambdaMOO/ToastStunt, \
                but not in mooR. Manual intervention is required. \
                Use --continue-on-errors to skip failed verbs.)",
                start_line + c.line_col.0
            ),
            e.clone(),
        ),
        _ => TextdumpReaderError::VerbCompileError(
            format!(
                "Compiling verb {objid}/{vn} ({names_str}) starting at line {}. \
                Use --continue-on-errors to skip failed verbs.",
                start_line + e.context().line_col.0,
            ),
            e.clone(),
        ),
    }
}

fn log_compile_warning(e: &CompileError, objid: &Obj, names_str: &str, start_line: usize) {
    match e {
        CompileError::InvalidTypeLiteralAssignment(t, c) => {
            warn!(
                "Verb {objid}:{names_str} (line {}) failed to compile: \
                assignment to type literal {t} is valid in LambdaMOO/ToastStunt \
                but not in mooR. Manual intervention required.",
                start_line + c.line_col.0
            );
        }
        _ => {
            warn!(
                "Verb {objid}:{names_str} (line {}) failed to compile: {e}. \
                Verb will be created with empty program.",
                start_line + e.context().line_col.0
            );
        }
    }
}

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
    import_options: TextdumpImportOptions,
) -> Result<(), TextdumpReaderError> {
    let textdump_import_span = span!(tracing::Level::INFO, "textdump_import");
    let _enter = textdump_import_span.enter();

    let corefile =
        File::open(path).map_err(|e| TextdumpReaderError::CouldNotOpenFile(e.to_string()))?;

    let br = BufReader::new(corefile);

    read_textdump(ldr, br, moor_version, features_config, import_options)
}

/// Returns true if the compile options are compatible with another configuration, for the purposes
/// of textdump loading.
///
/// Which means that if the other configuration has a feature enabled, this configuration
/// must also have it enabled.
/// The other way around is fine.
pub fn is_textdump_compatible(a: &CompileOptions, other: &CompileOptions) -> bool {
    (!other.lexical_scopes || a.lexical_scopes)
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
    import_options: TextdumpImportOptions,
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

    // Report any WAIFs that were found and converted to None
    if !tdr.waif_locations.is_empty() {
        warn!(
            "Found {} WAIF value(s) which were converted to None (WAIFs are unsupported):",
            tdr.waif_locations.len()
        );
        for loc in &tdr.waif_locations {
            let prop_name = td
                .objects
                .get(&loc.object)
                .and_then(|o| resolve_prop(&td.objects, loc.property_index, o))
                .map(|r| r.name.to_string())
                .unwrap_or_else(|| format!("property #{}", loc.property_index));
            warn!("  - {}:{} (line {})", loc.object, prop_name, loc.line_num);
        }
    }

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
                ObjectKind::Objid(*objid),
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
        loader
            .set_object_parent(objid, &o.parent, false)
            .map_err(|e| {
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
    let mut compile_errors = 0usize;
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
                Some(verb) if verb.program.is_some() => {
                    let source = verb.program.as_ref().unwrap();
                    let names_str = names
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    match compile_verb_source(
                        source,
                        compile_options.clone(),
                        &import_options,
                        objid,
                        vn,
                        &names_str,
                        verb.start_line,
                    ) {
                        VerbCompileResult::Ok(program) => program,
                        VerbCompileResult::SkippedWithWarning => {
                            compile_errors += 1;
                            Program::new()
                        }
                        VerbCompileResult::Error(e) => return Err(e),
                    }
                }
                // If the verb program is missing, use an empty program
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
    if compile_errors > 0 {
        warn!(
            "{} verb(s) failed to compile and were created with empty programs. \
            These will need manual intervention.",
            compile_errors
        );
    }
    info!("Verbs defined.");

    // Create import_export_id properties from sysrefs (properties on #0 that point to objects).
    // This enables proper constant generation when dumping to objdef format.
    info!("Creating import_export_id from sysrefs...");
    let import_export_id_sym = Symbol::mk("import_export_id");

    let Some(sysobj) = td.objects.get(&SYSTEM_OBJECT) else {
        info!("Import complete.");
        return Ok(());
    };

    // Find the root object (#1 typically) - it's #0's parent
    let root_obj = sysobj.parent;
    if root_obj == NOTHING {
        warn!("System object #0 has no parent, cannot define import_export_id");
        info!("Import complete.");
        return Ok(());
    }

    // Collect sysrefs: properties on #0 that have object values
    let mut sysrefs: Vec<(Symbol, Obj)> = Vec::new();
    for (pnum, _pval) in sysobj.propvals.iter().enumerate() {
        let Some(resolved) = resolve_prop(&td.objects, pnum, sysobj) else {
            continue;
        };
        // Only consider properties defined on #0 itself (not inherited)
        if resolved.definer != SYSTEM_OBJECT {
            continue;
        }
        let Some(target_obj) = resolved.value.as_object() else {
            continue;
        };
        // Skip special objects like $nothing, $ambiguous_match, $failed_match
        if target_obj.is_valid_object() {
            sysrefs.push((resolved.name, target_obj));
        }
    }

    if sysrefs.is_empty() {
        info!("Import complete.");
        return Ok(());
    }

    // Define import_export_id property on root object (will be inherited by all)
    let flags = BitEnum::new_with(PropFlag::Read) | PropFlag::Chown;
    loader
        .define_property(
            &root_obj,
            &root_obj,
            import_export_id_sym,
            &root_obj,
            flags,
            None,
        )
        .map_err(|e| {
            TextdumpReaderError::LoadError(
                format!("defining import_export_id on {root_obj}"),
                e.clone(),
            )
        })?;

    // Set import_export_id="sysobj" on #0 itself
    let _ = loader.set_property(
        &SYSTEM_OBJECT,
        import_export_id_sym,
        None,
        None,
        Some(v_str("sysobj")),
    );

    // Set import_export_id on each sysref target object
    let mut created = 1; // Count #0
    for (prop_name, target_obj) in sysrefs {
        // Skip if target object doesn't exist in the textdump
        if !td.objects.contains_key(&target_obj) {
            continue;
        }
        if loader
            .set_property(
                &target_obj,
                import_export_id_sym,
                None,
                None,
                Some(v_str(&prop_name.as_arc_str())),
            )
            .is_ok()
        {
            created += 1;
            trace!(target = ?target_obj, sysref = %prop_name, "Created import_export_id from sysref");
        }
    }
    info!("Created {created} import_export_id properties from sysrefs");

    info!("Import complete.");

    Ok(())
}
