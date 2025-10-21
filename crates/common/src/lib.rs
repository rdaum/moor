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

pub mod builtins;
pub mod matching;
pub mod model;
pub mod tasks;
pub mod tracing;
pub mod util;

/// When encoding or decoding types to/from data or network, this is a version tag put into headers
/// for validity / version checking.
pub const DATA_LAYOUT_VERSION: u8 = 1;

/// Build-time version and git information module
pub mod build {
    /// Package version from Cargo.toml
    pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

    /// Short git commit hash (first 7 characters of SHA)
    pub fn short_commit() -> &'static str {
        lazy_static::lazy_static! {
            static ref SHORT: String = {
                option_env!("VERGEN_GIT_SHA")
                    .map(|s| {
                        if s.len() >= 7 {
                            s[..7].to_string()
                        } else {
                            s.to_string()
                        }
                    })
                    .unwrap_or_else(|| "unknown".to_string())
            };
        }
        &SHORT
    }
}
