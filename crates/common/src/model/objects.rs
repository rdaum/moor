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

use std::fmt::{Display, Formatter};

use crate::util::{BitEnum, BitFlag};
use byteview::ByteView;
use moor_var::{ByteSized, NOTHING, Obj, Symbol};
use serde::{Deserialize, Serialize};

/// A reference to an object in the system, used in external interface (RPC, etc.) to refer to
/// objects.
///
/// Can be encoded to/from CURIEs (compact URIs) for ease of use in external interfaces.
///
///    oid:1234 -> #1234 ObjectRef::OId(1234)
///    sysobj:ident[.subident] -> $ident[.subident] ObjectRef::SysObj(["ident", "subident"])
///    match("phrase") -> env match onn "phrase" ObjectRef::Match("phrase")

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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
            ObjectRef::Id(oid) => {
                if oid.is_uuobjid() {
                    format!("uuid:{}", oid.uuobjid().unwrap().to_uuid_string())
                } else {
                    format!("oid:{}", oid.id().0)
                }
            }
            ObjectRef::SysObj(symbols) => {
                let mut s = String::new();
                for sym in symbols {
                    s.push_str(&sym.as_arc_str());
                    s.push('.');
                }
                format!("sysobj:{s}")
            }
            ObjectRef::Match(s) => format!("match(\"{s}\")"),
        }
    }

    pub fn parse_curie(s: &str) -> Option<ObjectRef> {
        if let Some(s) = s.strip_prefix("oid:") {
            let id: i32 = s.parse().ok()?;
            Some(ObjectRef::Id(Obj::mk_id(id)))
        } else if let Some(s) = s.strip_prefix("uuid:") {
            let uuid = moor_var::UuObjid::from_uuid_string(s).ok()?;
            Some(ObjectRef::Id(Obj::mk_uuobjid(uuid)))
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
            Self::Id(id) => write!(f, "{id}"),
            Self::SysObj(symbols) => {
                let mut s = String::new();
                for sym in symbols {
                    s.push_str(&sym.as_arc_str());
                    s.push('.');
                }
                write!(f, "${s}")
            }
            Self::Match(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Ord, PartialOrd, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u8)]
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

impl BitFlag for ObjFlag {
    fn bit_index(self) -> u8 {
        self as u8
    }
}

impl ObjFlag {
    #[must_use]
    pub fn all_flags() -> BitEnum<Self> {
        BitEnum::new_with(Self::User)
            | Self::Programmer
            | Self::Wizard
            | Self::Obsolete1
            | Self::Read
            | Self::Write
            | Self::Obsolete2
            | Self::Fertile
    }

    pub fn parse_str(s: &str) -> Option<BitEnum<Self>> {
        let mut flags: u8 = 0;
        for c in s.chars() {
            if c == 'u' {
                flags |= 1 << ObjFlag::User as u8;
            } else if c == 'p' {
                flags |= 1 << ObjFlag::Programmer as u8;
            } else if c == 'w' {
                flags |= 1 << ObjFlag::Wizard as u8;
            } else if c == 'r' {
                flags |= 1 << ObjFlag::Read as u8;
            } else if c == 'W' {
                // capital W to distinguish from wizard
                flags |= 1 << ObjFlag::Write as u8;
            } else if c == 'f' {
                flags |= 1 << ObjFlag::Fertile as u8;
            } else {
                return None;
            }
        }

        Some(BitEnum::from_u8(flags))
    }
}

pub fn obj_flags_string(flags: BitEnum<ObjFlag>) -> String {
    let mut flags_string = String::new();
    if flags.contains(ObjFlag::User) {
        flags_string.push('u');
    }
    if flags.contains(ObjFlag::Programmer) {
        flags_string.push('p');
    }
    if flags.contains(ObjFlag::Wizard) {
        flags_string.push('w');
    }
    if flags.contains(ObjFlag::Read) {
        flags_string.push('r');
    }
    if flags.contains(ObjFlag::Write) {
        flags_string.push('W');
    }
    if flags.contains(ObjFlag::Fertile) {
        flags_string.push('f');
    }

    flags_string
}

// The set of built-in object attributes
#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash)]
#[repr(u8)]
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

const OWNER_OFFSET: usize = 0;
const PARENT_OFFSET: usize = OWNER_OFFSET + 8;
const LOCATION_OFFSET: usize = PARENT_OFFSET + 8;
const FLAGS_OFFSET: usize = LOCATION_OFFSET + 8;
const NAME_OFFSET: usize = FLAGS_OFFSET + 2;

fn read_obj_at(buf: &[u8], offset: usize) -> Obj {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[offset..offset + 8]);
    Obj::from_bytes(&bytes).expect("Failed to decode object id")
}

fn write_obj_at(buf: &mut [u8], offset: usize, value: Obj) {
    buf[offset..offset + 8].copy_from_slice(&value.as_u64().to_le_bytes());
}

fn read_flags_at(buf: &[u8], offset: usize) -> BitEnum<ObjFlag> {
    let mut bytes = [0u8; 2];
    bytes.copy_from_slice(&buf[offset..offset + 2]);
    BitEnum::from_u16(u16::from_le_bytes(bytes))
}

fn write_flags_at(buf: &mut [u8], offset: usize, value: BitEnum<ObjFlag>) {
    buf[offset..offset + 2].copy_from_slice(&value.to_u16().to_le_bytes());
}

#[derive(Debug, Clone)]
pub struct ObjAttrs(ByteView);

impl ObjAttrs {
    #[must_use]
    pub fn empty() -> Self {
        let mut buffer = vec![0; NAME_OFFSET];
        write_obj_at(&mut buffer, OWNER_OFFSET, NOTHING);
        write_obj_at(&mut buffer, PARENT_OFFSET, NOTHING);
        write_obj_at(&mut buffer, LOCATION_OFFSET, NOTHING);
        write_flags_at(&mut buffer, FLAGS_OFFSET, BitEnum::new());
        Self(ByteView::from(buffer))
    }

    pub fn new(
        owner: Obj,
        parent: Obj,
        location: Obj,
        flags: BitEnum<ObjFlag>,
        name: &str,
    ) -> Self {
        let header_size = NAME_OFFSET;
        let name_bytes = name.as_bytes();
        let mut buf = vec![0; header_size + name_bytes.len()];
        write_obj_at(&mut buf, OWNER_OFFSET, owner);
        write_obj_at(&mut buf, PARENT_OFFSET, parent);
        write_obj_at(&mut buf, LOCATION_OFFSET, location);
        write_flags_at(&mut buf, FLAGS_OFFSET, flags);

        buf[header_size..].copy_from_slice(name_bytes);

        Self(ByteView::from(buf))
    }

    pub fn owner(&self) -> Option<Obj> {
        let oid = read_obj_at(self.0.as_ref(), OWNER_OFFSET);
        if oid == NOTHING { None } else { Some(oid) }
    }

    pub fn set_owner(&mut self, o: Obj) -> &mut Self {
        let mut buffer_as_vec = self.0.as_ref().to_vec();
        write_obj_at(&mut buffer_as_vec, OWNER_OFFSET, o);
        self.0 = ByteView::from(buffer_as_vec);
        self
    }

    pub fn location(&self) -> Option<Obj> {
        let oid = read_obj_at(self.0.as_ref(), LOCATION_OFFSET);
        if oid == NOTHING { None } else { Some(oid) }
    }

    pub fn set_location(&mut self, o: Obj) -> &mut Self {
        let mut buffer_as_vec = self.0.as_ref().to_vec();
        write_obj_at(&mut buffer_as_vec, LOCATION_OFFSET, o);
        self.0 = ByteView::from(buffer_as_vec);
        self
    }

    pub fn parent(&self) -> Option<Obj> {
        let oid = read_obj_at(self.0.as_ref(), PARENT_OFFSET);
        if oid == NOTHING { None } else { Some(oid) }
    }

    pub fn set_parent(&mut self, o: Obj) -> &mut Self {
        let mut buffer_as_vec = self.0.as_ref().to_vec();
        write_obj_at(&mut buffer_as_vec, PARENT_OFFSET, o);
        self.0 = ByteView::from(buffer_as_vec);
        self
    }

    pub fn flags(&self) -> BitEnum<ObjFlag> {
        read_flags_at(self.0.as_ref(), FLAGS_OFFSET)
    }

    pub fn set_flags(&mut self, flags: BitEnum<ObjFlag>) -> &mut Self {
        let mut buffer_as_vec = self.0.as_ref().to_vec();
        write_flags_at(&mut buffer_as_vec, FLAGS_OFFSET, flags);
        self.0 = ByteView::from(buffer_as_vec);
        self
    }

    pub fn name(&self) -> Option<String> {
        if self.0.len() == NAME_OFFSET {
            return None;
        }
        Some(String::from_utf8(self.0.as_ref()[NAME_OFFSET..].to_vec()).unwrap())
    }

    pub fn set_name(&mut self, s: &str) -> &mut Self {
        let mut buffer_as_vec = self.0.as_ref().to_vec();
        let name_as_vec = s.as_bytes().to_vec();
        buffer_as_vec.extend_from_slice(&name_as_vec);
        self.0 = ByteView::from(buffer_as_vec);
        self
    }
}

impl Default for ObjAttrs {
    fn default() -> Self {
        Self::empty()
    }
}

impl ByteSized for ObjAttrs {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }
}
