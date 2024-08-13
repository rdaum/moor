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

use std::cell::RefCell;
use std::cmp::Ordering;
use std::ffi::CString;
use std::fmt::{Debug, Formatter};
use std::pin::Pin;
use std::rc::Rc;

use crate::bindings::cursor_config::{BoundsConfig, CursorReconfigureOptions};
use crate::bindings::{wiredtiger::WT_CURSOR, wiredtiger::WT_ITEM, Error};

pub struct Datum {
    item: Pin<Box<WT_ITEM>>,
    data: Pin<Box<[u8]>>,
}

impl Datum {
    pub fn from_vec(data: Vec<u8>) -> Self {
        let zero_item: WT_ITEM = unsafe { std::mem::zeroed() };
        let mut item = Box::pin(zero_item);
        item.data = data.as_ptr() as *const _;
        item.size = data.len();
        let data = data.into_boxed_slice();
        let data = Pin::new(data);
        Self { item, data }
    }

    pub fn from_boxed(data: Pin<Box<[u8]>>) -> Self {
        let zero_item: WT_ITEM = unsafe { std::mem::zeroed() };
        let mut item = Box::pin(zero_item);
        item.data = data.as_ref().as_ptr() as *const _;
        item.size = data.len();
        Self { item, data }
    }

    fn copy_from_cursor_slice(item: &WT_ITEM, buf_ref: &[u8]) -> Self {
        // Make a copy
        let copy = buf_ref.to_vec().into_boxed_slice();
        let data = Pin::new(copy);
        Self {
            item: Box::pin(*item),
            data,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn wt_item(&self) -> &WT_ITEM {
        &self.item
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Clone for Datum {
    fn clone(&self) -> Self {
        // Memory copy.
        let data = self.as_slice().to_vec().into_boxed_slice();
        let data = Pin::new(data);
        Self {
            item: Box::pin(*self.wt_item()),
            data,
        }
    }
}
impl Eq for Datum {}
impl PartialEq for Datum {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Debug for Datum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.data)
    }
}

pub struct Cursor {
    cursor: *mut WT_CURSOR,
    current_key: RefCell<Option<Rc<Datum>>>,
    current_value: RefCell<Option<Rc<Datum>>>,
}

#[allow(dead_code)]
impl Cursor {
    pub(crate) fn new(cursor: *mut WT_CURSOR) -> Self {
        Self {
            cursor,
            current_key: RefCell::new(None),
            current_value: RefCell::new(None),
        }
    }

    /// Get the value for the current record
    pub fn get_key(&self) -> Result<Rc<Datum>, Error> {
        let mut item = Pin::new(Box::new(unsafe { std::mem::zeroed::<WT_ITEM>() }));
        let result = unsafe {
            let gey_key_func = (*self.cursor).get_key.unwrap();
            gey_key_func(self.cursor, item.as_mut())
        };

        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        let addr = item.data;

        let re = unsafe { std::slice::from_raw_parts(addr as *const u8, item.size) };

        let datum = Rc::new(Datum::copy_from_cursor_slice(&item, re));
        self.current_key.replace(Some(datum.clone()));
        Ok(datum)
    }

    pub fn set_key(&self, buffer: Datum) -> Result<(), Error> {
        unsafe { (*self.cursor).set_key.unwrap()(self.cursor, buffer.wt_item()) };
        self.current_key.replace(Some(Rc::new(buffer)));

        Ok(())
    }

    /// Get the value for the current record
    pub fn get_value(&self) -> Result<Rc<Datum>, Error> {
        let mut value = unsafe { std::mem::zeroed::<WT_ITEM>() };
        let result = unsafe {
            let get_value_func = (*self.cursor).get_value.unwrap();
            get_value_func(self.cursor, &mut value)
        };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        let addr = value.data;

        if value.size == 0 {
            let datum = Rc::new(Datum::from_vec(vec![]));
            self.current_value.replace(Some(datum.clone()));
            return Ok(datum);
        }

        let re = unsafe { std::slice::from_raw_parts(addr as *const u8, value.size) };

        let datum = Rc::new(Datum::copy_from_cursor_slice(&value, re));
        self.current_value.replace(Some(datum.clone()));
        Ok(datum)
    }

    pub fn set_value(&self, buffer: Datum) -> Result<(), Error> {
        unsafe { (*self.cursor).set_value.unwrap()(self.cursor, buffer.wt_item()) };
        self.current_value.replace(Some(Rc::new(buffer)));
        Ok(())
    }

    /// Return the ordering relationship between two cursors: both cursors must have the same data
    /// source and have valid keys.
    /// < 0 if cursor refers to a key that appears before other, 0 if the cursors refer to the same
    /// key, and > 0 if cursor refers to a key that appears after other.
    pub fn compare(&self, other: &Cursor) -> Result<i32, Error> {
        let mut compare = 0;
        let result =
            unsafe { (*self.cursor).compare.unwrap()(self.cursor, other.cursor, &mut compare) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(compare)
    }

    /// Return the ordering relationship between two cursors, testing only for equality: both
    /// cursors must have the same data source and have valid keys.
    pub fn equals(&self, other: &Cursor) -> Result<bool, Error> {
        let mut equal = 0;
        let result =
            unsafe { (*self.cursor).equals.unwrap()(self.cursor, other.cursor, &mut equal) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(equal != 0)
    }

    /// Attempt to advance the cursor to the next record.
    pub fn next(&self) -> Result<(), Error> {
        let result = unsafe { (*self.cursor).next.unwrap()(self.cursor) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(())
    }

    /// Attempt to rewind the cursor to the previous record.
    pub fn prev(&self) -> Result<(), Error> {
        let result = unsafe { (*self.cursor).prev.unwrap()(self.cursor) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(())
    }

    /// Remove a record
    /// If the cursor was configured with "overwrite=true" (the default), the key must be set; the
    /// key's record will be removed if it exists, no error will be returned if the record does not
    /// exist.
    pub fn remove(&self) -> Result<(), Error> {
        let result = unsafe { (*self.cursor).remove.unwrap()(self.cursor) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(())
    }

    /// Reset the position of the cursor.
    ///
    /// Any resources held by the cursor are released, and the cursor's key and position are no
    /// longer valid.
    pub fn reset(&self) -> Result<(), Error> {
        let result = unsafe { (*self.cursor).reset.unwrap()(self.cursor) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        self.current_key.replace(None);
        self.current_value.replace(None);
        Ok(())
    }

    /// Return the record matching the key.
    ///
    /// The key must first be set.
    /// On success, the cursor ends positioned at the returned record
    /// to minimize cursor resources, the reset method should be called as soon as the record has
    /// been retrieved and the cursor no longer needs that position.
    pub fn search(&self) -> Result<(), Error> {
        let result = unsafe { (*self.cursor).search.unwrap()(self.cursor) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(())
    }

    /// Return the record matching the key if it exists, or an adjacent record.
    /// The key must first be set.
    /// An adjacent record is either the smallest record larger than the key or the largest record
    /// smaller than the key (in other words, a logically adjacent key).
    /// On success, the cursor ends positioned at the returned record; to minimize cursor resources,
    /// the reset method should be called as soon as the record has been retrieved and the cursor no longer needs that position.
    pub fn search_near(&self) -> Result<bool, Error> {
        let mut exact = 0;
        let result = unsafe { (*self.cursor).search_near.unwrap()(self.cursor, &mut exact) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(exact != 0)
    }

    /// Update a record and optionally insert an existing record.
    ///
    /// If the cursor was configured with "overwrite=true" (the default), both the key and value must
    /// be set; if the record already exists, the key's value will be updated, otherwise, the record
    /// will be inserted
    /// On success, the cursor ends positioned at the modified record; to minimize cursor resources,
    /// the WT_CURSOR::reset method should be called as soon as the cursor no longer needs that position.
    ///
    /// The maximum length of a single column stored in a table is not fixed (as it partially depends
    /// on the underlying file configuration), but is always a small number of bytes less than 4GB.
    pub fn update(&self) -> Result<(), Error> {
        let result = unsafe { (*self.cursor).update.unwrap()(self.cursor) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }

        Ok(())
    }

    pub fn insert(&self) -> Result<(), Error> {
        let result = unsafe { (*self.cursor).insert.unwrap()(self.cursor) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(())
    }

    /// Reconfigure the cursor.
    /// The cursor is reset
    pub fn reconfigure(&self, config: Option<CursorReconfigureOptions>) -> Result<(), Error> {
        let config = config.map(|config| config.as_config_string());
        let config = config.as_deref().unwrap_or("");
        let config = CString::new(config).unwrap();
        let result = unsafe { (*self.cursor).reconfigure.unwrap()(self.cursor, config.as_ptr()) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(())
    }

    /// Set range bounds on the cursor.
    pub fn bound(&self, config: Option<BoundsConfig>) -> Result<(), Error> {
        let config = config.map(|config| config.as_config_string());
        let config = config.as_deref().unwrap_or("");
        let config = CString::new(config).unwrap();
        let result = unsafe { (*self.cursor).bound.unwrap()(self.cursor, config.as_ptr()) };
        if result != 0 {
            return Err(Error::from_errorcode(result));
        }
        Ok(())
    }
}

impl PartialEq<Self> for Cursor {
    fn eq(&self, other: &Self) -> bool {
        self.compare(other).unwrap() == 0
    }
}

impl Eq for Cursor {}

impl PartialOrd<Self> for Cursor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.compare(other).unwrap().cmp(&0))
    }
}

impl Ord for Cursor {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other).unwrap().cmp(&0)
    }
}

impl Drop for Cursor {
    fn drop(&mut self) {
        let result = unsafe { (*self.cursor).close.unwrap()(self.cursor) };
        if result != 0 {
            panic!("Failed to close cursor");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::bindings::connection::Connection;
    use crate::bindings::create_config::CreateConfig;
    use crate::bindings::cursor::Datum;
    use crate::bindings::data::{Pack, Unpack};
    use crate::bindings::open_config::OpenConfig;
    use crate::bindings::{session_config, CursorConfig, DataSource, FormatType};

    /// Test inserting a key/value via cursor, committing, then retrieving in a new transaction.
    #[test]
    fn test_basic_cursor_packed_data() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path().to_str().unwrap();
        let options = OpenConfig::new().create(true).exclusive(true);
        let connection = Connection::open(Path::new(tmpdirpath), options).unwrap();
        let key = "key";
        let value = "value";
        let entity = DataSource::Table("test_table".to_string());

        {
            let session = connection
                .clone()
                .open_session(session_config::SessionConfig::new())
                .unwrap();

            // Default options
            session.begin_transaction(None).unwrap();
            let config = CreateConfig::new().columns(&["name", "age"]);
            session.create(&entity, Some(config)).unwrap();

            let key_pack = Pack::mk_string(&session, key);
            let key_datum = key_pack.pack();
            let value_pack = Pack::mk_string(&session, value);
            let value_datum = value_pack.pack();

            let cursor = session
                .open_cursor(&entity, Some(CursorConfig::new().raw(true)))
                .unwrap();
            cursor.set_key(key_datum.clone()).unwrap();
            cursor.set_value(value_datum.clone()).unwrap();
            cursor.insert().unwrap();

            cursor.reset().unwrap();
            cursor.set_key(key_datum.clone()).unwrap();
            cursor.search().unwrap();

            let value_result = cursor.get_value().unwrap();
            assert_eq!(value_result.data.as_ref(), value_datum.data.as_ref());

            cursor.reset().unwrap();

            // Now seek...
            cursor.set_key(key_datum.clone()).unwrap();
            cursor.search().unwrap();
            assert_eq!(cursor.get_key().unwrap().as_slice(), key_datum.as_slice());
            assert_eq!(
                cursor.get_value().unwrap().as_slice(),
                value_datum.as_slice()
            );

            // Commit.
            session.commit().unwrap();
        }

        // Now do so in a new session.
        let session = connection
            .clone()
            .open_session(session_config::SessionConfig::new())
            .unwrap();
        session.begin_transaction(None).unwrap();
        let key_pack = Pack::mk_string(&session, key);
        let key_datum = key_pack.pack();
        let cursor = session
            .open_cursor(&entity, Some(CursorConfig::new().raw(true)))
            .unwrap();
        cursor.set_key(key_datum.clone()).unwrap();
        cursor.search().unwrap();
        assert_eq!(cursor.get_key().unwrap().as_slice(), key_datum.as_slice());
        let d = cursor.get_value().unwrap();
        let mut unpacked_value = Unpack::new(&session, &[FormatType::NulTerminatedString(None)], d);
        let str = unpacked_value.unpack_str();
        assert_eq!(str, value);
    }

    #[test]
    fn test_basic_secondary_indexed() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path().to_str().unwrap();
        let options = OpenConfig::new().create(true).exclusive(true);
        let connection = Connection::open(Path::new(tmpdirpath), options).unwrap();
        let key = "key";
        let value = "value";
        let entity = DataSource::Table("test_table".to_string());

        let session = connection
            .clone()
            .open_session(session_config::SessionConfig::new())
            .unwrap();

        // Default options
        session.begin_transaction(None).unwrap();
        let config = CreateConfig::new().columns(&["name", "age"]);
        session.create(&entity, Some(config)).unwrap();

        // Now create the secondary index on "age"
        let index_config = CreateConfig::new().columns(&["age"]);
        let index_entity = DataSource::Index {
            table: "test_table".to_string(),
            index_name: "age".to_string(),
            projection: None,
        };
        session.create(&index_entity, Some(index_config)).unwrap();
        session.commit().unwrap();

        let session = connection
            .clone()
            .open_session(session_config::SessionConfig::new())
            .unwrap();
        session.begin_transaction(None).unwrap();
        let cursor = session
            .open_cursor(&entity, Some(CursorConfig::new().raw(true)))
            .unwrap();
        let key_pack = Datum::from_vec(key.as_bytes().to_vec());
        let value_pack = Datum::from_vec(value.as_bytes().to_vec());
        cursor.set_key(key_pack.clone()).unwrap();
        cursor.set_value(value_pack.clone()).unwrap();
        cursor.insert().unwrap();
        cursor.reset().unwrap();

        let cursor = session
            .open_cursor(&entity, Some(CursorConfig::new().raw(true)))
            .unwrap();
        cursor.set_key(key_pack.clone()).unwrap();
        cursor.search().unwrap();
        assert_eq!(cursor.get_key().unwrap().as_slice(), key_pack.as_slice());

        let value_datum = cursor.get_value().unwrap();
        assert_eq!(value_datum.as_slice(), value.as_bytes());
    }

    /// Test inserting multiple key values, committing, and then iterating.
    #[test]
    fn test_cursor_iterate() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path().to_str().unwrap();
        let options = OpenConfig::new().create(true).exclusive(true);
        let connection = Connection::open(Path::new(tmpdirpath), options).unwrap();
        let entity = DataSource::Table("test_table".to_string());

        // Generate some key pairs as a Vec<(Vec<u8>, Vec<u8>)>
        let mut kvs: Vec<(String, String)> = (0..100)
            .map(|i| (format!("key{}", i), format!("value{}", i)))
            .collect();

        // Lexicographical sort just like in a btree
        kvs.sort_by(|a, b| a.0.cmp(&b.0));

        // Now insert them in a transaction
        {
            let session = connection
                .clone()
                .open_session(session_config::SessionConfig::new())
                .unwrap();
            session.begin_transaction(None).unwrap();
            let config = CreateConfig::new().columns(&["name", "age"]);
            session.create(&entity, Some(config)).unwrap();

            let cursor = session
                .open_cursor(&entity, Some(CursorConfig::new().raw(true)))
                .unwrap();
            for (key, value) in &kvs {
                let key_pack = Pack::mk_string(&session, key);
                let value_pack = Pack::mk_string(&session, value);
                cursor.set_key(key_pack.pack()).unwrap();
                cursor.set_value(value_pack.pack()).unwrap();
                cursor.update().unwrap();
            }
            session.commit().unwrap();
        }

        // Now iterate over them
        let session = connection
            .clone()
            .open_session(session_config::SessionConfig::new())
            .unwrap();
        session.begin_transaction(None).unwrap();
        let cursor = session
            .open_cursor(&entity, Some(CursorConfig::new().raw(true)))
            .unwrap();
        // Initial cursor is first record
        let first_key = kvs[0].0.clone();
        let first_key = Pack::mk_string(&session, &first_key).pack();
        cursor.set_key(first_key).unwrap();
        cursor.search().unwrap();

        // Now accumulate the keys and values into a new vec
        let mut kvs2 = vec![];
        loop {
            let mut unpack_key = Unpack::new(
                &session,
                &[FormatType::NulTerminatedString(None)],
                cursor.get_key().unwrap(),
            );
            let mut unpack_value = Unpack::new(
                &session,
                &[FormatType::NulTerminatedString(None)],
                cursor.get_value().unwrap(),
            );
            let key = unpack_key.unpack_str();
            let value = unpack_value.unpack_str();
            kvs2.push((key, value));
            if cursor.next().is_err() {
                break;
            }
        }

        // Now compare the two
        assert_eq!(kvs, kvs2);
    }

    #[test]
    fn test_pack_buffer() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path().to_str().unwrap();
        let options = OpenConfig::new().create(true).exclusive(true);
        let connection = Connection::open(Path::new(tmpdirpath), options).unwrap();
        let entity = DataSource::Table("test_table".to_string());

        let session = connection
            .open_session(session_config::SessionConfig::new())
            .unwrap();
        session.begin_transaction(None).unwrap();
        let value_format = &[
            FormatType::UInt64,
            FormatType::NulTerminatedString(Some(16)),
            FormatType::RawByte(Some(11)),
        ];
        let key_format = &[FormatType::UInt64, FormatType::NulTerminatedString(None)];
        let config = CreateConfig::new()
            .columns(&["id", "name", "age", "last_name", "bytes"])
            .key_format(key_format)
            .value_format(value_format);
        session.create(&entity, Some(config)).unwrap();

        let mut key_pack = Pack::new(&session, key_format, 256);
        key_pack.push_uint(123);
        key_pack.push_str("ryan");
        let key_pack = key_pack.pack();

        let mut value_pack = Pack::new(&session, value_format, 256);
        value_pack.push_uint(321);
        value_pack.push_str("hello world");
        value_pack.push_item(b"world hello");
        let value_pack = value_pack.pack();

        let cursor = session
            .open_cursor(&entity, Some(CursorConfig::new().raw(true)))
            .unwrap();
        cursor.set_key(key_pack.clone()).unwrap();
        cursor.set_value(value_pack).unwrap();
        cursor.insert().unwrap();
        cursor.reset().unwrap();

        cursor.set_key(key_pack).unwrap();
        cursor.search().unwrap();

        let mut unpack_key = Unpack::new(&session, key_format, cursor.get_key().unwrap());
        let id = unpack_key.unpack_uint();
        let name = unpack_key.unpack_str();
        unpack_key.close();
        assert_eq!(id, 123);
        assert_eq!(name, "ryan");

        let mut unpack_value = Unpack::new(&session, value_format, cursor.get_value().unwrap());
        let age = unpack_value.unpack_uint();
        let name = unpack_value.unpack_str();
        let bytes = unpack_value.unpack_item();
        unpack_value.close();

        assert_eq!(age, 321);
        assert_eq!(name, "hello world");
        assert_eq!(bytes, b"world hello");
    }
}
