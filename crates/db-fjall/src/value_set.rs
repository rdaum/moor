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

use bytes::Bytes;
use moor_values::AsByteBuffer;

pub struct ValueSet<Value>
where
    Value: Clone + Eq + PartialEq + AsByteBuffer,
{
    contents: Bytes,
    _phantom: std::marker::PhantomData<Value>,
}

pub(crate) struct ValueSetIterator<Value>
where
    Value: Clone + Eq + PartialEq + AsByteBuffer,
{
    contents: Bytes,
    offset: usize,
    _phantom: std::marker::PhantomData<Value>,
}

impl<Value> Iterator for ValueSetIterator<Value>
where
    Value: Clone + Eq + PartialEq + AsByteBuffer,
{
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.contents.len() {
            return None;
        }
        let len_bytes = &self.contents[self.offset..self.offset + 4];
        let len_bytes: [u8; 4] = len_bytes.try_into().unwrap();
        let len = u32::from_le_bytes(len_bytes) as usize;
        self.offset += 4;
        let value_bytes = &self.contents[self.offset..self.offset + len];
        self.offset += len;
        Value::from_bytes(Bytes::copy_from_slice(value_bytes)).ok()
    }
}

impl<Value> ValueSet<Value>
where
    Value: Clone + Eq + PartialEq + AsByteBuffer,
{
    // contents is encoded as:
    //      num values u32
    //          [ value_len u32, value_bytes ] ...
    pub(crate) fn new(contents: Bytes) -> Self {
        Self {
            contents,
            _phantom: Default::default(),
        }
    }

    pub(crate) fn iter(&self) -> ValueSetIterator<Value> {
        ValueSetIterator {
            contents: self.contents.clone(),
            offset: 4,
            _phantom: Default::default(),
        }
    }

    pub(crate) fn data(&self) -> Bytes {
        self.contents.clone()
    }

    pub(crate) fn len(&self) -> usize {
        let bytes = self.contents.as_ref();
        let mut len_bytes = [0; 4];
        len_bytes.copy_from_slice(&bytes[..4]);
        u32::from_le_bytes(len_bytes) as usize
    }

    pub(crate) fn at(&self, idx: usize) -> Option<Value> {
        let bytes = self.contents.as_ref();
        let len = self.len();
        if idx >= len {
            return None;
        }
        // linear scan.
        let mut offset = 4;
        for _ in 0..idx {
            let len_bytes_slice = &bytes[offset..offset + 4];
            let len_bytes = len_bytes_slice.try_into().unwrap();
            let len = u32::from_le_bytes(len_bytes) as usize;
            offset += len + 4;
        }

        let len_bytes_slice = &bytes[offset..offset + 4];
        let len_bytes = len_bytes_slice.try_into().unwrap();
        let len = u32::from_le_bytes(len_bytes) as usize;

        let value_bytes = &bytes[offset + 4..offset + 4 + len];
        Value::from_bytes(Bytes::copy_from_slice(value_bytes)).ok()
    }

    pub(crate) fn without_bytes(&self, value_bytes: &[u8]) -> Self {
        // Linear scan until we find a match for `value_bytes`.
        let mut num_values = self.len();
        let mut new_values = Vec::with_capacity(self.contents.len());

        // add the 'num values' field, but empty for now
        new_values.extend_from_slice(&[0, 0, 0, 0]);

        let bytes = self.contents.as_ref();
        let mut offset = 4;
        let mut found = false;
        for _ in 0..num_values {
            let len_bytes = &bytes[offset..offset + 4];
            let len_bytes: [u8; 4] = len_bytes.try_into().unwrap();
            let len = u32::from_le_bytes(len_bytes) as usize;
            let value = &bytes[offset + 4..offset + 4 + len];
            if value != value_bytes {
                new_values.extend_from_slice(&len_bytes);
                new_values.extend_from_slice(value);
            } else {
                found = true;
                num_values -= 1;
            }
            offset += len + 4;
        }
        if found {
            // update the 'num values' field
            let num_values_bytes = (num_values as u32).to_le_bytes();
            new_values[..4].copy_from_slice(&num_values_bytes);
            Self::new(Bytes::from(new_values))
        } else {
            Self::new(self.contents.clone())
        }
    }

    pub(crate) fn append(&self, value: Value) -> Self {
        let num_values = self.len();
        let mut new_values = Vec::with_capacity(self.contents.len() + value.size_bytes() + 4);
        new_values.extend_from_slice(&self.contents);
        let len_bytes = (value.size_bytes() as u32).to_le_bytes();
        new_values.extend_from_slice(&len_bytes);
        new_values.extend_from_slice(&value.as_bytes().unwrap());
        let num_values_bytes = ((num_values + 1) as u32).to_le_bytes();
        new_values[..4].copy_from_slice(&num_values_bytes);
        Self::new(Bytes::from(new_values))
    }

    pub(crate) fn find(&self, value: &Value) -> Option<usize> {
        let bytes = self.contents.as_ref();
        let mut len_bytes = [0; 4];
        len_bytes.copy_from_slice(&bytes[..4]);
        let mut offset = 4;
        let mut idx = 0;
        let v_bytes = value.as_bytes().unwrap();
        while offset < bytes.len() {
            let len = u32::from_le_bytes(len_bytes) as usize;
            let value_bytes = &bytes[offset..offset + len];
            if value_bytes == v_bytes {
                return Some(idx);
            }
            offset += len + 4;
            idx += 1;
        }
        None
    }
}
