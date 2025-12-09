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

use moor_compiler::{program_to_tree, unparse};
use moor_var::{ErrorCode, Lambda, Obj, Sequence, Var, VarType, Variant};
use std::{
    collections::{BTreeMap, HashMap},
    io,
};

use crate::{EncodingMode, Object, Propval, Textdump, Verb, Verbdef, read::TYPE_CLEAR};
use moor_var::Associative;

// Textdump-specific type constant for anonymous objects (matching ToastStunt)
const TYPE_ANON: i64 = 12;

pub struct TextdumpWriter<W: io::Write> {
    writer: W,
    encoding_mode: EncodingMode,
    anonymous_obj_ids: HashMap<Obj, i64>,
    next_anonymous_id: i64,
}

impl<W: io::Write> TextdumpWriter<W> {
    pub fn new(writer: W, encoding_mode: EncodingMode) -> Self {
        Self {
            writer,
            encoding_mode,
            anonymous_obj_ids: HashMap::new(),
            next_anonymous_id: 0, // Start from 0, following ToastStunt approach
        }
    }
}

impl<W: io::Write> TextdumpWriter<W> {
    fn write_obj(&mut self, obj: &Obj) -> Result<(), io::Error> {
        if obj.is_uuobjid() {
            // Write UUID objects with 'u' prefix followed by UUID string
            writeln!(self.writer, "u{}", obj.to_literal())
        } else if obj.is_anonymous() {
            // Write anonymous objects using temporary ID
            self.write_anonymous_obj(obj)
        } else {
            // Write regular numeric objects as their ID
            writeln!(self.writer, "{}", obj.id().0)
        }
    }

    fn write_anonymous_obj(&mut self, obj: &Obj) -> Result<(), io::Error> {
        // Following ToastStunt approach: assign a temporary sequential ID for anonymous objects
        // During textdump save/load, anonymous objects get sequential IDs for serialization
        // The actual anonymous object identity is preserved through its internal structure
        let temp_id = self.get_or_assign_anonymous_id(obj);
        writeln!(self.writer, "{temp_id}")
    }

    fn get_or_assign_anonymous_id(&mut self, obj: &Obj) -> i64 {
        if let Some(&existing_id) = self.anonymous_obj_ids.get(obj) {
            existing_id
        } else {
            let new_id = self.next_anonymous_id;
            self.anonymous_obj_ids.insert(*obj, new_id);
            self.next_anonymous_id += 1;
            new_id
        }
    }

    fn write_verbdef(&mut self, verbdef: &Verbdef) -> Result<(), io::Error> {
        writeln!(self.writer, "{}", verbdef.name)?;
        self.write_obj(&verbdef.owner)?;
        writeln!(self.writer, "{}", verbdef.flags)?;
        writeln!(self.writer, "{}", verbdef.prep)?;
        Ok(())
    }

    fn write_lambda(&mut self, lambda: &Lambda) -> Result<(), io::Error> {
        writeln!(self.writer, "{}", VarType::TYPE_LAMBDA as i64)?;

        // Write parameter specification
        writeln!(self.writer, "{}", lambda.0.params.labels.len())?;
        for label in &lambda.0.params.labels {
            match label {
                moor_var::program::opcode::ScatterLabel::Optional(name, opt_label) => {
                    writeln!(self.writer, "0")?; // Optional variant
                    writeln!(self.writer, "{}", name.0)?;
                    writeln!(self.writer, "{}", name.1)?;
                    writeln!(self.writer, "{}", name.2)?;
                    if let Some(label) = opt_label {
                        writeln!(self.writer, "1")?;
                        writeln!(self.writer, "{}", label.0)?;
                    } else {
                        writeln!(self.writer, "0")?;
                    }
                }
                moor_var::program::opcode::ScatterLabel::Required(name) => {
                    writeln!(self.writer, "1")?; // Required variant
                    writeln!(self.writer, "{}", name.0)?;
                    writeln!(self.writer, "{}", name.1)?;
                    writeln!(self.writer, "{}", name.2)?;
                }
                moor_var::program::opcode::ScatterLabel::Rest(name) => {
                    writeln!(self.writer, "2")?; // Rest variant
                    writeln!(self.writer, "{}", name.0)?;
                    writeln!(self.writer, "{}", name.1)?;
                    writeln!(self.writer, "{}", name.2)?;
                }
            }
        }
        writeln!(self.writer, "{}", lambda.0.params.done.0)?;

        // Decompile the lambda body to source code, just like we do for verbs
        let decompiled = program_to_tree(&lambda.0.body)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let unparsed = unparse(&decompiled, false, true)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Write the source code
        writeln!(self.writer, "{}", unparsed.len())?;
        for line in unparsed {
            writeln!(self.writer, "{line}")?;
        }

        // Write captured environment
        writeln!(self.writer, "{}", lambda.0.captured_env.len())?;
        for frame in &lambda.0.captured_env {
            writeln!(self.writer, "{}", frame.len())?;
            for var in frame {
                self.write_var(var, false)?;
            }
        }

        // Write self-reference variable name if present
        if let Some(self_var) = lambda.0.self_var {
            writeln!(self.writer, "1")?;
            writeln!(self.writer, "{}", self_var.0)?;
            writeln!(self.writer, "{}", self_var.1)?;
            writeln!(self.writer, "{}", self_var.2)?;
        } else {
            writeln!(self.writer, "0")?;
        }

        Ok(())
    }

    fn write_var(&mut self, var: &Var, is_clear: bool) -> Result<(), io::Error> {
        if is_clear {
            writeln!(self.writer, "{TYPE_CLEAR}")?;
            return Ok(());
        }
        match var.variant() {
            Variant::Int(i) => {
                writeln!(self.writer, "{}\n{}", VarType::TYPE_INT as i64, i)?;
            }
            Variant::Bool(b) => writeln!(self.writer, "{}\n{}", VarType::TYPE_BOOL as i64, b)?,
            Variant::Sym(s) => writeln!(
                self.writer,
                "{}\n{}",
                VarType::TYPE_SYMBOL as i64,
                s.as_arc_string()
            )?,
            Variant::Obj(o) => {
                if o.is_anonymous() {
                    // Write anonymous objects using textdump-specific type code
                    writeln!(self.writer, "{TYPE_ANON}")?;
                    self.write_anonymous_obj(&o)?;
                } else {
                    writeln!(self.writer, "{}", VarType::TYPE_OBJ as u64)?;
                    self.write_obj(&o)?;
                }
            }
            Variant::Str(s) => {
                match self.encoding_mode {
                    EncodingMode::ISO8859_1 => {
                        writeln!(self.writer, "{}", VarType::TYPE_STR as i64)?;
                        let encoding = encoding_rs::WINDOWS_1252;
                        let s = s.as_str();
                        let s = encoding.encode(s);
                        let written = self.writer.write(&s.0).unwrap();
                        assert_eq!(written, s.0.len());
                        writeln!(self.writer)?; // Add final newline
                    }
                    EncodingMode::UTF8 => {
                        writeln!(self.writer, "{}\n{}", VarType::TYPE_STR as i64, s)?;
                    }
                }
            }
            Variant::Err(e) => {
                // integer form errors get written with their classic MOO repr
                // "custom" we write the string literal
                match e.err_type {
                    ErrorCode::ErrCustom(s) => {
                        writeln!(self.writer, "{}\n{}", VarType::TYPE_ERR as i64, s)?;
                    }
                    _ => {
                        let v = e.to_int().unwrap();
                        writeln!(self.writer, "{}\n{}", VarType::TYPE_ERR as i64, v)?;
                    }
                }
            }
            Variant::List(l) => {
                writeln!(self.writer, "{}\n{}", VarType::TYPE_LIST as i64, l.len())?;
                for v in l.iter() {
                    self.write_var(&v, false)?;
                }
            }
            Variant::Map(m) => {
                writeln!(self.writer, "{}\n{}", VarType::TYPE_MAP as i64, m.len())?;
                for (k, v) in m.iter() {
                    self.write_var(&k, false)?;
                    self.write_var(&v, false)?;
                }
            }
            Variant::Binary(b) => {
                // Write binary data as base64-encoded string for portability
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(b.as_bytes());
                writeln!(self.writer, "{}\n{}", VarType::TYPE_BINARY as i64, encoded)?;
            }
            Variant::None => {
                writeln!(self.writer, "{}", VarType::TYPE_NONE as i64)?;
            }
            Variant::Float(f) => {
                // For MOO compat we need to do the same as:
                // 	sprintf(buffer, "%%.%dg\n", DBL_DIG + 4);
                writeln!(self.writer, "{}\n{:+e}", VarType::TYPE_FLOAT as i64, f)?;
            }
            Variant::Flyweight(flyweight) => {
                // delegate, slots (len, [key, value, ...]), contents (len, ...), seal (1/0, string)
                writeln!(self.writer, "{}", VarType::TYPE_FLYWEIGHT as i64)?;
                self.write_obj(flyweight.delegate())?;
                writeln!(self.writer, "{}", flyweight.slots().len())?;
                for (k, v) in flyweight.slots().iter() {
                    writeln!(self.writer, "{k}")?;
                    self.write_var(v, false)?;
                }
                writeln!(self.writer, "{}", flyweight.contents().len())?;
                for v in flyweight.contents().iter() {
                    self.write_var(&v, false)?;
                }
                writeln!(self.writer, "0")?;
            }
            Variant::Lambda(lambda) => {
                self.write_lambda(lambda)?;
            }
        }
        Ok(())
    }

    fn write_propval(&mut self, propval: &Propval) -> Result<(), io::Error> {
        self.write_var(&propval.value, propval.is_clear)?;
        self.write_obj(&propval.owner)?;
        writeln!(self.writer, "{}", propval.flags)?;
        Ok(())
    }

    fn write_object(&mut self, object: &Object) -> Result<(), io::Error> {
        if object.id.is_anonymous() {
            // For anonymous objects, write with temporary ID
            let temp_id = self.get_or_assign_anonymous_id(&object.id);
            writeln!(self.writer, "#{}\n{}\n", temp_id, &object.name)?;
        } else {
            writeln!(self.writer, "#{}\n{}\n", object.id.id().0, &object.name)?;
        }

        writeln!(self.writer, "{}", object.flags)?;
        self.write_obj(&object.owner)?;
        self.write_obj(&object.location)?;
        self.write_obj(&object.contents)?;
        self.write_obj(&object.next)?;
        self.write_obj(&object.parent)?;
        self.write_obj(&object.child)?;
        self.write_obj(&object.sibling)?;
        writeln!(self.writer, "{}", object.verbdefs.len())?;
        for verbdef in &object.verbdefs {
            self.write_verbdef(verbdef)?;
        }
        writeln!(self.writer, "{}", object.propdefs.len())?;
        for propdef in &object.propdefs {
            writeln!(self.writer, "{propdef}")?;
        }
        writeln!(self.writer, "{}", object.propvals.len())?;
        for propval in &object.propvals {
            self.write_propval(propval)?;
        }
        Ok(())
    }

    fn write_verbs(&mut self, verbs: &BTreeMap<(Obj, usize), Verb>) -> Result<(), io::Error> {
        for verb in verbs.values() {
            let Some(program) = verb.program.clone() else {
                continue;
            };

            writeln!(
                self.writer,
                "{}:{}\n{}\n.",
                verb.objid, verb.verbnum, program
            )?;
        }
        Ok(())
    }

    pub fn write_textdump(&mut self, textdump: &Textdump) -> Result<(), io::Error> {
        writeln!(self.writer, "{}", &textdump.version_string.to_string())?;

        // We only count the existence of programs, not verbs, here.
        let nprogs = textdump
            .verbs
            .iter()
            .filter(|(_, v)| v.program.is_some())
            .count();
        writeln!(
            self.writer,
            "{}\n{}\n0\n{}",
            textdump.objects.len(),
            nprogs,
            textdump.users.len()
        )?;
        for user in &textdump.users {
            self.write_obj(user)?;
        }
        for object in textdump.objects.values() {
            self.write_object(object)?;
        }
        self.write_verbs(&textdump.verbs)?;

        // TODO: Suspended tasks, clocks, queued tasks in textdump write
        //    actually write clocks/tasks/suspended tasks, but for now we just write 0 for each
        writeln!(self.writer, "0 clocks")?;
        writeln!(self.writer, "0 queued tasks")?;
        writeln!(self.writer, "0 suspended tasks")?;

        Ok(())
    }
}
