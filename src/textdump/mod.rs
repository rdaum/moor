/// Representation of the structure of objects verbs etc as read from a LambdaMOO textdump'd db
/// file.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use anyhow::anyhow;
use symbol_table::{Symbol, SymbolTable};
use text_io::scan;
use crate::model::var::{Error, Objid, Var, VarType};
use int_enum::IntEnum;

pub struct Verbdef {
    name: Symbol,
    owner: Objid,
    perms: u16,
    prep: u16,
}

pub struct Propval {
    var: Var,
    owner: Objid,
    perms: u16,
}

pub struct Object {
    id: Objid,
    owner: Objid,
    location: Objid,
    contents: Objid,
    next: Objid,
    parent: Objid,
    child: Objid,
    sibling: Objid,
    name: Symbol,
    flags: u16,
    verbdefs: Vec<Verbdef>,
    propdefs: Vec<Symbol>,
    propvals: Vec<Propval>,
}

#[derive(Clone, Debug)]
pub struct Verb {
    pub(crate) objid: Objid,
    pub(crate) verbnum: usize,
    pub(crate) program: String,
}

pub struct TextdumpReader<R: Read> {
    reader: BufReader<R>,
    symtab: SymbolTable,
}

impl<R: Read> TextdumpReader<R> {
    pub fn new(reader: BufReader<R>) -> Self {
        Self { reader, symtab: Default::default() }
    }
}

pub struct Textdump {
    version: String,
    objects : HashMap<Objid, Object>,
    users : Vec<Objid>,
    pub(crate) verbs : Vec<Verb>,
    symtab: SymbolTable
}

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
        Ok(buf)
    }
    fn read_string_intern(&mut self) -> Result<Symbol, anyhow::Error> {
        let mut buf = String::new();
        let _r = self.reader.read_line(&mut buf)?;
        let symbol = self.symtab.intern(buf.as_str());
        Ok(symbol)
    }
    fn read_verbdef(&mut self) -> Result<Verbdef, anyhow::Error> {
        let name = self.read_string_intern()?;
        let owner = self.read_objid()?;
        let perms = self.read_num()? as u16;
        let prep = self.read_num()? as u16;
        Ok(Verbdef {
            name,
            owner,
            perms,
            prep,
        })
    }
    fn read_var(&mut self) -> Result<Var, anyhow::Error> {
        let t_num = self.read_num()?;
        let vtype: VarType = VarType::from_int(t_num as u8)?;
        let v = match vtype {
            VarType::TYPE_INT => Var::Int(self.read_num()?),
            VarType::TYPE_OBJ => Var::Obj(self.read_objid()?),
            VarType::TYPE_STR => Var::Str(self.read_string()?),
            VarType::TYPE_ERR => {
                let e_num = self.read_num()?;
                let etype: Error = Error::from_int(e_num as u8)?;
                Var::Err(etype)
            }
            VarType::TYPE_LIST => {
                let l_size = self.read_num()?;
                let v: Vec<Var> = (0..l_size).map(|_l| self.read_var().unwrap()).collect();
                Var::List(v)
            }
            VarType::TYPE_CLEAR => {
                Var::Clear
            }
            VarType::TYPE_NONE => {
                Var::None
            }
            VarType::TYPE_CATCH => Var::_Catch(self.read_num()? as usize),
            VarType::TYPE_FINALLY => Var::_Finally(self.read_num()? as usize),
            VarType::TYPE_FLOAT => Var::Float(self.read_float()?),
        };
        Ok(v)
    }

    fn read_propval(&mut self) -> Result<Propval, anyhow::Error> {
        Ok(Propval {
            var: self.read_var()?,
            owner: self.read_objid()?,
            perms: self.read_num()? as u16,
        })
    }
    fn read_object(&mut self) -> Result<Option<Object>, anyhow::Error> {
        let ospec = self.read_string()?;
        let ospec = ospec.trim();

        let ospec_split = ospec.trim().split_once(' ');
        let ospec = match ospec_split {
            None => {
                ospec
            }
            Some(parts) => {
                if parts.1.trim() == "recycled" {
                    return Ok(None)
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
        let name = self.read_string_intern()?;
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
            propdefs.push(self.read_string_intern()?);
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
        let (mut oid, mut verbnum) : (i64, usize) = (0, 0);
        scan!(header.bytes() => "#{}:{}", oid, verbnum);
        let mut program = String::new();
        loop {
            let line = self.read_string()?;
            if line.trim() == "." {
                return Ok(Verb {
                    objid: Objid(oid),
                    verbnum,
                    program,

                })
            }
            program.push_str(line.as_str());
        }
    }

    pub fn read_textdump(&mut self) -> Result<Textdump, anyhow::Error> {
        let version = self.read_string()?;
        println!("version {}", version);
        let nobjs = self.read_num()? as usize;
        println!("# objs: {}", nobjs);
        let nprogs = self.read_num()? as usize;
        println!("# progs: {}", nprogs);
        let _dummy = self.read_num()?;
        let nusers = self.read_num()? as usize;
        println!("# users: {}", nusers);

        let mut users = Vec::with_capacity(nusers);
        for _ in 0..nusers {
            users.push(self.read_objid()?);
        }

        println!("Parsing objects...");
        let mut objects = HashMap::new();
        for _i in 0..nobjs {
            if let Some(o) = self.read_object()? {
                objects.insert(o.id, o);
            }
        }

        println!("Reading verbs...");
        let mut verbs = Vec::with_capacity(nprogs);
        for _p  in 0..nprogs {
            let verb = self.read_verb()?;
            verbs.push(verb);
        }

        Ok(Textdump {
            version,
            objects,
            users,
            verbs,
            symtab: std::mem::take(&mut self.symtab) // I own you now.
        })
    }
}
