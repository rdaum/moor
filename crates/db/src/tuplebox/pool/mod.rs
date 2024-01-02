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

mod buffer_pool;
mod size_class;

pub use buffer_pool::BufferPool;

/// The unique identifier for currently extant buffers. Buffer ids can be ephemeral.
/// It is up to the pager to map these to non-ephemeral, long lived, page identifiers (Bid) through
/// whatever means makes sense.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Bid(pub u64);

#[derive(thiserror::Error, Debug)]
pub enum PagerError {
    #[error("Error in setting up the page / buffer pool: {0}")]
    InitializationError(String),

    #[error("Insufficient room in buffer pool (wanted {desired:?}, had {available:?})")]
    InsufficientRoom { desired: usize, available: usize },

    #[error("Unsupported size class (wanted {0:?})")]
    UnsupportedSize(usize),

    #[error("Unable to allocate a buffer")]
    CouldNotAllocate,

    #[error("Invalid page access")]
    CouldNotAccess,
}
