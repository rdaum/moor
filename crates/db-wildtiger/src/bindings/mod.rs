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

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

use strum::FromRepr;

pub use connection::Connection;
pub use create_config::CreateConfig;
pub use cursor::Datum;
pub use cursor_config::{Bounds, BoundsConfig, CursorConfig};
pub use data::{FormatType, Pack, Unpack};
pub use open_config::{LogConfig, OpenConfig, SyncMethod, TransactionSync, Verbosity};
pub use session::Session;
pub use session_config::{Isolation, SessionConfig, TransactionConfig};

mod connection;
mod create_config;
mod cursor;
mod cursor_config;
mod data;
mod open_config;
mod session;
mod session_config;

#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]

mod wiredtiger {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

unsafe fn string_from_ptr(ptr: *const c_char) -> String {
    let slice = CStr::from_ptr(ptr);
    String::from_utf8(slice.to_bytes().to_vec()).unwrap()
}

unsafe fn get_error(code: c_int) -> String {
    string_from_ptr(wiredtiger::wiredtiger_strerror(code))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[non_exhaustive]
#[allow(non_camel_case_types)]
#[repr(u8)]
#[allow(clippy::upper_case_acronyms)]
pub enum PosixError {
    /// Operation not permitted
    EPERM = 1,
    /// No such file or directory
    ENOENT = 2,
    /// No such process
    ESRCH = 3,
    /// Interrupted system call
    EINTR = 4,
    /// I/O error
    EIO = 5,
    /// No such device or address
    ENXIO = 6,
    /// Argument list too long
    E2BIG = 7,
    /// Exec format error
    ENOEXEC = 8,
    /// Bad file number
    EBADF = 9,
    /// No child processes
    ECHILD = 10,
    /// Try again
    EAGAIN = 11,
    /// Out of memory
    ENOMEM = 12,
    /// Permission denied
    EACCES = 13,
    /// Bad address
    EFAULT = 14,
    /// Block device required
    ENOTBLK = 15,
    /// Device or resource busy
    EBUSY = 16,
    /// File exists
    EEXIST = 17,
    /// Cross-device link
    EXDEV = 18,
    /// No such device
    ENODEV = 19,
    /// Not a directory
    ENOTDIR = 20,
    /// Is a directory
    EISDIR = 21,
    /// Invalid argument
    EINVAL = 22,
    /// File table overflow
    ENFILE = 23,
    /// Too many open files
    EMFILE = 24,
    /// Not a typewriter
    ENOTTY = 25,
    /// Text file busy
    ETXTBSY = 26,
    /// File too large
    EFBIG = 27,
    /// No space left on device
    ENOSPC = 28,
    /// Illegal seek
    ESPIPE = 29,
    /// Read-only file system
    EROFS = 30,
    /// Too many links
    EMLINK = 31,
    /// Broken pipe
    EPIPE = 32,
    /// Math argument out of domain of func
    EDOM = 33,
    /// Math result not representable
    ERANGE = 34,
    /// Resource deadlock would occur
    EDEADLK = 35,
    /// File name too long
    ENAMETOOLONG = 36,
    /// No record locks available
    ENOLCK = 37,
    /// Function not implemented
    ENOSYS = 38,
    /// Directory not empty
    ENOTEMPTY = 39,
    /// Too many symbolic links encountered
    ELOOP = 40,
    /// No message of desired type
    ENOMSG = 42,
    /// Identifier removed
    EIDRM = 43,
    /// Channel number out of range
    ECHRNG = 44,
    /// Level 2 not synchronized
    EL2NSYNC = 45,
    /// Level 3 halted
    EL3HLT = 46,
    /// Level 3 reset
    EL3RST = 47,
    /// Link number out of range
    ELNRNG = 48,
    /// Protocol driver not attached
    EUNATCH = 49,
    /// No CSI structure available
    ENOCSI = 50,
    /// Level 2 halted
    EL2HLT = 51,
    /// Invalid exchange
    EBADE = 52,
    /// Invalid request descriptor
    EBADR = 53,
    /// Exchange full
    EXFULL = 54,
    /// No anode
    ENOANO = 55,
    /// Invalid request code
    EBADRQC = 56,
    /// Invalid slot
    EBADSLT = 57,
    /// Bad font file format
    EBFONT = 59,
    /// Device not a stream
    ENOSTR = 60,
    /// No data available
    ENODATA = 61,
    /// Timer expired
    ETIME = 62,
    /// Out of streams resources
    ENOSR = 63,
    /// Machine is not on the network
    ENONET = 64,
    /// Package not installed
    ENOPKG = 65,
    /// Object is remote
    EREMOTE = 66,
    /// Link has been severed
    ENOLINK = 67,
    /// Advertise error
    EADV = 68,
    /// Srmount error
    ESRMNT = 69,
    /// Communication error on send
    ECOMM = 70,
    /// Protocol error
    EPROTO = 71,
    /// Multihop attempted
    EMULTIHOP = 72,
    /// RFS specific error
    EDOTDOT = 73,
    /// Not a data message
    EBADMSG = 74,
    /// Value too large for defined data type
    EOVERFLOW = 75,
    /// Name not unique on network
    ENOTUNIQ = 76,
    /// File descriptor in bad state
    EBADFD = 77,
    /// Remote address changed
    EREMCHG = 78,
    /// Can not access a needed shared library
    ELIBACC = 79,
    /// Accessing a corrupted shared library
    ELIBBAD = 80,
    /// .lib section in a.out corrupted
    ELIBSCN = 81,
    /// Attempting to link in too many shared libraries
    ELIBMAX = 82,
    /// Cannot exec a shared library directly
    ELIBEXEC = 83,
    /// Illegal byte sequence
    EILSEQ = 84,
    /// Interrupted system call should be restarted
    ERESTART = 85,
    /// Streams pipe error
    ESTRPIPE = 86,
    /// Too many users
    EUSERS = 87,
    /// Socket operation on non-socket
    ENOTSOCK = 88,
    /// Destination address required
    EDESTADDRREQ = 89,
    /// Message too long
    EMSGSIZE = 90,
    /// Protocol wrong type for socket
    EPROTOTYPE = 91,
    /// Protocol not available
    ENOPROTOOPT = 92,
    /// Protocol not supported
    EPROTONOSUPPORT = 93,
    /// Socket type not supported
    ESOCKTNOSUPPORT = 94,
    /// Operation not supported on transport endpoint
    EOPNOTSUPP = 95,
    /// Protocol family not supported
    EPFNOSUPPORT = 96,
    /// Address family not supported by protocol
    EAFNOSUPPORT = 97,
    /// Address already in use
    EADDRINUSE = 98,
    /// Cannot assign requested address
    EADDRNOTAVAIL = 99,
    /// Network is down
    ENETDOWN = 100,
    /// Network is unreachable
    ENETUNREACH = 101,
    /// Network dropped connection because of reset
    ENETRESET = 102,
    /// Software caused connection abort
    ECONNABORTED = 103,
    /// Connection reset by peer
    ECONNRESET = 104,
    /// No buffer space available
    ENOBUFS = 105,
    /// Transport endpoint is already connected
    EISCONN = 106,
    /// Transport endpoint is not connected
    ENOTCONN = 107,
    /// Cannot send after transport endpoint shutdown
    ESHUTDOWN = 108,
    /// Too many references: cannot splice
    ETOOMANYREFS = 109,
    /// Connection timed out
    ETIMEDOUT = 110,
    /// Connection refused
    ECONNREFUSED = 111,
    /// Host is down
    EHOSTDOWN = 112,
    /// No route to host
    EHOSTUNREACH = 113,
    /// Operation already in progress
    EALREADY = 114,
    /// Operation now in progress
    EINPROGRESS = 115,
    /// Stale NFS file handle
    ESTALE = 116,
    /// Structure needs cleaning
    EUCLEAN = 117,
    /// Not a XENIX named type file
    ENOTNAM = 118,
    /// No XENIX semaphores available
    ENAVAIL = 119,
    /// Is a named type file
    EISNAM = 120,
    /// Remote I/O error
    EREMOTEIO = 121,
    /// Quota exceeded
    EDQUOT = 122,
    /// No medium found
    ENOMEDIUM = 123,
    /// Wrong medium type
    EMEDIUMTYPE = 124,
    /// Operation Canceled
    ECANCELED = 125,
    /// Required key not available
    ENOKEY = 126,
    /// Key has expired
    EKEYEXPIRED = 127,
    /// Key has been revoked
    EKEYREVOKED = 128,
    /// Key was rejected by service
    EKEYREJECTED = 129,
    /// Owner died
    EOWNERDEAD = 130,
    /// State not recoverable
    ENOTRECOVERABLE = 131,
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    /// Errors from the underlying Posix layer.
    Posix(PosixError),
    /// This error is generated when an operation cannot be completed due to a conflict with concurrent operations. The operation may be retried; if a transaction is in progress, it should be rolled back and the operation retried in a new transaction.
    Rollback,
    /// This error is generated when the application attempts to insert a record with the same key as an existing record without the 'overwrite' configuration to WT_SESSION::open_cursor.
    DuplicateKey,
    /// This error is returned when an error is not covered by a specific error return. The operation may be retried; if a transaction is in progress, it should be rolled back and the operation retried in a new transaction.
    Error,
    /// This error indicates an operation did not find a value to return. This includes cursor search and other operations where no record matched the cursor's search key such as WT_CURSOR::update or WT_CURSOR::remove.
    NotFound,
    /// This error indicates an underlying problem that requires a database restart. The application may exit immediately, no further WiredTiger calls are required (and further calls will themselves immediately fail).
    Panic,
    /// This error is generated when wiredtiger_open is configured to return an error if recovery is required to use the database.
    RunRecovery,
    /// This error is generated when wiredtiger_open is configured to run in-memory, and a data modification operation requires more than the configured cache size to complete. The operation may be retried; if a transaction is in progress, it should be rolled back and the operation retried in a new transaction.
    CacheFull,
    /// This error is generated when the application attempts to read an updated record which is part of a transaction that has been prepared but not yet resolved.
    PrepareConflict,
    /// This error is generated when corruption is detected in an on-disk file. During normal operations, this may occur in rare circumstances as a result of a system crash. The application may choose to salvage the file or retry wiredtiger_open with the 'salvage=true' configuration setting.
    TrySalvage,
}

impl Error {
    fn from_errorcode(code: c_int) -> Self {
        // Error codes greater than 0 are POSIX errors.
        if code > 0 {
            Self::Posix(PosixError::from_repr(code as u8).unwrap())
        } else {
            match code {
                wiredtiger::WT_ROLLBACK => Self::Rollback,
                wiredtiger::WT_DUPLICATE_KEY => Self::DuplicateKey,
                wiredtiger::WT_ERROR => Self::Error,
                wiredtiger::WT_NOTFOUND => Self::NotFound,
                wiredtiger::WT_PANIC => Self::Panic,
                wiredtiger::WT_RUN_RECOVERY => Self::RunRecovery,
                wiredtiger::WT_CACHE_FULL => Self::CacheFull,
                wiredtiger::WT_PREPARE_CONFLICT => Self::PrepareConflict,
                wiredtiger::WT_TRY_SALVAGE => Self::TrySalvage,
                _ => panic!("Unknown error code: {}", code),
            }
        }
    }
}

/// Data sources for cursors
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum DataSource {
    Table(String),
    ColGroup {
        table: String,
        column_group_name: String,
    },
    Index {
        table: String,
        index_name: String,
        projection: Option<Vec<String>>,
    },
    TableProjection {
        table: String,
        projection: Vec<String>,
    }, // TODO: backup, log, metadata, statistics, file
}

impl DataSource {
    pub fn as_string(&self) -> String {
        match self {
            DataSource::Table(table) => format!("table:{}", table),
            DataSource::TableProjection { table, projection } => {
                format!("table:{}:({})", table, projection.join(","))
            }
            DataSource::ColGroup {
                table,
                column_group_name,
            } => {
                format!("table:{}:{}", table, column_group_name)
            }
            DataSource::Index {
                table,
                index_name,
                projection,
            } => {
                let projection = projection
                    .as_ref()
                    .map(|p| format!("({})", p.join(",")))
                    .unwrap_or_else(|| "".to_string());
                format!("index:{}:{}{}", table, index_name, projection)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DumpFormat {
    Hex,
    Json,
    Print,
}

impl DumpFormat {
    pub fn as_str(&self) -> &str {
        match self {
            DumpFormat::Hex => "hex",
            DumpFormat::Json => "json",
            DumpFormat::Print => "print",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statistics {
    All,
    Fast,
    Clear,
}

impl Statistics {
    pub fn as_str(&self) -> &str {
        match self {
            Statistics::All => "all",
            Statistics::Fast => "fast",
            Statistics::Clear => "clear",
        }
    }
}
