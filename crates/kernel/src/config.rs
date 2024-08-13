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

//! Config is created by the host daemon, and passed through the scheduler, whereupon it is
//! available to all components. Used to hold things typically configured by CLI flags, etc.

use crate::textdump::EncodingMode;
use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct Config {
    /// Whether to allow notify() to send arbitrary MOO values to players. The interpretation of
    /// the values varies depending on host/client.
    /// If this is false, only strings are allowed, as in LambdaMOO.
    pub rich_notify: bool,
    /// Where to write periodic textdumps of the database.
    pub textdump_output: Option<PathBuf>,
    /// What encoding to use for textdumps (ISO-8859-1 or UTF-8).
    pub textdump_encoding: EncodingMode,
}
