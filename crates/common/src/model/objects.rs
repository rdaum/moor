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

use std::fmt::{Display, Formatter};

use crate::util::BitEnum;
use bincode::{Decode, Encode};
use enum_primitive_derive::Primitive;
use moor_var::NOTHING;
use moor_var::{Obj, Symbol};
use serde::{Deserialize, Serialize};

/// A reference to an object in the system, used in external interface (RPC, etc.) to refer to
/// objects.
///
/// Can be encoded to/from CURIEs (compact URIs) for ease of use in external interfaces.
///
///    oid:1234 -> #1234 ObjectRef::OId(1234)
///    sysobj:ident[.subident] -> $ident[.subident] ObjectRef::SysObj(["ident", "subident"])
///    match("phrase") -> env match onn "phrase" ObjectRef::Match("phrase")

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, Serialize, Deserialize)]
pub enum ObjectRef {
    /// An absolute numeric object reference (e.g. #1234)
    Id(Obj),
    /// A system object reference (e.g. $foo) or $foo.bar.baz
    SysObj(Vec<Symbol>),
    /// A string to use with the match facilities to find an object in the player's environment
    Match(String),
}

impl ObjectRef {
    pub fn to_curie(&self) -> String {
        match self {
            ObjectRef::Id(oid) => format!("oid:{}", oid.id()),
            ObjectRef::SysObj(symbols) => {
                let mut s = String::new();
                for sym in symbols {
                    s.push_str(sym.as_str());
                    s.push('.');
                }
                format!("sysobj:{}", s)
            }
            ObjectRef::Match(s) => format!("match(\"{}\")", s),
        }
    }

    pub fn parse_curie(s: &str) -> Option<ObjectRef> {
        if let Some(s) = s.strip_prefix("oid:") {
            let id: i32 = s.parse().ok()?;
            Some(ObjectRef::Id(Obj::mk_id(id)))
        } else if let Some(s) = s.strip_prefix("sysobj:") {
            let symbols = s.split('.').map(Symbol::mk).collect();
            Some(ObjectRef::SysObj(symbols))
        } else if let Some(s) = s.strip_prefix("match(\"") {
            let s = s.strip_suffix("\")")?;
            Some(ObjectRef::Match(s.to_string()))
        } else {
            None
        }
    }
}

impl Display for ObjectRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Id(id) => write!(f, "#{}", id),
            Self::SysObj(symbols) => {
                let mut s = String::new();
                for sym in symbols {
                    s.push_str(sym.as_str());
                    s.push('.');
                }
                write!(f, "${}", s)
            }
            Self::Match(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Ord, PartialOrd, Copy, Clone, Eq, PartialEq, Hash, Primitive, Encode, Decode)]
pub enum ObjFlag {
    User = 0,
    Programmer = 1,
    Wizard = 2,
    Obsolete1 = 3,
    Read = 4,
    Write = 5,
    Obsolete2 = 6,
    Fertile = 7,
}

// The set of built-in object attributes
#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash, Primitive, Decode, Encode)]
pub enum ObjAttr {
    Owner = 0,
    Name = 1,
    Parent = 2,
    Location = 3,
    Flags = 4,
}
impl Display for ObjAttr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owner => f.write_str("owner"),
            Self::Name => f.write_str("name"),
            Self::Parent => f.write_str("parent"),
            Self::Location => f.write_str("location"),
            Self::Flags => f.write_str("flags"),
        }
    }
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct ObjAttrs {
    owner: Obj,
    parent: Obj,
    location: Obj,
    flags: BitEnum<ObjFlag>,
    name: Symbol,
}

impl ObjAttrs {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            owner: NOTHING,
            parent: NOTHING,
            location: NOTHING,
            flags: BitEnum::new(),
            name: Symbol::mk(""),
        }
    }

    pub fn new(
        owner: Obj,
        parent: Obj,
        location: Obj,
        flags: BitEnum<ObjFlag>,
        name: Symbol,
    ) -> Self {
        Self {
            owner,
            parent,
            location,
            flags,
            name,
        }
    }

    pub fn owner(&self) -> Option<Obj> {
        if self.owner == NOTHING {
            None
        } else {
            Some(self.owner.clone())
        }
    }

    pub fn set_owner(&mut self, o: Obj) -> &mut Self {
        self.owner = o;
        self
    }

    pub fn location(&self) -> Option<Obj> {
        if self.location == NOTHING {
            None
        } else {
            Some(self.location.clone())
        }
    }

    pub fn set_location(&mut self, o: Obj) -> &mut Self {
        self.location = o;
        self
    }

    pub fn parent(&self) -> Option<Obj> {
        if self.parent == NOTHING {
            None
        } else {
            Some(self.parent.clone())
        }
    }

    pub fn set_parent(&mut self, o: Obj) -> &mut Self {
        self.parent = o;
        self
    }

    pub fn flags(&self) -> BitEnum<ObjFlag> {
        self.flags.clone()
    }

    pub fn set_flags(&mut self, flags: BitEnum<ObjFlag>) -> &mut Self {
        self.flags = flags;
        self
    }

    pub fn name(&self) -> Symbol {
        self.name.clone()
    }

    pub fn set_name(&mut self, s: &str) -> &mut Self {
        self.name = Symbol::mk(s);
        self
    }
}

impl Default for ObjAttrs {
    fn default() -> Self {
        Self::empty()
    }
}
