use std::collections::BTreeMap;
use std::io;

use moor_values::var::objid::Objid;
use moor_values::var::{variant, Var, VarType};
use variant::Variant;

use crate::textdump::read::TYPE_CLEAR;
use crate::textdump::{Object, Propval, Textdump, Verb, Verbdef};

pub struct TextdumpWriter<W: io::Write> {
    writer: W,
}

impl<W: io::Write> TextdumpWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: io::Write> TextdumpWriter<W> {
    fn write_verbdef(&mut self, verbdef: &Verbdef) -> Result<(), io::Error> {
        writeln!(
            self.writer,
            "{}\n{}\n{}\n{}",
            verbdef.name, verbdef.owner.0, verbdef.flags, verbdef.prep
        )
    }

    fn write_var(&mut self, var: &Var, is_clear: bool) -> Result<(), io::Error> {
        if is_clear {
            writeln!(self.writer, "{}", TYPE_CLEAR)?;
            return Ok(());
        }
        match var.variant() {
            Variant::Int(i) => {
                writeln!(self.writer, "{}\n{}", VarType::TYPE_INT as i64, i)?;
            }
            Variant::Obj(o) => {
                writeln!(self.writer, "{}\n{}", VarType::TYPE_OBJ as i64, o.0)?;
            }
            Variant::Str(s) => {
                writeln!(self.writer, "{}\n{}", VarType::TYPE_STR as i64, s)?;
            }
            Variant::Err(e) => {
                writeln!(self.writer, "{}\n{}", VarType::TYPE_ERR as i64, *e as u8)?;
            }
            Variant::List(l) => {
                writeln!(self.writer, "{}\n{}", VarType::TYPE_LIST as i64, l.len())?;
                for v in l.iter() {
                    self.write_var(v, false)?;
                }
            }
            Variant::None => {
                writeln!(self.writer, "{}", VarType::TYPE_NONE as i64)?;
            }
            Variant::Float(f) => {
                // For MOO compat we need to do the same as:
                // 	sprintf(buffer, "%%.%dg\n", DBL_DIG + 4);
                writeln!(self.writer, "{}\n{:+e}", VarType::TYPE_FLOAT as i64, f)?;
            }
        }
        Ok(())
    }

    fn write_propval(&mut self, propval: &Propval) -> Result<(), io::Error> {
        self.write_var(&propval.value, propval.is_clear)?;
        writeln!(self.writer, "{}", propval.owner.0)?;
        writeln!(self.writer, "{}", propval.flags)?;
        Ok(())
    }

    fn write_object(&mut self, object: &Object) -> Result<(), io::Error> {
        writeln!(self.writer, "#{}\n{}\n", object.id.0, &object.name,)?;

        writeln!(self.writer, "{}", object.flags)?;
        writeln!(self.writer, "{}", object.owner.0)?;
        writeln!(self.writer, "{}", object.location.0)?;
        writeln!(self.writer, "{}", object.contents.0)?;
        writeln!(self.writer, "{}", object.next.0)?;
        writeln!(self.writer, "{}", object.parent.0)?;
        writeln!(self.writer, "{}", object.child.0)?;
        writeln!(self.writer, "{}", object.sibling.0)?;
        writeln!(self.writer, "{}", object.verbdefs.len())?;
        for verbdef in &object.verbdefs {
            self.write_verbdef(verbdef)?;
        }
        writeln!(self.writer, "{}", object.propdefs.len())?;
        for propdef in &object.propdefs {
            writeln!(self.writer, "{}", propdef)?;
        }
        writeln!(self.writer, "{}", object.propvals.len())?;
        for propval in &object.propvals {
            self.write_propval(propval)?;
        }
        Ok(())
    }

    fn write_verbs(&mut self, verbs: &BTreeMap<(Objid, usize), Verb>) -> Result<(), io::Error> {
        for verb in verbs.values() {
            let Some(program) = verb.program.clone() else {
                continue;
            };

            writeln!(
                self.writer,
                "#{}:{}\n{}\n.",
                verb.objid.0, verb.verbnum, program
            )?;
        }
        Ok(())
    }

    pub fn write_textdump(&mut self, textdump: &Textdump) -> Result<(), io::Error> {
        writeln!(self.writer, "{}", &textdump.version)?;

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
            writeln!(self.writer, "{}", user.0)?;
        }
        for object in textdump.objects.values() {
            self.write_object(object)?;
        }
        self.write_verbs(&textdump.verbs)?;

        // TODO: actually write clocks/tasks/suspended tasks, but for now we just write 0 for each
        writeln!(self.writer, "0 clocks")?;
        writeln!(self.writer, "0 queued tasks")?;
        writeln!(self.writer, "0 suspended tasks")?;

        Ok(())
    }
}
