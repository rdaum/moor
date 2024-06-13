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

use std::cmp::min;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::hash::{Hash, Hasher};

use crate::{AsByteBuffer, DecodingError, EncodingError};
use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use daumtils::SliceRef;

use crate::var::variant::Variant;
use crate::var::{v_empty_list, Var};

#[derive(Clone, Debug)]
pub struct List(SliceRef);

fn offsets_end_pos(buf: &[u8]) -> usize {
    unsafe {
        let ptr = buf.as_ptr() as *const u32;
        *ptr as usize
    }
}

fn offset_at(buf: &[u8], index: usize) -> usize {
    let ptr = buf.as_ptr() as *const u32;
    unsafe { *ptr.add(index + 1) as usize }
}

impl List {
    pub fn new() -> Self {
        Self(SliceRef::from_vec(Vec::new()))
    }

    pub fn len(&self) -> usize {
        let l = self.0.len();
        if l == 0 || l == 4 {
            return 0;
        }

        let slc = self.0.as_slice();
        let offsets_end = offsets_end_pos(slc);
        let offsets_len = offsets_end - 4;
        // The offsets table is 4 bytes per offset.
        offsets_len >> 2
    }

    pub fn is_empty(&self) -> bool {
        let l = self.0.len();
        if l == 0 || l == 4 {
            return true;
        }
        false
    }

    pub fn get(&self, index: usize) -> Option<Var> {
        let len = self.len();
        if index >= len {
            return None;
        }

        let slc = self.0.as_slice();
        let offsets_end = offsets_end_pos(slc);

        // The offsets table is 4 bytes per offset.
        let data_offset = offset_at(slc, index);

        let data_section = &self.0.slice(offsets_end..);

        // If this is the last item, we can just slice to the end of the data section.
        if index == len - 1 {
            let slice_ref = data_section.slice(data_offset..);
            return Some(Var::from_sliceref(slice_ref).expect("could not decode var"));
        }

        // Otherwise, we need to slice from this offset to the next offset.
        let next_offset = offset_at(slc, index + 1);

        // Note that the offsets are relative to the start of the data section.
        let data = data_section.slice(data_offset..next_offset);
        Var::from_sliceref(data.clone()).ok()
    }

    pub fn from_slice(vec: &[Var]) -> List {
        let mut data = Vec::new();

        let mut relative_offset: u32 = 0;
        let mut offsets = Vec::with_capacity(vec.len() * 4);
        for v in vec.iter() {
            offsets.extend_from_slice(&relative_offset.to_le_bytes());
            let vsr = v.as_sliceref().unwrap();
            let bytes = vsr.as_slice();
            data.extend_from_slice(bytes);
            relative_offset += bytes.len() as u32;
        }

        let mut result = Vec::with_capacity(4 + offsets.len() + data.len());
        result.extend_from_slice(&(offsets.len() as u32 + 4).to_le_bytes());
        result.extend_from_slice(&offsets);
        result.extend_from_slice(&data);

        Self(SliceRef::from_vec(result))
    }

    // expensive because we need to extend both buffer length and the offsets length...
    pub fn push(&self, v: Var) -> Var {
        let len = self.len();

        let data_sr = v.as_sliceref().unwrap();

        // Special case if we're empty.
        if len == 0 {
            let mut new_offsets = Vec::with_capacity(4);
            let offset: u32 = 0;
            new_offsets.extend_from_slice(&offset.to_le_bytes());

            let mut result = Vec::with_capacity(4 + new_offsets.len() + data_sr.len());
            result.extend_from_slice(&8u32.to_le_bytes());
            result.extend_from_slice(&new_offsets);
            result.extend_from_slice(data_sr.as_slice());

            return Var::new(Variant::List(Self(SliceRef::from_vec(result))));
        }

        let slc = self.0.as_slice();
        let offsets_end = offsets_end_pos(slc);

        let existing_offset_table = &slc[4..offsets_end];
        let existing_data = &slc[offsets_end..];

        // Add the new offset to the offsets table. The new offset is end of the old data section,
        // that is, the length of the whole buffer.
        let mut new_offsets = Vec::with_capacity(existing_offset_table.len() + 4);
        new_offsets.extend_from_slice(existing_offset_table);
        let new_offset = existing_data.len() as u32;
        new_offsets.extend_from_slice(&new_offset.to_le_bytes());

        // Add the new data to the data section.
        let mut new_data = Vec::with_capacity(existing_data.len() + data_sr.len());
        new_data.extend_from_slice(existing_data);
        new_data.extend_from_slice(data_sr.as_slice());

        // Update offsets end
        let new_offsets_end = new_offsets.len() as u32 + 4;

        // Result is new_offsets_len + new_offsets + new_data
        let mut result = Vec::with_capacity(4 + new_offsets.len() + new_data.len());
        result.extend_from_slice(&new_offsets_end.to_le_bytes());
        result.extend_from_slice(&new_offsets);
        result.extend_from_slice(&new_data);

        Var::new(Variant::List(Self(SliceRef::from_vec(result))))
    }

    pub fn pop_front(&self) -> (Var, Var) {
        let len = self.len();
        if len == 0 {
            return (v_empty_list(), v_empty_list());
        }

        if len == 1 {
            return (self.get(0).unwrap(), v_empty_list());
        }

        let slc = self.0.as_slice();
        let offsets_end = offsets_end_pos(slc);

        // Get the offset table
        let offsets_table = &slc[4..offsets_end];

        // Splice off the data section after the first item
        let data_section = &self.0.slice(offsets_end..);
        let first_offset = offset_at(slc, 0);
        let next_offset = offset_at(slc, 1);
        let length = next_offset - first_offset;
        let data = data_section.slice(first_offset..next_offset);

        // Now rebuild the offset table, subtracting the length of the first item
        let mut new_offsets = Vec::with_capacity(offsets_table.len() - 4);
        for i in 1..len {
            let offset = offset_at(slc, i);
            let new_offset = (offset - length) as u32;
            new_offsets.extend_from_slice(&new_offset.to_le_bytes());
        }

        // Now reconstruct
        let mut result = Vec::with_capacity(4 + new_offsets.len() + data.len());
        result.extend_from_slice(&(new_offsets.len() as u32 + 4).to_le_bytes());
        result.extend_from_slice(&new_offsets);
        result.extend_from_slice(data_section.slice(next_offset..).as_slice());

        (
            Var::from_sliceref(data).unwrap(),
            Var::new(Variant::List(Self(SliceRef::from_vec(result)))),
        )
    }

    pub fn append(&self, other: &Self) -> Var {
        let len = self.len();
        if len == 0 {
            return Var::new(Variant::List(other.clone()));
        }

        let other_len = other.len();
        if other_len == 0 {
            return Var::new(Variant::List(self.clone()));
        }

        // Find the starts of the two data sections
        let slc = self.0.as_slice();
        let oth_slc = other.0.as_slice();

        let data_start_self = offsets_end_pos(slc);
        let data_start_other = offsets_end_pos(oth_slc);

        // Get their data sections
        let data_self = &slc[data_start_self..];
        let data_other = &oth_slc[data_start_other..];

        // Get the two offsets tables
        let offset_table_self = &slc[4..data_start_self];
        let offset_table_other = &oth_slc[4..data_start_other];

        let self_offset_len = offset_table_self.len();

        // Construct a new offset table, leaving self intact and then adjusting other.
        let mut new_offset_table = Vec::with_capacity(self_offset_len + offset_table_other.len());
        new_offset_table.extend_from_slice(offset_table_self);
        for i in 0..other_len {
            let offset = offset_at(oth_slc, i);
            let new_offset = (offset + data_self.len()) as u32;
            new_offset_table.extend_from_slice(&new_offset.to_le_bytes());
        }

        let mut result =
            Vec::with_capacity(4 + new_offset_table.len() + data_self.len() + data_other.len());
        result.extend_from_slice(&(new_offset_table.len() as u32 + 4).to_le_bytes());
        result.extend_from_slice(&new_offset_table);
        result.extend_from_slice(data_self);
        result.extend_from_slice(data_other);

        Var::new(Variant::List(Self(SliceRef::from_vec(result))))
    }

    pub fn remove_at(&self, index: usize) -> Var {
        let len = self.len();
        if len == 0 {
            return Var::new(Variant::List(self.clone()));
        }

        if len == 1 {
            return v_empty_list();
        }

        // This will involve rebuilding both the offsets and data sections.
        let slc = self.0.as_slice();
        let old_data_start = offsets_end_pos(slc);

        let old_data = &slc[old_data_start..];
        let old_offsets = &slc[4..old_data_start];

        let remove_item_offset =
            u32::from_le_bytes(old_offsets[index * 4..(index + 1) * 4].try_into().unwrap())
                as usize;
        let remove_item_length = if index == len - 1 {
            old_data.len() - remove_item_offset
        } else {
            let next_offset = offset_at(slc, index + 1);
            next_offset - remove_item_offset
        };

        let mut new_offsets = Vec::with_capacity(old_offsets.len() - 4);
        let mut new_data = Vec::with_capacity(old_data.len() - remove_item_length);

        // Iterate to generate
        for i in 0..len {
            if i == index {
                continue;
            }

            let offset = offset_at(slc, i);
            let data = if i == len - 1 {
                &old_data[offset..]
            } else {
                let next_offset = offset_at(slc, i + 1);
                &old_data[offset..next_offset]
            };
            let new_offset = new_data.len() as u32;
            new_data.extend_from_slice(data);
            new_offsets.extend_from_slice(&new_offset.to_le_bytes());
        }

        let mut result = Vec::with_capacity(4 + new_offsets.len() + new_data.len());
        result.extend_from_slice(&(new_offsets.len() as u32 + 4).to_le_bytes());
        result.extend_from_slice(&new_offsets);
        result.extend_from_slice(&new_data);
        Var::new(Variant::List(Self(SliceRef::from_vec(result))))
    }

    /// Remove the first found instance of the given value from the list.
    #[must_use]
    pub fn setremove(&self, value: &Var) -> Var {
        let len = self.len();
        if len == 0 {
            return Var::new(Variant::List(self.clone()));
        }

        if len == 1 {
            if self.get(0).unwrap().eq(value) {
                return v_empty_list();
            }
            return Var::new(Variant::List(self.clone()));
        }

        // This will involve rebuilding both the offsets and data sections.
        let slc = self.0.as_slice();
        let old_data_start = offsets_end_pos(slc);

        let old_data = &self.0.slice(old_data_start..);
        let old_offsets = &slc[4..old_data_start];

        let mut new_offsets = Vec::with_capacity(old_offsets.len() - 4);
        let mut new_data = Vec::with_capacity(old_data.len());

        // Iterate to generate
        let mut found = false;
        for i in 0..len {
            let offset = offset_at(slc, i);
            let data = if i == len - 1 {
                old_data.slice(offset..)
            } else {
                let next_offset = offset_at(slc, i + 1);
                old_data.slice(offset..next_offset)
            };
            let v = Var::from_sliceref(data.clone()).unwrap();
            if !found && v.eq(value) {
                found = true;
                continue;
            }

            let new_offset = new_data.len() as u32;
            new_data.extend_from_slice(data.as_slice());
            new_offsets.extend_from_slice(&new_offset.to_le_bytes());
        }

        let mut result = Vec::with_capacity(4 + new_offsets.len() + new_data.len());
        result.extend_from_slice(&(new_offsets.len() as u32 + 4).to_le_bytes());
        result.extend_from_slice(&new_offsets);
        result.extend_from_slice(&new_data);
        Var::new(Variant::List(Self(SliceRef::from_vec(result))))
    }

    pub fn insert(&self, index: isize, value: Var) -> Var {
        let index = if index < 0 {
            0
        } else {
            min(index as usize, self.len())
        };

        // Special case if inserting at end, it's just push
        if index == self.len() {
            return self.push(value);
        }

        // Special case if we're empty.
        if self.is_empty() {
            return Var::new(Variant::List(Self::from_slice(&[value])));
        }

        // Accumulate up to the insertion point, building the new offsets and data sections.
        // Then add the new item, and then add the rest of the items.
        let slc = self.0.as_slice();
        let old_data_start = offsets_end_pos(slc);
        let old_data = &slc[old_data_start..];
        let old_offsets = &slc[4..old_data_start];

        let mut new_offsets = Vec::with_capacity(old_offsets.len() + 4);
        let mut new_data = Vec::with_capacity(old_data.len() + value.as_sliceref().unwrap().len());

        for i in 0..self.len() {
            if i == index {
                let new_offset = new_data.len() as u32;
                new_offsets.extend_from_slice(&new_offset.to_le_bytes());
                new_data.extend_from_slice(value.as_sliceref().unwrap().as_slice());
            }
            let offset = offset_at(slc, i);
            let length = if i == self.len() - 1 {
                old_data.len() - offset
            } else {
                let next_offset = offset_at(slc, i + 1);
                next_offset - offset
            };
            let new_offset = new_data.len() as u32;
            new_offsets.extend_from_slice(&new_offset.to_le_bytes());
            new_data.extend_from_slice(&old_data[offset..offset + length]);
        }

        let mut result = Vec::with_capacity(4 + new_offsets.len() + new_data.len());
        result.extend_from_slice(&(new_offsets.len() as u32 + 4).to_le_bytes());
        result.extend_from_slice(&new_offsets);
        result.extend_from_slice(&new_data);
        Var::new(Variant::List(Self(SliceRef::from_vec(result))))
    }

    pub fn set(&self, index: usize, value: Var) -> Var {
        let len = self.len();
        if index >= len {
            return Var::new(Variant::List(self.clone()));
        }

        // This will involve rebuilding both the offsets and data sections.
        let slc = self.0.as_slice();
        let old_data_start = offsets_end_pos(slc);

        let old_data = &self.0.slice(old_data_start..);
        let old_offsets = &slc[4..old_data_start];

        let mut new_offsets = Vec::with_capacity(old_offsets.len());
        let mut new_data = Vec::with_capacity(old_data.len());

        // Iterate to generate
        for i in 0..len {
            let offset = offset_at(slc, i);
            let data = if i == len - 1 {
                old_data.slice(offset..)
            } else {
                let next_offset = offset_at(slc, i + 1);
                old_data.slice(offset..next_offset)
            };
            if i == index {
                let new_offset = new_data.len() as u32;
                new_offsets.extend_from_slice(&new_offset.to_le_bytes());
                new_data.extend_from_slice(value.as_sliceref().unwrap().as_slice());
            } else {
                let new_offset = new_data.len() as u32;
                new_offsets.extend_from_slice(&new_offset.to_le_bytes());
                new_data.extend_from_slice(data.as_slice());
            }
        }

        let mut result = Vec::with_capacity(4 + new_offsets.len() + new_data.len());
        result.extend_from_slice(&(new_offsets.len() as u32 + 4).to_le_bytes());
        result.extend_from_slice(&new_offsets);
        result.extend_from_slice(&new_data);
        Var::new(Variant::List(Self(SliceRef::from_vec(result))))
    }

    // Case insensitive
    pub fn contains(&self, v: &Var) -> bool {
        self.iter().any(|item| item.eq(v))
    }

    pub fn iter(&self) -> impl Iterator<Item = Var> + '_ {
        (0..self.len()).map(move |i| self.get(i).unwrap())
    }

    pub fn contains_case_sensitive(&self, v: &Var) -> bool {
        if let Variant::Str(s) = v.variant() {
            for item in self.iter() {
                if let Variant::Str(s2) = item.variant() {
                    if s.as_str() == s2.as_str() {
                        return true;
                    }
                }
            }
            return false;
        }
        self.contains(v)
    }
}

impl From<List> for Vec<Var> {
    fn from(value: List) -> Self {
        let len = value.len();
        let mut result = Vec::with_capacity(len);
        for i in 0..len {
            result.push(value.get(i).unwrap());
        }
        result
    }
}

impl AsByteBuffer for List {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.0.as_slice()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.as_slice().to_vec())
    }

    fn from_sliceref(bytes: SliceRef) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        Ok(Self(bytes))
    }

    fn as_sliceref(&self) -> Result<SliceRef, EncodingError> {
        Ok(self.0.clone())
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for List {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{{")?;
        let mut first = true;
        for v in self.iter() {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{v}")?;
        }
        write!(f, "}}")
    }
}

impl Encode for List {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.0.as_slice().encode(encoder)
    }
}

impl Decode for List {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vec = Vec::<u8>::decode(decoder)?;
        Ok(Self(SliceRef::from_vec(vec)))
    }
}

impl<'de> BorrowDecode<'de> for List {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vec = Vec::<u8>::borrow_decode(decoder)?;
        Ok(Self(SliceRef::from_vec(vec)))
    }
}

impl PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for (a, b) in self.iter().zip(other.iter()) {
            if !a.eq(&b) {
                return false;
            }
        }
        true
    }
}
impl Eq for List {}

impl Hash for List {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for item in self.iter() {
            item.hash(state);
        }
    }
}
impl PartialOrd for List {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for List {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let len = self.len();
        if len != other.len() {
            return len.cmp(&other.len());
        }

        for (a, b) in self.iter().zip(other.iter()) {
            match a.cmp(&b) {
                std::cmp::Ordering::Equal => continue,
                x => return x,
            }
        }
        std::cmp::Ordering::Equal
    }
}

impl From<Vec<Var>> for List {
    fn from(value: Vec<Var>) -> Self {
        Self::from_slice(&value)
    }
}

impl From<&[Var]> for List {
    fn from(value: &[Var]) -> Self {
        Self::from_slice(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::var::list::List;
    use crate::var::{v_int, v_list, v_string, Variant};

    #[test]
    pub fn weird_moo_insert_scenarios() {
        // MOO supports negative indexes, which just floor to 0...
        let list = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        assert_eq!(
            list.insert(-1, v_int(0)),
            v_list(&[v_int(0), v_int(1), v_int(2), v_int(3)])
        );

        // MOO supports indexes beyond length of the list, which just append to the end...
        let list = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        assert_eq!(
            list.insert(100, v_int(0)),
            v_list(&[v_int(1), v_int(2), v_int(3), v_int(0)])
        );
    }

    #[test]
    pub fn list_display() {
        let list = List::from_slice(&[v_int(1), v_string("foo".into()), v_int(3)]);
        assert_eq!(format!("{list}"), "{1, \"foo\", 3}");
    }

    #[test]
    pub fn list_make_get() {
        let l = List::new();
        assert_eq!(l.len(), 0);
        assert!(l.is_empty());
        // MOO is a bit weird here, it returns None for out of bounds.
        assert_eq!(l.get(0), None);

        let l = List::from_slice(&[v_int(1)]);
        assert_eq!(l.len(), 1);
        assert!(!l.is_empty());
        assert_eq!(l.get(0), Some(v_int(1)));
        assert_eq!(l.get(1), None);

        let l = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        assert_eq!(l.len(), 3);
        assert!(!l.is_empty());

        assert_eq!(l.get(0), Some(v_int(1)));
        assert_eq!(l.get(1), Some(v_int(2)));
        assert_eq!(l.get(2), Some(v_int(3)));
    }

    #[test]
    pub fn list_push() {
        let l = List::new();
        let l = l.push(v_int(1));
        let Variant::List(l) = l.variant() else {
            panic!()
        };

        assert_eq!(l.len(), 1);
        let l = l.push(v_int(2));
        let Variant::List(l) = l.variant() else {
            panic!()
        };

        assert_eq!(l.len(), 2);
        let l = l.push(v_int(3));
        let Variant::List(l) = l.variant() else {
            panic!()
        };

        assert_eq!(l.len(), 3);

        assert_eq!(l.get(0), Some(v_int(1)));
        assert_eq!(l.get(2), Some(v_int(3)));
        assert_eq!(l.get(1), Some(v_int(2)));
    }

    #[test]
    fn list_pop_front() {
        let l = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        let (item, l) = l.pop_front();
        let Variant::List(l) = l.variant() else {
            panic!()
        };

        assert_eq!(item, v_int(1));
        let (item, l) = l.pop_front();
        let Variant::List(l) = l.variant() else {
            panic!()
        };

        assert_eq!(item, v_int(2));
        let (item, l) = l.pop_front();
        let Variant::List(l) = l.variant() else {
            panic!()
        };

        assert_eq!(item, v_int(3));
        assert_eq!(l.len(), 0);
    }

    #[test]
    fn test_list_append() {
        let l1 = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        let l2 = List::from_slice(&[v_int(4), v_int(5), v_int(6)]);
        let l = l1.append(&l2);
        let Variant::List(l) = l.variant() else {
            panic!()
        };
        assert_eq!(l.len(), 6);
        assert_eq!(l.get(0), Some(v_int(1)));
        assert_eq!(l.get(5), Some(v_int(6)));
    }

    #[test]
    fn test_list_remove() {
        let l = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);

        let l = l.remove_at(1);
        let Variant::List(l) = l.variant() else {
            panic!()
        };
        assert_eq!(l.len(), 2);
        assert_eq!(l.get(1), Some(v_int(3)));
        assert_eq!(l.get(0), Some(v_int(1)));
    }

    #[test]
    fn test_list_setremove() {
        let l = List::from_slice(&[v_int(1), v_int(2), v_int(3), v_int(2)]);
        let l = l.setremove(&v_int(2));
        let Variant::List(l) = l.variant() else {
            panic!()
        };
        assert_eq!(l.len(), 3);
        assert_eq!(l.get(0), Some(v_int(1)));
        assert_eq!(l.get(1), Some(v_int(3)));
        assert_eq!(l.get(2), Some(v_int(2)));

        // setremove til empty
        let l = List::from_slice(&[v_int(1)]);
        let l = l.setremove(&v_int(1));
        let Variant::List(l) = l.variant() else {
            panic!()
        };
        assert_eq!(l.len(), 0);
        assert_eq!(l.get(0), None);
    }

    #[test]
    fn test_list_insert() {
        let l = List::new();
        let l = l.insert(0, v_int(4));
        let Variant::List(l) = l.variant() else {
            panic!()
        };
        assert_eq!(l.len(), 1);
        assert_eq!(l.get(0), Some(v_int(4)));

        let l = l.insert(0, v_int(3));
        let Variant::List(l) = l.variant() else {
            panic!()
        };
        assert_eq!(l.len(), 2);
        assert_eq!(l.get(0), Some(v_int(3)));
        assert_eq!(l.get(1), Some(v_int(4)));

        let l = l.insert(-1, v_int(5));
        let Variant::List(l) = l.variant() else {
            panic!()
        };
        assert_eq!(l.len(), 3);
        assert_eq!(l.get(0), Some(v_int(5)));
        assert_eq!(l.get(1), Some(v_int(3)));
        assert_eq!(l.get(2), Some(v_int(4)));
    }

    #[test]
    fn test_list_set() {
        let l = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        let l = l.set(1, v_int(4));
        let Variant::List(l) = l.variant() else {
            panic!()
        };
        assert_eq!(l.len(), 3);
        assert_eq!(l.get(1), Some(v_int(4)));
    }

    #[test]
    fn test_list_contains_case_insenstive() {
        let l = List::from_slice(&[v_string("foo".into()), v_string("bar".into())]);
        assert!(l.contains(&v_string("FOO".into())));
        assert!(l.contains(&v_string("BAR".into())));
    }

    #[test]
    fn test_list_contains_case_senstive() {
        let l = List::from_slice(&[v_string("foo".into()), v_string("bar".into())]);
        assert!(!l.contains_case_sensitive(&v_string("FOO".into())));
        assert!(!l.contains_case_sensitive(&v_string("BAR".into())));
    }
}
