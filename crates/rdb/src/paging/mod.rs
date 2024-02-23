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

use thiserror::Error;

pub use pager::Pager;
pub use slotted_page::SlotId;
pub use tuple_box::{PageId, TupleBox};
pub use tuple_ptr::TuplePtr;

mod backing;
mod cold_storage;
mod page_storage;
mod pager;
mod slotted_page;
mod tuple_box;
mod tuple_ptr;
mod wal;

#[derive(Debug, Clone, Error)]
pub enum TupleBoxError {
    #[error("Page is full, cannot insert slot of size {0} with {1} bytes remaining")]
    BoxFull(usize, usize),
    #[error("Tuple not found at index {0}")]
    TupleNotFound(usize),
}
