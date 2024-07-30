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
use std::io::{BufReader, Read};

use text_io::scan;
use tracing::info;

use moor_compiler::Label;
use moor_values::model::CompileError;
use moor_values::model::WorldStateError;
use moor_values::var::Objid;
use moor_values::var::{v_err, v_float, v_int, v_none, v_objid, v_str, Var, VarType};
use moor_values::var::{v_listv, Error};

use crate::textdump::{Object, Propval, Textdump, Verb, Verbdef};

pub const TYPE_CLEAR: i64 = 5;

pub struct TextdumpReader<R: Read> {
    line_num: usize,
    reader: BufReader<R>,
}

impl<R: Read> TextdumpReader<R> {
    pub fn new(reader: BufReader<R>) -> Self {
        Self {
            reader,
            line_num: 0,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum TextdumpReaderError {
    #[error("could not open file: {0}")]
    CouldNotOpenFile(String),
    #[error("io error: {0} @ line {1}")]
    IoError(std::io::Error, usize),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("db error while {0}: {1}")]
    LoadError(String, WorldStateError),
    #[error("compile error while {0}: {1}")]
    VerbCompileError(String, CompileError),
}

impl<R: Read> TextdumpReader<R> {
    fn read_next_line(&mut self) -> Result<String, TextdumpReaderError> {
        // Textdump lines are actually iso-8859-1 encoded, so we need to decode them as such.
        // Read char by char until LF, appending each char to the string.
        let mut line = String::new();
        loop {
            let mut buf = [0u8; 1];
            if let Err(e) = self.reader.read_exact(&mut buf) {
                return Err(TextdumpReaderError::IoError(e, self.line_num));
            }
            if buf[0] == b'\n' {
                break;
            }
            line.push(buf[0] as char);
        }
        self.line_num += 1;
        Ok(line)
    }

    fn read_num(&mut self) -> Result<i64, TextdumpReaderError> {
        let buf = self.read_next_line()?;
        let Ok(i) = buf.trim().parse() else {
            return Err(TextdumpReaderError::ParseError(format!(
                "invalid number: {}",
                buf
            )));
        };
        Ok(i)
    }
    fn read_objid(&mut self) -> Result<Objid, TextdumpReaderError> {
        let buf = self.read_next_line()?;
        let Ok(u) = buf.trim().parse() else {
            return Err(TextdumpReaderError::ParseError(format!(
                "invalid objid: {}",
                buf
            )));
        };
        Ok(Objid(u))
    }
    fn read_float(&mut self) -> Result<f64, TextdumpReaderError> {
        let buf = self.read_next_line()?;
        let Ok(f) = buf.trim().parse() else {
            return Err(TextdumpReaderError::ParseError(format!(
                "invalid float: {}",
                buf
            )));
        };
        Ok(f)
    }
    fn read_string(&mut self) -> Result<String, TextdumpReaderError> {
        let buf = self.read_next_line()?;
        let buf = buf.trim_matches('\n');
        Ok(buf.to_string())
    }
    fn read_verbdef(&mut self) -> Result<Verbdef, TextdumpReaderError> {
        let name = self.read_string()?;
        let owner = self.read_objid()?;
        let perms = self.read_num()? as u16;
        let prep = self.read_num()? as i16;
        Ok(Verbdef {
            name,
            owner,
            flags: perms,
            prep,
        })
    }
    fn read_var_value(&mut self, t_num: i64) -> Result<Var, TextdumpReaderError> {
        let vtype: VarType = VarType::from_repr(t_num as u8).expect("Invalid var type");
        let v = match vtype {
            VarType::TYPE_INT => v_int(self.read_num()?),
            VarType::TYPE_OBJ => v_objid(self.read_objid()?),
            VarType::TYPE_STR => v_str(&self.read_string()?),
            VarType::TYPE_ERR => {
                let e_num = self.read_num()?;
                let etype: Error = Error::from_repr(e_num as u8).expect("Invalid error code");
                v_err(etype)
            }
            VarType::TYPE_LIST => {
                let l_size = self.read_num()?;
                let v: Vec<Var> = (0..l_size).map(|_l| self.read_var().unwrap()).collect();
                v_listv(v)
            }
            VarType::TYPE_NONE => v_none(),
            VarType::TYPE_FLOAT => v_float(self.read_float()?),
            VarType::TYPE_LABEL => {
                let l_num = self.read_num()?;
                let l = Label(l_num as u16);
                v_int(l.0 as i64)
            }
        };
        Ok(v)
    }

    fn read_var(&mut self) -> Result<Var, TextdumpReaderError> {
        let t_num = self.read_num()?;
        self.read_var_value(t_num)
    }

    fn read_propval(&mut self) -> Result<Propval, TextdumpReaderError> {
        let t_num = self.read_num()?;
        // Special handling for 'clear' properties, we convert them into a special attribute,
        // because I really don't like the idea of having a special 'clear' Var type for for
        // properties.
        let is_clear = t_num == TYPE_CLEAR;
        let value = if is_clear {
            v_none()
        } else {
            self.read_var_value(t_num)?
        };
        Ok(Propval {
            value,
            owner: self.read_objid()?,
            flags: self.read_num()? as u8,
            is_clear,
        })
    }
    fn read_object(&mut self) -> Result<Option<Object>, TextdumpReaderError> {
        let ospec = self.read_string()?;
        let ospec = ospec.trim();

        let ospec_split = ospec.trim().split_once(' ');
        let ospec = match ospec_split {
            None => ospec,
            Some(parts) => {
                if parts.1.trim() == "recycled" {
                    return Ok(None);
                }
                parts.0
            }
        };

        match ospec.chars().next() {
            Some('#') => {}
            _ => {
                return Err(TextdumpReaderError::ParseError(format!(
                    "invalid object spec: {}",
                    ospec
                )))
            }
        }
        // TODO: handle "recycled" flag in textdump loading.
        let oid_str = &ospec[1..];
        let Ok(oid) = oid_str.trim().parse() else {
            return Err(TextdumpReaderError::ParseError(format!(
                "invalid objid: {}",
                oid_str
            )));
        };
        let oid = Objid(oid);
        let name = self.read_string()?;
        let _ohandles_string = self.read_string()?;
        let flags = self.read_num()? as u8;
        let owner = self.read_objid()?;
        let location = self.read_objid()?;
        let contents = self.read_objid()?;
        let next = self.read_objid()?;
        let parent = self.read_objid()?;
        let child = self.read_objid()?;
        let sibling = self.read_objid()?;
        let num_verbs = self.read_num()? as usize;
        let mut verbdefs = Vec::with_capacity(num_verbs);
        for _ in 0..num_verbs {
            verbdefs.push(self.read_verbdef()?);
        }
        let num_pdefs = self.read_num()? as usize;
        let mut propdefs = Vec::with_capacity(num_pdefs);
        for _ in 0..num_pdefs {
            propdefs.push(self.read_string()?);
        }
        let num_pvals = self.read_num()? as usize;
        let mut propvals = Vec::with_capacity(num_pvals);
        for _ in 0..num_pvals {
            propvals.push(self.read_propval()?);
        }

        Ok(Some(Object {
            id: oid,
            owner,
            location,
            contents,
            next,
            parent,
            child,
            sibling,
            name,
            flags,
            verbdefs,
            propdefs,
            propvals,
        }))
    }

    fn read_verb(&mut self) -> Result<Verb, TextdumpReaderError> {
        let header = self.read_string()?;
        let (oid, verbnum): (i64, usize);
        scan!(header.bytes() => "#{}:{}", oid, verbnum);

        // Collect lines
        let mut program_lines = vec![];
        loop {
            let line = self.read_string()?;
            if line.trim() == "." {
                break;
            }
            program_lines.push(line);
        }
        let program = program_lines.join("\n");
        Ok(Verb {
            objid: Objid(oid),
            verbnum,
            program: Some(program),
        })
    }

    pub fn read_textdump(&mut self) -> Result<Textdump, TextdumpReaderError> {
        let version = self.read_string()?;
        info!("version {}", version);
        let nobjs = self.read_num()? as usize;
        info!("# objs: {}", nobjs);
        let nprogs = self.read_num()? as usize;
        info!("# progs: {}", nprogs);
        let _dummy = self.read_num()?;
        let nusers = self.read_num()? as usize;
        info!("# users: {}", nusers);

        let mut users = Vec::with_capacity(nusers);
        for _ in 0..nusers {
            users.push(self.read_objid()?);
        }

        info!("Parsing objects...");
        let mut objects = BTreeMap::new();
        for _i in 0..nobjs {
            if let Some(o) = self.read_object()? {
                objects.insert(o.id, o);
            }
        }

        info!("Reading verbs...");
        let mut verbs = BTreeMap::new();
        for _p in 0..nprogs {
            let verb = self.read_verb()?;
            verbs.insert((verb.objid, verb.verbnum), verb);
        }

        Ok(Textdump {
            version,
            objects,
            users,
            verbs,
        })
    }
}
