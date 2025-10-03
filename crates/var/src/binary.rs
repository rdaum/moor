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

use crate::{
    Error, Sequence,
    error::ErrorCode::{E_INVARG, E_RANGE, E_TYPE},
    var::Var,
    variant::Variant,
};
use byteview::ByteView;
use std::{
    cmp::max,
    fmt::{Display, Formatter},
    hash::Hash,
};

/// A binary blob type that wraps `byteview::ByteView` for efficient handling of binary data.
#[derive(Clone)]
pub struct Binary(ByteView);

impl Binary {
    /// Create a new Binary from a Vec<u8>
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Binary(ByteView::from(bytes))
    }

    /// Create a new Binary from a slice
    pub fn from_slice(bytes: &[u8]) -> Self {
        Binary(ByteView::from(bytes.to_vec()))
    }

    /// Create a new Binary from a ByteView
    pub fn from_byte_view(byte_view: ByteView) -> Self {
        Binary(byte_view)
    }

    /// Get the raw bytes as a slice
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    /// Get the underlying ByteView
    pub fn as_byte_view(&self) -> &ByteView {
        &self.0
    }

    /// Append another Binary to this one, returning a new Binary
    pub fn append(&self, other: &Self) -> Var {
        let mut new_bytes = Vec::with_capacity(self.len() + other.len());
        new_bytes.extend_from_slice(self.as_bytes());
        new_bytes.extend_from_slice(other.as_bytes());
        let binary = Binary::from_bytes(new_bytes);
        Var::from_variant(Variant::Binary(Box::new(binary)))
    }

    /// Get the length in bytes
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Sequence for Binary {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn index_in(&self, value: &Var, _case_sensitive: bool) -> Result<Option<usize>, Error> {
        let search_bytes = match value.variant() {
            Variant::Binary(b) => b.as_bytes(),
            Variant::Int(i) => {
                if *i < 0 || *i > 255 {
                    return Err(
                        E_INVARG.with_msg(|| format!("Byte value {i} out of range (0-255)"))
                    );
                }
                let byte = *i as u8;
                return Ok(self.as_bytes().iter().position(|&b| b == byte));
            }
            _ => {
                return Err(E_TYPE.with_msg(|| {
                    format!(
                        "Cannot search binary for {}",
                        value.type_code().to_literal()
                    )
                }));
            }
        };

        // Search for the byte sequence
        if search_bytes.is_empty() {
            return Ok(Some(0));
        }

        let haystack = self.as_bytes();
        if search_bytes.len() > haystack.len() {
            return Ok(None);
        }

        for i in 0..=(haystack.len() - search_bytes.len()) {
            if haystack[i..i + search_bytes.len()] == *search_bytes {
                return Ok(Some(i));
            }
        }

        Ok(None)
    }

    fn contains(&self, value: &Var, case_sensitive: bool) -> Result<bool, Error> {
        self.index_in(value, case_sensitive)
            .map(|opt| opt.is_some())
    }

    fn index(&self, index: usize) -> Result<Var, Error> {
        if index >= self.len() {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "Index {} out of range for binary of length {}",
                    index,
                    self.len()
                )
            }));
        }
        let byte = self.as_bytes()[index];
        Ok(Var::from_variant(Variant::Int(byte as i64)))
    }

    fn index_set(&self, index: usize, value: &Var) -> Result<Var, Error> {
        if index >= self.len() {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "Index {} out of range for binary of length {}",
                    index,
                    self.len()
                )
            }));
        }

        // Value must be an integer between 0-255
        let byte_value = match value.variant() {
            Variant::Int(i) => {
                if *i < 0 || *i > 255 {
                    return Err(
                        E_INVARG.with_msg(|| format!("Byte value {i} out of range (0-255)"))
                    );
                }
                *i as u8
            }
            _ => {
                return Err(E_TYPE.with_msg(|| {
                    format!(
                        "Cannot set binary index {} with {}",
                        index,
                        value.type_code().to_literal()
                    )
                }));
            }
        };

        let mut new_bytes = self.as_bytes().to_vec();
        new_bytes[index] = byte_value;
        let binary = Binary::from_bytes(new_bytes);
        Ok(Var::from_variant(Variant::Binary(Box::new(binary))))
    }

    fn push(&self, value: &Var) -> Result<Var, Error> {
        // Value must be an integer between 0-255 or another binary
        match value.variant() {
            Variant::Int(i) => {
                if *i < 0 || *i > 255 {
                    return Err(
                        E_INVARG.with_msg(|| format!("Byte value {i} out of range (0-255)"))
                    );
                }
                let mut new_bytes = self.as_bytes().to_vec();
                new_bytes.push(*i as u8);
                let binary = Binary::from_bytes(new_bytes);
                Ok(Var::from_variant(Variant::Binary(Box::new(binary))))
            }
            Variant::Binary(other) => Ok(self.append(other)),
            _ => Err(E_TYPE.with_msg(|| {
                format!("Cannot append {} to binary", value.type_code().to_literal())
            })),
        }
    }

    fn insert(&self, index: usize, value: &Var) -> Result<Var, Error> {
        if index > self.len() {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "Index {} out of range for binary of length {}",
                    index,
                    self.len()
                )
            }));
        }

        // Value must be an integer between 0-255
        let byte_value = match value.variant() {
            Variant::Int(i) => {
                if *i < 0 || *i > 255 {
                    return Err(
                        E_INVARG.with_msg(|| format!("Byte value {i} out of range (0-255)"))
                    );
                }
                *i as u8
            }
            _ => {
                return Err(E_TYPE.with_msg(|| {
                    format!(
                        "Cannot insert {} into binary",
                        value.type_code().to_literal()
                    )
                }));
            }
        };

        let mut new_bytes = self.as_bytes().to_vec();
        new_bytes.insert(index, byte_value);
        let binary = Binary::from_bytes(new_bytes);
        Ok(Var::from_variant(Variant::Binary(Box::new(binary))))
    }

    fn range(&self, from: isize, to: isize) -> Result<Var, Error> {
        let len = self.len() as isize;

        // Handle negative indices
        let from = if from < 0 { max(0, len + from) } else { from };
        let to = if to < 0 { max(0, len + to) } else { to };

        // Ensure indices are within bounds
        let from = from.max(0) as usize;
        let to = (to.max(0) as usize).min(self.len());

        if from > to {
            // Return empty binary
            return Ok(Var::from_variant(Variant::Binary(Box::new(
                Binary::from_bytes(Vec::new()),
            ))));
        }

        let slice = &self.as_bytes()[from..=to.min(self.len().saturating_sub(1))];
        let binary = Binary::from_slice(slice);
        Ok(Var::from_variant(Variant::Binary(Box::new(binary))))
    }

    fn range_set(&self, from: isize, to: isize, with: &Var) -> Result<Var, Error> {
        let len = self.len() as isize;

        // Handle negative indices
        let from = if from < 0 { max(0, len + from) } else { from };
        let to = if to < 0 { max(0, len + to) } else { to };

        // Ensure indices are within bounds
        let from = from.max(0) as usize;
        let to = (to.max(0) as usize).min(self.len());

        if from > to {
            return Err(E_RANGE.with_msg(|| "Invalid range".to_string()));
        }

        // The replacement value must be a binary
        let replacement_bytes = match with.variant() {
            Variant::Binary(b) => b.as_bytes(),
            _ => {
                return Err(E_TYPE.with_msg(|| {
                    format!(
                        "Cannot replace binary range with {}",
                        with.type_code().to_literal()
                    )
                }));
            }
        };

        let mut new_bytes = Vec::new();
        new_bytes.extend_from_slice(&self.as_bytes()[..from]);
        new_bytes.extend_from_slice(replacement_bytes);
        if to + 1 < self.len() {
            new_bytes.extend_from_slice(&self.as_bytes()[to + 1..]);
        }

        let binary = Binary::from_bytes(new_bytes);
        Ok(Var::from_variant(Variant::Binary(Box::new(binary))))
    }

    fn append(&self, other: &Var) -> Result<Var, Error> {
        match other.variant() {
            Variant::Binary(other_binary) => Ok(self.append(other_binary)),
            _ => Err(E_TYPE.with_msg(|| {
                format!("Cannot append {} to binary", other.type_code().to_literal())
            })),
        }
    }

    fn remove_at(&self, index: usize) -> Result<Var, Error> {
        if index >= self.len() {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "Index {} out of range for binary of length {}",
                    index,
                    self.len()
                )
            }));
        }

        let mut new_bytes = self.as_bytes().to_vec();
        new_bytes.remove(index);
        let binary = Binary::from_bytes(new_bytes);
        Ok(Var::from_variant(Variant::Binary(Box::new(binary))))
    }
}

impl Hash for Binary {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ref().hash(state);
    }
}

impl PartialEq for Binary {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ref() == other.0.as_ref()
    }
}

impl Eq for Binary {}

impl std::fmt::Debug for Binary {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Binary({} bytes)", self.len())
    }
}

impl Display for Binary {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Binary({} bytes)", self.len())
    }
}

impl std::cmp::PartialOrd for Binary {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for Binary {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.as_ref().cmp(other.0.as_ref())
    }
}
