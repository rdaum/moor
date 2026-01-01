// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! XDG path helpers for moor: consistent resolution of config/data/runtime directories and paths.
//!
//! These utilities centralize the logic to resolve XDG-compliant locations with sensible fallbacks,
//! so callers don't need to duplicate environment probing or path-joining rules.

use std::env;
use std::path::{Path, PathBuf};

/// Resolve the moor configuration directory:
/// - $XDG_CONFIG_HOME/moor
/// - else $HOME/.config/moor
/// - else current working directory (".")
#[must_use]
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(dir).join("moor")
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".config/moor")
    } else {
        PathBuf::from(".")
    }
}

/// Resolve a path relative to the moor configuration directory if not absolute.
/// Absolute paths are returned unchanged.
///
/// Examples:
/// - config_path("enrollment-token") -> $XDG_CONFIG_HOME/moor/enrollment-token
/// - config_path("/etc/moor/custom.yaml") -> /etc/moor/custom.yaml
#[must_use]
pub fn config_path<P: AsRef<Path>>(relative_or_abs: P) -> PathBuf {
    let p = relative_or_abs.as_ref();
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        config_dir().join(p)
    }
}

/// Resolve the moor data directory:
/// - $XDG_DATA_HOME/moor
/// - else $HOME/.local/share/moor
/// - else current working directory (".")
#[must_use]
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = env::var("XDG_DATA_HOME") {
        PathBuf::from(dir).join("moor")
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".local/share/moor")
    } else {
        PathBuf::from(".")
    }
}

/// Resolve the moor runtime directory (if available):
/// - $XDG_RUNTIME_DIR/moor
/// - else None
#[must_use]
pub fn runtime_dir() -> Option<PathBuf> {
    env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(PathBuf::from)
        .map(|p| p.join("moor"))
}
