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
use std::os::raw::c_char;
use std::path::Path;
use std::ptr::null;

use tracing::info;

use crate::bindings;
use crate::bindings::open_config::OpenConfig;
use crate::bindings::session_config::SessionConfig;
use crate::bindings::{session, wiredtiger::WT_CONNECTION, wiredtiger::WT_EVENT_HANDLER, Error};

pub struct Connection {
    connection: *mut WT_CONNECTION,
}

// "All WT_CONNECTION methods are thread safe, and WT_CONNECTION handles can be shared between threads."
unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}

#[allow(dead_code)]
impl Connection {
    pub fn open(path: &Path, options: OpenConfig) -> Result<Self, Error> {
        let options_string = options.as_option_string();
        let mut connection: *mut WT_CONNECTION = null::<WT_CONNECTION>() as _;
        let event_handler: *mut WT_EVENT_HANDLER = null::<WT_EVENT_HANDLER>() as _;
        let path = path.to_str().unwrap();
        let path = CString::new(path).unwrap();
        let options = CString::new(options_string).unwrap();
        unsafe {
            let result = bindings::wiredtiger::wiredtiger_open(
                path.as_ptr(),
                event_handler,
                options.as_ptr(),
                &mut connection,
            );
            if result != 0 {
                return Err(Error::from_errorcode(result));
            }
        }
        Ok(Connection { connection })
    }

    /// Open a session.
    pub fn open_session(&self, config: SessionConfig) -> Result<session::Session, u8> {
        let mut session: *mut bindings::wiredtiger::WT_SESSION =
            null::<bindings::wiredtiger::WT_SESSION>() as _;
        let config_str = config.as_config_string();
        let config_str = CString::new(config_str).unwrap();

        let result = unsafe {
            (*self.connection).open_session.unwrap()(
                self.connection,
                null::<c_char>() as _,
                config_str.as_ptr(),
                &mut session,
            )
        };
        if result != 0 {
            return Err(result as u8);
        }
        Ok(session::Session { session })
    }

    pub fn close(&self) -> Result<(), Error> {
        let result =
            unsafe { (*self.connection).close.unwrap()(self.connection, null::<c_char>() as _) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(())
    }

    /// The home directory of the connection.
    pub fn get_home(&self) -> String {
        let home = unsafe { (*self.connection).get_home.unwrap()(self.connection) };
        unsafe { bindings::string_from_ptr(home) }
    }

    pub fn is_new(&self) -> bool {
        let is_new = unsafe { (*self.connection).is_new.unwrap()(self.connection) };
        is_new != 0
    }

    pub fn reconfigure(&self, options: OpenConfig) -> Result<(), Error> {
        let options_string = options.as_option_string();
        let options_string = CString::new(options_string).unwrap();
        let result = unsafe {
            (*self.connection).reconfigure.unwrap()(self.connection, options_string.as_ptr())
        };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(())
    }

    /// Rollback tables to an earlier point in time, discarding all updates to checkpoint durable
    /// tables that have commit times more recent than the current global stable timestamp.
    ///
    /// No updates made to logged tables or updates made without an associated commit timestamp will
    /// be discarded.
    pub fn rollback_to_stable(&self, options: RollbackToStableOptions) -> Result<(), Error> {
        let options_string = options.as_option_string();
        let options_string = CString::new(options_string).unwrap();
        let result = unsafe {
            (*self.connection).rollback_to_stable.unwrap()(self.connection, options_string.as_ptr())
        };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct RollbackToStableOptions {
    dryrun: Option<bool>,
    threads: Option<i8>,
}

#[allow(dead_code)]
impl RollbackToStableOptions {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn as_option_string(&self) -> String {
        let mut options = vec![];

        if let Some(dryrun) = &self.dryrun {
            options.push(format!("dryrun={}", dryrun));
        }

        if let Some(threads) = &self.threads {
            options.push(format!("threads={}", threads));
        }

        options.join(",")
    }

    pub fn dryrun(mut self, dryrun: bool) -> Self {
        self.dryrun = Some(dryrun);
        self
    }

    pub fn threads(mut self, threads: i8) -> Self {
        self.threads = Some(threads);
        self
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        //  TODO: "Multi-threaded programs must wait for all other threads to exit before closing
        //      the WT_CONNECTION handle because that will implicitly close all other handles."
        //  For now, just always use in an Arc<>
        info!("Closing connection");
        self.close().expect("Failed to close connection")
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::bindings::connection::Connection;
    use crate::bindings::open_config::OpenConfig;

    #[test]
    fn sanity_test() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path().to_str().unwrap();
        let options = OpenConfig::new().create(true).exclusive(true);
        let _connection = Connection::open(Path::new(tmpdirpath), options).unwrap();
    }
}
