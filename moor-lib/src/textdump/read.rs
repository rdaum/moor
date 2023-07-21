use std::collections::HashMap;
use std::io::{BufRead, Read};

use anyhow::anyhow;
use int_enum::IntEnum;
use text_io::scan;
use tracing::info;

use crate::compiler::labels::Label;
use crate::textdump::{Object, Propval, Textdump, TextdumpReader, Verb, Verbdef};
use crate::var::error::Error;
use crate::var::{
    v_catch, v_err, v_finally, v_float, v_int, v_list, v_objid, v_str, Objid, Var, VarType,
    VAR_CLEAR, VAR_NONE,
};

impl<R: Read> TextdumpReader<R> {
    fn read_num(&mut self) -> Result<i64, anyhow::Error> {
        let mut buf = String::new();
        let _r = self.reader.read_line(&mut buf)?;
        let i: i64 = buf.trim().parse()?;
        Ok(i)
    }
    fn read_objid(&mut self) -> Result<Objid, anyhow::Error> {
        let mut buf = String::new();
        let _r = self.reader.read_line(&mut buf)?;
        let u: i64 = buf.trim().parse()?;
        Ok(Objid(u))
    }
    fn read_float(&mut self) -> Result<f64, anyhow::Error> {
        let mut buf = String::new();
        let _r = self.reader.read_line(&mut buf)?;
        let f: f64 = buf.trim().parse()?;
        Ok(f)
    }
    fn read_string(&mut self) -> Result<String, anyhow::Error> {
        let mut buf = String::new();
        let _r = self.reader.read_line(&mut buf)?;
        let buf = String::from(buf.trim());
        Ok(buf)
    }
    fn read_verbdef(&mut self) -> Result<Verbdef, anyhow::Error> {
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
    fn read_var(&mut self) -> Result<Var, anyhow::Error> {
        let t_num = self.read_num()?;
        let vtype: VarType = VarType::from_int(t_num as u8)?;
        let v = match vtype {
            VarType::TYPE_INT => v_int(self.read_num()?),
            VarType::TYPE_OBJ => v_objid(self.read_objid()?),
            VarType::TYPE_STR => v_str(&self.read_string()?),
            VarType::TYPE_ERR => {
                let e_num = self.read_num()?;
                let etype: Error = Error::from_int(e_num as u8)?;
                v_err(etype)
            }
            VarType::TYPE_LIST => {
                let l_size = self.read_num()?;
                let v: Vec<Var> = (0..l_size).map(|_l| self.read_var().unwrap()).collect();
                v_list(v)
            }
            VarType::TYPE_CLEAR => VAR_CLEAR,
            VarType::TYPE_NONE => VAR_NONE,
            VarType::TYPE_CATCH => v_catch(Label(self.read_num()? as u32)),
            VarType::TYPE_FINALLY => v_finally(Label(self.read_num()? as u32)),
            VarType::TYPE_FLOAT => v_float(self.read_float()?),
        };
        Ok(v)
    }

    fn read_propval(&mut self) -> Result<Propval, anyhow::Error> {
        Ok(Propval {
            value: self.read_var()?,
            owner: self.read_objid()?,
            flags: self.read_num()? as u8,
        })
    }
    fn read_object(&mut self) -> Result<Option<Object>, anyhow::Error> {
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
                return Err(anyhow!("invalid objid: {}", ospec));
            }
        }
        // TODO: handle "recycled" in spec.
        let oid_str = &ospec[1..];
        let oid: i64 = oid_str.trim().parse()?;
        let oid = Objid(oid);
        let name = self.read_string()?;
        let _ohandles_string = self.read_string()?;
        let _flags = self.read_num()?;
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
            flags: 0,
            verbdefs,
            propdefs,
            propvals,
        }))
    }

    fn read_verb(&mut self) -> Result<Verb, anyhow::Error> {
        let header = self.read_string()?;
        let (oid, verbnum): (i64, usize);
        scan!(header.bytes() => "#{}:{}", oid, verbnum);
        let mut program = String::new();
        loop {
            let line = self.read_string()?;
            if line.trim() == "." {
                return Ok(Verb {
                    objid: Objid(oid),
                    verbnum,
                    program,
                });
            }
            program.push_str(line.as_str());
            program.push('\n');
        }
    }

    pub fn read_textdump(&mut self) -> Result<Textdump, anyhow::Error> {
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
        let mut objects = HashMap::new();
        for _i in 0..nobjs {
            if let Some(o) = self.read_object()? {
                objects.insert(o.id, o);
            }
        }

        info!("Reading verbs...");
        let mut verbs = HashMap::with_capacity(nprogs);
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
