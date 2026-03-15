// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use moor_common::tasks::Exception;
use moor_compiler::{Label, Offset};
use moor_var::Var;

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum FinallyReason {
    Fallthrough,
    Raise(Box<Exception>),
    Return(Var),
    Abort,
    Exit { stack: Offset, label: Label },
}
