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

use std::{
    collections::{BTreeMap, HashMap},
    io::{BufRead, BufReader, Read},
};

use base64::{Engine, engine::general_purpose};
use tracing::{info, warn};

use crate::LambdaMOODBVersion::DbvFloat;

use crate::{
    EncodingMode, Object, Propval, Textdump, TextdumpVersion,
    TextdumpVersion::{LambdaMOO, ToastStunt},
    ToastStuntDBVersion::{
        ToastDbvAnon, ToastDbvInterrupt, ToastDbvLastMove, ToastDbvNextGen, ToastDbvTaskLocal,
        ToastDbvThis, ToastDbvThreaded,
    },
    Verb, Verbdef,
};
use moor_common::model::{CompileError, WorldStateError};
use moor_compiler::{CompileOptions, compile};
use moor_var::{
    Error, ErrorCode, List, NOTHING, Obj, Sequence, Symbol, Var, VarType, Variant,
    program::{
        labels::Label,
        names::Name,
        opcode::{ScatterArgs, ScatterLabel},
    },
    v_binary, v_bool_int, v_err, v_error, v_float, v_flyweight, v_int, v_list, v_map, v_none,
    v_obj, v_str, v_sym,
};

pub const TYPE_CLEAR: i64 = 5;
// Textdump-specific type constant for anonymous objects (matching ToastStunt)
const TYPE_ANON: i64 = 12;

pub struct TextdumpReader<R: Read> {
    pub line_num: usize,
    pub version: TextdumpVersion,
    pub version_string: String,
    pub reader: BufReader<R>,
    pub encoding_mode: EncodingMode,
    anonymous_obj_map: HashMap<i64, Obj>,
}

impl<R: Read> TextdumpReader<R> {
    pub fn new(mut reader: BufReader<R>) -> Result<Self, TextdumpReaderError> {
        // Read the first line from the file to pull out the version information.
        let mut version_string = String::new();
        reader.read_line(&mut version_string).map_err(|e| {
            TextdumpReaderError::VersionError(format!("could not read textdump version: {e}"))
        })?;
        // Strip linefeeds/carriage returns from the end of the version string.
        version_string.retain(|c| c != '\n' && c != '\r');

        info!("version {}", version_string);

        // Parse the version, and we will use that to determine the encoding mode.
        let version = TextdumpVersion::parse(&version_string).ok_or_else(|| {
            TextdumpReaderError::ParseError(format!("invalid version: {version_string}"), 1)
        })?;

        let encoding_mode = match version {
            TextdumpVersion::LambdaMOO(_) | TextdumpVersion::ToastStunt(_) => {
                EncodingMode::ISO8859_1
            }
            TextdumpVersion::Moor(_, _, encoding) => encoding,
        };

        Ok(Self {
            version,
            version_string,
            encoding_mode,
            reader,
            line_num: 2,
            anonymous_obj_map: HashMap::new(),
        })
    }
}
#[derive(Debug, thiserror::Error)]
pub enum TextdumpReaderError {
    #[error("could not open file: {0}")]
    CouldNotOpenFile(String),
    #[error("io error: {0} @ line {1}")]
    IoError(std::io::Error, usize),
    #[error("parse error: {0} @ line {1}")]
    ParseError(String, usize),
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
            return Err(TextdumpReaderError::ParseError(
                format!("invalid number: {buf}"),
                self.line_num,
            ));
        };
        Ok(i)
    }
    fn read_objid(&mut self) -> Result<Obj, TextdumpReaderError> {
        let buf = self.read_next_line()?;
        let trimmed = buf.trim();

        // Check if this is a UUID object (prefixed with 'u')
        if let Some(uuid_str) = trimmed.strip_prefix('u') {
            // Parse as UUID format
            match Obj::try_from(uuid_str) {
                Ok(obj) => Ok(obj),
                Err(_) => Err(TextdumpReaderError::ParseError(
                    format!("invalid UUID objid: {uuid_str}"),
                    self.line_num,
                )),
            }
        } else {
            // Parse as regular numeric object ID
            let Ok(u) = trimmed.parse() else {
                return Err(TextdumpReaderError::ParseError(
                    format!("invalid objid: {trimmed}"),
                    self.line_num,
                ));
            };
            Ok(Obj::mk_id(u))
        }
    }
    fn read_float(&mut self) -> Result<f64, TextdumpReaderError> {
        let buf = self.read_next_line()?;
        let Ok(f) = buf.trim().parse() else {
            return Err(TextdumpReaderError::ParseError(
                format!("invalid float: {buf}"),
                self.line_num,
            ));
        };
        Ok(f)
    }
    fn read_string(&mut self) -> Result<String, TextdumpReaderError> {
        let buf = self.read_next_line()?;
        let buf = buf.trim_end_matches(['\n', '\r']);
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
        // Handle textdump-specific anonymous object type before VarType parsing
        if t_num == TYPE_ANON {
            return self.read_anonymous_obj();
        }

        let vtype: VarType = VarType::from_repr(t_num as u8).expect("Invalid var type");
        let v = match vtype {
            VarType::TYPE_INT => v_int(self.read_num()?),
            VarType::TYPE_BOOL => {
                let s = self.read_string()?;
                v_bool_int(s == "true")
            }
            VarType::TYPE_SYMBOL => {
                let s = self.read_string()?;
                let s = Symbol::mk(&s);
                v_sym(s)
            }
            VarType::TYPE_OBJ => v_obj(self.read_objid()?),
            VarType::TYPE_STR => v_str(&self.read_string()?),
            VarType::TYPE_BINARY => {
                let base64_string = self.read_string()?;
                match general_purpose::STANDARD.decode(base64_string.as_bytes()) {
                    Ok(bytes) => v_binary(bytes),
                    Err(_) => {
                        return Err(TextdumpReaderError::ParseError(
                            "invalid base64 data for binary type".into(),
                            self.line_num,
                        ));
                    }
                }
            }
            VarType::TYPE_ERR => {
                let s = self.read_string()?;
                // If it's a number, parse as classic LambdaMOO errir
                match s.parse::<i64>() {
                    Ok(e_num) => {
                        let etype: Error =
                            Error::from_repr(e_num as u8).expect("Invalid error code");
                        v_error(etype)
                    }
                    Err(..) => {
                        let s = Symbol::mk(&s);
                        v_err(ErrorCode::ErrCustom(s))
                    }
                }
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

                v_flyweight(delegate, &slots, List::from_iter(contents))
            }
            VarType::_TOAST_TYPE_WAIF => {
                warn!("found ToastStunt WAIF type; treating as None");
                // We turn WAIFs into nothing, but have to parse them enough to skip past them.
                let ref_index = self.read_string()?;
                if &ref_index[0..1] == "r" {
                    let _terminator = self.read_string()?;
                    return Ok(v_none());
                }
                let _class = self.read_objid()?;
                let _owner = self.read_objid()?;
                let _propdefs_length = self.read_num()? as usize;
                loop {
                    let cur = self.read_num()?;
                    if cur == -1 {
                        break;
                    }
                    let _val = self.read_var()?;
                }
                let _terminator = self.read_string()?;
                v_none()
            }
            VarType::TYPE_LAMBDA => self.read_lambda()?,
            _ => {
                return Err(TextdumpReaderError::ParseError(
                    format!("invalid var type: {vtype:?}"),
                    self.line_num,
                ));
            }
        };
        Ok(v)
    }

    fn read_lambda(&mut self) -> Result<Var, TextdumpReaderError> {
        // Read parameter specification
        let param_count = self.read_num()? as usize;
        let mut labels = Vec::with_capacity(param_count);
        for _ in 0..param_count {
            let variant = self.read_num()? as u8;
            match variant {
                0 => {
                    // Optional variant
                    let offset = self.read_num()? as u16;
                    let scope_depth = self.read_num()? as u8;
                    let scope_id = self.read_num()? as u16;
                    let name = Name(offset, scope_depth, scope_id);
                    let has_label = self.read_num()? != 0;
                    let opt_label = if has_label {
                        Some(Label(self.read_num()? as u16))
                    } else {
                        None
                    };
                    labels.push(ScatterLabel::Optional(name, opt_label));
                }
                1 => {
                    // Required variant
                    let offset = self.read_num()? as u16;
                    let scope_depth = self.read_num()? as u8;
                    let scope_id = self.read_num()? as u16;
                    let name = Name(offset, scope_depth, scope_id);
                    labels.push(ScatterLabel::Required(name));
                }
                2 => {
                    // Rest variant
                    let offset = self.read_num()? as u16;
                    let scope_depth = self.read_num()? as u8;
                    let scope_id = self.read_num()? as u16;
                    let name = Name(offset, scope_depth, scope_id);
                    labels.push(ScatterLabel::Rest(name));
                }
                _ => {
                    return Err(TextdumpReaderError::ParseError(
                        format!("Invalid scatter label variant: {variant}"),
                        self.line_num,
                    ));
                }
            }
        }
        let done_label = Label(self.read_num()? as u16);
        let params = ScatterArgs {
            labels,
            done: done_label,
        };

        // Read source code
        let source_line_count = self.read_num()? as usize;
        let mut source_lines = Vec::with_capacity(source_line_count);
        for _ in 0..source_line_count {
            source_lines.push(self.read_string()?);
        }
        let source_code = source_lines.join("\n");

        // Compile the source code back into a Program
        let program = compile(&source_code, CompileOptions::default()).map_err(|e| {
            TextdumpReaderError::ParseError(
                format!("Failed to compile lambda source: {e:?}"),
                self.line_num,
            )
        })?;

        // Read captured environment
        let frame_count = self.read_num()? as usize;
        let mut captured_env = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let var_count = self.read_num()? as usize;
            let mut frame = Vec::with_capacity(var_count);
            for _ in 0..var_count {
                frame.push(self.read_var()?);
            }
            captured_env.push(frame);
        }

        // Read self-reference variable name if present
        let has_self_var = self.read_num()? != 0;
        let self_var = if has_self_var {
            let offset = self.read_num()? as u16;
            let scope_depth = self.read_num()? as u8;
            let scope_id = self.read_num()? as u16;
            Some(Name(offset, scope_depth, scope_id))
        } else {
            None
        };

        // Create the lambda
        Ok(Var::mk_lambda(params, program, captured_env, self_var))
    }

    fn read_anonymous_obj(&mut self) -> Result<Var, TextdumpReaderError> {
        let temp_id = self.read_num()?;

        // Check if we've already created this anonymous object
        if let Some(&existing_obj) = self.anonymous_obj_map.get(&temp_id) {
            return Ok(v_obj(existing_obj));
        }

        // Create new anonymous object
        let anon_obj = Obj::mk_anonymous_generated();
        self.anonymous_obj_map.insert(temp_id, anon_obj);
        Ok(v_obj(anon_obj))
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
                return Err(TextdumpReaderError::ParseError(
                    format!("invalid object spec: {ospec}"),
                    self.line_num,
                ));
            }
        }
        // TODO: handle "recycled" flag in textdump loading.
        let oid_str = &ospec[1..];
        let Ok(oid) = oid_str.trim().parse() else {
            return Err(TextdumpReaderError::ParseError(
                format!("invalid objid: {oid_str}"),
                self.line_num,
            ));
        };
        let name = self.read_string()?;
        let oid = if name == "*anonymous*" {
            // This is an anonymous object - check if we've already created it
            if let Some(&existing_obj) = self.anonymous_obj_map.get(&oid) {
                existing_obj
            } else {
                // Create new anonymous object and map the temporary ID to it
                let anon_obj = Obj::mk_anonymous_generated();
                self.anonymous_obj_map.insert(oid, anon_obj);
                anon_obj
            }
        } else {
            Obj::mk_id(oid.try_into().unwrap())
        };
        match self.version {
            ToastStunt(v) if v >= ToastDbvNextGen => {}
            _ => {
                let _ohandles_string = self.read_string()?;
            }
        }

        let flags = self.read_num()? as u8;
        let owner = self.read_objid()?;
        let location = match self.version {
            ToastStunt(_) => {
                let location = self.read_var()?;
                let Some(location) = location.as_object() else {
                    return Err(TextdumpReaderError::ParseError(
                        format!("invalid location: {location:?}"),
                        self.line_num,
                    ));
                };
                location
            }
            _ => self.read_objid()?,
        };
        if let ToastStunt(v) = self.version
            && v >= ToastDbvLastMove
        {
            let _last_move = self.read_var()?;
        }
        let (contents, next, parent, child, sibling) = match self.version {
            ToastStunt(_) => {
                let _contents = self.read_var()?;
                let Some(_contents) = _contents.as_list() else {
                    return Err(TextdumpReaderError::ParseError(
                        format!("invalid contents list: {_contents:?}"),
                        self.line_num,
                    ));
                };
                let parents = self.read_var()?;
                let parent = match parents.variant() {
                    Variant::Obj(parent) => parent,
                    Variant::List(parents) => {
                        if parents.is_empty() {
                            NOTHING
                        } else {
                            let Ok(first) = parents.index(0) else {
                                return Err(TextdumpReaderError::ParseError(
                                    format!("invalid parent: {parents:?}"),
                                    self.line_num,
                                ));
                            };

                            let Some(parent) = first.as_object() else {
                                return Err(TextdumpReaderError::ParseError(
                                    format!("invalid parent: {parents:?}"),
                                    self.line_num,
                                ));
                            };

                            parent
                        }
                    }
                    _ => {
                        return Err(TextdumpReaderError::ParseError(
                            format!("invalid parent: {parents:?}"),
                            self.line_num,
                        ));
                    }
                };
                let _children = self.read_var()?;
                let Some(_children) = _children.as_list() else {
                    return Err(TextdumpReaderError::ParseError(
                        format!("invalid children list: {_children:?}"),
                        self.line_num,
                    ));
                };
                (NOTHING, NOTHING, parent, NOTHING, NOTHING)
            }
            _ => {
                let contents = self.read_objid()?;
                let next = self.read_objid()?;
                let parent = self.read_objid()?;
                let child = self.read_objid()?;
                let sibling = self.read_objid()?;
                (contents, next, parent, child, sibling)
            }
        };
        let num_verbs = self.read_num()? as usize;
        let mut verbdefs = Vec::with_capacity(num_verbs);
        for _ in 0..num_verbs {
            verbdefs.push(self.read_verbdef()?);
        }
        let num_pdefs = self.read_num()? as usize;
        let mut propdefs = Vec::with_capacity(num_pdefs);
        for _ in 0..num_pdefs {
            propdefs.push(Symbol::mk(&self.read_string()?));
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

    fn read_program(&mut self) -> Result<Vec<String>, TextdumpReaderError> {
        let mut program = vec![];
        loop {
            let line = self.read_string()?;
            if line.trim() == "." {
                break;
            }
            program.push(line);
        }
        Ok(program)
    }
    fn read_verb(&mut self) -> Result<Verb, TextdumpReaderError> {
        let header = self.read_string()?;

        let (oid, verbnum) = match header.strip_prefix('#').and_then(|s| s.split_once(':')) {
            Some((oid_str, verbnum_str)) => {
                let oid = oid_str.parse::<i32>().map_err(|e| {
                    TextdumpReaderError::ParseError(
                        format!("invalid object id: {e}"),
                        self.line_num,
                    )
                })?;
                let verbnum = verbnum_str.parse::<usize>().map_err(|e| {
                    TextdumpReaderError::ParseError(
                        format!("invalid verb number: {e}"),
                        self.line_num,
                    )
                })?;
                (oid, verbnum)
            }
            None => {
                return Err(TextdumpReaderError::ParseError(
                    format!("invalid verb header format: {header}"),
                    self.line_num,
                ));
            }
        };

        // Capture the starting line number before reading the program
        let start_line = self.line_num;

        // Collect lines
        let program_lines = self.read_program()?;
        let program = program_lines.join("\n");
        Ok(Verb {
            objid: Obj::mk_id(oid),
            verbnum,
            program: Some(program),
            start_line,
        })
    }

    /// Read a line which is a series of numbers.
    fn read_number_line(&mut self, expected_count: usize) -> Result<Vec<i64>, TextdumpReaderError> {
        let line = self.read_string()?;
        let mut numbers = Vec::with_capacity(expected_count);
        for n in line.split_whitespace() {
            let n = n.parse::<i64>().map_err(|e| {
                TextdumpReaderError::ParseError(format!("invalid number: {e}"), self.line_num)
            })?;
            numbers.push(n);
        }
        if numbers.len() != expected_count {
            return Err(TextdumpReaderError::ParseError(
                format!("expected {} numbers, got {}", expected_count, numbers.len()),
                self.line_num,
            ));
        }
        Ok(numbers)
    }

    fn read_vm(&mut self) -> Result<(), TextdumpReaderError> {
        let has_task_local = matches!(self.version, ToastStunt(v) if v >= ToastDbvTaskLocal);

        if has_task_local {
            let _local = self.read_string()?;
        }
        let vm_header = self.read_number_line(3)?;
        let top = vm_header[0] as usize;

        for _ in 0..top {
            self.read_activ()?;
        }
        Ok(())
    }

    fn read_rt_env(&mut self) -> Result<Vec<(Symbol, Var)>, TextdumpReaderError> {
        let num_variables_line = self.read_string()?;
        let num_variables = num_variables_line.trim_end_matches(" variables");
        let num_variables = num_variables.parse::<usize>().map_err(|e| {
            TextdumpReaderError::ParseError(
                format!("invalid number of variables: {e}"),
                self.line_num,
            )
        })?;
        let mut rt_env = Vec::with_capacity(num_variables);
        for _ in 0..num_variables {
            rt_env.push((Symbol::mk(&self.read_string()?), self.read_var()?));
        }
        Ok(rt_env)
    }

    fn read_activ(&mut self) -> Result<(), TextdumpReaderError> {
        match self.version {
            LambdaMOO(v) if v > DbvFloat => {
                let _lang_version_str = self.read_string()?;
            }
            ToastStunt(_) => {
                let _lang_version_str = self.read_string()?;
            }
            _ => {}
        }
        let _program = self.read_program()?;
        let _env = self.read_rt_env()?;

        let stack_in_use_line = self.read_string()?;
        let stack_in_use_str = stack_in_use_line.trim_end_matches(" rt_stack slots in use");
        let stack_in_use = stack_in_use_str.parse::<usize>().map_err(|e| {
            TextdumpReaderError::ParseError(
                format!("invalid stack in use string: {e}"),
                self.line_num,
            )
        })?;
        for _ in 0..stack_in_use {
            let _entry = self.read_var()?;
        }
        let _ = self.read_activ_as_pi();
        let _ = self.read_var();

        Ok(())
    }

    fn read_activ_as_pi(&mut self) -> Result<(), TextdumpReaderError> {
        let _ = self.read_var()?;
        if let ToastStunt(v) = self.version {
            if v >= ToastDbvThis {
                let _this = self.read_var()?;
            }
            if v >= ToastDbvAnon {
                let _vloc = self.read_var()?;
            }
            if v >= ToastDbvThreaded {
                let _threaded = self.read_num()?;
            }
        }
        let _a_line = self.read_number_line(9);
        let _argstr = self.read_string()?;
        let _dobjstr = self.read_string()?;
        let _iobjstr = self.read_string()?;
        let _prepstr = self.read_string()?;
        let _verb = self.read_string()?;
        let _verbname = self.read_string()?;

        Ok(())
    }

    // TODO: we just throw away the task information for now
    fn read_task_queue(&mut self) -> Result<(), TextdumpReaderError> {
        let clocks_line = self.read_string()?;
        let clocks_str = clocks_line.trim_end_matches(" clocks");
        let clocks = clocks_str.parse::<usize>().map_err(|e| {
            TextdumpReaderError::ParseError(format!("invalid clocks string: {e}"), self.line_num)
        })?;

        for _ in 0..clocks {
            let _ = self.read_string();
        }

        let queued_tasks_line = self.read_string()?;
        let queued_tasks_str = queued_tasks_line.trim_end_matches(" queued tasks");
        let num_queued_tasks = queued_tasks_str.parse::<usize>().map_err(|e| {
            TextdumpReaderError::ParseError(
                format!("invalid queued tasks string: {e}"),
                self.line_num,
            )
        })?;

        for _ in 0..num_queued_tasks {
            let task_desc = self.read_number_line(4)?;
            let (_first_line_no, _st, _id) = (
                task_desc[1] as usize,
                task_desc[2] as usize,
                task_desc[3] as usize,
            );
            // Read (and throw away) activation.
            self.read_activ_as_pi()?
        }

        let suspended_tasks_line = self.read_string()?;
        let suspended_tasks_str = suspended_tasks_line.trim_end_matches(" suspended tasks");
        let num_suspended_tasks = suspended_tasks_str.parse::<usize>().map_err(|e| {
            TextdumpReaderError::ParseError(
                format!("invalid suspended tasks string: {e}"),
                self.line_num,
            )
        })?;
        for _ in 0..num_suspended_tasks {
            let _task_line = self.read_string();
            self.read_vm()?;
        }

        let has_interrupted_tasks = matches!(self.version, ToastStunt(v) if v >= ToastDbvInterrupt);
        if !has_interrupted_tasks {
            return Ok(());
        }

        let interrupted_tasks_line = self.read_string()?;
        let interrupted_tasks_str = interrupted_tasks_line.trim_end_matches(" interrupted tasks");
        let num_interrupted_tasks = interrupted_tasks_str.parse::<usize>().map_err(|e| {
            TextdumpReaderError::ParseError(
                format!("invalid interrupted tasks string: {e}"),
                self.line_num,
            )
        })?;
        for _ in 0..num_interrupted_tasks {
            let _task_line = self.read_string();
        }

        Ok(())
    }

    fn read_active_connections(&mut self) -> Result<(), TextdumpReaderError> {
        let active_connections_line = self.read_string()?;
        let has_listeners = active_connections_line.ends_with(" with listeners");
        let active_connections_str = if has_listeners {
            active_connections_line.trim_end_matches(" active connections with listeners")
        } else {
            active_connections_line.trim_end_matches(" active connections")
        };
        let num_active_connections = active_connections_str.parse::<i64>().map_err(|e| {
            TextdumpReaderError::ParseError(
                format!("invalid active connections string ({active_connections_str}): {e}"),
                self.line_num,
            )
        })?;
        for _ in 0..num_active_connections {
            if has_listeners {
                let listener_items = self.read_number_line(2)?;
                let (_who, _listener) = (listener_items[0], listener_items[1]);
            } else {
                let _who = self.read_num()?;
            }
        }
        Ok(())
    }
    pub fn read_textdump(&mut self) -> Result<Textdump, TextdumpReaderError> {
        let (objects, users, verbs) = match &self.version {
            TextdumpVersion::ToastStunt(_) => {
                // The Toast versions of the textdump have a different format, where a bunch of stuff
                // (like tasks, etc. are mixed inline)
                let nusers = self.read_num()?;
                info!("# users: {}", nusers);
                let mut users = Vec::with_capacity(nusers as usize);
                for _ in 0..nusers {
                    users.push(self.read_objid()?);
                }

                // Now "values pending finalization" which we ignore for now.
                let pending_finalization_str = self.read_string()?;
                if !pending_finalization_str.ends_with("values pending finalization") {
                    return Err(TextdumpReaderError::ParseError(
                        format!("invalid pending finalization string: {pending_finalization_str}"),
                        self.line_num,
                    ));
                }

                let mut pending_finalization_pieces = pending_finalization_str.split(" ");
                let Some(pending_finalization) = pending_finalization_pieces.next() else {
                    return Err(TextdumpReaderError::ParseError(
                        format!("invalid pending finalization string: {pending_finalization_str}",),
                        self.line_num,
                    ));
                };
                let num_pending = pending_finalization.trim().parse::<usize>().map_err(|e| {
                    TextdumpReaderError::ParseError(
                        format!("invalid pending finalization string: {e}"),
                        self.line_num,
                    )
                })?;
                for _ in 0..num_pending {
                    self.read_var()?;
                }

                warn!("Skipped {num_pending} ToastStunt 'pending finalization' values");

                // Now read the forked and suspended tasks
                self.read_task_queue()?;

                // Now read 'formerly active connections'
                self.read_active_connections()?;

                // Now read nbjs
                let nobjs = self.read_num()?;
                info!("# objs: {}", nobjs);
                info!("Parsing objects...");
                let mut objects = BTreeMap::new();
                for _i in 0..nobjs {
                    if let Some(o) = self.read_object()? {
                        objects.insert(o.id, o);
                    }
                }

                // Now read some anon objects? and throw away. Toast stuff.
                loop {
                    let nobjs = self.read_num()?;
                    if nobjs == 0 {
                        break;
                    }
                    for _i in 0..nobjs {
                        let _anon = self.read_object()?;
                    }
                }

                let nprogs = self.read_num()?;
                info!("# progs: {}", nprogs);
                let mut verbs = BTreeMap::new();
                for _p in 0..nprogs {
                    let verb = self.read_verb()?;
                    verbs.insert((verb.objid, verb.verbnum), verb);
                }
                (objects, users, verbs)
            }
            TextdumpVersion::LambdaMOO(_) => {
                let (nobjs, nprogs, _, nusers) = (
                    self.read_num()?,
                    self.read_num()?,
                    self.read_num()?,
                    self.read_num()?,
                );
                info!("# users: {}", nusers);
                let mut users = Vec::with_capacity(nusers as usize);
                for _ in 0..nusers {
                    users.push(self.read_objid()?);
                }

                info!("# objs: {}", nobjs);
                info!("# progs: {}", nprogs);

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

                (objects, users, verbs)
            }
            TextdumpVersion::Moor(_, _, _) => {
                let (nobjs, nprogs, _, nusers) = (
                    self.read_num()?,
                    self.read_num()?,
                    self.read_num()?,
                    self.read_num()?,
                );
                info!("# users: {}", nusers);
                let mut users = Vec::with_capacity(nusers as usize);
                for _ in 0..nusers {
                    users.push(self.read_objid()?);
                }

                info!("# objs: {}", nobjs);
                info!("# progs: {}", nprogs);

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

                // Moor format has simple task queue at the end
                self.read_task_queue()?;

                (objects, users, verbs)
            }
        };

        Ok(Textdump {
            version_string: self.version_string.clone(),
            objects,
            users,
            verbs,
        })
    }
}
