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

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};

use text_io::scan;
use tracing::info;

use crate::config::TextdumpVersion;
use crate::textdump::{EncodingMode, Object, Propval, Textdump, Verb, Verbdef};
use moor_common::model::CompileError;
use moor_common::model::WorldStateError;
use moor_compiler::Label;
use moor_var::{Error, v_list, v_map};
use moor_var::{
    List, Symbol, Var, VarType, v_bool_int, v_err, v_float, v_int, v_none, v_obj, v_str, v_sym,
};
use moor_var::{Obj, v_flyweight};

pub const TYPE_CLEAR: i64 = 5;

pub struct TextdumpReader<R: Read> {
    line_num: usize,
    reader: BufReader<R>,
    encoding_mode: EncodingMode,
}

impl<R: Read> TextdumpReader<R> {
    pub fn new(reader: BufReader<R>) -> Self {
        Self {
            reader,
            line_num: 0,
            encoding_mode: EncodingMode::UTF8,
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
    #[error("textdump version error: {0}")]
    VersionError(String),
}

impl<R: Read> TextdumpReader<R> {
    fn read_next_line(&mut self) -> Result<String, TextdumpReaderError> {
        let line = match &self.encoding_mode {
            EncodingMode::ISO8859_1 => {
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
                line
            }
            EncodingMode::UTF8 => {
                let mut line = String::new();
                if let Err(e) = self.reader.read_line(&mut line) {
                    return Err(TextdumpReaderError::IoError(e, self.line_num));
                }
                line
            }
        };
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
    fn read_objid(&mut self) -> Result<Obj, TextdumpReaderError> {
        let buf = self.read_next_line()?;
        let Ok(u) = buf.trim().parse() else {
            return Err(TextdumpReaderError::ParseError(format!(
                "invalid objid: {}",
                buf
            )));
        };
        Ok(Obj::mk_id(u))
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
            VarType::TYPE_BOOL => {
                let s = self.read_string()?;
                v_bool_int(s == "true")
            }
            VarType::TYPE_SYMBOL => {
                let s = self.read_string()?;
                let s = Symbol::mk_case_insensitive(&s);
                v_sym(s)
            }
            VarType::TYPE_OBJ => v_obj(self.read_objid()?),
            VarType::TYPE_STR => v_str(&self.read_string()?),
            VarType::TYPE_ERR => {
                let e_num = self.read_num()?;
                let etype: Error = Error::from_repr(e_num as u8).expect("Invalid error code");
                v_err(etype)
            }
            VarType::TYPE_LIST => {
                let l_size = self.read_num()?;
                let v: Vec<Var> = (0..l_size).map(|_l| self.read_var().unwrap()).collect();
                v_list(&v)
            }
            VarType::TYPE_MAP => {
                let num_pairs = self.read_num()?;
                let pairs: Vec<(Var, Var)> = (0..num_pairs)
                    .map(|_i| {
                        let key = self.read_var().unwrap();
                        let value = self.read_var().unwrap();
                        (key, value)
                    })
                    .collect();
                v_map(&pairs)
            }
            VarType::TYPE_NONE => v_none(),
            VarType::TYPE_FLOAT => v_float(self.read_float()?),
            VarType::TYPE_LABEL => {
                let l_num = self.read_num()?;
                let l = Label(l_num as u16);
                v_int(l.0 as i64)
            }
            VarType::TYPE_FLYWEIGHT => {
                let delegate = self.read_objid()?;
                let num_slots = self.read_num()?;
                let mut slots = Vec::with_capacity(num_slots as usize);
                for _ in 0..num_slots {
                    let key = self.read_string().unwrap();
                    let key = Symbol::mk(&key);
                    let value = self.read_var().unwrap();
                    slots.push((key, value));
                }
                let c_size = self.read_num()?;
                let contents: Vec<Var> = (0..c_size).map(|_i| self.read_var().unwrap()).collect();
                let seal = if self.read_num()? == 1 {
                    Some(self.read_string()?)
                } else {
                    None
                };

                v_flyweight(delegate, &slots, List::from_iter(contents), seal)
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
                )));
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
        let oid = Obj::mk_id(oid);
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
        let (oid, verbnum): (i32, usize);
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
            objid: Obj::mk_id(oid),
            verbnum,
            program: Some(program),
        })
    }

    pub fn read_textdump(&mut self) -> Result<(Textdump, TextdumpVersion), TextdumpReaderError> {
        let version_string = self.read_string()?;
        info!("version {}", version_string);

        // Parse the version, and we will use that to determine the encoding mode.
        let version = TextdumpVersion::parse(&version_string)
            .ok_or_else(|| TextdumpReaderError::ParseError("parsing version string".to_string()))?;

        self.encoding_mode = match version {
            TextdumpVersion::LambdaMOO(_) => EncodingMode::ISO8859_1,
            TextdumpVersion::Moor(_, _, encoding) => encoding,
        };

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
                objects.insert(o.id.clone(), o);
            }
        }

        info!("Reading verbs...");
        let mut verbs = BTreeMap::new();
        for _p in 0..nprogs {
            let verb = self.read_verb()?;
            verbs.insert((verb.objid.clone(), verb.verbnum), verb);
        }

        Ok((
            Textdump {
                version: version_string,
                objects,
                users,
                verbs,
            },
            version,
        ))
    }
}
