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

use moor_common::model::{
    ArgSpec, HasUuid, Named, PrepSpec, PropDef, ValSet, VerbDef, VerbFlag,
    loader::SnapshotInterface,
};
use moor_compiler::{CompileOptions, program_to_tree, unparse};
use moor_textdump::{EncodingMode, TextdumpVersion};
use moor_var::{Associative, ErrorCode, Obj, Sequence, Var, VarType, NOTHING, Variant, program::ProgramType};
use semver::Version;
use std::io::{self, Write};

const VF_READ: u16 = 1;
const VF_WRITE: u16 = 2;
const VF_EXEC: u16 = 4;
const VF_DEBUG: u16 = 10;
const VF_DOBJSHIFT: u16 = 4;
const VF_IOBJSHIFT: u16 = 6;
const VF_ASPEC_NONE: u16 = 0;
const VF_ASPEC_ANY: u16 = 1;
const VF_ASPEC_THIS: u16 = 2;

pub struct TextdumpWriteConfig {
    pub version: Version,
    pub compile_options: CompileOptions,
    pub encoding: EncodingMode,
}

impl Default for TextdumpWriteConfig {
    fn default() -> Self {
        Self {
            version: Version::new(0, 1, 0),
            compile_options: CompileOptions::default(),
            encoding: EncodingMode::UTF8,
        }
    }
}

pub fn write_textdump(
    snapshot: &dyn SnapshotInterface,
    out: &mut dyn Write,
    config: &TextdumpWriteConfig,
) -> Result<(), io::Error> {
    let version = TextdumpVersion::Moor(
        config.version.clone(),
        config.compile_options.clone(),
        config.encoding,
    );
    writeln!(out, "{}", version.to_version_string())?;

    let mut objects: Vec<Obj> = snapshot
        .get_objects()
        .map_err(to_io_err)?
        .iter()
        .collect();
    objects.sort();

    let mut verb_total = 0usize;
    for obj in &objects {
        let verb_defs = snapshot.get_object_verbs(obj).map_err(to_io_err)?;
        verb_total += verb_defs.iter().count();
    }

    writeln!(out, "{}", objects.len())?;
    writeln!(out, "{}", verb_total)?;
    writeln!(out, "0")?;
    writeln!(out, "0")?;

    let mut verb_program_order = Vec::with_capacity(verb_total);

    for obj in &objects {
        let attrs = snapshot.get_object(obj).map_err(to_io_err)?;
        let name = attrs.name().unwrap_or_default();
        writeln!(out, "{}", format_object_header(*obj))?;
        writeln!(out, "{name}")?;
        writeln!(out, "0")?;

        writeln!(out, "{}", attrs.flags().to_u16())?;
        write_objid(out, attrs.owner().unwrap_or(NOTHING))?;
        write_objid(out, attrs.location().unwrap_or(NOTHING))?;
        write_objid(out, NOTHING)?;
        write_objid(out, NOTHING)?;
        write_objid(out, attrs.parent().unwrap_or(NOTHING))?;
        write_objid(out, NOTHING)?;
        write_objid(out, NOTHING)?;

        let mut verb_defs: Vec<VerbDef> = snapshot
            .get_object_verbs(obj)
            .map_err(to_io_err)?
            .iter()
            .collect();
        verb_defs.sort_by_key(|verb| (verb_names(verb), verb.uuid()));
        writeln!(out, "{}", verb_defs.len())?;
        for (verb_index, verb) in verb_defs.iter().enumerate() {
            write_verbdef(out, verb)?;
            let program = snapshot
                .get_verb_program(obj, verb.uuid())
                .map_err(to_io_err)?;
            let lines = program_to_lines(&program)?;
            verb_program_order.push((*obj, verb_index, lines));
        }

        let mut prop_defs: Vec<PropDef> = snapshot
            .get_object_properties(obj)
            .map_err(to_io_err)?
            .iter()
            .filter(|prop| prop.definer() == *obj)
            .collect();
        prop_defs.sort_by_key(|prop| prop.name().to_string());
        writeln!(out, "{}", prop_defs.len())?;
        for prop in &prop_defs {
            writeln!(out, "{}", prop.name())?;
        }

        let ordered_props = ordered_propdefs(snapshot, *obj)?;
        writeln!(out, "{}", ordered_props.len())?;
        for prop in &ordered_props {
            let (value, perms) = snapshot
                .get_property_value(obj, prop.uuid())
                .map_err(to_io_err)?;
            write_propval(out, value.as_ref(), &perms)?;
        }
    }

    for (obj, verbnum, lines) in verb_program_order {
        writeln!(out, "#{}:{verbnum}", format_object_id(obj))?;
        for line in lines {
            writeln!(out, "{line}")?;
        }
        writeln!(out, ".")?;
    }

    writeln!(out, "0 clocks")?;
    writeln!(out, "0 queued tasks")?;
    writeln!(out, "0 suspended tasks")?;

    Ok(())
}

fn write_verbdef(out: &mut dyn Write, verb: &VerbDef) -> Result<(), io::Error> {
    writeln!(out, "{}", verb_names(verb))?;
    write_objid(out, verb.owner())?;
    let (flags, prep) = moo_verb_flags(verb);
    writeln!(out, "{flags}")?;
    writeln!(out, "{prep}")?;
    Ok(())
}

fn write_propval(
    out: &mut dyn Write,
    value: Option<&Var>,
    perms: &moor_common::model::PropPerms,
) -> Result<(), io::Error> {
    match value {
        Some(var) => write_var(out, var)?,
        None => writeln!(out, "{}", VarType::_TYPE_CLEAR as u8)?,
    }
    write_objid(out, perms.owner())?;
    writeln!(out, "{}", perms.flags().to_u16())?;
    Ok(())
}

fn write_var(out: &mut dyn Write, value: &Var) -> Result<(), io::Error> {
    match value.variant() {
        Variant::None => writeln!(out, "{}", VarType::TYPE_NONE as u8)?,
        Variant::Bool(b) => {
            writeln!(out, "{}", VarType::TYPE_BOOL as u8)?;
            writeln!(out, "{}", if b { "true" } else { "false" })?;
        }
        Variant::Int(i) => {
            writeln!(out, "{}", VarType::TYPE_INT as u8)?;
            writeln!(out, "{i}")?;
        }
        Variant::Float(f) => {
            writeln!(out, "{}", VarType::TYPE_FLOAT as u8)?;
            writeln!(out, "{f}")?;
        }
        Variant::Obj(obj) => {
            writeln!(out, "{}", VarType::TYPE_OBJ as u8)?;
            write_objid(out, obj)?;
        }
        Variant::Sym(sym) => {
            writeln!(out, "{}", VarType::TYPE_SYMBOL as u8)?;
            writeln!(out, "{sym}")?;
        }
        Variant::Str(s) => {
            writeln!(out, "{}", VarType::TYPE_STR as u8)?;
            writeln!(out, "{s}")?;
        }
        Variant::List(list) => {
            writeln!(out, "{}", VarType::TYPE_LIST as u8)?;
            writeln!(out, "{}", list.len())?;
            for item in list.iter() {
                write_var(out, &item)?;
            }
        }
        Variant::Map(map) => {
            writeln!(out, "{}", VarType::TYPE_MAP as u8)?;
            writeln!(out, "{}", map.len())?;
            for (key, value) in map.iter() {
                write_var(out, &key)?;
                write_var(out, &value)?;
            }
        }
        Variant::Err(err) => {
            writeln!(out, "{}", VarType::TYPE_ERR as u8)?;
            if let Some(code) = err.to_int() {
                writeln!(out, "{code}")?;
            } else if let ErrorCode::ErrCustom(sym) = err.err_type {
                writeln!(out, "{sym}")?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported error encoding",
                ));
            }
        }
        Variant::Binary(_)
        | Variant::Flyweight(_)
        | Variant::Lambda(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported var type for textdump writer",
            ));
        }
    }
    Ok(())
}

fn ordered_propdefs(
    snapshot: &dyn SnapshotInterface,
    obj: Obj,
) -> Result<Vec<PropDef>, io::Error> {
    let mut ordered = Vec::new();
    let mut current = Some(obj);
    while let Some(objid) = current {
        let mut local: Vec<PropDef> = snapshot
            .get_object_properties(&objid)
            .map_err(to_io_err)?
            .iter()
            .filter(|prop| prop.definer() == objid)
            .collect();
        local.sort_by_key(|prop| prop.name().to_string());
        ordered.extend(local);

        let attrs = snapshot.get_object(&objid).map_err(to_io_err)?;
        current = attrs.parent();
    }
    Ok(ordered)
}

fn verb_names(verb: &VerbDef) -> String {
    let mut names = String::new();
    for (idx, name) in verb.names().iter().enumerate() {
        if idx > 0 {
            names.push(' ');
        }
        names.push_str(&name.to_string());
    }
    names
}

fn moo_verb_flags(verb: &VerbDef) -> (u16, i16) {
    let mut flags = 0u16;
    let verb_flags = verb.flags();
    if verb_flags.contains(VerbFlag::Read) {
        flags |= VF_READ;
    }
    if verb_flags.contains(VerbFlag::Write) {
        flags |= VF_WRITE;
    }
    if verb_flags.contains(VerbFlag::Exec) {
        flags |= VF_EXEC;
    }
    if verb_flags.contains(VerbFlag::Debug) {
        flags |= VF_DEBUG;
    }

    let dobj = match verb.args().dobj {
        ArgSpec::None => VF_ASPEC_NONE,
        ArgSpec::Any => VF_ASPEC_ANY,
        ArgSpec::This => VF_ASPEC_THIS,
    };
    let iobj = match verb.args().iobj {
        ArgSpec::None => VF_ASPEC_NONE,
        ArgSpec::Any => VF_ASPEC_ANY,
        ArgSpec::This => VF_ASPEC_THIS,
    };
    flags |= dobj << VF_DOBJSHIFT;
    flags |= iobj << VF_IOBJSHIFT;

    let prep = match verb.args().prep {
        PrepSpec::Any => -2,
        PrepSpec::None => -1,
        PrepSpec::Other(p) => p as i16,
    };

    (flags, prep)
}

fn write_objid(out: &mut dyn Write, obj: Obj) -> Result<(), io::Error> {
    writeln!(out, "{}", format_object_id(obj))
}

fn format_object_id(obj: Obj) -> String {
    if let Some(uu) = obj.uuobjid() {
        format!("u{}", uu.to_uuid_string())
    } else {
        obj.to_literal().trim_start_matches('#').to_string()
    }
}

fn format_object_header(obj: Obj) -> String {
    if let Some(uu) = obj.uuobjid() {
        format!("#u{}", uu.to_uuid_string())
    } else {
        obj.to_literal()
    }
}

fn to_io_err<E: std::fmt::Display>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err.to_string())
}

fn program_to_lines(program: &ProgramType) -> Result<Vec<String>, io::Error> {
    match program {
        ProgramType::MooR(program) => {
            let tree = program_to_tree(program)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            unparse(&tree, false, false)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
        }
    }
}
