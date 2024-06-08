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
use std::ptr::{null, null_mut};

use tracing::info;

use crate::bindings::create_config::{CreateConfig, DropConfig};
use crate::bindings::cursor::Cursor;
use crate::bindings::session_config::{CheckpointConfig, Isolation, TransactionConfig};
use crate::bindings::{
    cursor_config, get_error, wiredtiger::WT_CURSOR, wiredtiger::WT_EVENT_HANDLER,
    wiredtiger::WT_SESSION, DataSource, Error, PosixError,
};

pub struct Session {
    pub session: *mut WT_SESSION,
}

#[allow(dead_code)]
impl Session {
    /// Create a table, column group, index or file.
    pub fn create(&self, entity: &DataSource, config: Option<CreateConfig>) -> Result<(), Error> {
        let config = config.map(|c| c.as_config_string());
        let config = config.map(|c| CString::new(c).unwrap());
        let entity = entity.as_string();
        let entity = CString::new(entity).unwrap();
        let config = config.as_ref().map(|c| c.as_ptr()).unwrap_or(null());
        let result = unsafe {
            let wt_session = *self.session;
            let create_function = wt_session.create.unwrap();
            create_function(self.session, entity.as_ptr() as *const c_char, config)
        };
        if result != 0 {
            Err(Error::from_errorcode(result))
        } else {
            Ok(())
        }
    }

    pub fn drop(&self, entity: &DataSource, config: Option<DropConfig>) -> Result<(), Error> {
        let config = config.map(|c| c.as_config_string());
        let config = config.map(|c| CString::new(c).unwrap());
        let config = config.as_ref().map(|c| c.as_ptr()).unwrap_or(null());
        let entity = entity.as_string();
        let entity = CString::new(entity).unwrap();
        let result = unsafe {
            let wt_session = *self.session;
            let drop_function = wt_session.drop.unwrap();
            drop_function(self.session, entity.as_ptr() as *const c_char, config)
        };
        if result != 0 {
            Err(Error::from_errorcode(result))
        } else {
            Ok(())
        }
    }

    /// Compact a live row- or column-store btree or LSM tree.
    pub fn compact(&self, table_name: &str, timeout: Option<usize>) -> Result<(), Error> {
        let timeout_str = timeout.map(|t| CString::new(format!("timeout={}", t)).unwrap());
        let timeout_str = timeout_str.as_ref().map(|s| s.as_ptr()).unwrap_or(null());
        let table_name = format!("table:{}{}", table_name, table_name);
        let table_name = CString::new(table_name).unwrap();

        let result = unsafe {
            (*self.session).compact.unwrap()(
                self.session,
                table_name.as_ptr() as *const c_char,
                timeout_str,
            )
        };
        if result != 0 {
            Err(Error::from_errorcode(result))
        } else {
            Ok(())
        }
    }

    pub fn begin_transaction(&self, config: Option<TransactionConfig>) -> Result<(), Error> {
        let config = config.map(|c| c.as_config_string());
        let config = config.map(|c| CString::new(c).unwrap());
        let config = config.as_ref().map(|c| c.as_ptr()).unwrap_or(null());
        let result = unsafe {
            let wt_session = *self.session;
            let begin_transaction_function = wt_session.begin_transaction.unwrap();
            begin_transaction_function(self.session, config)
        };
        if result != 0 {
            Err(Error::from_errorcode(result))
        } else {
            Ok(())
        }
    }

    /// Write a transactionally consistent snapshot of a database or set of objects.
    /// The checkpoint includes all transactions committed before the checkpoint starts. Additionally, checkpoints may optionally be discarded.
    pub fn checkpoint(&self, config: Option<CheckpointConfig>) -> Result<(), Error> {
        let config = config.map(|c| c.as_config_string());
        let config = config.map(|c| CString::new(c).unwrap());
        let config = config.as_ref().map(|c| c.as_ptr()).unwrap_or(null());
        let result = unsafe {
            let wt_session = *self.session;
            let checkpoint_function = wt_session.checkpoint.unwrap();
            checkpoint_function(self.session, config)
        };
        if result != 0 {
            Err(Error::from_errorcode(result))
        } else {
            Ok(())
        }
    }

    /// Commit the current transaction.
    /// A transaction must be in progress when this method is called.
    pub fn commit(&self) -> Result<(), Error> {
        let result = unsafe {
            let wt_session = *self.session;
            let commit_function = wt_session.commit_transaction.unwrap();
            commit_function(self.session, null::<c_char>())
        };
        if result != 0 {
            Err(Error::from_errorcode(result))
        } else {
            Ok(())
        }
    }

    /// Roll back the current transaction.
    // A transaction must be in progress when this method is called.
    // All cursors are reset.
    pub fn rollback_transaction(&self) -> Result<(), Error> {
        info!("Rolling back transaction");
        let result = unsafe {
            let wt_session = *self.session;
            let rollback_function = wt_session.rollback_transaction.unwrap();
            rollback_function(self.session, null::<c_char>())
        };
        if result != 0 {
            let error = Error::from_errorcode(result);

            // EINVAL is returned if there is no transaction in progress (because maybe already
            // rolled back). We can ignore this error for rollback.
            if let Error::Posix(PosixError::EINVAL) = error {
                return Ok(());
            }
            Err(error)
        } else {
            Ok(())
        }
    }

    /// Reconfigure a session handle.
    /// Will fail if a transaction is in progress in the session.
    pub fn reconfigure(&self, isolation: Option<Isolation>) -> Result<(), Error> {
        let config = isolation.map(|c| c.as_string().to_string());
        let config = config.map(|c| CString::new(c).unwrap());
        let config = config.as_ref().map(|c| c.as_ptr()).unwrap_or(null());
        let result = unsafe {
            let wt_session = *self.session;
            let reconfigure_function = wt_session.reconfigure.unwrap();
            reconfigure_function(self.session, config)
        };
        if result != 0 {
            Err(Error::from_errorcode(result))
        } else {
            Ok(())
        }
    }

    /// Open a new cursor on a data source
    pub fn open_cursor(
        &self,
        entity: &DataSource,
        config: Option<cursor_config::CursorConfig>,
    ) -> Result<Cursor, Error> {
        let entity = entity.as_string();
        let entity = CString::new(entity).unwrap();
        let config = config.map(|c| c.as_option_string());
        let config = config.map(|c| CString::new(c).unwrap());
        let config = config.as_ref().map(|c| c.as_ptr()).unwrap_or(null());
        let mut cursor = std::ptr::null_mut();
        let result = unsafe {
            let wt_session = *self.session;
            let open_cursor_function = wt_session.open_cursor.unwrap();
            open_cursor_function(
                self.session,
                entity.as_ptr() as *const c_char,
                null_mut::<WT_CURSOR>(),
                config,
                &mut cursor,
            )
        };
        if result != 0 {
            Err(Error::from_errorcode(result))
        } else {
            Ok(Cursor::new(cursor))
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            let result = (*self.session).close.unwrap()(
                self.session,
                null::<c_char>() as *mut WT_EVENT_HANDLER as _,
            );
            if result != 0 {
                panic!("Failed to close: {}", get_error(result));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::bindings::connection::Connection;
    use crate::bindings::create_config::CreateConfig;
    use crate::bindings::data::FormatType;
    use crate::bindings::open_config::OpenConfig;
    use crate::bindings::session_config::Isolation;
    use crate::bindings::{session_config, DataSource, Error, PosixError};

    #[test]
    fn sanity_test() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path().to_str().unwrap();
        let options = OpenConfig::new().create(true).exclusive(true);
        let connection = Connection::open(Path::new(tmpdirpath), options).unwrap();
        let _session = connection
            .open_session(session_config::SessionConfig::new())
            .unwrap();
    }

    #[test]
    fn create_table_and_indexes_and_drop() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path().to_str().unwrap();
        let options = OpenConfig::new().create(true).exclusive(true);
        let connection = Connection::open(Path::new(tmpdirpath), options).unwrap();
        let session = connection
            .open_session(session_config::SessionConfig::new())
            .unwrap();
        let entity = DataSource::Table("test_table".to_string());
        let config = CreateConfig::new()
            .columns(&["name", "age"])
            .key_format(&[FormatType::Int8])
            .value_format(&[FormatType::FixedLengthString(16)]);
        session.create(&entity, Some(config)).unwrap();

        let name_index = DataSource::Index {
            table: "test_table".to_string(),
            index_name: "name_idx".to_string(),
            projection: None,
        };
        let index_config = CreateConfig::new().columns(&["name"]);
        session.create(&name_index, Some(index_config)).unwrap();

        let value_index = DataSource::Index {
            table: "test_table".to_string(),
            index_name: "value_idx".to_string(),
            projection: None,
        };
        let index_config = CreateConfig::new().columns(&["age"]);
        session.create(&value_index, Some(index_config)).unwrap();

        session.drop(&name_index, None).unwrap();
    }

    #[test]
    fn begin_transaction() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path().to_str().unwrap();
        let options = OpenConfig::new().create(true).exclusive(true);
        let connection = Connection::open(Path::new(tmpdirpath), options).unwrap();
        let session = connection
            .open_session(session_config::SessionConfig::new())
            .unwrap();

        // Default options
        session.begin_transaction(None).unwrap();

        drop(session);

        // Now try with isolation level set
        let session = connection
            .open_session(session_config::SessionConfig::new())
            .unwrap();
        let config = session_config::TransactionConfig::new().isolation(Isolation::ReadCommitted);
        session.begin_transaction(Some(config)).unwrap();

        // Invarg if we try to begin a transaction again
        let result = session.begin_transaction(None);
        assert_eq!(
            result,
            Err(Error::from_errorcode(PosixError::EINVAL as i32))
        );

        // Checkpoint should fail because transaction is running
        let result = session.checkpoint(None);
        assert_eq!(
            result,
            Err(Error::from_errorcode(PosixError::EINVAL as i32))
        );

        // Commit
        session.commit().unwrap();

        // Checkpoint should now succeed
        session.checkpoint(None).unwrap();
    }
}
