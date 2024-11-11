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

pub use crate::fjall_db::FjallDb;
pub use crate::fjall_relation::FjallTransaction;
use moor_db::WorldStateTable;
mod fjall_db;
mod fjall_relation;
mod fjall_worldstate;
mod value_set;

pub type FjallDbWorldStateSource = FjallDb<WorldStateTable>;
pub type FjallDbTransaction = FjallTransaction<WorldStateTable>;
