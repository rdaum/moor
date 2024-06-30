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

use std::ffi::CString;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::Ordering;

use crate::bindings::cursor::Datum;
use crate::bindings::wiredtiger::{
    wiredtiger_pack_close, wiredtiger_pack_item, wiredtiger_pack_start, wiredtiger_pack_str,
    wiredtiger_pack_uint, wiredtiger_unpack_item, wiredtiger_unpack_start, wiredtiger_unpack_str,
    wiredtiger_unpack_uint, WT_ITEM, WT_PACK_STREAM,
};
use crate::bindings::{Error, Session};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FormatType {
    /// signed byte
    Int8,
    /// unsigned byte
    UInt8,
    /// signed 16-bit
    Int16,
    /// unsigned 16-bit
    UInt16,
    /// signed 32-bit
    Int32,
    /// unsigned 32-bit
    UInt32,
    /// signed 64-bit
    Int64,
    /// unsigned 64-bit
    UInt64,
    /// record number
    RecordNumber(usize),
    /// fixed-length string
    FixedLengthString(usize),
    /// NUL-terminated string
    NulTerminatedString(Option<usize>),
    /// fixed-length bit field
    FixedLengthBitField(Option<u8>),
    /// raw byte array
    RawByte(Option<usize>),
}

// TOOD: validate
pub fn pack_format_string(format_types: &[FormatType]) -> String {
    format_types
        .iter()
        .map(|ft| ft.to_encoding())
        .collect::<Vec<String>>()
        .join("")
}

impl FormatType {
    pub fn to_encoding(&self) -> String {
        match self {
            FormatType::Int8 => "b".into(),
            FormatType::UInt8 => "B".into(),
            FormatType::Int16 => "h".into(),
            FormatType::UInt16 => "H".into(),
            FormatType::Int32 => "i".into(),
            FormatType::UInt32 => "I".into(),
            FormatType::Int64 => "q".into(),
            FormatType::UInt64 => "Q".into(),
            FormatType::RecordNumber(r_num) => format!("{}r", r_num),
            FormatType::FixedLengthString(len) => format!("{}s", len),
            FormatType::NulTerminatedString(Some(len)) => format!("{}S", len),
            FormatType::NulTerminatedString(None) => "S".into(),
            FormatType::FixedLengthBitField(Some(len)) => format!("{}t", len),
            FormatType::FixedLengthBitField(None) => "t".into(),
            FormatType::RawByte(sized) => {
                if let Some(size) = sized {
                    format!("{}u", size)
                } else {
                    "u".into()
                }
            }
        }
    }
}

#[allow(dead_code)]
pub struct Unpack {
    open: bool,
    format: CString,
    pack_stream: *mut WT_PACK_STREAM,
    buffer: Rc<Datum>,
}

#[allow(dead_code)]
impl Unpack {
    pub fn new(session: &Session, format: &[FormatType], datum: Rc<Datum>) -> Self {
        let mut stream: *mut WT_PACK_STREAM = std::ptr::null_mut();
        let format = format
            .iter()
            .map(|f| f.to_encoding())
            .collect::<Vec<_>>()
            .join("");
        let format = CString::new(format).unwrap();
        let data = datum.as_ref();
        let err = unsafe {
            wiredtiger_unpack_start(
                session.session.load(Ordering::Relaxed),
                format.as_ptr(),
                data.as_slice().as_ptr() as _,
                data.len(),
                &mut stream as _,
            )
        };
        if err != 0 {
            panic!("Failed to create pack stream");
        }
        Self {
            format,
            open: true,
            pack_stream: stream,
            buffer: datum,
        }
    }

    pub fn unpack_str(&mut self) -> String {
        assert!(self.open);
        let mut s = std::ptr::null();
        let result = unsafe { wiredtiger_unpack_str(self.pack_stream, &mut s as _) };
        if result != 0 {
            panic!("Failed to unpack string");
        }
        let s = unsafe { std::ffi::CStr::from_ptr(s as _) };
        s.to_str().unwrap().to_string()
    }

    pub fn unpack_uint(&mut self) -> u64 {
        assert!(self.open);
        let mut u = 0;
        let result = unsafe { wiredtiger_unpack_uint(self.pack_stream, &mut u as _) };
        if result != 0 {
            panic!("Failed to unpack uint");
        }
        u
    }

    pub fn unpack_item(&mut self) -> Vec<u8> {
        if self.buffer.len() == 0 {
            return vec![];
        }
        assert!(self.open);
        let mut item = unsafe { std::mem::zeroed::<WT_ITEM>() };
        let result = unsafe { wiredtiger_unpack_item(self.pack_stream, &mut item as *mut WT_ITEM) };
        if result != 0 {
            panic!("Failed to unpack item");
        }
        let item = unsafe { std::slice::from_raw_parts(item.data as *const u8, item.size) };
        item.to_vec()
    }

    pub fn close(&mut self) {
        let result = unsafe { wiredtiger_pack_close(self.pack_stream, std::ptr::null_mut()) };
        if result != 0 {
            panic!("Failed to close pack stream");
        }
        self.open = false;
    }
}

#[allow(dead_code)]
pub struct Pack {
    open: bool,
    format: CString,
    pack_stream: *mut WT_PACK_STREAM,
    buffer: Pin<Box<[u8]>>,
}

#[allow(dead_code)]
impl Pack {
    // TODO: buffer size from FormatType...
    //   only made tricky by dynamic lengthed things like strings and bytearrays
    pub fn new(session: &Session, format: &[FormatType], buffer_size: usize) -> Self {
        let mut stream: *mut WT_PACK_STREAM = std::ptr::null_mut();
        let format = format
            .iter()
            .map(|f| f.to_encoding())
            .collect::<Vec<_>>()
            .join("");
        let format = CString::new(format).unwrap();
        let buffer = vec![0u8; buffer_size];
        let buffer = buffer.into_boxed_slice();
        let buffer = Pin::new(buffer);
        let err = unsafe {
            wiredtiger_pack_start(
                session.session.load(Ordering::Relaxed),
                format.as_ptr(),
                buffer.as_ptr() as _,
                buffer_size,
                &mut stream as _,
            )
        };
        if err != 0 {
            panic!("Failed to create pack stream");
        }
        Self {
            format,
            open: true,
            pack_stream: stream,
            buffer,
        }
    }

    pub fn mk_string(session: &Session, s: &str) -> Self {
        let mut pack = Self::new(
            session,
            &[FormatType::NulTerminatedString(None)],
            s.len() + 1,
        );
        pack.push_str(s);
        pack
    }

    pub fn mk_bytes(session: &Session, bytes: &[u8]) -> Self {
        let mut pack = Self::new(session, &[FormatType::RawByte(None)], bytes.len());
        pack.push_item(bytes);
        pack
    }

    pub fn mk_uint(session: &Session, u: u64) -> Self {
        let mut pack = Self::new(session, &[FormatType::UInt64], 8);
        pack.push_uint(u);
        pack
    }

    pub fn push_str(&mut self, s: &str) {
        assert!(self.open);
        let s = CString::new(s).unwrap();
        let result = unsafe { wiredtiger_pack_str(self.pack_stream, s.as_ptr()) };
        if result != 0 {
            panic!("Failed to pack string");
        }
    }

    pub fn push_uint(&mut self, u: u64) {
        assert!(self.open);
        let result = unsafe { wiredtiger_pack_uint(self.pack_stream, u) };
        if result != 0 {
            panic!("Failed to pack uint");
        }
    }

    pub fn push_item(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        assert!(self.open);
        let mut item = unsafe { std::mem::zeroed::<WT_ITEM>() };
        item.data = bytes.as_ptr() as *const _;
        item.size = bytes.len();
        let result = unsafe { wiredtiger_pack_item(self.pack_stream, &mut item as *mut WT_ITEM) };
        if result != 0 {
            let err = Error::from_errorcode(result);
            panic!("Failed to pack item: {:?}", err);
        }
    }

    pub fn pack(self) -> Datum {
        let mut completed_size = 0;

        let result = unsafe { wiredtiger_pack_close(self.pack_stream, &mut completed_size) };
        if result != 0 {
            panic!("Failed to close pack stream");
        }

        Datum::from_boxed(self.buffer)
    }
}
