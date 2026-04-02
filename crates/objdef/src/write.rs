// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::dump::ObjectDumpError;
use moor_common::model::{ObjFlag, PrepSpec, prop_flags_string, verb_perms_string};
use moor_compiler::{
    ObjPropDef, ObjPropOverride, ObjVerbDef, ObjectDefinition, program_to_tree, to_literal,
    to_literal_objsub, unparse,
};
use moor_var::{NOTHING, Obj, Symbol, Var, program::ProgramType, v_arc_str, v_str, v_string};
use std::{collections::HashMap, io::Write, path::Path};

pub(crate) struct DumpLines {
    pub(crate) lines: Vec<Var>,
}

struct LineCollector {
    lines: Vec<Var>,
    current: Vec<u8>,
}

impl LineCollector {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current: Vec::new(),
        }
    }

    fn finish(mut self) -> Result<DumpLines, ObjectDumpError> {
        if !self.current.is_empty() {
            let line = String::from_utf8(self.current).map_err(|e| {
                ObjectDumpError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })?;
            self.lines.push(v_str(&line));
        }
        Ok(DumpLines { lines: self.lines })
    }
}

impl Write for LineCollector {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.current.extend_from_slice(buf);
        while let Some(pos) = self.current.iter().position(|b| *b == b'\n') {
            let line_bytes = self.current.drain(..pos).collect::<Vec<u8>>();
            self.current.drain(..1);
            let line = String::from_utf8(line_bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            self.lines.push(v_str(&line));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

// Return the object number and if this is $nameable thing, put a // $comment.
// For anonymous objects, show the full internal ID in objdef context.
fn canon_name(oid: &Obj, index_names: &HashMap<Obj, String>) -> String {
    if let Some(name) = index_names.get(oid) {
        return name.clone();
    };

    if oid.is_anonymous()
        && let Some(anon_id) = oid.anonymous_objid()
    {
        let (autoincrement, rng, epoch_ms) = anon_id.components();
        let first_group = ((autoincrement as u64) << 6) | (rng as u64);
        return format!("#anon_{first_group:06X}-{epoch_ms:010X}");
    }

    format!("{oid}")
}

fn propname(pname: Symbol) -> String {
    let s = pname.as_arc_str();
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        s.to_string()
    } else {
        let name = v_arc_str(s);
        to_literal(&name)
    }
}

pub(crate) fn generate_constants_file(
    index_names: &HashMap<Obj, String>,
    hierarchies: &HashMap<Obj, Vec<String>>,
    directory_path: &Path,
) -> Result<(), ObjectDumpError> {
    let mut grouped: HashMap<String, Vec<(Obj, String)>> = HashMap::new();

    for (obj, constant_name) in index_names.iter() {
        let hierarchy_path = hierarchies
            .get(obj)
            .map(|h| h.join("/"))
            .unwrap_or_default();
        grouped
            .entry(hierarchy_path)
            .or_default()
            .push((*obj, constant_name.clone()));
    }

    let mut sorted_groups: Vec<_> = grouped.into_iter().collect();
    sorted_groups.sort_by(|a, b| a.0.cmp(&b.0));

    let mut constants_file = std::fs::File::create(directory_path.join("constants.moo"))?;
    let mut wrote_group = false;

    for (hierarchy_path, mut objects) in sorted_groups {
        objects.sort_by(|a, b| a.0.as_u64().cmp(&b.0.as_u64()));

        if !hierarchy_path.is_empty() {
            if wrote_group {
                constants_file.write_all(b"\n")?;
            }
            writeln!(constants_file, "// {}", hierarchy_path)?;
            wrote_group = true;
        }

        for (obj, constant_name) in objects {
            let obj_ref = canon_name(&obj, &HashMap::new());
            writeln!(
                constants_file,
                "define {} = {};",
                constant_name.to_ascii_uppercase(),
                obj_ref
            )?;
            wrote_group = true;
        }
    }

    Ok(())
}

pub(crate) fn collect_dump_object_lines(
    index_names: &HashMap<Obj, String>,
    o: &ObjectDefinition,
) -> Result<DumpLines, ObjectDumpError> {
    let mut collector = LineCollector::new();
    write_dump_object(index_names, o, &mut collector)?;
    collector.finish()
}

pub(crate) fn write_dump_object<W: Write>(
    index_names: &HashMap<Obj, String>,
    o: &ObjectDefinition,
    writer: &mut W,
) -> Result<(), ObjectDumpError> {
    writeln!(writer, "object {}", canon_name(&o.oid, index_names))?;
    write_dump_object_header(index_names, o, "  ", writer)?;
    if !o.property_definitions.is_empty() {
        writeln!(writer)?;
    }
    for pd in &o.property_definitions {
        write_property_definition(index_names, "  ", pd, writer)?;
    }
    if !o.property_overrides.is_empty() {
        writeln!(writer)?;
    }
    for ps in &o.property_overrides {
        write_property_override(index_names, "  ", ps, writer)?;
    }
    for v in &o.verbs {
        writeln!(writer)?;
        write_verb(index_names, "  ", v, &o.oid, writer)?;
    }
    writeln!(writer, "endobject")?;
    Ok(())
}

fn write_dump_object_header<W: Write>(
    index_names: &HashMap<Obj, String>,
    o: &ObjectDefinition,
    indent: &str,
    writer: &mut W,
) -> Result<(), std::io::Error> {
    let parent = canon_name(&o.parent, index_names);
    let location = canon_name(&o.location, index_names);
    let owner = canon_name(&o.owner, index_names);
    let name = v_str(&o.name);

    writeln!(writer, "{indent}name: {}", to_literal(&name))?;
    if o.parent != NOTHING {
        writeln!(writer, "{indent}parent: {parent}")?;
    }
    if o.location != NOTHING {
        writeln!(writer, "{indent}location: {location}")?;
    }
    writeln!(writer, "{indent}owner: {owner}")?;
    if o.flags.contains(ObjFlag::User) {
        writeln!(writer, "{indent}player: true")?;
    }
    if o.flags.contains(ObjFlag::Wizard) {
        writeln!(writer, "{indent}wizard: true")?;
    }
    if o.flags.contains(ObjFlag::Programmer) {
        writeln!(writer, "{indent}programmer: true")?;
    }
    if o.flags.contains(ObjFlag::Fertile) {
        writeln!(writer, "{indent}fertile: true")?;
    }
    if o.flags.contains(ObjFlag::Read) {
        writeln!(writer, "{indent}readable: true")?;
    }
    if o.flags.contains(ObjFlag::Write) {
        writeln!(writer, "{indent}writeable: true")?;
    }
    Ok(())
}

fn write_verb<W: Write>(
    index_names: &HashMap<Obj, String>,
    indent: &str,
    v: &ObjVerbDef,
    obj: &Obj,
    writer: &mut W,
) -> Result<(), ObjectDumpError> {
    let owner = canon_name(&v.owner, index_names);
    let vflags = verb_perms_string(v.flags);

    let prepspec = match v.argspec.prep {
        PrepSpec::Any => "any".to_string(),
        PrepSpec::None => "none".to_string(),
        PrepSpec::Other(p) => p.to_string_single().to_string(),
    };
    let verbargsspec = format!(
        "{} {} {}",
        v.argspec.dobj.to_string(),
        prepspec,
        v.argspec.iobj.to_string(),
    );

    let joined_names: String = v
        .names
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
        .join(" ");

    let names = if v.names.len() == 1
        && v.names[0]
            .as_arc_str()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        v.names[0].as_arc_str().to_string()
    } else {
        to_literal(&v_string(joined_names.clone()))
    };

    let ProgramType::MooR(program) = &v.program;
    let decompiled = program_to_tree(program).map_err(|e| ObjectDumpError::DecompileError {
        obj: *obj,
        verb_name: joined_names.clone(),
        reason: e.to_string(),
    })?;
    let unparsed =
        unparse(&decompiled, false, true).map_err(|e| ObjectDumpError::UnparseError {
            obj: *obj,
            verb_name: joined_names.clone(),
            reason: e.to_string(),
        })?;

    writeln!(
        writer,
        "{indent}verb {names} ({verbargsspec}) owner: {owner} flags: \"{vflags}\""
    )?;
    for line in unparsed {
        writeln!(writer, "{indent}{indent}{line}")?;
    }
    writeln!(writer, "{indent}endverb")?;
    Ok(())
}

fn write_property_definition<W: Write>(
    index_names: &HashMap<Obj, String>,
    indent: &str,
    pd: &ObjPropDef,
    writer: &mut W,
) -> Result<(), std::io::Error> {
    let owner = canon_name(&pd.perms.owner(), index_names);
    let flags = prop_flags_string(pd.perms.flags());
    let name = propname(pd.name);

    write!(
        writer,
        "{indent}property {name} (owner: {owner}, flags: \"{flags}\")"
    )?;
    if let Some(value) = &pd.value {
        let value = to_literal_objsub(value, index_names, 2);
        write!(writer, " = {value}")?;
    }
    writeln!(writer, ";")?;
    Ok(())
}

fn write_property_override<W: Write>(
    index_names: &HashMap<Obj, String>,
    indent: &str,
    ps: &ObjPropOverride,
    writer: &mut W,
) -> Result<(), std::io::Error> {
    let name = propname(ps.name);
    write!(writer, "{indent}override {name}")?;
    if let Some(perms) = &ps.perms_update {
        let flags = prop_flags_string(perms.flags());
        let owner = canon_name(&perms.owner(), index_names);
        write!(writer, " (owner: {owner}, flags: \"{flags}\")")?;
    }
    if let Some(value) = &ps.value {
        let value = to_literal_objsub(value, index_names, 2);
        write!(writer, " = {value}")?;
    }
    writeln!(writer, ";")?;
    Ok(())
}
