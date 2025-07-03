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

extern crate core;

use shadow_rs::shadow;

pub mod builtins;
pub mod matching;
pub mod model;
pub mod tasks;
pub mod util;

/// When encoding or decoding types to/from data or network, this is a version tag put into headers
/// for validity / version checking.
pub const DATA_LAYOUT_VERSION: u8 = 1;

shadow!(build);
