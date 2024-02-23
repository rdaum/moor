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

pub use tuple_ref::TupleRef;

use crate::paging::{PageId, SlotId};

mod tuple_ref;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct TupleId {
    pub page: PageId,
    pub slot: SlotId,
}
